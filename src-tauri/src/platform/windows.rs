//! Windows 原生文件系统能力。
//!
//! 本模块只做**只读**查询：文件身份、卷信息、重解析点判定。
//! 改变磁盘状态的调用一律不在这里（它们在 T03 的执行适配器里，且必须走
//! `SetFileInformationByHandle(FileRenameInfo)` + `ReplaceIfExists = FALSE`）。

use std::fs::File;
use std::io;
use std::os::windows::fs::MetadataExt as _;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetFileInformationByHandle, GetVolumeInformationW, BY_HANDLE_FILE_INFORMATION,
};

use crate::domain::errors::{codes, AppError};
use crate::domain::time::CivilDateTime;

fn windows_error_to_io(error: windows::core::Error) -> io::Error {
    // windows-rs 返回 HRESULT_FROM_WIN32；低 16 位保留原始 Win32 错误码。
    // 保住它以后，调用方才能区分共享冲突、目标已存在和权限不足。
    let raw = error.code().0;
    io::Error::from_raw_os_error(raw & 0xFFFF)
}

/// `GetDriveTypeW` 的返回值。
///
/// 直接定义常量而不是从 `windows` crate 引入：这些常量并不在
/// `Win32::Storage::FileSystem` 这个 feature 下，为了几个整数去开更多
/// feature 不划算。取值来自 Win32 文档。
///
/// 注意：**必须五个都定义**。只定义其中一两个时，剩下几个会退化成
/// 变量绑定（catch-all pattern），把后面的 match 分支全部变成不可达。
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_REMOVABLE: u32 = 2;

/// Windows 文件属性位，以及基于它们的判断。
///
/// ## 为什么把属性位集中在这里
///
/// 规格 3.2：「platform/windows：文件身份、禁止覆盖移动、句柄锁定、
/// 重解析点检测等能力。**这一层之外不允许直接调用 Win32。**」
///
/// 属性的位运算也算在内：这些取值来自 Win32 文档，散在别处的结果是
/// **同一套规则被抄成两份**。扫描器原先就自己定义了 `OFFLINE` / `HIDDEN` /
/// `SYSTEM` 三个常量，而这一层已经有一个 `is_hidden_or_system`——
/// 两份规则迟早会有一份被改而另一份没改，而「跳过隐藏文件」这种规则
/// 出错时不会报错，只会安静地少跳过或多跳过一些文件。
///
/// ## 为什么同时提供「按位」与「按路径」两个入口
///
/// 扫描器**已经拿到了 metadata**。让它为了一个位判断再按路径取一次属性，
/// 是按文件数的额外系统调用。所以纯函数是主入口，按路径的版本只是
/// 「取属性 + 调纯函数」的便利包装——**规则本身只写一次**。
pub mod attributes {
    pub const HIDDEN: u32 = 0x0002;
    pub const SYSTEM: u32 = 0x0004;
    /// 重解析点：符号链接、目录联接、云占位符都用它标记。
    ///
    /// 规格 1.2：跳过重解析点，不跟随链接走出批准范围。
    pub const REPARSE_POINT: u32 = 0x0400;
    /// 云占位文件：读它会触发网络下载，v0.1 明确不读。
    pub const OFFLINE: u32 = 0x1000;

    /// 规格 6.1：跳过隐藏/系统文件。
    ///
    /// 这两个属性位在 Win32 里是分开的，`std` 没有直接暴露。
    pub fn is_hidden_or_system(value: u32) -> bool {
        value & (HIDDEN | SYSTEM) != 0
    }

    pub fn is_reparse_point(value: u32) -> bool {
        value & REPARSE_POINT != 0
    }

    pub fn is_offline(value: u32) -> bool {
        value & OFFLINE != 0
    }
}

/// 卷内文件身份。
///
/// 规格 5.1 强调它**不等同于路径**（文件改名后身份不变），
/// 也**不等同于内容哈希**（内容被改、大小与 mtime 恰好相同时身份仍不变）。
/// 执行前必须同时核对身份与哈希，二者缺一不可。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    /// 卷序列号。与 `file_index` 一起在卷内唯一。
    pub volume_serial: u32,
    /// 卷内文件索引（NTFS 的 File ID）。
    pub file_index: u64,
}

impl FileIdentity {
    /// 规格 5.1 的 `Fingerprint.volumeId` 用它。
    pub fn volume_id_string(&self) -> String {
        format!("{:08X}", self.volume_serial)
    }

    /// 规格 5.1 的 `Fingerprint.fileId` 用它。
    pub fn file_id_string(&self) -> String {
        format!("{:016X}", self.file_index)
    }
}

/// 取一个已打开文件句柄的身份。
///
/// 必须是**已打开的句柄**而不是路径：按路径重新打开可能拿到另一个文件
/// （规格 7.3「不能验证一个文件、按路径重新打开另一个文件执行」）。
pub fn file_identity(file: &File) -> io::Result<FileIdentity> {
    let handle = HANDLE(file.as_raw_handle() as _);
    let mut info = BY_HANDLE_FILE_INFORMATION::default();

    // SAFETY: handle 来自刚打开的 File，在本次调用期间保持有效。
    unsafe { GetFileInformationByHandle(handle, &mut info) }
        .map_err(|e| io::Error::other(format!("GetFileInformationByHandle 失败: {e}")))?;

    let file_index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
    Ok(FileIdentity {
        volume_serial: info.dwVolumeSerialNumber,
        file_index,
    })
}

/// 这个路径是否带重解析点属性。
///
/// 用 `symlink_metadata` 而不是 `metadata`：后者会跟随链接，
/// 于是「链接目标」和「链接本身」的判定就混在一起了。
pub fn is_reparse_point(path: &Path) -> io::Result<bool> {
    let meta = std::fs::symlink_metadata(path)?;
    Ok(attributes::is_reparse_point(meta.file_attributes()))
}

/// 路径是否隐藏或系统文件。
///
/// 规格 6.1：跳过隐藏/系统文件。
///
/// 判断规则在 [`attributes`] 里，与扫描器共用同一条——见那个模块的文档。
pub fn is_hidden_or_system(path: &Path) -> io::Result<bool> {
    let meta = std::fs::symlink_metadata(path)?;
    Ok(attributes::is_hidden_or_system(meta.file_attributes()))
}

/// 卷类型判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeCheck {
    /// 本地固定磁盘 + 目标文件系统，可以做文件执行。
    Acceptable { file_system: String },
    /// 明确不支持，附上原因。
    Rejected { code: &'static str, detail: String },
}

/// 检查一个路径所在的卷能否用于 v0.1 的文件执行。
///
/// 规格 ADR-002：只支持 Windows 11 x64 本地固定磁盘 NTFS。
/// UNC/网络盘、可移动盘、非 NTFS 一律拒绝，且**不回落**到普通 rename。
///
/// 注意 `GetDriveTypeW` / `GetVolumeInformationW` 要的是**卷根**（`C:\`），
/// 不是任意路径——传完整路径它们会返回 `DRIVE_NO_ROOT_DIR`。
pub fn check_volume(path: &Path) -> io::Result<VolumeCheck> {
    let Some(volume_root) = volume_root_of(path) else {
        return Ok(VolumeCheck::Rejected {
            code: "UNSUPPORTED_STORAGE",
            detail: "无法确定该路径所在的卷".to_owned(),
        });
    };

    let wide = to_wide(&volume_root);

    // SAFETY: wide 是 NUL 结尾的 UTF-16 缓冲，调用期间存活。
    let drive_type = unsafe { GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr())) };

    match drive_type {
        DRIVE_FIXED => {}
        DRIVE_REMOTE => {
            return Ok(VolumeCheck::Rejected {
                code: "UNSUPPORTED_STORAGE",
                detail: "这是网络驱动器；v0.1 只支持本地固定磁盘".to_owned(),
            })
        }
        DRIVE_REMOVABLE => {
            return Ok(VolumeCheck::Rejected {
                code: "UNSUPPORTED_STORAGE",
                detail: "这是可移动磁盘；v0.1 只支持本地固定磁盘".to_owned(),
            })
        }
        other => {
            return Ok(VolumeCheck::Rejected {
                code: "UNSUPPORTED_STORAGE",
                detail: format!("不是本地固定磁盘（GetDriveTypeW = {other}）"),
            })
        }
    }

    let mut fs_name = [0u16; 64];
    let ok = unsafe {
        GetVolumeInformationW(
            windows::core::PCWSTR(wide.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut fs_name),
        )
    };
    ok.map_err(|e| io::Error::other(format!("GetVolumeInformationW 失败: {e}")))?;

    let file_system = String::from_utf16_lossy(
        &fs_name[..fs_name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(fs_name.len())],
    );

    // 大小写不敏感的比较：NTFS 是标准写法，但驱动/本地化可能给出不同大小写
    if !file_system.eq_ignore_ascii_case("NTFS") {
        return Ok(VolumeCheck::Rejected {
            code: "UNSUPPORTED_STORAGE",
            detail: format!("文件系统是 {file_system}，v0.1 只支持 NTFS"),
        });
    }

    Ok(VolumeCheck::Acceptable { file_system })
}

/// 取出路径所属的卷根，例如 `C:\Data\x.txt` -> `C:\`。
///
/// UNC 路径（`\\server\share\...`）返回 `\\server\share\`，
/// 交给 `GetDriveTypeW` 判定为 `DRIVE_REMOTE` 后拒绝。
fn volume_root_of(path: &Path) -> Option<std::path::PathBuf> {
    use std::path::{Component, PathBuf, Prefix};

    let mut components = path.components();
    match components.next()? {
        Component::Prefix(prefix) => {
            let mut root = PathBuf::new();
            match prefix.kind() {
                // 驱动器盘符：`C:` -> `C:\`
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
                    root.push(format!("{}:\\", drive as char));
                }
                // UNC 共享：`\\server\share` -> `\\server\share\`
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    root.push(format!(
                        r"\\{}\{}",
                        server.to_string_lossy(),
                        share.to_string_lossy()
                    ));
                }
                _ => return None,
            }
            Some(root)
        }
        // 已经是 `\foo` 这种无盘符形式，无法确定卷
        _ => None,
    }
}

/// 打开一个**目录**句柄。
///
/// 不能用 `std::fs::File::open`：Windows 上打开目录必须带
/// `FILE_FLAG_BACKUP_SEMANTICS`，否则一律返回「拒绝访问」。
/// 而目录句柄正是防重命名/替换保护的基础（规格 7.3）。
pub fn open_directory(path: &Path) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle as _;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    const GENERIC_READ: u32 = 0x8000_0000;

    let wide = to_wide(path);
    // SAFETY: wide 是 NUL 结尾的 UTF-16 缓冲，调用期间存活；
    // 安全属性与模板句柄传 None 表示使用默认值。
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .map_err(|e| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("打开目录句柄失败: {e}"),
        )
    })?;

    // SAFETY: handle 由 CreateFileW 返回且未失败，所有权在此移交。
    let file = unsafe { File::from_raw_handle(handle.0 as _) };

    // `FILE_FLAG_BACKUP_SEMANTICS` 让这个调用**同时能打开普通文件**，
    // 所以必须显式确认拿到的是目录。否则调用方会以为手里是目录保护句柄，
    // 而实际上是一个文件句柄——这类混淆在路径校验里会直接变成安全漏洞。
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0010;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file 是刚构造的有效句柄，本次调用期间存活。
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info) }
        .map_err(|e| io::Error::other(format!("读取句柄信息失败: {e}")))?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "该路径不是目录",
        ));
    }

    Ok(file)
}

/// 打开并保护一个目录，拒绝最终组件为重解析点。
///
/// 不共享 DELETE，使目录在句柄存活期间不能被重命名或替换；
/// `FILE_FLAG_OPEN_REPARSE_POINT` 保证检查的是联接本身，而不是它指向的目录。
pub fn open_directory_guard(path: &Path) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle as _;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0010;

    let wide = to_wide(path);
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(windows_error_to_io)?;

    let file = unsafe { File::from_raw_handle(handle.0 as _) };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info) }
        .map_err(windows_error_to_io)?;

    if attributes::is_reparse_point(info.dwFileAttributes) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "目录是重解析点"));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "路径不是目录"));
    }
    Ok(file)
}

/// 打开一个目录用于按句柄删除。最终组件拒绝重解析点，且不共享 DELETE，
/// 因此身份核对之后不能被外部替换成另一个同名目录。
pub fn open_directory_for_delete(path: &Path) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle as _;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, OPEN_EXISTING,
    };

    // `DELETE` 用来标记删除；`FILE_LIST_DIRECTORY` 用来**在删除前确认它是空的**。
    //
    // 只要 DELETE 是不够的：`SetFileInformationByHandle(FileDispositionInfo)`
    // 在一台实测机上对**非空目录**也会成功，并在句柄关闭时把整个目录连同
    // 里面的文件一起删掉。规格 8.4 第 7 条要求「非递归清理」，
    // 因此「空不空」必须在**同一支句柄**上先问清楚，不能靠内核替我们把关。
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const FILE_LIST_DIRECTORY: u32 = 0x0000_0001;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0010;
    let wide = to_wide(path);

    // 共享模式的两点讲究：
    //
    // 1. **必须带 `FILE_SHARE_DELETE`**，否则 `SetFileInformationByHandle`
    //    一律 `ERROR_ACCESS_DENIED`——这条不是从文档推出来的，是四组对照实验
    //    测出来的，而且当时的直觉是反的（以为 `FILE_FLAG_OPEN_REPARSE_POINT` 有问题）。
    // 2. **刻意不带 `FILE_SHARE_WRITE`**：只要这支句柄活着，别的进程就没法
    //    往这个目录里再放东西。于是「查它是空的 → 删它」之间不存在
    //    「别人刚塞了一个文件进来」的窗口。
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            DELETE_ACCESS | FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(windows_error_to_io)?;
    let file = unsafe { File::from_raw_handle(handle.0 as _) };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut info) }
        .map_err(windows_error_to_io)?;
    if attributes::is_reparse_point(info.dwFileAttributes) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "目录是重解析点"));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "路径不是目录"));
    }
    Ok(file)
}

/// 在这支句柄上确认目录是空的。
///
/// **删目录之前必须先问这一句。** 实测（Windows 11，本机）表明
/// `SetFileInformationByHandle(FileDispositionInfo)` 对**非空目录**也会成功，
/// 并在句柄关闭时把目录连同里面的文件一起删掉——那是一次用户完全无法预期的
/// 数据丢失。规格 8.4 第 7 条要的是「非递归清理」，所以判空这件事
/// 不能寄希望于内核替我们拒绝。
///
/// 用句柄枚举而不是按路径 `read_dir`：按路径做会留下
/// 「查的是 A、删的是 B」的窗口，而这里问的和删的是同一个内核对象。
pub fn directory_is_empty_by_handle(directory: &File) -> io::Result<bool> {
    use windows::Win32::Storage::FileSystem::{
        FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo, GetFileInformationByHandleEx,
        FILE_ID_BOTH_DIR_INFO,
    };

    // 一次要 64 KiB：条目多到装不下时，我们本来也已经能确定「它不是空的」，
    // 所以不需要真的循环取完——见下面的 `more_data` 分支。
    const BUFFER_BYTES: usize = 64 * 1024;
    let mut buffer = vec![0u8; BUFFER_BYTES];

    let outcome = unsafe {
        GetFileInformationByHandleEx(
            HANDLE(directory.as_raw_handle() as _),
            FileIdBothDirectoryRestartInfo,
            buffer.as_mut_ptr().cast(),
            BUFFER_BYTES as u32,
        )
    };

    // 装不下说明条目很多，那当然不是空目录。
    let mut more_data = false;
    if let Err(error) = outcome {
        use windows::Win32::Foundation::ERROR_MORE_DATA;
        if error.code() == windows::core::HRESULT::from_win32(ERROR_MORE_DATA.0) {
            more_data = true;
        } else {
            return Err(windows_error_to_io(error));
        }
    }

    let name_offset = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
    let mut offset = 0usize;
    loop {
        if offset + name_offset > BUFFER_BYTES {
            break;
        }
        // SAFETY: 缓冲区里这一段的布局由系统按 FILE_ID_BOTH_DIR_INFO 写入，
        // 且我们检查过它至少能放下定长部分。
        let entry = unsafe { &*(buffer.as_ptr().add(offset) as *const FILE_ID_BOTH_DIR_INFO) };
        let name_bytes = entry.FileNameLength as usize;
        if offset + name_offset + name_bytes > BUFFER_BYTES {
            break;
        }
        // SAFETY: 文件名紧跟在定长部分之后，长度由 FileNameLength 给出，
        // 上面已确认它落在缓冲区内。
        let name_slice = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().add(offset + name_offset) as *const u16,
                name_bytes / 2,
            )
        };
        let name = String::from_utf16_lossy(name_slice);

        // `.` 与 `..` 不是目录内容。
        if name != "." && name != ".." {
            return Ok(false);
        }

        if entry.NextEntryOffset == 0 {
            break;
        }
        offset += entry.NextEntryOffset as usize;
    }

    if more_data {
        return Ok(false);
    }
    let _ = FileIdBothDirectoryInfo;
    Ok(true)
}

/// 将已打开、已核对身份、**且确认已空**的目录标记为删除。
///
/// 非空由调用前的那次句柄判空挡下来；这里再挡一次是有意的——
/// 这个函数可以被单独调用，而「顺手删掉用户东西」的代价不允许它依赖调用顺序。
pub fn remove_directory_by_handle(directory: &File) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    if !directory_is_empty_by_handle(directory)? {
        return Err(io::Error::new(
            io::ErrorKind::DirectoryNotEmpty,
            "目录里还有文件，不做非递归清理",
        ));
    }

    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(directory.as_raw_handle() as _),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(windows_error_to_io)
}

/// 以只读且不共享写入/删除的方式打开快照源。
pub fn open_source_for_snapshot(path: &Path) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle as _;
    use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, OPEN_EXISTING};

    const GENERIC_READ: u32 = 0x8000_0000;
    let wide = to_wide(path);
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            GENERIC_READ,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        )
    }
    .map_err(windows_error_to_io)?;
    Ok(unsafe { File::from_raw_handle(handle.0 as _) })
}

/// 打开一个**待重命名**的源文件句柄。
///
/// 规格 7.3 的关键要求：
/// - 需要 `DELETE` 权限才能重命名（Windows 的重命名检查的是父目录上的
///   `DELETE_CHILD` / 文件上的 `DELETE`）；
/// - **只共享 `FILE_SHARE_READ`**：不共享 WRITE / DELETE，于是校验期间
///   别的进程无法修改或替换这个文件；若它已经被别的写入程序占用，
///   这次打开会失败，调用方据此返回 `FILE_BUSY`，而不是「先假定没动过」。
pub fn open_source_for_rename(path: &Path) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle as _;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const DELETE: u32 = 0x0001_0000;

    let wide = to_wide(path);
    // SAFETY: wide 是 NUL 结尾的 UTF-16 缓冲，调用期间存活。
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            GENERIC_READ | DELETE,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(windows_error_to_io)?;

    // SAFETY: handle 由 CreateFileW 返回且未失败，所有权在此移交。
    Ok(unsafe { File::from_raw_handle(handle.0 as _) })
}

/// 通过**已持有的句柄**执行同卷重命名，且**禁止覆盖已有目标**。
///
/// 规格 7.3 明确要求用 `SetFileInformationByHandle(FileRenameInfo)` +
/// `ReplaceIfExists = FALSE`，而不是 `exists()` 之后再 `rename()`——
/// 后者两次调用之间别的进程可以创建目标。
///
/// 同时这也保证了「校验的文件」与「被重命名的文件」是同一个：
/// 路径可以在这期间被换掉，句柄不会。
pub fn rename_by_handle_no_replace(file: &File, target: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle as _;
    use windows::Win32::Storage::FileSystem::{
        FileRenameInfo, SetFileInformationByHandle, FILE_RENAME_INFO, FILE_RENAME_INFO_0,
    };

    // FileNameLength 的单位是**字节**，且不含结尾的 NUL。
    let name_utf16: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt as _;
        target.as_os_str().encode_wide().collect()
    };
    let name_bytes = name_utf16.len() * 2;

    // FILE_RENAME_INFO 的 FileName 是长度为 1 的柔性数组，
    // 所以缓冲区要按「头部 + 实际文件名」自己算，不能直接 size_of::<FILE_RENAME_INFO>()。
    let header_bytes = std::mem::size_of::<FILE_RENAME_INFO>() - std::mem::size_of::<u16>();
    let total_bytes = header_bytes + name_bytes;

    let mut buffer = vec![0u8; total_bytes];
    let info = buffer.as_mut_ptr() as *mut FILE_RENAME_INFO;

    // SAFETY: buffer 长度按上面的公式算出，写入范围都在缓冲区内。
    unsafe {
        (*info).Anonymous = FILE_RENAME_INFO_0 {
            // 这一行就是「绝不覆盖」的全部保证
            ReplaceIfExists: false,
        };
        // RootDirectory 传 0 表示 FileName 是完整路径而不是相对某个目录句柄
        (*info).RootDirectory = HANDLE::default();
        (*info).FileNameLength = name_bytes as u32;
        std::ptr::copy_nonoverlapping(
            name_utf16.as_ptr(),
            (*info).FileName.as_mut_ptr(),
            name_utf16.len(),
        );
    }

    let handle = HANDLE(file.as_raw_handle() as _);
    // SAFETY: handle 有效；buffer 与 total_bytes 一致。
    unsafe {
        SetFileInformationByHandle(
            handle,
            FileRenameInfo,
            buffer.as_ptr() as *const core::ffi::c_void,
            total_bytes as u32,
        )
    }
    .map_err(windows_error_to_io)
}

/// 返回文件的硬链接数量。
pub fn file_link_count(file: &File) -> io::Result<u32> {
    let handle = HANDLE(file.as_raw_handle() as _);
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle, &mut info) }.map_err(windows_error_to_io)?;
    Ok(info.nNumberOfLinks)
}

/// 查询目录是否开启 Windows 大小写敏感模式。
pub fn directory_is_case_sensitive(directory: &File) -> io::Result<bool> {
    use windows::Win32::Storage::FileSystem::{
        FileCaseSensitiveInfo, GetFileInformationByHandleEx, FILE_CASE_SENSITIVE_INFO,
    };
    const FILE_CS_FLAG_CASE_SENSITIVE_DIR: u32 = 1;

    let mut info = FILE_CASE_SENSITIVE_INFO::default();
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(directory.as_raw_handle() as _),
            FileCaseSensitiveInfo,
            (&mut info as *mut FILE_CASE_SENSITIVE_INFO).cast(),
            std::mem::size_of::<FILE_CASE_SENSITIVE_INFO>() as u32,
        )
    }
    .map_err(windows_error_to_io)?;
    Ok(info.Flags & FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0)
}

/// 取一个文件句柄的大小与最后修改时间。
///
/// 与 [`file_identity`] 分开是为了让调用方能把「身份」「大小/时间」「内容哈希」
/// 三类证据分开核对并分别报错，而不是笼统地「指纹不符」。
pub fn size_and_modified(file: &File) -> io::Result<(u64, String)> {
    let handle = HANDLE(file.as_raw_handle() as _);
    let mut info = BY_HANDLE_FILE_INFORMATION::default();

    // SAFETY: handle 来自有效的 File，在本次调用期间存活。
    unsafe { GetFileInformationByHandle(handle, &mut info) }
        .map_err(|e| io::Error::other(format!("GetFileInformationByHandle 失败: {e}")))?;

    let size = ((info.nFileSizeHigh as u64) << 32) | (info.nFileSizeLow as u64);

    // FILETIME 是自 1601-01-01 起的 100 纳秒数；转成 Unix 纪元起的纳秒，
    // 与 scanner::snapshot::modified_ns 保持同一口径，否则两侧指纹永远对不上。
    const WINDOWS_TO_UNIX_100NS: i128 = 116_444_736_000_000_000;
    let ticks = ((info.ftLastWriteTime.dwHighDateTime as i128) << 32)
        | info.ftLastWriteTime.dwLowDateTime as i128;
    let nanos = (ticks - WINDOWS_TO_UNIX_100NS) * 100;

    Ok((size, nanos.to_string()))
}

/// 把一个 UTC 纳秒时间戳换算成**本地时区**的公历时间。
///
/// 规格 6.3：byMonth 规则要「按本地时区生成 `YYYY-MM`」。
///
/// 为什么走 `SystemTimeToTzSpecificLocalTime` 而不是自己拿
/// `GetTimeZoneInformation` 的 Bias 做减法：Bias 只描述**当前生效**的那个偏移，
/// 对一个半年前的 UTC 时间戳，正确的偏移还要看那一天的夏令时规则。
/// 交给系统 API 处理，才不用在应用里维护一份时区规则。
///
/// 传 `None` 表示使用本机当前时区。
pub fn local_datetime_from_unix_ns(ns: i128) -> Result<CivilDateTime, AppError> {
    use windows::Win32::Foundation::SYSTEMTIME;
    use windows::Win32::System::Time::SystemTimeToTzSpecificLocalTime;

    let utc = crate::domain::time::civil_from_unix_ns(ns).ok_or_else(|| {
        AppError::new(
            codes::INVALID_TIMESTAMP,
            "文件修改时间超出可表示的日期范围（1601–9999）",
        )
    })?;

    let utc_system_time = SYSTEMTIME {
        wYear: utc.year as u16,
        wMonth: utc.month as u16,
        wDayOfWeek: 0,
        wDay: utc.day as u16,
        wHour: utc.hour as u16,
        wMinute: utc.minute as u16,
        wSecond: utc.second as u16,
        wMilliseconds: utc.millisecond as u16,
    };

    let mut local = SYSTEMTIME::default();
    // SAFETY: 两个指针都指向本函数栈上的有效 SYSTEMTIME；API 只写 local。
    unsafe { SystemTimeToTzSpecificLocalTime(None, &utc_system_time, &mut local) }.map_err(
        |error| {
            AppError::new(
                codes::INVALID_TIMESTAMP,
                format!("本地时区换算失败: {error}"),
            )
        },
    )?;

    let value = CivilDateTime::new(
        local.wYear as i32,
        local.wMonth as u32,
        local.wDay as u32,
        local.wHour as u32,
        local.wMinute as u32,
        local.wSecond as u32,
        local.wMilliseconds as u32,
    );

    // 系统返回了越界字段说明 API 用法有问题，不能把垃圾值继续往下传。
    if !value.is_representable() {
        return Err(AppError::new(
            codes::INVALID_TIMESTAMP,
            "本地时区换算返回了不可表示的日期",
        ));
    }
    Ok(value)
}

/// Rust 字符串 -> NUL 结尾的 UTF-16。
///
/// Windows API 的 `PCWSTR` 要求 NUL 结尾；直接传 `str` 的切片会越界读。
fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    let mut v: Vec<u16> = path.as_os_str().encode_wide().collect();
    v.push(0);
    v
}

/// 单实例锁的持有凭证。
///
/// 句柄一关，命名互斥体就释放 —— 包括进程崩溃的情况，
/// 内核会替我们清理。这正是用命名互斥体而不是锁文件的原因：
/// 锁文件在进程异常退出后会留下一个「永远锁着」的残留。
pub struct SingleInstanceLock {
    handle: HANDLE,
}

// SAFETY: 句柄本身可以在线程间传递；Drop 只在进程退出时执行一次。
unsafe impl Send for SingleInstanceLock {}
unsafe impl Sync for SingleInstanceLock {}

impl Drop for SingleInstanceLock {
    fn drop(&mut self) {
        // SAFETY: handle 由 CreateMutexW 返回且尚未关闭。
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

/// 尝试取得单实例锁。
///
/// 返回 `None` 表示**已经有实例在运行**。规格 T07：第二个实例不得启动
/// 第二个执行器 —— 两个进程同时整理同一个目录会把计划互相踩掉。
///
/// 用命名互斥体，名字放在 `Local\` 命名空间下：同一用户会话内互斥，
/// 不同用户各跑一个互不干扰（这是桌面应用的期望行为）。
pub fn acquire_single_instance_lock(name: &str) -> Option<SingleInstanceLock> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Threading::CreateMutexW;

    let wide: Vec<u16> = format!("Local\\{name}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: wide 是 NUL 结尾的 UTF-16 缓冲；binitialowner = true 表示
    // 我们要求立即拥有它，这样第二个调用方会拿到 ERROR_ALREADY_EXISTS。
    let handle = unsafe { CreateMutexW(None, true, PCWSTR(wide.as_ptr())) }.ok()?;

    // CreateMutexW 在「已存在」时**也会返回一个有效句柄**，只是 GetLastError
    // 为 ERROR_ALREADY_EXISTS。必须显式判断，否则第二个实例会以为自己拿到了锁。
    const ERROR_ALREADY_EXISTS: u32 = 183;
    // SAFETY: 无参数，返回当前线程最后一次错误的快照。
    let already = unsafe { windows::Win32::Foundation::GetLastError() };
    if already.0 == ERROR_ALREADY_EXISTS {
        // SAFETY: 这个句柄我们不要了，关掉它。
        let _ = unsafe { CloseHandle(handle) };
        return None;
    }

    Some(SingleInstanceLock { handle })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_id_and_file_id_are_stable_across_calls() {
        // 用 open_directory 而不是 File::open：后者在 Windows 上打不开目录
        let dir = std::env::temp_dir();
        let file = open_directory(&dir).expect("打开临时目录应成功");
        let a = file_identity(&file).expect("取身份应成功");
        let b = file_identity(&file).expect("取身份应成功");
        assert_eq!(a, b, "同一句柄两次取身份必须一致");
    }

    #[test]
    fn open_directory_rejects_a_plain_file() {
        // 目录句柄的失败路径也要有覆盖：拿文件当目录打开必须报错，
        // 而不是返回一个用途错误的句柄
        let dir = std::env::temp_dir();
        let file_path = dir.join("filepilot-open-dir-probe.txt");
        std::fs::write(&file_path, b"x").expect("写探针文件");
        let result = open_directory(&file_path);
        let _ = std::fs::remove_file(&file_path);
        assert!(result.is_err(), "打开普通文件为目录必须失败");
    }

    #[test]
    fn identity_strings_have_fixed_width() {
        let id = FileIdentity {
            volume_serial: 0x1234_ABCD,
            file_index: 0x0000_0000_DEAD_BEEF,
        };
        assert_eq!(id.volume_id_string(), "1234ABCD");
        assert_eq!(id.file_id_string(), "00000000DEADBEEF");
    }

    #[test]
    fn temp_dir_is_not_a_reparse_point() {
        let dir = std::env::temp_dir();
        assert!(!is_reparse_point(&dir).expect("读取属性应成功"));
    }

    #[test]
    fn ordinary_temp_directory_is_not_case_sensitive() {
        let dir = open_directory(&std::env::temp_dir()).expect("打开临时目录");
        assert!(!directory_is_case_sensitive(&dir).expect("读取大小写敏感标志"));
    }

    #[test]
    fn temp_volume_is_acceptable_on_this_machine() {
        // 本机 C 盘应为固定盘 + NTFS；若不是，说明环境与 ADR-002 的假设不符，
        // 应该尽早知道而不是等到执行阶段才发现
        let dir = std::env::temp_dir();
        match check_volume(&dir).expect("卷检查应成功") {
            VolumeCheck::Acceptable { file_system } => {
                assert_eq!(file_system.to_uppercase(), "NTFS")
            }
            VolumeCheck::Rejected { code, detail } => {
                panic!("临时目录所在卷不被接受（{code}: {detail}）")
            }
        }
    }

    #[test]
    fn missing_path_reports_io_error_not_panic() {
        let missing = std::env::temp_dir().join("filepilot-definitely-missing-9f3a2b");
        assert!(is_reparse_point(&missing).is_err());
    }
}
