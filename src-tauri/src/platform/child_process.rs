//! 受限子进程：作业对象资源上限 + 「只继承一支句柄」的派生。
//!
//! 规格 6.2 对解析工作进程提了四条硬要求，这个模块负责其中三条：
//!
//! | 规格要求 | 这里怎么做到 |
//! |---|---|
//! | Job Object 控制 256 MiB 工作内存与超时终止 | [`JobObject`] |
//! | 禁止创建子进程 | 作业的 `JOB_OBJECT_LIMIT_ACTIVE_PROCESS = 1` |
//! | 只接收批准的只读文件句柄 | [`spawn_with_inherited_handles`] 的可继承句柄清单 |
//!
//! ## 为什么用「可继承句柄清单」而不是「把所有句柄都传过去」
//!
//! `CreateProcessW` 的 `bInheritHandles = TRUE` 会把父进程里**所有**标了可继承的
//! 句柄都复制给子进程。那等于把「父进程碰巧开着什么」变成子进程的攻击面——
//! 一个被攻破的解析器可以直接去读那些句柄指向的东西。
//!
//! `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 把清单收紧到**恰好那几支**：
//! 我们打开的只读文件句柄，加上它的标准输出管道写端。仅此而已。
//!
//! ## 为什么不直接用 `std::process::Command`
//!
//! 两个原因，都是硬约束：
//!
//! 1. `Command` 不暴露「只继承这些句柄」的能力；
//! 2. 它也不返回可用于 `AssignProcessToJobObject` 的**裸进程句柄**——
//!    而作业限制必须在子进程做任何值得限制的事之前套上去。
//!
//! 代价是这个模块里有较多 `unsafe`。所以它把不安全集中在一处、
//! 对外只暴露三个安全方法，而不是让调用方各自去拼 Win32 调用。
//!
//! ## 关于「禁止联网」
//!
//! 作业对象**管不了网络**——Windows 没有「这个进程不许联网」的作业限制。
//! 这一条在本项目里是**构造性**的：`extract_worker` 不链接任何网络代码，
//! 它收到的输入里也没有 URL（见 `extractors::protocol`）。
//! 这里如实写明，是为了不让读代码的人以为作业对象包办了一切。

use std::fs::File;
use std::io::{self, Read};
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _};
use std::path::Path;
use std::time::Duration;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
    STILL_ACTIVE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_READ, OPEN_EXISTING,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    STARTUPINFOEXW,
};

/// 只读、且只共享读的文件句柄。`FILE_SHARE_READ` 是关键：
/// 从打开这一刻起，别的进程不能再写这个文件——比「读完再核对没变」更强，
/// 因为它让「读期间被改」根本发生不了。
///
/// 内部持有 `File` 而不是裸 `HANDLE`：`File` 自己负责关闭，
/// 而且调用方要在这支**同一支**句柄上取指纹（前后各一次），
/// 那需要能把 `&File` 交出去。
pub struct ReadOnlyHandle {
    file: File,
}

impl ReadOnlyHandle {
    /// 以只读方式打开，并标记为可继承（但**不会**自动被子进程继承——
    /// 是否继承由派生时的句柄清单决定）。
    pub fn open(path: &Path) -> io::Result<Self> {
        const GENERIC_READ: u32 = 0x8000_0000;

        let wide = to_wide(path);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: windows::core::BOOL(1),
        };

        // SAFETY: wide 是 NUL 结尾的缓冲；attributes 在调用期间存活。
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ,
                FILE_SHARE_READ,
                Some(&attributes),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_SEQUENTIAL_SCAN,
                None,
            )
        }
        .map_err(map_open_error)?;

        // SAFETY: handle 由 CreateFileW 返回且未失败，所有权在此移交。
        Ok(Self {
            file: unsafe { File::from_raw_handle(handle.0 as _) },
        })
    }

    /// 传给子进程的那个整数值。
    ///
    /// 句柄值只在**子进程自己的**句柄表里有意义：别人拿到这个数字，
    /// 在自己的进程里什么也打不开，因此它不构成一种能力外泄。
    pub fn raw_value(&self) -> u64 {
        self.file.as_raw_handle() as u64
    }

    /// 底层句柄。取指纹与核对身份都在它上面做。
    pub fn file(&self) -> &File {
        &self.file
    }
}

/// 一个作业对象。drop 时句柄关闭，作业里的残留进程会被内核一并带走。
pub struct JobObject {
    handle: HANDLE,
}

// SAFETY: 作业对象句柄是内核对象，跨线程使用安全——它没有用户态可变状态，
// 所有操作都由内核串行化。Rust 不会自动给裸指针包装实现 `Send`/`Sync`。
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    /// 建作业并**一次性**设好三条限制。
    ///
    /// `max_processes = 1` 就是「子进程不能再开子进程」——由内核执行，
    /// 而不是靠约定。
    ///
    /// 三条一起设而不是分次设：作业限制是「设置即生效」的，分两次设中间
    /// 会有一个「只限了内存、没限进程数」的窗口，而那个窗口足够让一个
    /// 恶意解析器把它想开的子进程都生出来。
    pub fn create(memory_limit_bytes: u64, max_processes: u32) -> io::Result<Self> {
        // SAFETY: 名字与安全属性都传 None（默认值），返回句柄由本结构接管。
        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(io::Error::other)?;
        if handle.is_invalid() {
            return Err(io::Error::other("CreateJobObjectW 返回了无效句柄"));
        }

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.BasicLimitInformation.ActiveProcessLimit = max_processes;
        info.ProcessMemoryLimit = memory_limit_bytes as usize;

        // SAFETY: info 按结构体真实大小传入，类型与之匹配。
        unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(io::Error::other)?;

        Ok(Self { handle })
    }

    /// 把已经起来的进程放进作业。
    ///
    /// 调用方应当在 `spawn` 之后**立刻**调用它：早一步套上，就少一段
    /// 「它还没被限制」的窗口。
    pub fn assign(&self, child: &ChildProcess) -> io::Result<()> {
        // SAFETY: 两个句柄都有效且在调用期间存活。
        unsafe { AssignProcessToJobObject(self.handle, child.process) }.map_err(io::Error::other)
    }

    /// 强制结束作业里的**所有**进程。
    ///
    /// 超时后用它，而不是 `ChildProcess::terminate()`：作业终止是
    /// 「不管里面有什么，一并清掉」，不依赖我们是否认得每一个进程。
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        // SAFETY: 作业句柄在调用期间存活。
        unsafe { TerminateJobObject(self.handle, exit_code) }.map_err(io::Error::other)
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        // `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`：句柄一关，内核就终止作业里
        // 还活着的进程。这是「进程退出回收」的最后一道保险——即使调用方在
        // 某条提前返回的路径上漏了 `terminate`，drop 也会兜住。
        //
        // SAFETY: 句柄来自 CreateJobObjectW 且尚未关闭。
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// 一个由我们直接管理的子进程。
///
/// 不用 `std::process::Child`：我们需要**裸进程句柄**去
/// `AssignProcessToJobObject`，而 `Child` 不把它交出来，
/// 也不允许从裸句柄构造。
pub struct ChildProcess {
    process: HANDLE,
    stdout: Option<File>,
}

impl ChildProcess {
    /// 等到进程结束，最多等 `timeout`。
    ///
    /// 返回 `Ok(Some(code))` 表示已结束并拿到退出码；`Ok(None)` 表示超时。
    /// **超时不是错误**——调用方据此去终止作业，那是一条正常路径。
    pub fn wait_timeout(&self, timeout: Duration) -> io::Result<Option<u32>> {
        let millis = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        // SAFETY: 进程句柄在调用期间存活。
        let waited = unsafe { WaitForSingleObject(self.process, millis) };
        if waited == WAIT_TIMEOUT {
            return Ok(None);
        }
        if waited != WAIT_OBJECT_0 {
            return Err(io::Error::other("WaitForSingleObject 返回了意外结果"));
        }
        self.exit_code().map(Some)
    }

    /// 进程的退出码。进程仍在跑时返回 `STILL_ACTIVE`。
    pub fn exit_code(&self) -> io::Result<u32> {
        let mut code: u32 = 0;
        // SAFETY: 进程句柄在调用期间存活。
        unsafe { GetExitCodeProcess(self.process, &mut code) }.map_err(io::Error::other)?;
        Ok(code)
    }

    /// 进程是否还在运行。
    pub fn is_running(&self) -> bool {
        self.exit_code()
            .map(|code| code == STILL_ACTIVE.0 as u32)
            .unwrap_or(false)
    }

    /// 读走标准输出的全部内容。
    ///
    /// **必须在 `wait_timeout` 之前调用**：管道缓冲区只有几十 KiB，
    /// 而工作进程的响应里带着最多 12,000 个字符的正文。先等进程结束再读，
    /// 一旦内容装不下缓冲区，双方就会互相等——一个教科书式的管道死锁。
    /// 读到 EOF（也就是子进程关闭了写端）自然返回。
    pub fn read_stdout(&mut self) -> io::Result<String> {
        let Some(handle) = self.stdout.as_mut() else {
            return Ok(String::new());
        };
        let mut text = String::new();
        handle.read_to_string(&mut text)?;
        Ok(text)
    }

    /// 关闭读端，让后续的等待不再被未读数据牵住。
    pub fn close_stdout(&mut self) {
        self.stdout = None;
    }

    /// 把标准输出的读端**取走**，交给别的线程去读。
    ///
    /// 超时与读输出必须并行，不能顺序做：
    ///
    /// * 先读再等 → 子进程卡住时 `read` 会一直阻塞，超时永远检测不到；
    /// * 先等再读 → 响应装不下管道缓冲区时双方互等（死锁）。
    ///
    /// 所以调用方把读端挪到一个线程里，主线程去做带超时的等待；
    /// 超时后 `TerminateJobObject` 一杀，子进程的写端关闭，
    /// 那个线程自然读到 EOF。
    pub fn take_stdout(&mut self) -> Option<File> {
        self.stdout.take()
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        // 只关我们自己持有的句柄，**不**杀进程：进程的生死由作业管。
        // 在这里顺手杀进程会让「先读输出再判断」的调用方永远读不到东西。
        // SAFETY: 句柄由本结构独占。
        unsafe {
            let _ = CloseHandle(self.process);
        }
        self.stdout = None;
    }
}

/// 派生一个子进程，并且**只**把 `handles` 里列出的句柄交给它继承，
/// 同时把它的标准输出接到一支匿名管道上。
///
/// 返回的子进程还没有被套上作业限制——调用方必须紧接着 `JobObject::assign`。
/// 之所以分成两步而不是在这里一起做：作业的创建策略（内存上限、进程数）
/// 属于调用方的资源策略，不该由这个「怎么起进程」的模块替它决定。
pub fn spawn_with_inherited_handles(
    program: &Path,
    args: &[String],
    handles: &[u64],
) -> io::Result<ChildProcess> {
    // ---- 1) 匿名管道：读端留给自己，写端给子进程 ----
    let mut read_end = INVALID_HANDLE_VALUE;
    let mut write_end = INVALID_HANDLE_VALUE;
    let pipe_attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: windows::core::BOOL(1),
    };
    // SAFETY: 两个输出参数都是有效的局部变量。
    unsafe { CreatePipe(&mut read_end, &mut write_end, Some(&pipe_attributes), 0) }
        .map_err(io::Error::other)?;

    // 读端**不能**被子进程继承，否则子进程会一直握着它，
    // 我们这边就永远读不到 EOF。
    // SAFETY: read_end 是刚由 CreatePipe 返回的有效句柄。
    unsafe {
        SetHandleInformation(
            read_end,
            HANDLE_FLAG_INHERIT.0,
            windows::Win32::Foundation::HANDLE_FLAGS(0),
        )
    }
    .map_err(io::Error::other)?;

    // ---- 2) 属性清单：只有我们点名的句柄可以继承 ----
    let mut inherited: Vec<HANDLE> = handles.iter().map(|value| HANDLE(*value as _)).collect();
    inherited.push(write_end);

    let mut size: usize = 0;
    // 第一次调用只为了问需要多大的缓冲区，返回 false 是预期的。
    // SAFETY: 传空列表指针、只要大小，是文档规定的用法。
    // 返回值在这里**故意丢弃**：它失败恰恰表示「还需要一个 size」，
    // 而那个 size 已经写进 `size` 了。
    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut size) };
    let mut buffer = vec![0u8; size];
    // 0.62 里这是个新类型结构体，不是裸指针别名；每次用之前从同一块缓冲区构造。
    let attribute_buffer: *mut core::ffi::c_void = buffer.as_mut_ptr().cast();
    // SAFETY: buffer 按上一步问到的 size 分配。
    unsafe {
        InitializeProcThreadAttributeList(
            Some(LPPROC_THREAD_ATTRIBUTE_LIST(attribute_buffer)),
            1,
            None,
            &mut size,
        )
    }
    .map_err(io::Error::other)?;

    // SAFETY: 值缓冲区在本次调用期间存活，长度按字节数给出。
    let update = unsafe {
        UpdateProcThreadAttribute(
            LPPROC_THREAD_ATTRIBUTE_LIST(attribute_buffer),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(inherited.as_ptr().cast()),
            std::mem::size_of_val(inherited.as_slice()),
            None,
            None,
        )
    };
    if let Err(error) = update {
        // SAFETY: 列表已初始化。
        unsafe { DeleteProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(attribute_buffer)) };
        unsafe {
            let _ = CloseHandle(read_end);
            let _ = CloseHandle(write_end);
        }
        return Err(io::Error::other(error));
    }

    // ---- 3) 建进程 ----
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = windows::Win32::System::Threading::STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdOutput = write_end;
    startup.StartupInfo.hStdError = write_end;
    startup.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    startup.lpAttributeList = LPPROC_THREAD_ATTRIBUTE_LIST(attribute_buffer);

    let mut command_line = command_line_for(program, args);
    let mut process_info = PROCESS_INFORMATION::default();

    // SAFETY: 命令行是可写的 NUL 结尾缓冲；startup 与 process_info 都存活；
    // 句柄清单已设好，因此 bInheritHandles=TRUE 只会复制清单里的那几支。
    let created = unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            None,
            None,
            &startup.StartupInfo,
            &mut process_info,
        )
    };

    // 属性清单与写端在 CreateProcessW 返回后就可以释放了：
    // 子进程已经拿到自己的副本。
    // SAFETY: 列表已初始化且不再使用。
    unsafe { DeleteProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(attribute_buffer)) };
    unsafe {
        let _ = CloseHandle(write_end);
    }

    if let Err(error) = created {
        // SAFETY: 读端由我们持有且尚未移交。
        unsafe {
            let _ = CloseHandle(read_end);
        }
        return Err(io::Error::other(error));
    }

    // 线程句柄用不到，关掉。进程句柄交给 ChildProcess。
    // SAFETY: 两个句柄都由 CreateProcessW 返回。
    unsafe {
        let _ = CloseHandle(process_info.hThread);
    }

    // SAFETY: read_end 由 CreatePipe 返回、尚未移交，从这里起归 File 所有。
    let stdout = unsafe { File::from_raw_handle(read_end.0 as _) };

    Ok(ChildProcess {
        process: process_info.hProcess,
        stdout: Some(stdout),
    })
}

/// 拼一条 Windows 命令行。
///
/// 每个参数都按 `CommandLineToArgvW` 的规则加引号：路径里带空格是常态
/// （`C:\Users\张三\AppData\Local\FilePilot\extract_worker.exe`），
/// 不加引号会被切成两个参数，错误信息还会让人以为是参数表写错了。
fn command_line_for(program: &Path, args: &[String]) -> Vec<u16> {
    let mut line = quote_windows_arg(&program.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_windows_arg(arg));
    }
    line.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 按 `CommandLineToArgvW` 的反规则加引号。
fn quote_windows_arg(value: &str) -> String {
    if !value.is_empty() && !value.contains([' ', '\t', '"']) {
        return value.to_owned();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in value.chars() {
        match c {
            '\\' => {
                backslashes += 1;
                out.push('\\');
            }
            '"' => {
                // 引号前面的反斜杠要翻倍，否则会把引号本身转义掉。
                for _ in 0..=backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            }
            _ => {
                backslashes = 0;
                out.push(c);
            }
        }
    }
    // 结尾的反斜杠同样要翻倍，免得转义掉我们补上的收尾引号。
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 把 `CreateFileW` 的失败翻成**保留语义**的 `io::Error`。
///
/// 不能直接 `io::Error::other`：那会把错误种类压成 `Other`，
/// 而调用方要靠种类区分「文件不存在」与「没有权限」——
/// 前者让用户去确认文件还在不在，后者让他去检查占用或权限。
/// 压平之后两种完全不同的处理方式被合并成一句「打不开」，用户只能瞎试。
fn map_open_error(error: windows::core::Error) -> io::Error {
    // Win32 错误码在 HRESULT 的低 16 位。
    let raw = (error.code().0 as u32) & 0xFFFF;
    let kind = match raw {
        2 | 3 => io::ErrorKind::NotFound, // FILE_NOT_FOUND / PATH_NOT_FOUND
        5 => io::ErrorKind::PermissionDenied, // ACCESS_DENIED
        32 | 33 => io::ErrorKind::WouldBlock, // SHARING_VIOLATION / LOCK_VIOLATION
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("CreateFileW 失败: {error}"))
}

/// 从 `std::process::Child` 取裸进程句柄。
///
/// 只给测试用：生产路径用的是 [`ChildProcess`]。
#[cfg(test)]
pub fn raw_handle_of(child: &std::process::Child) -> u64 {
    use std::os::windows::io::AsRawHandle as _;
    child.as_raw_handle() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 跑真实子进程的用例之间的串行闸门。
    ///
    /// 这四条用例各自要 `CreateProcess`、操作**进程级**的句柄继承标志，
    /// 并在父进程里读写管道。实测（2026-09-21，lib 348 条并行）：全量跑时
    /// `a_handle_that_was_not_listed_is_not_inherited` 偶发
    /// `句柄无效（os error 6）`，而单跑、以及这四条彼此串行时必过。
    ///
    /// 它们共用进程级的全局资源，并行本身就是一种不真实的使用方式——
    /// 产品一次只解析一个文件。加闸门是为了让门禁**可信**：一条会因为
    /// 环境而随机变红的测试，会把真正的回归淹没在噪声里。
    static PROCESS_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn process_gate() -> std::sync::MutexGuard<'static, ()> {
        // 某个用例 panic 会让锁中毒，但那不影响下一个用例：
        // 这里要的是「同一时刻只有一个」，而不是「锁没被毒过」。
        PROCESS_GATE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn a_plain_argument_is_not_quoted() {
        assert_eq!(quote_windows_arg("--mode"), "--mode");
        assert_eq!(quote_windows_arg("text"), "text");
    }

    #[test]
    fn an_argument_with_spaces_is_quoted() {
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg(""), "\"\"");
    }

    #[test]
    fn trailing_backslashes_do_not_escape_the_closing_quote() {
        // 这是命令行拼接最经典的一个坑：`"C:\dir\"` 里的 `\"` 会把收尾引号
        // 转义掉，于是后面所有参数都被吞进同一个参数里。
        assert_eq!(
            quote_windows_arg(r"C:\Program Files\"),
            r#""C:\Program Files\\""#
        );
    }

    #[test]
    fn embedded_quotes_are_escaped() {
        assert_eq!(quote_windows_arg("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn the_command_line_round_trips_through_the_same_rules() {
        let program = Path::new(r"C:\Program Files\FilePilot\extract_worker.exe");
        let args = vec![
            "--handle".to_owned(),
            "4242".to_owned(),
            "--file-id".to_owned(),
            "abc".to_owned(),
        ];
        let wide = command_line_for(program, &args);
        let text = String::from_utf16_lossy(&wide[..wide.len() - 1]);

        assert_eq!(
            text,
            r#""C:\Program Files\FilePilot\extract_worker.exe" --handle 4242 --file-id abc"#
        );
    }

    #[test]
    fn a_job_object_can_be_created_and_dropped() {
        let job = JobObject::create(64 * 1024 * 1024, 1).expect("应当能建作业");
        drop(job);
    }

    #[test]
    fn opening_a_missing_file_for_reading_fails() {
        let missing = std::path::Path::new(r"C:\__filepilot_definitely_absent__\x.txt");
        assert!(ReadOnlyHandle::open(missing).is_err());
    }

    #[test]
    fn a_read_only_handle_can_be_opened_on_a_real_file() {
        let dir = tempfile::tempdir().expect("临时目录");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello").expect("写文件");

        let handle = ReadOnlyHandle::open(&path).expect("应当能打开");
        assert!(handle.raw_value() != 0, "句柄值不该是 0");
    }

    #[test]
    fn the_child_receives_only_the_handles_we_listed() {
        let _gate = process_gate();
        // 派生一个真的子进程，让它把自己句柄表里能看到的文件数报回来。
        // 这是「只继承一支句柄」这条保证唯一的验证方式——协议层面
        // 「没有路径字段」只说明我们**没传**，说明不了**没继承**。
        let dir = tempfile::tempdir().expect("临时目录");
        let path = dir.path().join("payload.txt");
        std::fs::write(&path, b"payload").expect("写文件");

        let handle = ReadOnlyHandle::open(&path).expect("打开只读句柄");

        let script = format!(
            "$h = [IntPtr]{}; \
             $fs = New-Object System.IO.FileStream($h, [System.IO.FileAccess]::Read, $false); \
             $b = New-Object byte[] 7; \
             $null = $fs.Read($b, 0, 7); \
             [Console]::Out.Write([Text.Encoding]::ASCII.GetString($b)); \
             $fs.Dispose()",
            handle.raw_value()
        );

        let mut child = spawn_with_inherited_handles(
            Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            &[
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                script,
            ],
            &[handle.raw_value()],
        )
        .expect("应当能派生子进程");

        let captured = child.read_stdout().expect("读输出");
        let exit = child.wait_timeout(Duration::from_secs(30)).expect("等待");
        assert_eq!(exit, Some(0), "子进程应当正常结束：{captured}");
        assert_eq!(
            captured.trim(),
            "payload",
            "子进程应当能通过继承来的句柄读到内容"
        );
    }

    #[test]
    fn a_handle_that_was_not_listed_is_not_inherited() {
        let _gate = process_gate();
        // 反面对照：同一支句柄，**不放进清单**，子进程就不该能读它。
        // 没有这条断言，「只继承清单里的句柄」就只是一句话。
        let dir = tempfile::tempdir().expect("临时目录");
        let path = dir.path().join("payload.txt");
        std::fs::write(&path, b"payload").expect("写文件");

        let handle = ReadOnlyHandle::open(&path).expect("打开只读句柄");

        let script = format!(
            "$h = [IntPtr]{}; \
             try {{ \
               $fs = New-Object System.IO.FileStream($h, [System.IO.FileAccess]::Read, $false); \
               [Console]::Out.Write('READ_OK'); \
               $fs.Dispose() \
             }} catch {{ [Console]::Out.Write('FAILED') }}",
            handle.raw_value()
        );

        let mut child = spawn_with_inherited_handles(
            Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            &[
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                script,
            ],
            // 清单里**没有**这支句柄。
            &[],
        )
        .expect("应当能派生子进程");

        let captured = child.read_stdout().expect("读输出");
        let _ = child.wait_timeout(Duration::from_secs(30)).expect("等待");
        assert_eq!(
            captured.trim(),
            "FAILED",
            "没列进清单的句柄不该出现在子进程里"
        );
    }

    #[test]
    fn a_job_with_a_one_process_limit_rejects_grandchildren() {
        let _gate = process_gate();
        // 规格 6.2：「禁止创建子进程」。这条不能靠约定，必须由内核执行——
        // 因此它必须被真的验证一次。
        let job = JobObject::create(256 * 1024 * 1024, 1).expect("建作业");

        let mut child = spawn_with_inherited_handles(
            Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            &[
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                // 试着再开一个进程，并把结果报回来。
                "try { $p = Start-Process -FilePath cmd.exe -ArgumentList '/c','exit' \
                 -PassThru -Wait -ErrorAction Stop; [Console]::Out.Write('SPAWNED') } \
                 catch { [Console]::Out.Write('BLOCKED') }"
                    .to_owned(),
            ],
            &[],
        )
        .expect("派生子进程");

        job.assign(&child).expect("放进作业");

        let captured = child.read_stdout().expect("读输出");
        let _ = child.wait_timeout(Duration::from_secs(60)).expect("等待");

        assert_eq!(
            captured.trim(),
            "BLOCKED",
            "作业的活动进程数为 1 时，子进程不该能再开子进程：{captured}"
        );
    }

    #[test]
    fn terminating_the_job_kills_the_process_inside_it() {
        let _gate = process_gate();
        let job = JobObject::create(256 * 1024 * 1024, 1).expect("建作业");

        let mut child = spawn_with_inherited_handles(
            Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            &[
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                "Start-Sleep -Seconds 120".to_owned(),
            ],
            &[],
        )
        .expect("派生子进程");

        job.assign(&child).expect("放进作业");
        assert!(child.is_running(), "先说清楚现场：它正在跑");

        // 超时路径：等一小会儿，然后终止整个作业。
        let waited = child
            .wait_timeout(Duration::from_millis(300))
            .expect("等待");
        assert!(waited.is_none(), "这段时间里它不该自己结束");

        job.terminate(1).expect("终止作业");
        let after = child
            .wait_timeout(Duration::from_secs(10))
            .expect("再等一次");
        assert!(
            after.is_some(),
            "终止之后必须真的结束，不能只是「发了信号」"
        );
        assert!(!child.is_running());

        // 读一次，确保管道不会被漏掉（也让它进入一个干净的状态）。
        let _ = child.read_stdout();
    }
}
