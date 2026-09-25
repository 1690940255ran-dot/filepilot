//! 根目录授权。
//!
//! 规格 1.2「根目录限制」与 3.3「根目录由原生目录选择框取得，后端生成 rootId」。
//!
//! 核心约束：[`ApprovedRoot`] 只能由 [`approve_root`] 构造，
//! **不能从 IPC 反序列化得到**。前端和模型后续只能提交 `rootId`，
//! 由后端查表换回授权对象——这样「前端伪造一个路径」在类型层面就不成立。

use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

use crate::domain::{errors::AppError, types::Fingerprint};
use crate::platform::windows::{self, FileIdentity, VolumeCheck};

/// 一个已通过全部检查的根目录。
///
/// 字段全部私有：外部只能通过方法读取，无法自行构造。
#[derive(Debug, Clone)]
pub struct ApprovedRoot {
    id: Uuid,
    /// 规范化后的真实路径（符号链接已解析）。
    canonical: PathBuf,
    /// 用于界面展示的原始路径。
    display: String,
    volume_id: String,
    identity: FileIdentity,
}

impl ApprovedRoot {
    /// 把一次新的原生选择绑定到数据库里已有的 rootId。
    ///
    /// 仅供后端在已经核对卷身份和目录身份后使用；前端仍然不能构造授权根。
    pub(crate) fn with_id(&self, id: Uuid) -> Self {
        Self {
            id,
            canonical: self.canonical.clone(),
            display: self.display.clone(),
            volume_id: self.volume_id.clone(),
            identity: self.identity.clone(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn canonical(&self) -> &Path {
        &self.canonical
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn volume_id(&self) -> &str {
        &self.volume_id
    }

    pub fn identity(&self) -> &FileIdentity {
        &self.identity
    }

    /// 重新核对磁盘上的这个根还是不是当初授权的那个。
    ///
    /// 规格 3.3：程序重启后执行/撤销需重新选择并验证同一根目录与卷身份。
    /// 只比对卷身份与文件身份，**不比对路径字符串**——路径可能被重命名、
    /// 大小写可能变化，但身份不会。
    pub fn verify_still_valid(&self) -> Result<(), AppError> {
        let meta = std::fs::symlink_metadata(&self.canonical).map_err(|_| {
            AppError::new(
                crate::domain::errors::codes::ROOT_CHANGED,
                "根目录已不存在或不可访问",
            )
        })?;
        if !meta.is_dir() {
            return Err(AppError::new(
                crate::domain::errors::codes::ROOT_CHANGED,
                "根目录路径现在指向的不是目录",
            ));
        }

        let file = windows::open_directory(&self.canonical).map_err(|_| {
            AppError::new(
                crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
                "无法打开根目录",
            )
        })?;
        let now = windows::file_identity(&file).map_err(|e| {
            AppError::new(
                crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
                format!("读取根目录身份失败: {e}"),
            )
        })?;

        if now != self.identity {
            return Err(AppError::new(
                crate::domain::errors::codes::ROOT_CHANGED,
                "根目录已被替换为另一个目录（文件身份不一致）",
            ));
        }
        Ok(())
    }

    /// 判断一个相对路径是否稳稳地落在本根目录内。
    ///
    /// 规格 7.3 明确要求**按组件逐级比较**，并点名 `C:\Data` 与
    /// `C:\Database` 这类前缀误判。因此这里不做字符串前缀比较，
    /// 而是先把相对路径规范化成组件序列，再逐级拼到 canonical 上。
    pub fn resolve_within(&self, relative: &[String]) -> Result<PathBuf, AppError> {
        let mut path = self.canonical.clone();
        for component in relative {
            super::path::validate_component(component)?;
            path.push(component);
        }
        Ok(path)
    }

    /// 解析已经存在的源路径。源文件名不受新名称的 80-unit 产品限制，
    /// 但仍必须是不能逃逸的相对组件数组。
    pub fn resolve_existing_within(&self, relative: &[String]) -> Result<PathBuf, AppError> {
        super::path::validate_existing_relative_path(relative)?;
        let mut path = self.canonical.clone();
        for component in relative {
            path.push(component);
        }
        Ok(path)
    }
}

/// 授权一个用户选定的目录作为整理根。
///
/// 顺序有意如此：**先解析真实路径，再做拒绝判断**。反过来会把
/// 「链接指向系统目录」这种情况放过去。
pub fn approve_root(selected: &Path) -> Result<ApprovedRoot, AppError> {
    // 用户主目录这个精确输入无需访问磁盘就能确定应拒绝。先给出可操作的理由，
    // 也避免受限环境中 `canonicalize(USERPROFILE)` 先因权限失败而掩盖真实原因。
    // 这里只是快速拒绝；通过联接/别名指向主目录的路径仍会在下方 canonicalize
    // 之后由 `reject_reason` 再次识别，不能借此绕过。
    if dirs_home()
        .as_deref()
        .is_some_and(|home| same_path(selected, home))
    {
        return Err(AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            "不能直接整理用户主目录本身，请选择一个子目录",
        ));
    }

    // 1) 解析真实路径。canonicalize 会跟随符号链接，这正是我们要的：
    //    后续所有判断都基于「这个目录到底在哪」。
    let canonical = strip_verbatim_prefix(selected.canonicalize().map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            format!("无法解析所选目录的真实路径: {e}"),
        )
    })?);

    if !canonical.is_dir() {
        return Err(AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            "所选路径不是目录",
        ));
    }

    // 2) 拒绝清单（规格 1.2）
    if let Some(reason) = reject_reason(&canonical)? {
        return Err(reason);
    }

    // 3) 卷检查：固定盘 + NTFS（ADR-002）
    match windows::check_volume(&canonical).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            format!("读取卷信息失败: {e}"),
        )
    })? {
        VolumeCheck::Acceptable { .. } => {}
        VolumeCheck::Rejected { code, detail } => {
            return Err(AppError::new(
                code,
                format!("根目录所在的卷不受支持：{detail}"),
            ))
        }
    }

    // 4) 根目录自身不能是重解析点，否则「根内」这个前提就不可靠
    if windows::is_reparse_point(&canonical).unwrap_or(false) {
        return Err(AppError::new(
            crate::domain::errors::codes::REPARSE_POINT,
            "所选目录是一个链接（重解析点），请选择其真实目录",
        ));
    }

    // 5) 取卷身份与文件身份
    // 目录句柄：必须走 backup semantics，理由见 platform::windows::open_directory
    let file = windows::open_directory(&canonical).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("无法打开所选目录: {e}"),
        )
    })?;
    let identity = windows::file_identity(&file).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            format!("读取目录身份失败: {e}"),
        )
    })?;

    Ok(ApprovedRoot {
        id: Uuid::new_v4(),
        volume_id: identity.volume_id_string(),
        identity,
        display: selected.to_string_lossy().into_owned(),
        canonical,
    })
}

/// 规格 1.2 的拒绝清单。
///
/// 返回 `Some(error)` 表示这个目录不能作为整理根。
fn reject_reason(canonical: &Path) -> Result<Option<AppError>, AppError> {
    use crate::domain::errors::codes;

    let deny = |msg: String| Ok(Some(AppError::new(codes::ROOT_NOT_AUTHORIZED, msg)));

    // 盘符根，如 C:\
    if canonical.parent().is_none() || is_drive_root(canonical) {
        return deny(format!(
            "{} 是盘符根目录，不能作为整理根",
            canonical.display()
        ));
    }

    // 用户主目录本身（其下的普通子目录可选）
    if let Some(home) = dirs_home() {
        if same_path(canonical, &home) {
            return deny("不能直接整理用户主目录本身，请选择一个子目录".to_owned());
        }
    }

    // 系统目录与应用数据目录：凡是位于这些根**之下**的一律拒绝
    for base in system_and_app_roots() {
        if is_within(canonical, &base) {
            return deny(format!(
                "{} 位于系统或应用目录 {} 之下，v0.1 不支持",
                canonical.display(),
                base.display()
            ));
        }
    }

    // 已知同步根（OneDrive / Dropbox / Google Drive 的常见位置）
    for base in known_sync_roots() {
        if is_within(canonical, &base) {
            return deny(format!(
                "{} 位于同步目录 {} 之下；v0.1 仅支持不受同步软件管理的本地文件夹",
                canonical.display(),
                base.display()
            ));
        }
    }

    // 源码仓库根及其内部目录
    if let Some(repo) = find_enclosing_repo(canonical) {
        return deny(format!(
            "{} 位于源码仓库 {} 内，不能作为整理根",
            canonical.display(),
            repo.display()
        ));
    }

    Ok(None)
}

fn is_drive_root(path: &Path) -> bool {
    let mut components = path.components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Prefix(_)), Some(Component::RootDir))
    ) && components.next().is_none()
}

/// 用户主目录。用环境变量而不是 `dirs` crate：少一个依赖，且行为可预期。
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|v| strip_verbatim_prefix(PathBuf::from(v)))
}

/// 剥掉 `canonicalize` 在 Windows 上加的 verbatim 前缀。
///
/// **这是一个安全相关的处理**：`std::fs::canonicalize` 返回的是
/// `\\?\C:\Users\me` 这种扩展长度路径，而环境变量（`USERPROFILE`、
/// `SystemRoot`、`LOCALAPPDATA`…）给出的是普通的 `C:\Users\me`。
/// 若不统一，后面基于组件的归属判断会**全部失效**——系统目录、用户主目录、
/// 应用数据目录都会被当成普通路径放行。
///
/// 由于规格 7.1 把目标总路径限制在 240 个 UTF-16 单位以内，
/// 去掉前缀不会触碰到 MAX_PATH 的限制。
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    let Some(rest) = text.strip_prefix(r"\\?\") else {
        return path;
    };
    // `\\?\UNC\server\share` 对应普通的 `\\server\share`
    if let Some(unc) = rest.strip_prefix(r"UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    PathBuf::from(rest)
}

/// 系统目录与应用数据目录。
///
/// 同样要剥 verbatim 前缀：环境变量给的是普通形式。
fn system_and_app_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    for key in [
        "SystemRoot",
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Some(v) = std::env::var_os(key) {
            roots.push(strip_verbatim_prefix(PathBuf::from(v)));
        }
    }
    // 应用自身的数据目录
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(strip_verbatim_prefix(PathBuf::from(local)));
    }
    if let Some(roaming) = std::env::var_os("APPDATA") {
        roots.push(strip_verbatim_prefix(PathBuf::from(roaming)));
    }

    roots
}

/// 系统可获得的已知同步根（规格 1.2：不声称能识别所有第三方同步软件）。
fn known_sync_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(od) = std::env::var_os("OneDrive") {
        roots.push(PathBuf::from(od));
    }
    if let Some(home) = dirs_home() {
        for name in [
            "OneDrive",
            "Dropbox",
            "Google Drive",
            "iCloudDrive",
            "坚果云",
        ] {
            let p = home.join(name);
            if p.is_dir() {
                roots.push(p);
            }
        }
    }
    roots
}

/// 从 `start` 逐级向上找 `.git`，返回仓库根。
///
/// 只向上找到盘符根为止，不做全盘扫描。这样可以拦住「在仓库内部选一个子目录」
/// 这种最容易把源码搅乱的操作。
fn find_enclosing_repo(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        // 目录本身不是仓库（.git 是文件的情况也认，worktree 就是这么存的）
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// 大小写不敏感地比较两个路径是否相同。
fn same_path(a: &Path, b: &Path) -> bool {
    let na = normalize_for_compare(a);
    let nb = normalize_for_compare(b);
    na == nb
}

/// `child` 是否在 `base` 之内（含相等）。
///
/// **按组件比较**：`C:\Data` 不会是 `C:\Database` 的父目录。
fn is_within(child: &Path, base: &Path) -> bool {
    let c = normalize_components(child);
    let b = normalize_components(base);
    if b.is_empty() || c.len() < b.len() {
        return false;
    }
    c[..b.len()] == b[..]
}

/// 把路径拆成「小写化 + 去掉尾部分隔符」的组件数组。
fn normalize_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Prefix(p) => Some(p.as_os_str().to_string_lossy().to_lowercase()),
            Component::RootDir => Some("\\".to_owned()),
            Component::Normal(s) => Some(s.to_string_lossy().to_lowercase()),
            Component::CurDir | Component::ParentDir => None,
        })
        .collect()
}

fn normalize_for_compare(path: &Path) -> String {
    normalize_components(path).join("\\")
}

/// 规格 5.1：`Fingerprint` 要求卷身份。根授权阶段就能填好卷，
/// 文件身份与哈希在每个文件上单独取。
pub fn root_fingerprint(root: &ApprovedRoot) -> Fingerprint {
    Fingerprint {
        volume_id: root.volume_id().to_owned(),
        file_id: root.identity().file_id_string(),
        size: "0".to_owned(),
        modified_ns: "0".to_owned(),
        sha256: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_drive_root() {
        let err = reject_reason(Path::new(r"C:\"))
            .expect("不应报错")
            .expect("应被拒绝");
        assert_eq!(err.code, crate::domain::errors::codes::ROOT_NOT_AUTHORIZED);
    }

    #[test]
    fn data_is_not_treated_as_parent_of_database() {
        // 规格 7.3 点名的前缀误判
        assert!(is_within(Path::new(r"C:\Data\sub"), Path::new(r"C:\Data")));
        assert!(!is_within(Path::new(r"C:\Database"), Path::new(r"C:\Data")));
        assert!(!is_within(
            Path::new(r"C:\Database\x"),
            Path::new(r"C:\Data")
        ));
    }

    #[test]
    fn is_within_is_case_insensitive() {
        assert!(is_within(Path::new(r"c:\DATA\Sub"), Path::new(r"C:\Data")));
    }

    #[test]
    fn path_equality_ignores_trailing_separator_and_case() {
        assert!(same_path(Path::new(r"C:\Temp\"), Path::new(r"c:\temp")));
        assert!(!same_path(Path::new(r"C:\Temp"), Path::new(r"C:\Temp2")));
    }

    #[test]
    fn approve_rejects_system_and_home_directories() {
        // 这两个是本机必然存在的目录，且都应被拒绝
        if let Some(home) = dirs_home() {
            let err = approve_root(&home).expect_err("用户主目录必须被拒绝");
            assert_eq!(err.code, crate::domain::errors::codes::ROOT_NOT_AUTHORIZED);
        }
        if let Ok(windir) = std::env::var("SystemRoot") {
            let err = approve_root(Path::new(&windir)).expect_err("系统目录必须被拒绝");
            assert_eq!(err.code, crate::domain::errors::codes::ROOT_NOT_AUTHORIZED);
        }
    }

    #[test]
    fn approve_rejects_directories_under_app_data() {
        // `tempfile::TempDir` 默认建在 %TEMP%，而 %TEMP% 位于
        // `%LOCALAPPDATA%\Temp`——所以它**必然**被规格 1.2 的「应用数据目录」
        // 规则拒绝。集成测试因此不能直接用 `TempDir::new()`，
        // 必须把它建在一个用户主目录下的普通子目录里（见 tests/support/mod.rs）。
        let tmp = tempfile::tempdir().expect("创建临时目录应成功");
        let target = tmp.path().join("资料");
        std::fs::create_dir(&target).expect("创建子目录应成功");

        let err = approve_root(&target).expect_err("AppData 之下的目录应被拒绝");
        assert_eq!(err.code, crate::domain::errors::codes::ROOT_NOT_AUTHORIZED);
        assert!(
            err.message.contains("系统或应用目录"),
            "拒绝理由要说清是哪条规则，实际是：{}",
            err.message
        );
    }

    #[test]
    fn user_home_itself_is_rejected_but_subdirectories_are_the_intended_target() {
        let home = dirs_home().expect("本机应有 USERPROFILE");
        let err = reject_reason(&home)
            .expect("判断不应出错")
            .expect("主目录本身必须被拒绝");
        assert!(err.message.contains("用户主目录"));

        // 主目录下的**普通子目录**不应命中任何拒绝规则
        let sub = home.join("文档备份");
        assert!(
            reject_reason(&sub).expect("判断不应出错").is_none(),
            "主目录下的普通子目录应当是合法的整理目标"
        );
    }
}
