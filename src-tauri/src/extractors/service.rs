//! 界面进程侧的提取协调器（规格 6.2）。
//!
//! 它负责把「一次提取」拆成一段可以逐条解释的过程，并且**每一段失败都只影响
//! 那一个文件**——规格要求「单文件失败不拖垮扫描会话；UI 进度持续响应」。
//!
//! ## 每个文件走这九步
//!
//! 1. 由扩展名定格式（判不出来直接 `unsupported`，不猜）；
//! 2. 在批准根内解析路径；
//! 3. 以 `FILE_SHARE_READ` 打开——**从这一刻起别的进程不能再写它**；
//! 4. 在这支句柄上取**前**指纹；
//! 5. 派生工作进程，只继承这一支句柄，套上作业对象；
//! 6. 一个线程读它的 stdout，主线程带超时等它；
//! 7. 超时/被杀 → 终止整个作业，回 `EXTRACTION_TIMEOUT` 或 `EXTRACTION_KILLED`；
//! 8. 在同一支句柄上取**后**指纹，与前一比对，不一致回 `SOURCE_CHANGED`；
//! 9. 把 worker 的结论组装成 `Extraction`。
//!
//! ## 第 3 步与第 8 步为什么都要
//!
//! 规格 6.2 给的是「**或**」：要么用固定只读句柄阻止并发写入，要么用前后指纹
//! 一致性证明读取期间稳定。这里两条都做——它们防的不是同一件事：
//! 共享模式挡住的是**别的进程**，指纹比对还能抓到「我们自己读的那份字节变了」
//! （比如磁盘层面的问题）。多花一次哈希，换一个可以说清的结论，值。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use super::format_for;
use super::limits::{EXTRACT_TIMEOUT_MS, MAX_EVIDENCE, WORKER_MEMORY_LIMIT_BYTES};
use super::protocol::{
    OcrAvailabilityReport, SourceFormat, WorkerMode, WorkerOutcome, PROTOCOL_VERSION,
};
use crate::domain::errors::{codes, AppError};
use crate::domain::types::{Evidence, Extraction, ExtractionStatus, Fingerprint, RelPath};
use crate::platform::child_process::{spawn_with_inherited_handles, JobObject, ReadOnlyHandle};
use crate::safety::root::ApprovedRoot;

/// 一次提取请求。
#[derive(Debug, Clone)]
pub struct ExtractRequest {
    pub file_id: String,
    /// 相对批准根的路径组件数组。**不接受绝对路径**：与项目其余部分一致，
    /// 路径只在根之内解析。
    pub relative_path: RelPath,
}

/// 进度与取消。与执行器的 `ExecutionObserver` 同形，理由也一样：
/// 它们由同一件事驱动（「还在跑」），分成两个接口只会让调用方两处都要实现。
pub trait ExtractObserver: Send + Sync {
    /// `processed` 含失败与跳过的项——不报的话进度条会停在中间不动。
    fn on_progress(&self, processed: u32, total: u32);
    fn should_stop(&self) -> bool {
        false
    }
}

/// 不上报、不取消。
pub struct NoopExtractObserver;

impl ExtractObserver for NoopExtractObserver {
    fn on_progress(&self, _processed: u32, _total: u32) {}
}

/// 按 `EXTRACT_CONCURRENCY` 并发提取一批文件。
///
/// 返回的顺序与 `requests` **一一对应**：调用方要能把结果贴回它自己的行上，
/// 顺序错乱会让「这个文件为什么提取失败」指向另一个文件。
pub fn extract_batch(
    root: &ApprovedRoot,
    requests: &[ExtractRequest],
    observer: &dyn ExtractObserver,
) -> Vec<Extraction> {
    let total = requests.len() as u32;
    observer.on_progress(0, total);

    let worker = match worker_binary() {
        Ok(path) => path,
        Err(error) => {
            // 工作进程找不到是**环境问题**，不是这些文件的问题。
            // 如实把整批标成失败，而不是逐个文件报「无法解析」——
            // 后者会让用户去检查一堆本来完全正常的文件。
            return requests
                .iter()
                .map(|request| {
                    failed(
                        request,
                        codes::EXTRACTION_WORKER_FAILED,
                        &format!("找不到解析工作进程：{error}"),
                    )
                })
                .collect();
        }
    };

    let mut results: Vec<Option<Extraction>> = vec![None; requests.len()];
    let mut next = 0usize;
    let mut processed = 0u32;

    // 规格 6.2：提取并发 2。每个提取是一个独立进程、各自可能吃满 256 MiB，
    // 所以这个数字是**内存策略**而不是性能调优。
    let concurrency = super::limits::EXTRACT_CONCURRENCY.max(1);
    std::thread::scope(|scope| {
        let mut running = Vec::new();

        loop {
            while running.len() < concurrency && next < requests.len() && !observer.should_stop() {
                let index = next;
                let request = &requests[index];
                let worker = worker.clone();
                running.push(scope.spawn(move || (index, extract_one(root, request, &worker))));
                next += 1;
            }

            if running.is_empty() {
                break;
            }

            // 先到先收。用 join 逐条收而不是 select：并发上限只有 2，
            // 一条慢的挡住第二条的回收最多多等同样长的时间，
            // 而换成条件变量会让这段代码多出「谁唤醒谁」的推理成本。
            let (index, outcome) = running.remove(0).join().unwrap_or_else(|_| {
                // 提取线程 panic 了。不把整批带走：把这一条标成内部错误，
                // 其余照常。`unwrap_or_else` 需要给 (usize, Extraction)，
                // 但这里拿不到 index —— 用哨兵值在下面剔除。
                (
                    usize::MAX,
                    Extraction {
                        file_id: String::new(),
                        source_fingerprint: empty_fingerprint(root),
                        status: ExtractionStatus::Failed,
                        text: String::new(),
                        evidence: Vec::new(),
                        truncated: false,
                        code: Some(codes::INTERNAL.to_owned()),
                    },
                )
            });
            if index != usize::MAX {
                results[index] = Some(outcome);
            }
            processed += 1;
            observer.on_progress(processed, total);
        }

        // 取消时剩下的项不再派发，但也要给出一份结果——
        // 否则调用方拿到的数组长度与请求数对不上。
        for (index, slot) in results.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(failed(
                    &requests[index],
                    codes::EXTRACTION_WORKER_FAILED,
                    "已取消，未提取",
                ));
            }
        }
    });

    results
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            // 这里**不写 `unreachable!`**。
            //
            // 这个函数的整个设计前提是「任何失败都变成一份 `Extraction`，
            // 不向外抛」（见 `extract_one` 的文档）。一个 panic 会把**整批**
            // 已经提取好的结果一起带走，而用户要的恰恰是「能提多少提多少」。
            //
            // 上面那个补齐循环保证这里不该出现 `None`。万一将来有人改动了
            // 它，下面这份结果给出的信息比一次崩溃有用得多。
            slot.unwrap_or_else(|| {
                failed(&requests[index], codes::INTERNAL, "内部状态缺失，未提取")
            })
        })
        .collect()
}

/// 提取一个文件。**任何失败都变成一份 `Extraction`**，不向外抛：
/// 单文件的问题不该让整批停下来。
fn extract_one(root: &ApprovedRoot, request: &ExtractRequest, worker: &Path) -> Extraction {
    let extension = request
        .relative_path
        .last()
        .and_then(|name| name.rfind('.').map(|index| name[index..].to_lowercase()))
        .unwrap_or_default();

    let Some(format) = format_for(&extension) else {
        // 规格 6.2 最后一行：其他类型不提取内容，只用文件名/扩展名/时间。
        // 这不是失败，是「这类文件本来就没有可提的正文」。
        return unsupported(request, codes::UNSUPPORTED_FORMAT);
    };

    let path = match root.resolve_existing_within(&request.relative_path) {
        Ok(path) => path,
        Err(error) => return failed(request, &error.code, &error.message),
    };

    // 只读、且只共享读：从这一刻起别的进程写不了它。
    let handle = match ReadOnlyHandle::open(&path) {
        Ok(handle) => handle,
        Err(error) => {
            let code = match error.kind() {
                std::io::ErrorKind::NotFound => codes::SOURCE_MISSING,
                std::io::ErrorKind::PermissionDenied => codes::FILE_BUSY,
                _ => codes::PERMISSION_DENIED,
            };
            return failed(request, code, &format!("无法打开文件: {error}"));
        }
    };

    let before = match fingerprint_of(handle.file(), root.volume_id()) {
        Ok(fingerprint) => fingerprint,
        Err(error) => return failed(request, &error.code, &error.message),
    };

    let outcome = run_worker(worker, request, format, handle.raw_value());
    let (status, code, text, truncated, evidence) = match outcome {
        Ok(worker_outcome) => (
            worker_outcome.status,
            worker_outcome.code,
            worker_outcome.text,
            worker_outcome.truncated,
            worker_outcome.evidence,
        ),
        Err(error) => (
            ExtractionStatus::Failed,
            Some(error.code),
            String::new(),
            false,
            Vec::new(),
        ),
    };

    // 读完之后再取一次指纹。共享模式已经挡住了别的进程，这里再比一次是
    // 为了能对「读期间字节变过」给出一个**明确结论**，而不是假定它没变。
    let after = match fingerprint_of(handle.file(), root.volume_id()) {
        Ok(fingerprint) => fingerprint,
        Err(error) => return failed(request, &error.code, &error.message),
    };
    if before != after {
        return failed(
            request,
            codes::SOURCE_CHANGED,
            "文件在提取期间发生了变化，这份正文不能用于建议",
        );
    }

    Extraction {
        file_id: request.file_id.clone(),
        source_fingerprint: before,
        status,
        text,
        evidence: evidence.into_iter().take(MAX_EVIDENCE).collect(),
        truncated,
        code,
    }
}

/// 派生工作进程、读回它的结论。
fn run_worker(
    worker: &Path,
    request: &ExtractRequest,
    format: SourceFormat,
    handle: u64,
) -> Result<WorkerOutcome, AppError> {
    let args = vec![
        "--version".to_owned(),
        PROTOCOL_VERSION.to_string(),
        "--mode".to_owned(),
        WorkerMode::Extract.as_str().to_owned(),
        "--handle".to_owned(),
        handle.to_string(),
        "--file-id".to_owned(),
        request.file_id.clone(),
        "--format".to_owned(),
        format.as_str().to_owned(),
    ];

    let mut child = spawn_with_inherited_handles(worker, &args, &[handle]).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法启动解析工作进程: {error}"),
        )
    })?;

    // 作业必须在子进程做任何值得限制的事之前套上。
    let job = JobObject::create(WORKER_MEMORY_LIMIT_BYTES, 1).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法创建解析作业对象: {error}"),
        )
    })?;
    job.assign(&child).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法把工作进程放进作业: {error}"),
        )
    })?;

    // 读输出与等进程必须并行：先读会被卡住的子进程拖死，先等会被
    // 装满管道缓冲区的响应拖死。把读端挪到线程里就两边都不卡。
    let mut stdout = child.take_stdout();
    let (sender, receiver) = mpsc::channel::<String>();
    let reader = stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            let _ = sender.send(text);
        })
    });

    let timeout = Duration::from_millis(EXTRACT_TIMEOUT_MS);
    let finished = child.wait_timeout(timeout).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("等待工作进程失败: {error}"),
        )
    })?;

    if finished.is_none() {
        // 超时。**必须真的杀掉它**：只把这一侧放弃会让一个吃满内存的
        // 解析器在后台继续跑，而界面上已经显示「超时」了。
        job.terminate(codes::EXTRACTION_TIMEOUT.len() as u32)
            .map_err(|error| {
                AppError::new(
                    codes::EXTRACTION_KILLED,
                    format!("超时后无法终止工作进程: {error}"),
                )
            })?;
        let _ = child.wait_timeout(Duration::from_secs(5));
        if let Some(reader) = reader {
            let _ = reader.join();
        }
        return Err(AppError::new(
            codes::EXTRACTION_TIMEOUT,
            "解析超时，工作进程已被终止",
        ));
    }

    // 子进程退出时写端关闭，读线程随之结束。给一个小上限兜底：
    // 正常情况下它早就读完了。
    let payload = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();
    if let Some(reader) = reader {
        let _ = reader.join();
    }

    let exit_code = finished.unwrap_or_default();
    if payload.trim().is_empty() {
        // 没拿到任何输出。区分「被资源限制杀掉」与「自己崩了」：
        // 作业的内存上限触发时进程会被终止，退出码通常非 0。
        let code = if exit_code == 0 {
            codes::EXTRACTION_WORKER_FAILED
        } else {
            codes::EXTRACTION_KILLED
        };
        return Err(AppError::new(
            code,
            format!("解析工作进程没有返回结果（退出码 {exit_code}）"),
        ));
    }

    serde_json::from_str::<WorkerOutcome>(payload.trim()).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("解析工作进程返回了无法识别的结果: {error}"),
        )
    })
}

/// 在**已有的句柄**上取完整指纹。
///
/// 用句柄而不是路径：路径可能在两次调用之间指向另一个文件，
/// 而指纹的全部意义就是「这份字节属于哪一个对象」。
///
/// **必须先把读取位置挪回 0**，两个原因：
///
/// 1. 同一个句柄会被用来取**前**、**后**两次指纹。`sha256_of_handle`
///    从当前位置读到底，第一次读完之后位置停在 EOF——不归零的话
///    第二次读到 0 字节，哈希与第一次「不同」，于是每一个文件都被
///    判成 `SOURCE_CHANGED`。这条曾经让整套验收用例一起变红。
/// 2. 工作进程通过**继承**拿到的是同一支句柄，而 Windows 的文件位置
///    属于 FILE_OBJECT、被两支句柄共享。子进程读完也会把位置留在 EOF。
fn fingerprint_of(handle: &std::fs::File, volume_id: &str) -> Result<Fingerprint, AppError> {
    use crate::platform::windows;
    use std::io::{Seek as _, SeekFrom};

    let identity = windows::file_identity(handle).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("读取文件身份失败: {error}"),
        )
    })?;
    if identity.volume_id_string() != volume_id {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "文件的卷身份与批准根不一致",
        ));
    }
    let (size, modified_ns) = windows::size_and_modified(handle).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("读取文件属性失败: {error}"),
        )
    })?;

    // `Seek` 对 `&File` 也有实现，所以不必把句柄交出去（也不能交）。
    let mut seekable = handle;
    seekable.seek(SeekFrom::Start(0)).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("无法把读取位置归零: {error}"),
        )
    })?;

    let sha256 = crate::scanner::snapshot::sha256_of_handle(handle).map_err(|error| {
        AppError::new(
            codes::PERMISSION_DENIED,
            format!("计算内容哈希失败: {error}"),
        )
    })?;

    Ok(Fingerprint {
        volume_id: volume_id.to_owned(),
        file_id: identity.file_id_string(),
        size: size.to_string(),
        modified_ns,
        sha256: Some(sha256),
    })
}

/// 找到 `extract_worker` 可执行文件。
///
/// 两种布局都要认：
/// * **开发/测试**：测试可执行文件在 `target/debug/deps/`，而 `[[bin]]` 在
///   `target/debug/`——所以要往回退一层；
/// * **打包后**：两者在同一目录。
fn worker_binary() -> Result<PathBuf, std::io::Error> {
    let name = format!("extract_worker{}", std::env::consts::EXE_SUFFIX);
    let mut directory = std::env::current_exe()?;
    directory.pop();

    for _ in 0..2 {
        let candidate = directory.join(&name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !directory.pop() {
            break;
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "在 {} 附近找不到 {name}",
            std::env::current_exe()?.display()
        ),
    ))
}

fn failed(request: &ExtractRequest, code: &str, message: &str) -> Extraction {
    Extraction {
        file_id: request.file_id.clone(),
        source_fingerprint: empty_fingerprint_of(),
        status: ExtractionStatus::Failed,
        text: String::new(),
        evidence: if message.is_empty() {
            Vec::new()
        } else {
            vec![Evidence {
                locator: "error".to_owned(),
                excerpt: message.to_owned(),
            }]
        },
        truncated: false,
        code: Some(code.to_owned()),
    }
}

fn unsupported(request: &ExtractRequest, code: &str) -> Extraction {
    Extraction {
        file_id: request.file_id.clone(),
        source_fingerprint: empty_fingerprint_of(),
        status: ExtractionStatus::Unsupported,
        text: String::new(),
        evidence: Vec::new(),
        truncated: false,
        code: Some(code.to_owned()),
    }
}

fn empty_fingerprint_of() -> Fingerprint {
    Fingerprint {
        volume_id: String::new(),
        file_id: String::new(),
        size: String::new(),
        modified_ns: String::new(),
        sha256: None,
    }
}

fn empty_fingerprint(root: &ApprovedRoot) -> Fingerprint {
    Fingerprint {
        volume_id: root.volume_id().to_owned(),
        ..empty_fingerprint_of()
    }
}

/// 问一次系统 OCR 的可用状态。
///
/// **通过工作进程问，而不是在本进程直接调 WinRT。** 两个理由：
///
/// 1. OCR 是一种解析，界面进程不该碰解析器——这是规格 6.2 的立场；
/// 2. 更要紧的是 WinRT 的公寓模型**绑定线程**。界面进程里会有多个线程
///    （Tauri 的命令线程、我们的后台线程），而第一个线程初始化公寓后退出、
///    第二个线程再调 WinRT 就是 `STATUS_ACCESS_VIOLATION`（实测段错误）。
///    工作进程是短命的单线程进程，天然没有这个问题。
pub fn ocr_availability() -> Result<OcrAvailabilityReport, AppError> {
    let worker = worker_binary().map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("找不到解析工作进程：{error}"),
        )
    })?;

    let args = vec![
        "--version".to_owned(),
        PROTOCOL_VERSION.to_string(),
        "--mode".to_owned(),
        WorkerMode::OcrAvailability.as_str().to_owned(),
    ];

    // 这个模式不读文件，因此**没有句柄要继承**——清单传空。
    let mut child = spawn_with_inherited_handles(&worker, &args, &[]).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法启动解析工作进程: {error}"),
        )
    })?;

    let job = JobObject::create(WORKER_MEMORY_LIMIT_BYTES, 1).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法创建解析作业对象: {error}"),
        )
    })?;
    job.assign(&child).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法把工作进程放进作业: {error}"),
        )
    })?;

    let mut stdout = child.take_stdout();
    let (sender, receiver) = mpsc::channel::<String>();
    let reader = stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            let _ = sender.send(text);
        })
    });

    let finished = child
        .wait_timeout(Duration::from_millis(EXTRACT_TIMEOUT_MS))
        .map_err(|error| {
            AppError::new(
                codes::EXTRACTION_WORKER_FAILED,
                format!("等待工作进程失败: {error}"),
            )
        })?;
    if finished.is_none() {
        let _ = job.terminate(1);
        let _ = child.wait_timeout(Duration::from_secs(5));
        if let Some(reader) = reader {
            let _ = reader.join();
        }
        return Err(AppError::new(
            codes::EXTRACTION_TIMEOUT,
            "探测 OCR 可用性超时，工作进程已被终止",
        ));
    }

    let payload = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();
    if let Some(reader) = reader {
        let _ = reader.join();
    }

    serde_json::from_str::<OcrAvailabilityReport>(payload.trim()).map_err(|error| {
        AppError::new(
            codes::EXTRACTION_WORKER_FAILED,
            format!("无法解析 OCR 可用性结果: {error}"),
        )
    })
}
