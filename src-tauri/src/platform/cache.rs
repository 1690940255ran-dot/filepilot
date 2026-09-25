//! 应用缓存目录与它的清理（规格 6.2）。
//!
//! 规格原文：「文本提取缓存默认只保存在会话内存，必要临时文件仅在应用专用
//! 缓存目录，**任务结束与下次启动清理**。**清缓存不清操作日志**。」
//!
//! 三句话对应三件事：
//!
//! | 规格 | 落点 |
//! |---|---|
//! | 缓存只在会话内存 | [`SessionCache`]：进程内、按指纹索引、不落盘 |
//! | 临时文件只在应用缓存目录 | [`app_cache_dir`]：`%LOCALAPPDATA%\FilePilot\cache` |
//! | 任务结束与下次启动清理 | [`purge`]：启动时调一次；它**绝不碰数据库** |
//!
//! ## 「清理路径严格限制在应用缓存目录」是一个数据安全问题
//!
//! 清理动作会删文件。一个写错的路径拼接（少一层、多一个 `..`、
//! 或者调用方传进来一个用户目录）就会变成「删用户的文件」。
//! 因此 [`purge`] 的第一件事不是删，而是**确认这个目录确实是我们的缓存目录**：
//!
//! * 路径必须是绝对路径，且不以盘符根或用户主目录结尾；
//! * 目录名必须是 `cache`，父目录名必须是 `FilePilot`；
//! * 遇到重解析点（junction/符号链接）**不跟随**——否则一个指向上级目录的
//!   联接会把「清缓存」变成「清整棵目录树」。
//!
//! 这三条都不靠「调用方会传对」——而是把传错的情况变成一次明确的拒绝。
//!
//! ## 重解析点：三条实测事实（别凭直觉改）
//!
//! Windows 的 junction 在这套 API 下的行为**不符合直觉**，以下每条都是
//! 在本机跑出来、写在 [`remove_entry`] 的注释里的：
//!
//! | 事实 | 实测值 |
//! |---|---|
//! | `symlink_metadata(junction)` | `is_dir()==false`、`is_symlink()==true`、`attrs==0x410` |
//! | `remove_file(junction)` | `Err(ACCESS_DENIED)`，链接与目标都在 |
//! | `remove_dir(junction)` | `Ok(())`，**只删链接**，目标文件与目录原样保留 |
//!
//! 所以：junction 虽然带 `FILE_ATTRIBUTE_DIRECTORY` 位，Rust 仍把它判成
//! **链接而非目录**（不会误递归进去——这是好事），但它也**不能用
//! `remove_file` 删**。用错了的后果不是数据丢失，而是**永远清不干净**：
//! 每一轮清理都留下一条 `skipped`，用户没有任何办法自己修好。

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::domain::errors::{codes, AppError};
use crate::domain::types::Fingerprint;

/// 应用目录名（`%LOCALAPPDATA%\FilePilot`）。
const APP_DIR_NAME: &str = "FilePilot";
/// 缓存子目录名。
const CACHE_DIR_NAME: &str = "cache";

/// 应用数据目录：`%LOCALAPPDATA%\FilePilot`。
///
/// 与 `lib::open_application_database` 用同一套规则——数据库和缓存必须
/// 落在同一个应用目录下，否则「清缓存」与「数据库在哪」会各说各话。
pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR_NAME)
}

/// 应用缓存目录：`%LOCALAPPDATA%\FilePilot\cache`。
pub fn app_cache_dir() -> PathBuf {
    app_data_dir().join(CACHE_DIR_NAME)
}

/// 一次清理的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeReport {
    /// 删掉了多少个文件。
    pub removed_files: u32,
    /// 删掉了多少个空目录（自底向上）。
    pub removed_dirs: u32,
    /// 没能删掉的，按路径列出原因。**不是错误**——
    /// 缓存清不掉不该阻止应用启动。
    pub skipped: Vec<String>,
}

impl PurgeReport {
    /// 什么都没删、也没遇到任何阻碍。
    ///
    /// 首次启动就是这样（缓存目录还不存在）。调用方据此决定要不要打日志，
    /// 免得每次启动都刷一行「清理了 0 个文件」。
    pub fn is_empty(&self) -> bool {
        self.removed_files == 0 && self.removed_dirs == 0 && self.skipped.is_empty()
    }
}

/// 清空应用缓存目录。
///
/// **不碰数据库**：规格明确区分了「缓存」与「操作日志」，
/// 把两者混在一起会让「清缓存」变成一次审计记录的销毁。
///
/// ## 返回 `Err` 的唯一原因是「这不是我们的缓存目录」
///
/// 这一条是刻意的，不是偶然：`purge` 只有两种结局——**拒绝执行**，
/// 或者**尽力执行完并把没删掉的列出来**。没有第三种。
///
/// 反过来说，任何一个「删不掉」（文件被占用、只读、权限不足、目录读不了）
/// 都不该变成 `Err`。因为调用方在启动路径上调它，一旦把删不掉当成错误，
/// 用户看到的就是**一个因为清不掉垃圾缓存而打不开的应用**。
/// 所以这些一律进 [`PurgeReport::skipped`]。
pub fn purge(cache_dir: &Path) -> Result<PurgeReport, AppError> {
    guard(cache_dir)?;

    let mut report = PurgeReport::default();
    if !cache_dir.exists() {
        // 目录还不存在是最常见的情况（第一次启动）。
        // 这不是错误：没有缓存要清，就是清完了。
        return Ok(report);
    }

    // 自底向上删：先删文件与深层目录，再删空的父目录。
    let mut directories = vec![cache_dir.to_path_buf()];
    collect(cache_dir, &mut directories, &mut report);

    // 深的先删。
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        if directory == cache_dir {
            // 缓存目录本身留着：下次启动还要用它，
            // 反复创建删除会让「应用目录不存在」与「缓存没清过」分不清。
            continue;
        }
        match std::fs::remove_dir(&directory) {
            Ok(()) => report.removed_dirs += 1,
            Err(error) => report
                .skipped
                .push(format!("{}: {error}", directory.display())),
        }
    }

    Ok(report)
}

/// 收集目录下的文件并逐个删除；把子目录记下来稍后处理。
///
/// **不返回 `Result`**：读不到某个子目录（被占用、权限不足）只影响那一支，
/// 不该让整轮清理提前收场。能删的先删掉，删不掉的如实记进 `skipped`。
fn collect(directory: &Path, directories: &mut Vec<PathBuf>, report: &mut PurgeReport) {
    let reader = match std::fs::read_dir(directory) {
        Ok(reader) => reader,
        Err(error) => {
            report
                .skipped
                .push(format!("{}: {error}", directory.display()));
            return;
        }
    };

    for entry in reader.flatten() {
        let path = entry.path();
        // 用 `symlink_metadata` 而不是 `metadata`：后者会跟随链接，
        // 于是「缓存目录里的一个联接」会被当成它指向的那个目录处理——
        // 那正是「清缓存」变成「清用户的目录树」的路径。
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                report.skipped.push(format!("{}: {error}", path.display()));
                continue;
            }
        };

        // 重解析点（junction / 符号链接）**绝不递归进去**。
        //
        // 这里刻意把「是链接」单独判一次，而不是只信 `is_dir()`：
        // 链接在文件系统层面可能同时带着「目录」属性，一旦哪天
        // 判定变成 `true`，走进链接就成了「删用户的目录树」。
        // 显式写出来，这个保证就不依赖 std 的内部细节。
        let is_reparse = metadata.file_type().is_symlink();
        if metadata.is_dir() && !is_reparse {
            directories.push(path.clone());
            collect(&path, directories, report);
            continue;
        }

        // 链接本身也只是个条目，直接删掉即可——**不要**跟随它去删目标。
        match remove_entry(&path, &metadata) {
            Ok(()) => report.removed_files += 1,
            Err(error) => report.skipped.push(format!("{}: {error}", path.display())),
        }
    }
}

/// 删一个条目，必要时先摘掉只读位再试一次。
///
/// Windows 上**只读位会让 `remove_file` 直接 `ACCESS_DENIED`**，而这个位
/// 很容易凭空出现：从 zip 解出来的文件、从备份还原的文件、被某些同步工具
/// 摸过的文件都会带上它。缓存文件正好经常是这几种来源。
///
/// 不处理的话，「清缓存」会稳定地留下一批 `skipped`——用户看到的是
/// 「明明点了清理，缓存目录里还是有东西」，而且他没有任何办法自己修好。
fn remove_entry(path: &Path, metadata: &std::fs::Metadata) -> std::io::Result<()> {
    if metadata.file_type().is_symlink() {
        // 重解析点虽然被判成「非目录」，但它不能用 `remove_file` 删：
        // 实测 junction 会返回 `ACCESS_DENIED`，于是它永远留在缓存里、
        // 每轮清理都报一条 `skipped`。
        //
        // `remove_dir` 才是对的——它删掉的**只是链接本身**，
        // 既不跟随、也不碰目标（实测：链接消失，目标文件与目录原样还在）。
        // 文件符号链接则反过来，`remove_dir` 失败后由 `remove_file` 收尾。
        return std::fs::remove_dir(path).or_else(|_| std::fs::remove_file(path));
    }

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(first) => {
            // 只在「确实是只读」时才动属性。别的失败原因（被占用、
            // 权限不足）重试一次也是一样的结果，不值得再撞一次。
            let mut permissions = metadata.permissions();
            if !permissions.readonly() {
                return Err(first);
            }
            // `clippy::permissions_set_readonly_false` 担心的是：在 Unix 上
            // 这会把文件变成 **world writable**（0o777）。
            //
            // 那条顾虑在本模块不成立：这里处理的是 Windows 的只读**属性位**
            // （`FILE_ATTRIBUTE_READONLY`），清掉它不改变任何 ACL，只是把
            // 「不许删」恢复成「可以删」。整个 `platform` 层都是 Windows 语义，
            // 这段代码不会在别的平台上跑。
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            std::fs::set_permissions(path, permissions)?;
            std::fs::remove_file(path)
        }
    }
}

/// 确认这个路径确实是应用缓存目录。
fn guard(cache_dir: &Path) -> Result<(), AppError> {
    let deny = |reason: String| Err(AppError::new(codes::INVALID_PATH, reason));

    let components: Vec<_> = cache_dir.components().collect();
    if components.is_empty() {
        return deny("缓存目录不能为空".to_owned());
    }
    if !cache_dir.is_absolute() {
        return deny(format!("缓存目录必须是绝对路径：{}", cache_dir.display()));
    }

    // 必须以 `FilePilot\cache` 结尾。这一条挡住的是最危险的一种误用：
    // 调用方传进来一个用户目录（或者因为 `..` 拼错而算到了别处）。
    let tail: Vec<&str> = components
        .iter()
        .rev()
        .take(2)
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    if tail.len() != 2 || tail[0] != CACHE_DIR_NAME || tail[1] != APP_DIR_NAME {
        return deny(format!(
            "{} 不是应用缓存目录（必须以 {APP_DIR_NAME}\\{CACHE_DIR_NAME} 结尾）；\
             拒绝在别处执行清理",
            cache_dir.display()
        ));
    }

    // 盘符根、用户主目录本身：这两类一旦被当成缓存目录，
    // 后果是「删掉一大片与本应用无关的东西」。
    if components.len() < 3 {
        return deny(format!(
            "{} 的层级过浅，不像是应用缓存目录",
            cache_dir.display()
        ));
    }

    Ok(())
}

/// 会话内的提取结果缓存（规格 6.2：**默认只保存在会话内存**）。
///
/// 键里带上指纹：同一个 fileId 的文件在两次扫描之间可能被改过，
/// 只按 fileId 命中会**把旧正文当成新文件的内容**——而下游会拿它去分类。
///
/// ## 为什么存整份 `Extraction` 而不是只存正文
///
/// 正文不是提取结果的唯一有价值部分：**证据定位**（`page:1`、
/// `chars:0-120`）同样是——规格 6.4 要求用它核对模型给的
/// `evidenceLocator` 是不是编的。
///
/// 只缓存正文的话，那条校验就得在每次预览时**重新提取一遍**才能拿到定位，
/// 而重新提取的结果未必与当初发送的那一份一致（文件可能被改过、
/// OCR 可能给出不同的分页）。那会让「核对」变成一件说不清的事。
#[derive(Default)]
pub struct SessionCache {
    entries: HashMap<String, (Fingerprint, crate::domain::types::Extraction)>,
}

impl SessionCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取一份缓存结果。指纹不一致就当没命中，并把旧的那份丢掉。
    pub fn get(
        &mut self,
        file_id: &str,
        fingerprint: &Fingerprint,
    ) -> Option<&crate::domain::types::Extraction> {
        // 先用一次**短**的不可变借用判断得失，借用在这里就结束——
        // 不能在持有 `get()` 返回值的 arm 里调 `remove()`，那是未定义的一段
        // 「同时可变与不可变借用」。
        let matched = matches!(
            self.entries.get(file_id),
            Some((cached, _)) if cached == fingerprint
        );

        if !matched {
            // 文件变了，或者压根没有这条：旧结果已经没有意义，
            // 留着只会占内存。命中失败时顺手确保没有残留。
            self.entries.remove(file_id);
            return None;
        }

        self.entries.get(file_id).map(|(_, extraction)| extraction)
    }

    pub fn put(
        &mut self,
        file_id: &str,
        fingerprint: &Fingerprint,
        extraction: crate::domain::types::Extraction,
    ) {
        self.entries
            .insert(file_id.to_owned(), (fingerprint.clone(), extraction));
    }

    /// 会话结束：整份丢掉。
    ///
    /// 规格要求「任务结束与下次启动清理」——这份缓存是进程内的，
    /// 进程退出自然就没了；显式清一次是为了让**长驻进程**下的任务边界
    /// 也能释放内存。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 一个待写入缓存目录的临时文件路径。
///
/// 存在的理由与 [`guard`] 相同：让「往缓存目录写」这件事有一个
/// 唯一的、带校验的入口。调用方拼路径时就无从拼错。
pub fn temp_file(name: &str) -> Result<PathBuf, AppError> {
    let directory = app_cache_dir();
    guard(&directory)?;
    // 名字里不允许有路径分隔符：否则 `..\..\x` 这类名字会逃出缓存目录。
    if name.contains(['/', '\\']) || name.contains("..") {
        return Err(AppError::new(
            codes::INVALID_PATH,
            format!("缓存文件名不能包含路径分隔符：{name}"),
        ));
    }
    std::fs::create_dir_all(&directory).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("无法创建缓存目录 {}: {error}", directory.display()),
        )
    })?;
    Ok(directory.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(marker: &str) -> Fingerprint {
        Fingerprint {
            volume_id: "V".to_owned(),
            file_id: marker.to_owned(),
            size: "1".to_owned(),
            modified_ns: "0".to_owned(),
            sha256: Some(marker.to_owned()),
        }
    }

    /// 造一个形状合法的缓存目录：`<temp>/FilePilot/cache`。
    fn shaped_cache() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("临时目录");
        let cache = temp.path().join(APP_DIR_NAME).join(CACHE_DIR_NAME);
        std::fs::create_dir_all(&cache).expect("建缓存目录");
        (temp, cache)
    }

    #[test]
    fn the_default_cache_dir_is_under_the_app_directory() {
        let cache = app_cache_dir();
        assert!(cache.ends_with(Path::new(APP_DIR_NAME).join(CACHE_DIR_NAME)));
        assert!(cache.is_absolute(), "{}", cache.display());
        assert!(guard(&cache).is_ok(), "默认路径必须能通过自查");
    }

    #[test]
    fn a_path_outside_the_app_cache_is_refused() {
        // 这条是本模块最重要的一条断言：清理动作会删文件，
        // 而「调用方传错了目录」必须变成一次**拒绝**，不是一次误删。
        let temp = tempfile::tempdir().expect("临时目录");
        let user_dir = temp.path().join("我的文档");
        std::fs::create_dir_all(&user_dir).expect("建目录");
        std::fs::write(user_dir.join("重要.txt"), "别删我".as_bytes()).expect("写文件");

        let error = purge(&user_dir).expect_err("非缓存目录必须拒绝");
        assert_eq!(error.code, codes::INVALID_PATH);
        assert!(
            user_dir.join("重要.txt").is_file(),
            "被拒绝之后，用户目录里的东西必须原样还在"
        );
    }

    #[test]
    fn a_directory_that_merely_ends_with_cache_is_still_refused() {
        // 只有 `cache` 这一层名字对还不够：父目录必须是 FilePilot。
        // 否则 `C:\Users\me\cache` 这样的目录会被当成我们的缓存。
        let temp = tempfile::tempdir().expect("临时目录");
        let wrong = temp.path().join("NotFilePilot").join(CACHE_DIR_NAME);
        std::fs::create_dir_all(&wrong).expect("建目录");

        assert!(purge(&wrong).is_err(), "父目录名不对也必须拒绝");
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert!(purge(Path::new(r"FilePilot\cache")).is_err());
    }

    #[test]
    fn purging_removes_files_and_empties_directories() {
        let (_temp, cache) = shaped_cache();
        std::fs::create_dir_all(cache.join("子目录")).expect("建子目录");
        std::fs::write(cache.join("a.tmp"), b"x").expect("写文件");
        std::fs::write(cache.join("子目录").join("b.tmp"), b"y").expect("写文件");

        let report = purge(&cache).expect("清理应成功");

        assert_eq!(report.removed_files, 2, "{report:?}");
        assert!(report.skipped.is_empty(), "{report:?}");
        assert!(cache.is_dir(), "缓存目录本身要留着，下次还要用");
        assert!(
            std::fs::read_dir(&cache).expect("读目录").next().is_none(),
            "缓存目录里不该还剩东西"
        );
    }

    #[test]
    fn purging_a_missing_directory_is_not_an_error() {
        // 第一次启动时缓存目录还不存在——那是「没有缓存要清」，不是失败。
        let temp = tempfile::tempdir().expect("临时目录");
        let cache = temp.path().join(APP_DIR_NAME).join(CACHE_DIR_NAME);

        let report = purge(&cache).expect("目录不存在不是错误");
        assert_eq!(report, PurgeReport::default());
    }

    #[test]
    fn purging_never_touches_anything_outside_the_cache_directory() {
        // 「清缓存不清操作日志」的落点：数据库在应用目录下、缓存目录的**外面**。
        let (temp, cache) = shaped_cache();
        let database = temp.path().join(APP_DIR_NAME).join("filepilot.db");
        std::fs::write(&database, "审计记录".as_bytes()).expect("写数据库");
        std::fs::write(cache.join("junk.tmp"), b"junk").expect("写缓存");

        purge(&cache).expect("清理应成功");

        assert!(
            database.is_file(),
            "清缓存绝不能碰数据库——那是唯一的审计依据"
        );
        assert_eq!(
            std::fs::read(&database).expect("读数据库"),
            "审计记录".as_bytes(),
            "数据库的内容一个字节都不能变"
        );
    }

    #[test]
    fn a_junction_inside_the_cache_is_removed_without_touching_its_target() {
        // 缓存目录里如果有一个指向上级目录的联接，「清缓存」必须:
        // ① 删掉这个链接本身；② **绝不**走进它去删目标。
        //
        // 两条都要断言。只断言①（或者只断言②）都漏得掉真实缺陷：
        // 只要链接还在，用户就会一直看到「缓存没清干净」；
        // 而一旦走进链接，后果是删掉缓存目录外的东西。
        let (temp, cache) = shaped_cache();
        let outside = temp.path().join("外面");
        std::fs::create_dir_all(&outside).expect("建外部目录");
        std::fs::write(outside.join("别删我.txt"), "MINE".as_bytes()).expect("写外部文件");

        let link = cache.join("链接");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .expect("无法调用 mklink");
        // 建不出来就直接红：静默跳过会让这条测试永远「绿」着什么都不验。
        assert!(
            output.status.success(),
            "无法创建 junction，本测试无法验证「不跟随链接」：{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(link.exists(), "链接应当已经建出来");

        let report = purge(&cache).expect("清理应成功");

        assert!(
            outside.join("别删我.txt").is_file(),
            "联接指向的目录必须原样保留，报告：{report:?}"
        );
        assert_eq!(
            std::fs::read(outside.join("别删我.txt")).expect("读外部文件"),
            "MINE".as_bytes(),
            "外部文件的内容一个字节都不能变"
        );
        assert!(outside.is_dir(), "外部目录本身也必须还在");
        assert!(!link.exists(), "链接本身要被删掉，否则缓存永远清不干净");
        assert!(report.skipped.is_empty(), "删链接不该留下 skip：{report:?}");
    }

    /// 造一份最小的提取结果。缓存现在存的是整份结果（含证据定位），
    /// 而不只是正文——见 `SessionCache` 的文档。
    fn cached(text: &str) -> crate::domain::types::Extraction {
        crate::domain::types::Extraction {
            file_id: "f1".to_owned(),
            source_fingerprint: fingerprint("v1"),
            status: crate::domain::types::ExtractionStatus::Ok,
            text: text.to_owned(),
            evidence: vec![crate::domain::types::Evidence {
                locator: "chars:0-3".to_owned(),
                excerpt: text.to_owned(),
            }],
            truncated: false,
            code: None,
        }
    }

    #[test]
    fn the_session_cache_misses_when_the_file_changed() {
        // 同一个 fileId 的文件被改过之后，缓存必须**不命中**。
        // 命中会把旧正文当成新文件的内容，而下游会拿它去分类——
        // 那是一个用户完全看不出来的错误结论。
        let mut cache = SessionCache::new();
        cache.put("f1", &fingerprint("v1"), cached("旧正文"));

        assert_eq!(
            cache
                .get("f1", &fingerprint("v1"))
                .map(|hit| hit.text.as_str()),
            Some("旧正文")
        );
        assert!(
            cache.get("f1", &fingerprint("v2")).is_none(),
            "指纹变了就不能命中"
        );
        assert!(cache.is_empty(), "错过一次之后旧条目应当被丢掉，不占内存");
    }

    #[test]
    fn the_session_cache_keeps_the_evidence_locators_too() {
        // 证据定位是提取结果的一部分（规格 6.4 要用它核对模型给的
        // `evidenceLocator`）。只缓存正文的话，那条核对就得重新提取一遍。
        let mut cache = SessionCache::new();
        cache.put("f1", &fingerprint("v1"), cached("旧正文"));

        let hit = cache.get("f1", &fingerprint("v1")).expect("应当命中");
        assert_eq!(hit.evidence.len(), 1);
        assert_eq!(hit.evidence[0].locator, "chars:0-3");
    }

    #[test]
    fn clearing_the_session_cache_drops_everything() {
        let mut cache = SessionCache::new();
        cache.put("f1", &fingerprint("v1"), cached("a"));
        cache.put("f2", &fingerprint("v1"), cached("b"));
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn a_cache_file_name_cannot_escape_the_cache_directory() {
        for hostile in ["..\\..\\x", "../../x", r"a\b"] {
            assert!(temp_file(hostile).is_err(), "{hostile} 这种名字必须被拒绝");
        }
    }

    #[test]
    fn a_read_only_file_is_still_purged() {
        // 只读位会让 Windows 的 `remove_file` 直接 ACCESS_DENIED。
        // 而缓存文件很容易带上这个位（从 zip 解出、从备份还原）。
        // 不处理的话用户会稳定地看到「点了清理但缓存里还有东西」，
        // 且他自己没有任何办法修好。
        let (_temp, cache) = shaped_cache();
        let stubborn = cache.join("只读.tmp");
        std::fs::write(&stubborn, b"x").expect("写文件");
        let mut permissions = std::fs::metadata(&stubborn).expect("读属性").permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&stubborn, permissions).expect("设只读");

        let report = purge(&cache).expect("清理应成功");

        assert!(
            report.skipped.is_empty(),
            "只读文件不该留下 skip：{report:?}"
        );
        assert!(!stubborn.exists(), "只读文件也必须被清掉");
    }
}
