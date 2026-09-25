//! 目录枚举。
//!
//! 规格 6.1 的扫描限制与跳过规则集中在这里。
//! 规格 INV-01：本模块**只读**——不创建、不删除、不重命名任何东西。

use std::fs;
use std::os::windows::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use crate::domain::{errors::AppError, types::RelPath};
// 属性的**位与判断**都来自 platform 层（规格 3.2：这一层之外不允许直接
// 调用 Win32）。扫描器已经拿到了 metadata，所以用按位的纯函数，
// 而不是按路径再取一次属性——见 `platform::windows::attributes` 的文档。
use crate::platform::windows::attributes;

/// 扫描参数。默认值取自规格 6.1。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkOptions {
    pub recursive: bool,
    pub max_files: u32,
    pub max_depth: u32,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            max_files: 10_000,
            max_depth: 20,
        }
    }
}

/// 跳过原因。集中定义成常量，避免散落的字面量让统计口径漂移。
pub mod skip_codes {
    pub const HIDDEN_OR_SYSTEM: &str = "HIDDEN_OR_SYSTEM";
    pub const REPARSE_POINT: &str = "REPARSE_POINT";
    pub const SKIPPED_DIR: &str = "SKIPPED_DIR";
    pub const TEMP_DOWNLOAD: &str = "TEMP_DOWNLOAD";
    pub const PERMISSION_DENIED: &str = "PERMISSION_DENIED";
    pub const DEPTH_EXCEEDED: &str = "DEPTH_EXCEEDED";
    pub const TOO_LARGE: &str = "TOO_LARGE";
    pub const SPECIAL_FILE: &str = "SPECIAL_FILE";
    pub const HARD_LINK: &str = "HARD_LINK";
    pub const OFFLINE_PLACEHOLDER: &str = "OFFLINE_PLACEHOLDER";
}

/// 规格 6.1：跳过这些目录名（版本控制、依赖、构建缓存、编辑器元数据）。
const SKIPPED_DIR_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
];

/// 规格 6.1：尚未完成的下载文件。
const TEMP_EXTENSIONS: &[&str] = &[".part", ".crdownload", ".tmp"];

/// 枚举到的一个条目。可能是文件，也可能是「被跳过的文件」。
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub relative_path: RelPath,
    /// 小写扩展名，含前导点；无扩展名时为空串。
    pub extension: String,
    /// `None` 表示这是一个可参与整理的文件；`Some(code)` 表示被跳过。
    pub skip_code: Option<&'static str>,
    pub size: u64,
    pub modified_ns: String,
    pub file_id: String,
}

#[derive(Debug, Clone)]
pub struct WalkResult {
    pub entries: Vec<WalkEntry>,
    /// 规格 6.1：达到上限时**必须**如实标记截断，不能把部分扫描当成完整扫描。
    pub truncated: bool,
}

/// 枚举 `root` 下的条目。
///
/// 根目录自身不可读会返回 `Err`（规格 6.1：根目录不可读则任务失败）；
/// 子目录不可读只记录该目录的跳过原因，不影响其余部分。
pub fn walk(root: &Path, _volume_id: &str, options: &WalkOptions) -> Result<WalkResult, AppError> {
    walk_cancellable(root, _volume_id, options, &|| false)?.ok_or_else(|| {
        AppError::new(
            crate::domain::errors::codes::INTERNAL,
            "不可取消扫描意外进入取消状态",
        )
    })
}

/// 可取消的枚举。返回 `None` 表示已观察到取消请求，不保留不完整结果。
pub fn walk_cancellable(
    root: &Path,
    _volume_id: &str,
    options: &WalkOptions,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<WalkResult>, AppError> {
    // 先确认根目录可读，避免「一个文件都没扫到」和「根目录没权限」被混为一谈
    fs::read_dir(root).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("根目录不可读: {e}"),
        )
    })?;

    let mut state = WalkState {
        entries: Vec::new(),
        usable_count: 0,
        truncated: false,
        options: *options,
    };

    let mut rel: RelPath = Vec::new();
    match visit(root, &mut rel, 0, &mut state, cancelled) {
        Ok(true) => return Ok(None),
        Ok(false) => {}
        Err(e) if e.code == crate::domain::errors::codes::PERMISSION_DENIED => {
            // 只有根这一层不可读才算任务失败，visit 内部对子目录已自行降级
            return Err(e);
        }
        Err(e) => return Err(e),
    }

    // 规格 T02：分页结果稳定排序。按相对路径组件逐项比较，
    // 保证「相同输入得到相同顺序」，翻页时不会因顺序变化而漏项或重复。
    state
        .entries
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(Some(WalkResult {
        entries: state.entries,
        truncated: state.truncated,
    }))
}

struct WalkState {
    entries: Vec<WalkEntry>,
    /// 已收集的**普通文件**数。
    ///
    /// 单独计数而不是每次遍历 `entries`：上限判断在枚举循环里逐条调用，
    /// 现算会让整个扫描退化成 O(n²)。
    usable_count: u32,
    truncated: bool,
    options: WalkOptions,
}

impl WalkState {
    /// 是否已经到达文件数上限。
    ///
    /// 规格 6.1 的上限针对的是**普通文件**。若把被跳过的条目也计入，
    /// 一个装满 `.tmp`、隐藏文件或 `.git` 的目录会先把额度吃光，
    /// 真正要整理的文件反而一条都扫不到。
    fn at_limit(&self) -> bool {
        self.usable_count >= self.options.max_files
    }

    fn push(&mut self, entry: WalkEntry) {
        let is_usable = entry.skip_code.is_none();

        // 只有可整理的文件才受上限约束。被跳过的条目仍然记录，
        // 否则界面无法解释「这个文件夹里明明有东西，为什么结果是空的」。
        if is_usable && self.at_limit() {
            self.truncated = true;
            return;
        }

        if is_usable {
            self.usable_count += 1;
        }
        self.entries.push(entry);
    }
}

fn visit(
    dir: &Path,
    rel: &mut RelPath,
    depth: u32,
    state: &mut WalkState,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AppError> {
    if cancelled() {
        return Ok(true);
    }
    if state.at_limit() {
        state.truncated = true;
        return Ok(false);
    }

    let reader = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // 规格 6.1：权限错误按文件（这里是目录）记录，不中断整次扫描
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension: String::new(),
                skip_code: Some(skip_codes::PERMISSION_DENIED),
                size: 0,
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            return Ok(false);
        }
        Err(e) => {
            return Err(AppError::new(
                crate::domain::errors::codes::PERMISSION_DENIED,
                format!("读取目录失败: {e}"),
            ))
        }
    };

    for item in reader {
        if cancelled() {
            return Ok(true);
        }
        if state.at_limit() {
            state.truncated = true;
            return Ok(false);
        }

        let item = match item {
            Ok(i) => i,
            Err(_) => continue,
        };
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();

        // 一次 stat 拿全：属性、类型、大小、时间。
        // 不用 DirEntry::file_type()，因为它对目录联接（junction）的判定
        // 不如 FILE_ATTRIBUTE_REPARSE_POINT 可靠。
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                rel.push(name);
                state.push(WalkEntry {
                    relative_path: rel.clone(),
                    extension: String::new(),
                    skip_code: Some(skip_codes::PERMISSION_DENIED),
                    size: 0,
                    modified_ns: "0".to_owned(),
                    file_id: String::new(),
                });
                rel.pop();
                continue;
            }
            Err(_) => continue,
        };

        let attrs = meta.file_attributes();

        // 云占位文件可能在读取时触发网络下载，v0.1 明确不读取。
        if attributes::is_offline(attrs) {
            rel.push(name);
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension: String::new(),
                skip_code: Some(skip_codes::OFFLINE_PLACEHOLDER),
                size: meta.len(),
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        // 重解析点：符号链接、目录联接、云占位符都在这一类。
        // 规格 1.2 要求跳过，且**不跟随**走出批准范围。
        if attributes::is_reparse_point(attrs) {
            rel.push(name);
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension: String::new(),
                skip_code: Some(skip_codes::REPARSE_POINT),
                size: 0,
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        // 隐藏/系统项
        if attributes::is_hidden_or_system(attrs) {
            rel.push(name);
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension: String::new(),
                skip_code: Some(skip_codes::HIDDEN_OR_SYSTEM),
                size: 0,
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        if meta.is_dir() {
            if SKIPPED_DIR_NAMES
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&name))
            {
                rel.push(name);
                state.push(WalkEntry {
                    relative_path: rel.clone(),
                    extension: String::new(),
                    skip_code: Some(skip_codes::SKIPPED_DIR),
                    size: 0,
                    modified_ns: "0".to_owned(),
                    file_id: String::new(),
                });
                rel.pop();
                continue;
            }

            if !state.options.recursive {
                continue;
            }
            if depth + 1 > state.options.max_depth {
                state.truncated = true;
                rel.push(name);
                state.push(WalkEntry {
                    relative_path: rel.clone(),
                    extension: String::new(),
                    skip_code: Some(skip_codes::DEPTH_EXCEEDED),
                    size: 0,
                    modified_ns: "0".to_owned(),
                    file_id: String::new(),
                });
                rel.pop();
                continue;
            }

            rel.push(name);
            if visit(&path, rel, depth + 1, state, cancelled)? {
                return Ok(true);
            }
            rel.pop();
            continue;
        }

        if !meta.is_file() {
            // 命名管道、设备等：不是普通文件，v0.1 不处理
            rel.push(name);
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension: String::new(),
                skip_code: Some(skip_codes::SPECIAL_FILE),
                size: 0,
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        // —— 普通文件 ——
        let extension = extension_of(&name);
        rel.push(name);

        if TEMP_EXTENSIONS.iter().any(|e| *e == extension) {
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension,
                skip_code: Some(skip_codes::TEMP_DOWNLOAD),
                size: meta.len(),
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        if meta.len() > crate::scanner::snapshot::MAX_HASHABLE_BYTES {
            state.push(WalkEntry {
                relative_path: rel.clone(),
                extension,
                skip_code: Some(skip_codes::TOO_LARGE),
                size: meta.len(),
                modified_ns: "0".to_owned(),
                file_id: String::new(),
            });
            rel.pop();
            continue;
        }

        // 身份、大小和时间都取自同一个不共享写入/删除的句柄，避免拼出混合快照。
        let (file_id, stable_size, modified_ns) =
            match crate::platform::windows::open_source_for_snapshot(&path) {
                Ok(f) => {
                    let identity = crate::platform::windows::file_identity(&f);
                    let stable_meta = crate::platform::windows::size_and_modified(&f);
                    let links = crate::platform::windows::file_link_count(&f);
                    match (identity, stable_meta, links) {
                        (Ok(identity), Ok((size, modified)), Ok(link_count)) if link_count <= 1 => {
                            (identity.file_id_string(), size, modified)
                        }
                        (Ok(identity), Ok((size, modified)), Ok(_)) => {
                            state.push(WalkEntry {
                                relative_path: rel.clone(),
                                extension,
                                skip_code: Some(skip_codes::HARD_LINK),
                                size,
                                modified_ns: modified,
                                file_id: identity.file_id_string(),
                            });
                            rel.pop();
                            continue;
                        }
                        _ => {
                            state.push(WalkEntry {
                                relative_path: rel.clone(),
                                extension,
                                skip_code: Some(skip_codes::PERMISSION_DENIED),
                                size: meta.len(),
                                modified_ns: "0".to_owned(),
                                file_id: String::new(),
                            });
                            rel.pop();
                            continue;
                        }
                    }
                }
                Err(_) => {
                    state.push(WalkEntry {
                        relative_path: rel.clone(),
                        extension,
                        skip_code: Some(skip_codes::PERMISSION_DENIED),
                        size: meta.len(),
                        modified_ns: "0".to_owned(),
                        file_id: String::new(),
                    });
                    rel.pop();
                    continue;
                }
            };

        state.push(WalkEntry {
            relative_path: rel.clone(),
            extension,
            skip_code: None,
            size: stable_size,
            modified_ns,
            file_id,
        });
        rel.pop();
    }

    Ok(false)
}

/// 取小写扩展名，含前导点。无扩展名返回空串。
///
/// 规格 7.1 第 1 条：无扩展名的文件保持无扩展名，
/// 因此这里必须能把「没有扩展名」和「扩展名是空串」区分开。
fn extension_of(name: &str) -> String {
    match name.rfind('.') {
        // 前导点开头的是隐藏文件（.gitignore），不是「扩展名是 .gitignore」
        Some(0) | None => String::new(),
        Some(idx) => name[idx..].to_lowercase(),
    }
}

/// 把相对路径拼回人类可读形式，仅用于展示与日志。
///
/// 规格 5.1：**安全判断一律用组件数组**，这个函数不许参与任何校验。
pub fn join_for_display(relative: &[String]) -> String {
    relative.join("\\")
}

/// 供测试构造路径用。
pub fn path_of(root: &Path, relative: &[String]) -> PathBuf {
    let mut p = root.to_path_buf();
    for c in relative {
        p.push(c);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_extraction_handles_the_edge_cases() {
        assert_eq!(extension_of("a.pdf"), ".pdf");
        assert_eq!(extension_of("A.PDF"), ".pdf");
        assert_eq!(extension_of("no_ext"), "");
        // 前导点是隐藏文件标记，不是扩展名
        assert_eq!(extension_of(".gitignore"), "");
        assert_eq!(extension_of("archive.tar.gz"), ".gz");
        assert_eq!(extension_of("name."), ".");
    }

    #[test]
    fn skipped_entries_do_not_consume_the_file_limit() {
        // 回归测试：规格 6.1 的上限针对**普通文件**。
        // 早期实现用 entries.len() 判上限，于是一个塞满未完成下载的目录
        // 会先把额度吃光，真正的文件一条都扫不到。
        let tmp = tempfile::tempdir().expect("建临时目录");
        for i in 0..20 {
            fs::write(tmp.path().join(format!("part{i}.crdownload")), b"x").expect("写文件");
        }
        for i in 0..3 {
            fs::write(tmp.path().join(format!("real{i}.txt")), b"x").expect("写文件");
        }

        let opts = WalkOptions {
            recursive: true,
            max_files: 3,
            max_depth: 20,
        };
        let result = walk(tmp.path(), "V", &opts).expect("扫描应成功");

        let usable = result
            .entries
            .iter()
            .filter(|e| e.skip_code.is_none())
            .count();
        assert_eq!(usable, 3, "被跳过的条目不应占用普通文件额度");
        assert!(
            !result.truncated,
            "上限按普通文件算，3 个正好触顶但不应截断"
        );
        assert!(
            result.entries.len() > 3,
            "被跳过的条目仍要保留，界面才能解释为什么文件夹看起来是空的"
        );
    }

    #[test]
    fn walk_depth_limit_is_reported_not_silently_dropped() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        let mut p = tmp.path().to_path_buf();
        for i in 0..4 {
            p.push(format!("d{i}"));
        }
        fs::create_dir_all(&p).expect("建深层目录");
        fs::write(p.join("deep.txt"), b"x").expect("写文件");

        let opts = WalkOptions {
            recursive: true,
            max_files: 100,
            max_depth: 2,
        };
        let result = walk(tmp.path(), "V", &opts).expect("扫描应成功");

        assert!(
            result
                .entries
                .iter()
                .any(|e| e.skip_code == Some(skip_codes::DEPTH_EXCEEDED)),
            "超过深度的目录必须留下跳过记录，不能静默丢弃"
        );
        assert!(
            !result.entries.iter().any(|e| e.skip_code.is_none()),
            "被深度限制挡住时不应扫到里面的文件"
        );
    }

    #[test]
    fn walk_respects_max_files_and_marks_truncated() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        for i in 0..10 {
            fs::write(tmp.path().join(format!("f{i}.txt")), b"x").expect("写文件");
        }

        let opts = WalkOptions {
            recursive: true,
            max_files: 3,
            max_depth: 20,
        };
        let result = walk(tmp.path(), "V", &opts).expect("扫描应成功");

        assert_eq!(result.entries.len(), 3);
        assert!(result.truncated, "达到上限必须标记 truncated");
    }

    #[test]
    fn walk_skips_temp_download_extensions() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        fs::write(tmp.path().join("done.pdf"), b"x").expect("写文件");
        fs::write(tmp.path().join("half.crdownload"), b"x").expect("写文件");

        let result = walk(tmp.path(), "V", &WalkOptions::default()).expect("扫描应成功");
        let skipped: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.skip_code == Some(skip_codes::TEMP_DOWNLOAD))
            .collect();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].relative_path, vec!["half.crdownload".to_owned()]);
    }

    #[test]
    fn walk_skips_known_directories() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        let git = tmp.path().join(".git");
        fs::create_dir(&git).expect("建目录");
        fs::write(git.join("config"), b"x").expect("写文件");

        let result = walk(tmp.path(), "V", &WalkOptions::default()).expect("扫描应成功");
        assert!(
            result
                .entries
                .iter()
                .any(|e| e.skip_code == Some(skip_codes::SKIPPED_DIR)),
            ".git 必须被跳过并留下记录"
        );
    }

    #[test]
    fn walk_is_deterministic() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        for name in ["c.txt", "a.txt", "b.txt"] {
            fs::write(tmp.path().join(name), b"x").expect("写文件");
        }

        let first = walk(tmp.path(), "V", &WalkOptions::default()).expect("扫描");
        let second = walk(tmp.path(), "V", &WalkOptions::default()).expect("扫描");
        assert_eq!(
            first.entries.len(),
            second.entries.len(),
            "两次扫描条目数必须一致"
        );
        for (a, b) in first.entries.iter().zip(second.entries.iter()) {
            assert_eq!(a.relative_path, b.relative_path, "排序必须稳定");
        }
    }
}
