//! 指纹核对与安全移动原语。
//!
//! 规格 7.3 的全部要求都落在这一层：
//! **在同一个持有的文件句柄上**校验身份、大小、时间与内容，
//! 再**通过那个句柄**重命名。绝不能「验证一个文件、按路径重新打开另一个文件执行」。

use std::fs::File;

use crate::domain::{errors::codes, errors::AppError, types::Fingerprint};
use crate::platform::windows;
use crate::safety::path::{
    validate_existing_relative_path, validate_relative_path, MAX_TOTAL_PATH_UTF16,
};
use crate::safety::root::ApprovedRoot;

/// 一次成功移动的证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveReceipt {
    /// 移动后源文件的卷内身份（应当与 `expected.file_id` 一致）。
    pub file_id: String,
    /// 移动后核对到的内容哈希。
    pub sha256: String,
    pub size: String,
    /// 目标相对路径，供日志与撤销计划使用。
    pub target: Vec<String>,
}

/// 核对一个已打开句柄的内容与身份是否仍与 `expected` 相符。
///
/// 三类证据分开检查并给出**不同的错误码**，因为用户能做的事不一样：
/// - 身份不符 → 这个路径上已经是别的文件了
/// - 大小/时间不符 → 文件被改过
/// - 内容哈希不符 → 大小与时间恰好相同的篡改（规格 E01 专门点名的情况）
pub fn verify_handle_matches(
    file: &File,
    expected: &Fingerprint,
    actual_sha256: &str,
) -> Result<(), AppError> {
    let identity = windows::file_identity(file)
        .map_err(|e| AppError::new(codes::PERMISSION_DENIED, format!("读取文件身份失败: {e}")))?;

    if identity.volume_id_string() != expected.volume_id {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "文件所在的卷与计划中记录的不一致",
        ));
    }
    if identity.file_id_string() != expected.file_id {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "该路径上现在是另一个文件（卷内身份不符）",
        ));
    }

    let (size, modified_ns) = windows::size_and_modified(file).map_err(|e| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("读取文件大小或修改时间失败: {e}"),
        )
    })?;
    if size.to_string() != expected.size {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "文件大小与计划中记录的不一致",
        ));
    }
    if modified_ns != expected.modified_ns {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "文件修改时间与计划中记录的不一致",
        ));
    }

    match expected.sha256.as_deref() {
        Some(want) if want != actual_sha256 => Err(AppError::new(
            codes::SOURCE_CHANGED,
            "文件内容与计划中记录的不一致",
        )),
        // 规格 5.1：生成可执行计划时 sha256 必须非 null。
        // 这里再挡一道，避免调用方绕过计划直接执行。
        None => Err(AppError::new(
            codes::SOURCE_CHANGED,
            "计划没有绑定文件内容哈希，拒绝执行",
        )),
        _ => Ok(()),
    }
}

/// 在批准根内把文件移动到新的相对路径，**绝不覆盖已有目标**。
///
/// 规格 T03：本原语当前只用于测试，不接通 UI 执行。
///
/// 执行顺序（顺序本身就是安全性的一部分）：
/// 1. 校验源与目标的相对路径组件；
/// 2. 解析成绝对路径，确认仍在根内；
/// 3. 打开源文件句柄（不共享 WRITE/DELETE，阻止校验期间被改动）；
/// 4. **在该句柄上**核对身份、大小、时间与内容哈希；
/// 5. 检查目标父目录存在且是目录、目标不存在；
/// 6. 通过同一句柄执行 `ReplaceIfExists = FALSE` 的重命名；
/// 7. 复核目标身份。
pub fn move_no_replace(
    root: &ApprovedRoot,
    source: &[String],
    target: &[String],
    expected: &Fingerprint,
) -> Result<MoveReceipt, AppError> {
    move_no_replace_with(
        root,
        source,
        target,
        expected,
        MoveFailPoints::Apply,
        |_| Ok(()),
    )
}

/// 这次移动该用哪一对故障注入点。
///
/// 必须区分开：正向整理与撤销走的是同一套安全原语，但恢复流程对两者的
/// 判定方向**正好相反**（一个是「文件到目标了吗」，一个是「文件回原处了吗」）。
/// 用同一对注入点，就没法单独验证「撤销在改名之后崩掉」这条路径究竟走对了没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveFailPoints {
    /// 正向整理：`before-rename` / `after-rename`。
    Apply,
    /// 撤销：`undo-before-rename` / `undo-after-rename`。
    Undo,
}

impl MoveFailPoints {
    fn before_rename(self) -> crate::platform::failpoint::FailPointName {
        match self {
            MoveFailPoints::Apply => crate::platform::failpoint::FAILPOINT_BEFORE_RENAME,
            MoveFailPoints::Undo => crate::platform::failpoint::FAILPOINT_UNDO_BEFORE_RENAME,
        }
    }

    fn after_rename(self) -> crate::platform::failpoint::FailPointName {
        match self {
            MoveFailPoints::Apply => crate::platform::failpoint::FAILPOINT_AFTER_RENAME,
            MoveFailPoints::Undo => crate::platform::failpoint::FAILPOINT_UNDO_AFTER_RENAME,
        }
    }
}

/// 与 [`move_no_replace`] 相同，但在「核对通过」与「执行重命名」之间
/// 给调用方一个插入点。
///
/// 执行器用它来写 durable journal：规格 8.2 要求 `prepared` 必须**先于**
/// 真实的 rename 落盘。若那一步失败，本函数会连同整个移动一起放弃——
/// 宁可什么都不做，也不要做出一个日志里查不到的改动。
///
/// 回调拿得到源文件句柄，因此它记录的指纹与即将被移动的文件是同一个；
/// 路径在这期间被换掉也不影响。
pub fn move_no_replace_with<F>(
    root: &ApprovedRoot,
    source: &[String],
    target: &[String],
    expected: &Fingerprint,
    fail_points: MoveFailPoints,
    before_rename: F,
) -> Result<MoveReceipt, AppError>
where
    F: FnOnce(&File) -> Result<(), AppError>,
{
    validate_existing_relative_path(source)?;
    match fail_points {
        MoveFailPoints::Apply => validate_relative_path(target)?,
        // 撤销目标是数据库中记录的原路径，不是用户新建议的名称。
        // 它仍需满足不可逃逸的组件规则，但不能套用“新名称 80 UTF-16”限制。
        MoveFailPoints::Undo => validate_existing_relative_path(target)?,
    }

    if source == target {
        return Err(AppError::new(codes::INVALID_PATH, "源与目标相同，无需移动"));
    }

    // 根可能在授权之后被替换，每次执行前重新确认
    root.verify_still_valid()?;

    let source_path = root.resolve_existing_within(source)?;
    let target_path = match fail_points {
        MoveFailPoints::Apply => root.resolve_within(target)?,
        MoveFailPoints::Undo => root.resolve_existing_within(target)?,
    };

    use std::os::windows::ffi::OsStrExt as _;
    if fail_points == MoveFailPoints::Apply
        && target_path.as_os_str().encode_wide().count() > MAX_TOTAL_PATH_UTF16
    {
        return Err(AppError::new(
            codes::PATH_TOO_LONG,
            format!("目标完整路径超过 {MAX_TOTAL_PATH_UTF16} 个 UTF-16 字符单位"),
        ));
    }

    // 逐级打开并保护根、源父目录、目标父目录。最终组件以
    // FILE_FLAG_OPEN_REPARSE_POINT 打开，因此预先存在的 junction 也不会被跟随。
    // 所有句柄保持到 rename 完成，使检查后的目录不能被替换。
    let mut directory_guards = lock_directory_chain(root, &source[..source.len() - 1])?;
    directory_guards.extend(lock_directory_chain(root, &target[..target.len() - 1])?);

    // ---- 目标父目录与目标自身的先决条件 ----
    let parent = target_path
        .parent()
        .ok_or_else(|| AppError::new(codes::INVALID_PATH, "目标路径没有父目录"))?;

    match std::fs::symlink_metadata(parent) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(AppError::new(
                codes::TARGET_PARENT_IS_FILE,
                "目标的父路径是一个文件，不能作为目录使用",
            ))
        }
        Err(_) => {
            return Err(AppError::new(
                codes::INVALID_PATH,
                "目标的父目录不存在；本阶段不创建目录",
            ))
        }
    }

    // 目标已存在就直接拒绝。真正的「不覆盖」由内核保证（ReplaceIfExists=FALSE），
    // 这次预检查只是为了给出更清楚的错误信息。
    if std::fs::symlink_metadata(&target_path).is_ok() {
        return Err(AppError::new(codes::TARGET_EXISTS, "目标位置已存在同名项"));
    }

    // ---- 打开源并核对 ----
    let file = windows::open_source_for_rename(&source_path).map_err(|e| {
        // 打不开最常见的原因是被别的程序占用；这与「没有权限」要分开报，
        // 因为用户能做的事不同（前者关掉占用程序，后者改权限或换目录）。
        let code = match e.raw_os_error() {
            Some(2 | 3) => codes::SOURCE_MISSING,
            Some(32 | 33) => codes::FILE_BUSY,
            Some(5) => codes::PERMISSION_DENIED,
            _ => codes::PERMISSION_DENIED,
        };
        AppError::new(code, format!("无法打开源文件: {e}")).retryable()
    })?;

    let actual_sha256 = crate::scanner::snapshot::sha256_of_handle(&file)
        .map_err(|e| AppError::new(codes::PERMISSION_DENIED, format!("读取文件内容失败: {e}")))?;

    verify_handle_matches(&file, expected, &actual_sha256)?;

    // ---- 落盘意图（由调用方提供）----
    // 这一步失败就整体放弃：文件一个字节都还没动，报错是安全的。
    before_rename(&file)?;

    // 故障注入：意图已落盘、重命名还没发生。
    crate::platform::failpoint::fail_point(fail_points.before_rename());

    // ---- 执行重命名 ----
    windows::rename_by_handle_no_replace(&file, &target_path).map_err(|e| {
        // 这里最常见的失败是「目标在预检查之后被别的进程创建了」。
        // 内核返回 ERROR_ALREADY_EXISTS，必须如实报成 TARGET_EXISTS，
        // 而不是含糊的 IO 错误——用户需要知道是别人抢先创建了目标。
        let code = match e.raw_os_error() {
            Some(80 | 183) => codes::TARGET_EXISTS,
            Some(32 | 33) => codes::FILE_BUSY,
            Some(5) => codes::PERMISSION_DENIED,
            _ => codes::PERMISSION_DENIED,
        };
        AppError::new(code, format!("重命名被系统拒绝: {e}"))
    })?;

    // 故障注入：**最危险的一个点**。
    // 磁盘已经变了，而日志里只有 `prepared` —— 恢复流程必须靠磁盘事实判断，
    // 不能因为「日志说没做」就当成没做。
    crate::platform::failpoint::fail_point(fail_points.after_rename());

    // ---- 复核目标 ----
    // 重命名成功后句柄仍然有效，直接用它读，避免「按路径重新打开」带来的偏差。
    let after = windows::file_identity(&file).map_err(|e| {
        AppError::new(
            codes::RECOVERY_REQUIRED,
            format!("文件已移动，复核目标身份失败: {e}"),
        )
    })?;
    if after.file_id_string() != expected.file_id {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "重命名后目标身份与源不一致，已停止",
        ));
    }

    Ok(MoveReceipt {
        file_id: after.file_id_string(),
        sha256: actual_sha256,
        size: expected.size.clone(),
        target: target.to_vec(),
    })
}

pub(crate) fn lock_directory_chain(
    root: &ApprovedRoot,
    relative_directories: &[String],
) -> Result<Vec<File>, AppError> {
    let mut guards = Vec::with_capacity(relative_directories.len() + 1);
    let root_guard =
        windows::open_directory_guard(root.canonical()).map_err(map_directory_error)?;
    reject_case_sensitive_directory(&root_guard)?;
    let root_identity = windows::file_identity(&root_guard).map_err(|e| {
        AppError::new(
            codes::ROOT_NOT_AUTHORIZED,
            format!("读取根目录身份失败: {e}"),
        )
    })?;
    if &root_identity != root.identity() {
        return Err(AppError::new(codes::ROOT_CHANGED, "批准根目录已被替换"));
    }
    guards.push(root_guard);

    let mut current = root.canonical().to_path_buf();
    for component in relative_directories {
        current.push(component);
        let guard = windows::open_directory_guard(&current).map_err(map_directory_error)?;
        reject_case_sensitive_directory(&guard)?;
        guards.push(guard);
    }
    Ok(guards)
}

/// 在保护根与全部祖先目录句柄期间读取一个文件指纹。
///
/// 仅 `NotFound` 表示不存在；权限、重解析点和身份变化都必须显式失败，不能被
/// 恢复流程误判成“这个位置是空的”。
pub(crate) fn guarded_fingerprint_at(
    root: &ApprovedRoot,
    relative: &[String],
    volume_id: &str,
) -> Result<Option<Fingerprint>, AppError> {
    validate_existing_relative_path(relative)?;
    if relative.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "文件相对路径不能为空"));
    }
    root.verify_still_valid()?;
    let mut ancestor = root.canonical().to_path_buf();
    for component in &relative[..relative.len() - 1] {
        ancestor.push(component);
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(AppError::new(
                    codes::PERMISSION_DENIED,
                    format!("无法读取文件祖先目录: {error}"),
                ))
            }
        }
    }
    let _guards = lock_directory_chain(root, &relative[..relative.len() - 1])?;
    let path = root.resolve_existing_within(relative)?;
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(AppError::new(codes::INVALID_PATH, "路径不是普通文件")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AppError::new(
                codes::PERMISSION_DENIED,
                format!("无法读取文件属性: {error}"),
            ))
        }
    }
    crate::scanner::snapshot::fingerprint(&path, volume_id, true).map(Some)
}

/// 只删除身份与日志一致的空目录。父目录链与待删目录都由句柄保护，
/// 删除也作用于已核对的同一个句柄，不再存在“检查 A、按路径删除 B”的窗口。
pub(crate) fn remove_created_directory(
    root: &ApprovedRoot,
    relative: &[String],
    expected_identity: &str,
) -> Result<bool, AppError> {
    validate_existing_relative_path(relative)?;
    if relative.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "不允许删除批准根目录"));
    }
    root.verify_still_valid()?;
    let _guards = lock_directory_chain(root, &relative[..relative.len() - 1])?;
    let path = root.resolve_existing_within(relative)?;
    let directory = match windows::open_directory_for_delete(&path) {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(map_directory_error(error)),
    };
    let actual = windows::file_identity(&directory).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("读取目录身份失败: {error}"),
        )
    })?;
    if actual.file_id_string() != expected_identity {
        return Err(AppError::new(
            codes::ROOT_CHANGED,
            "目录身份与创建记录不一致",
        ));
    }
    windows::remove_directory_by_handle(&directory).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("目录非空或无法安全删除: {error}"),
        )
    })?;
    Ok(true)
}

fn reject_case_sensitive_directory(directory: &File) -> Result<(), AppError> {
    match windows::directory_is_case_sensitive(directory) {
        Ok(false) => Ok(()),
        Ok(true) => Err(AppError::new(
            codes::UNSUPPORTED_STORAGE,
            "目录启用了大小写敏感模式，v0.1 不支持文件执行",
        )),
        Err(error) => Err(AppError::new(
            codes::PERMISSION_DENIED,
            format!("无法读取目录大小写敏感设置: {error}"),
        )),
    }
}

fn map_directory_error(error: std::io::Error) -> AppError {
    let code = match error.kind() {
        std::io::ErrorKind::InvalidData => codes::REPARSE_POINT,
        std::io::ErrorKind::InvalidInput => codes::TARGET_PARENT_IS_FILE,
        std::io::ErrorKind::NotFound => codes::INVALID_PATH,
        std::io::ErrorKind::PermissionDenied => codes::PERMISSION_DENIED,
        _ => codes::INVALID_PATH,
    };
    AppError::new(code, format!("无法安全锁定路径目录: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_receipt_carries_the_evidence() {
        let receipt = MoveReceipt {
            file_id: "ABC".to_owned(),
            sha256: "def".to_owned(),
            size: "3".to_owned(),
            target: vec!["学习".to_owned(), "a.txt".to_owned()],
        };
        assert_eq!(receipt.target.len(), 2);
        assert_eq!(receipt.file_id, "ABC");
    }

    #[test]
    fn created_directory_cleanup_is_bound_to_the_opened_directory_identity() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("工作区父目录");
        let temp = tempfile::Builder::new()
            .prefix("filepilot-safe-dir-")
            .tempdir_in(workspace)
            .expect("工作区同级临时目录");
        let root = crate::safety::root::approve_root(temp.path()).expect("授权临时根");
        let child = temp.path().join("分类");
        std::fs::create_dir(&child).expect("创建目录");
        let handle = windows::open_directory(&child).expect("打开目录");
        let identity = windows::file_identity(&handle)
            .expect("读取身份")
            .file_id_string();
        drop(handle);

        assert!(remove_created_directory(&root, &["分类".to_owned()], &identity).unwrap());
        assert!(!child.exists());
    }

    #[test]
    fn created_directory_cleanup_refuses_a_replacement_identity() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("工作区父目录");
        let temp = tempfile::Builder::new()
            .prefix("filepilot-safe-dir-")
            .tempdir_in(workspace)
            .expect("工作区同级临时目录");
        let root = crate::safety::root::approve_root(temp.path()).expect("授权临时根");
        let child = temp.path().join("分类");
        std::fs::create_dir(&child).expect("创建目录");

        let error = remove_created_directory(&root, &["分类".to_owned()], "not-that-directory")
            .expect_err("身份不一致必须拒绝");
        assert_eq!(error.code, codes::ROOT_CHANGED);
        assert!(child.is_dir(), "后来出现的同名目录必须保留");
    }

    /// **绝不删除非空目录。** 规格 8.4 第 7 条：只做非递归清理。
    ///
    /// 这条断言值钱在它是一个**数据丢失**的边界：目录里可能有用户自己放进去的
    /// 文件，而「撤销顺手把它删了」是那种用户永远不会预期的后果。
    ///
    /// 它是被真实缺陷逼出来的：`undo_windows.rs` 的
    /// `a_directory_that_still_holds_user_files_is_left_alone` 曾经失败，
    /// 现场实测用户文件随目录一起消失，而且**没有告警**——
    /// 说明删除被当成了成功。根因见 ADR-020 的「决策 4 的更正」：
    /// 打开句柄时少了 `FILE_SHARE_DELETE` 会让删除恒失败，
    /// 而那层恒失败**掩盖**了「非空也照删」这个真问题。
    #[test]
    fn created_directory_cleanup_never_removes_a_directory_that_still_has_files() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("工作区父目录");
        let temp = tempfile::Builder::new()
            .prefix("filepilot-safe-dir-")
            .tempdir_in(workspace)
            .expect("工作区同级临时目录");
        let root = crate::safety::root::approve_root(temp.path()).expect("授权临时根");
        let child = temp.path().join("分类");
        std::fs::create_dir(&child).expect("创建目录");
        // 用户自己放进去的东西。撤销不能碰它。
        std::fs::write(child.join("用户的资料.txt"), b"MINE").expect("写用户文件");

        let handle = windows::open_directory(&child).expect("打开目录");
        let identity = windows::file_identity(&handle)
            .expect("读取身份")
            .file_id_string();
        drop(handle);

        let outcome = remove_created_directory(&root, &["分类".to_owned()], &identity);

        // 先把事实全取出来再断言：第一个断言失败会让后面的永远不执行，
        // 而这一条要回答的是「到底丢了什么」，不是「是不是丢了」。
        let dir_still_there = child.is_dir();
        let file_still_there = child.join("用户的资料.txt").is_file();
        let file_content = std::fs::read(child.join("用户的资料.txt")).ok();

        assert!(
            dir_still_there,
            "非空目录绝不能被删掉：outcome={outcome:?} 文件还在吗={file_still_there} 内容={file_content:?}"
        );
        assert_eq!(
            file_content.as_deref(),
            Some(b"MINE".as_slice()),
            "用户文件的内容一个字节都不能变"
        );
        assert!(
            outcome.is_err() || outcome == Ok(false),
            "非空目录应当明确报「没删成」，而不是回一个成功：{outcome:?}"
        );
    }
}
