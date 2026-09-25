//! 工作进程侧：拿到一支句柄，把内容读出来，给出一个统一的结论。
//!
//! 这个模块**只被 `extract_worker` 可执行文件调用**。放在库里而不是 bin 里，
//! 是为了让它能被编译检查到——一个只存在于 bin 里的模块，
//! 在 `cargo test --lib` 里是看不见的。
//!
//! ## 这里的每一行都跑在「不可信输入」上
//!
//! 它读的是用户的文件。因此：
//! * 读之前先按格式的上限**定量**，而不是读到多少算多少；
//! * 不做任何路径操作——手里只有一支句柄；
//! * 任何解析库的错误都翻译成错误码，不 panic、不 `unwrap`。

use std::io::Read;

use super::limits::{MAX_DOCX_BYTES, MAX_IMAGE_BYTES, MAX_PDF_BYTES, MAX_TEXT_BYTES};
use super::protocol::{
    OcrAvailabilityReport, SourceFormat, WorkerArgs, WorkerMode, WorkerOutcome, PROTOCOL_VERSION,
};
use crate::domain::errors::codes;
use crate::domain::types::{Evidence, ExtractionStatus};

/// 每种格式在**读入内存之前**要守住的字节上限。
///
/// 与解析器内部的检查是两道闸：这一道防的是「把 2 GiB 读进内存」，
/// 解析器那一道防的是「解压/展开之后变得很大」。两道都要有——
/// 只有后者的话，内存已经在读的时候花掉了。
fn read_limit(format: SourceFormat) -> u64 {
    match format {
        SourceFormat::Text => MAX_TEXT_BYTES,
        SourceFormat::Pdf => MAX_PDF_BYTES,
        SourceFormat::Docx => MAX_DOCX_BYTES,
        SourceFormat::Image => MAX_IMAGE_BYTES,
    }
}

/// 工作进程的产出。用枚举而不是「一个万能结构」：
/// 两种模式的结果形状完全不同，而**形状由模式决定**——
/// 让调用方按一个空字段去猜「这次是提取还是探测」正是错误的来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerOutput {
    Extraction(WorkerOutcome),
    OcrAvailability(OcrAvailabilityReport),
}

impl WorkerOutput {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        match self {
            WorkerOutput::Extraction(outcome) => serde_json::to_string(outcome),
            WorkerOutput::OcrAvailability(report) => serde_json::to_string(report),
        }
    }
}

/// 按模式干活。
pub fn run(args: &WorkerArgs) -> WorkerOutput {
    match args.mode {
        // OCR 可用性探测。**这是 WinRT 在生产里唯一的入口**：
        // 界面进程从不直接调它（公寓模型绑定线程，多线程调用会段错误）。
        WorkerMode::OcrAvailability => {
            WorkerOutput::OcrAvailability(crate::platform::ocr::availability().to_report())
        }
        WorkerMode::Extract => WorkerOutput::Extraction(extract_from_handle(args)),
    }
}

/// 提取模式：按句柄读文件并提取。
///
/// `handle` 是从父进程继承来的只读句柄。这里**不接受路径**：
/// 路径会把「只能读授权过的文件」变成一句需要被检查的约定，
/// 而句柄让它成为一个无从表达的事实。
fn extract_from_handle(args: &WorkerArgs) -> WorkerOutcome {
    if args.version != PROTOCOL_VERSION {
        return WorkerOutcome::failed(
            args.file_id.as_deref().unwrap_or_default(),
            codes::EXTRACTION_WORKER_FAILED,
        );
    }
    // `validate()` 在 `main` 里已经跑过；这里再挡一次是因为这个函数
    // 也可以被单测直接调用，而「少了句柄还能继续跑」是不能接受的。
    let Some((handle, file_id, format)) = args.extraction() else {
        return WorkerOutcome::failed(
            args.file_id.as_deref().unwrap_or_default(),
            codes::EXTRACTION_WORKER_FAILED,
        );
    };

    let bytes = match read_handle(handle, read_limit(format)) {
        Ok(bytes) => bytes,
        Err(code) => return WorkerOutcome::failed(file_id, code),
    };

    match format {
        SourceFormat::Text => extract_text(file_id, &bytes),
        SourceFormat::Pdf => extract_pdf(file_id, &bytes),
        SourceFormat::Docx => extract_docx(file_id, &bytes),
        SourceFormat::Image => extract_image(file_id, &bytes),
    }
}

/// 图片：解码 + 像素上限 + 系统 OCR。
fn extract_image(file_id: &str, bytes: &[u8]) -> WorkerOutcome {
    match super::image::extract(bytes) {
        Ok(result) => {
            let evidence = super::image::evidence(&result);
            WorkerOutcome {
                version: PROTOCOL_VERSION,
                file_id: file_id.to_owned(),
                status: ExtractionStatus::Ok,
                code: None,
                text: result.text,
                truncated: result.truncated,
                evidence,
            }
        }
        // 这里**不能**把图片的所有错误都当成 unsupported。
        //
        // 第一版就是这么写的，后果是：一张损坏的 PNG 报 `Unsupported`，
        // 而一份损坏的 PDF / DOCX 报 `Failed`——同一个 `EXTRACTION_CORRUPT`
        // 在两个格式上给出不同结论。用户看到的会是「不支持此格式」，
        // 而实际上那份文件本该能读、只是坏了，两者的下一步动作并不相同。
        // 分流规则与其他格式对齐：
        //
        // * `UNSUPPORTED_FORMAT` —— 图里确实没有文字。这不是错误，
        //   它是一张照片或一张纯色图，只是没有可提的正文。
        // * `OCR_UNAVAILABLE` —— 文件没问题，是**这台机器**还没准备好。
        //   用户去装个语言包就能用，所以它不是「这份文件读不下去」。
        // * 其余（损坏、超限）—— 本该能读却没读成，如实报 failed。
        Err(code) => {
            if matches!(code, codes::UNSUPPORTED_FORMAT | codes::OCR_UNAVAILABLE) {
                WorkerOutcome::unsupported(file_id, code)
            } else {
                WorkerOutcome::failed(file_id, code)
            }
        }
    }
}

/// 从原始句柄读入全部内容，最多 `limit` 字节。
///
/// 多读一个字节是有意的：正好读到 `limit` 时无法区分「文件就这么大」
/// 和「文件更大但被截了」。多要一个字节，两者立刻分明。
///
/// **先归零再读**：这支句柄是从父进程继承来的，而 Windows 的文件位置属于
/// FILE_OBJECT、被父子两支句柄**共享**。父进程在交给我们之前刚在它上面
/// 算过一遍哈希，位置停在 EOF——不归零的话我们读到的是空文件，
/// 而「提取到 0 个字符」会被下游当成「这文档没内容」。
fn read_handle(handle: u64, limit: u64) -> Result<Vec<u8>, &'static str> {
    use std::io::{Seek as _, SeekFrom};
    use std::os::windows::io::FromRawHandle;

    // SAFETY: `handle` 由父进程通过可继承句柄传入，在本进程的句柄表里有效。
    // 这里**接管**它的所有权：进程退出时句柄随之关闭，这正是我们要的。
    let mut file =
        unsafe { std::fs::File::from_raw_handle(handle as std::os::windows::raw::HANDLE) };

    let meta = file
        .metadata()
        .map_err(|_| codes::EXTRACTION_WORKER_FAILED)?;
    if !meta.is_file() {
        // 目录、设备、管道都不该走到这里。传进来的应当是一支普通文件句柄。
        return Err(codes::EXTRACTION_WORKER_FAILED);
    }

    file.seek(SeekFrom::Start(0))
        .map_err(|_| codes::EXTRACTION_WORKER_FAILED)?;

    let mut buffer = Vec::new();
    let mut limited = file.by_ref().take(limit + 1);
    limited
        .read_to_end(&mut buffer)
        .map_err(|_| codes::PERMISSION_DENIED)?;

    if buffer.len() as u64 > limit {
        return Err(codes::EXTRACTION_TOO_LARGE);
    }
    Ok(buffer)
}

fn extract_text(file_id: &str, bytes: &[u8]) -> WorkerOutcome {
    match super::text::decode(bytes) {
        Ok(decoded) => {
            let (text, truncated) = super::text::truncate(&decoded.text);
            let evidence = text_evidence(&text, decoded.encoding.label(), truncated);
            WorkerOutcome {
                version: PROTOCOL_VERSION,
                file_id: file_id.to_owned(),
                status: ExtractionStatus::Ok,
                code: None,
                text,
                truncated,
                evidence,
            }
        }
        Err(code) => WorkerOutcome::failed(file_id, code),
    }
}

fn extract_pdf(file_id: &str, bytes: &[u8]) -> WorkerOutcome {
    match super::pdf::extract(bytes) {
        Ok(result) => {
            // 先把两个布尔量算出来，再移动 `text`。顺序反了会让
            // 「部分读取」的判定发生在正文已经被移走之后。
            let read_only_first_pages = result.read_only_first_pages();
            let evidence = pdf_evidence(&result);
            WorkerOutcome {
                version: PROTOCOL_VERSION,
                file_id: file_id.to_owned(),
                status: if read_only_first_pages {
                    // 只读了前面几页：正文是完整的（在已读范围内），
                    // 但**不是**整篇。规格用 `truncated` 表达这件事。
                    ExtractionStatus::Partial
                } else {
                    ExtractionStatus::Ok
                },
                code: None,
                text: result.text,
                truncated: result.truncated || read_only_first_pages,
                evidence,
            }
        }
        // 扫描型 PDF（没有文字层）是 unsupported，不是 failed。
        Err(codes::UNSUPPORTED_FORMAT) => {
            WorkerOutcome::unsupported(file_id, codes::UNSUPPORTED_FORMAT)
        }
        Err(code) => WorkerOutcome::failed(file_id, code),
    }
}

fn extract_docx(file_id: &str, bytes: &[u8]) -> WorkerOutcome {
    match super::docx::extract(bytes) {
        Ok(result) => {
            let evidence = plain_evidence(&result.text, result.truncated);
            WorkerOutcome {
                version: PROTOCOL_VERSION,
                file_id: file_id.to_owned(),
                // 正文 XML 在、但里面没有文字：这是「没有可提的正文」，
                // 与一个坏文件不同。
                status: if result.text.trim().is_empty() {
                    ExtractionStatus::Unsupported
                } else {
                    ExtractionStatus::Ok
                },
                code: if result.text.trim().is_empty() {
                    Some(codes::UNSUPPORTED_FORMAT.to_owned())
                } else {
                    None
                },
                text: result.text,
                truncated: result.truncated,
                evidence,
            }
        }
        Err(code) => WorkerOutcome::failed(file_id, code),
    }
}

/// 文本类文件的证据片段。
///
/// 规格 6.4 要求 `evidenceLocator` 必须对应**真实提取结果**——
/// 前端要拿它去本地取值给用户看。因此这里的 locator 说的是
/// 「这段片段在本次提取结果里的字符范围」，而不是一个编造的页码。
fn text_evidence(text: &str, encoding: &str, truncated: bool) -> Vec<Evidence> {
    let mut evidence = plain_evidence(text, truncated);
    evidence.insert(
        0,
        Evidence {
            locator: format!(
                "chars:0-{}",
                text.chars().count().min(super::limits::MAX_EVIDENCE_CHARS)
            ),
            excerpt: format!("编码 {encoding}"),
        },
    );
    evidence
}

fn pdf_evidence(result: &super::pdf::PdfText) -> Vec<Evidence> {
    let mut evidence = plain_evidence(&result.text, result.truncated);
    evidence.insert(
        0,
        Evidence {
            locator: format!("pages:1-{}", result.pages_read),
            excerpt: format!(
                "共 {} 页，本次读了前 {} 页",
                result.total_pages, result.pages_read
            ),
        },
    );
    evidence
}

/// 「第一段正文」这一条通用证据。
fn plain_evidence(text: &str, truncated: bool) -> Vec<Evidence> {
    let excerpt: String = text
        .chars()
        .take(super::limits::MAX_EVIDENCE_CHARS)
        .collect();
    if excerpt.trim().is_empty() {
        return Vec::new();
    }
    vec![Evidence {
        locator: format!("chars:0-{}", excerpt.chars().count()),
        excerpt: if truncated {
            format!("{excerpt}…（已截断）")
        } else {
            excerpt
        },
    }]
}

impl super::pdf::PdfText {
    /// 是否只读了文档的一部分。
    fn read_only_first_pages(&self) -> bool {
        self.pages_read < self.total_pages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 跑提取并取出结果。模式不对就直接失败——测试里不该出现别的模式。
    fn extract(args: &WorkerArgs) -> WorkerOutcome {
        match run(args) {
            WorkerOutput::Extraction(outcome) => outcome,
            other => panic!("期望提取结果，得到 {other:?}"),
        }
    }

    /// 用一个真实临时文件造出一支句柄，模拟父进程传进来的东西。
    ///
    /// 不用 `File::open` 直接造 `WorkerArgs`：`run` 接的是**裸句柄**，
    /// 所以测试也必须走一遍「拿到裸句柄」这条路，否则测的是另一条代码路径。
    fn worker_args_for(
        path: &std::path::Path,
        format: SourceFormat,
    ) -> (WorkerArgs, std::fs::File) {
        use std::os::windows::io::AsRawHandle;
        let file = std::fs::File::open(path).expect("打开测试文件");
        let handle = file.as_raw_handle() as u64;
        (
            WorkerArgs {
                version: PROTOCOL_VERSION,
                mode: WorkerMode::Extract,
                handle: Some(handle),
                file_id: Some("file-1".to_owned()),
                format: Some(format),
            },
            file,
        )
    }

    fn write_temp(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("写测试文件");
        (dir, path)
    }

    #[test]
    fn a_text_file_is_read_through_the_handle() {
        let (_dir, path) = write_temp("a.txt", "会议纪要\n第二行".as_bytes());
        let (args, _file) = worker_args_for(&path, SourceFormat::Text);

        let outcome = extract(&args);

        assert_eq!(outcome.file_id, "file-1");
        assert!(matches!(outcome.status, ExtractionStatus::Ok));
        assert!(outcome.text.contains("会议纪要"));
        assert!(!outcome.evidence.is_empty(), "提取结果必须带证据");
    }

    #[test]
    fn a_file_over_the_format_limit_is_reported_as_too_large() {
        // 文本上限 10 MiB：写 10 MiB + 1 字节，证明「多读一个字节」的判定生效。
        let oversized = vec![b'a'; (MAX_TEXT_BYTES + 1) as usize];
        let (_dir, path) = write_temp("big.txt", &oversized);
        let (args, _file) = worker_args_for(&path, SourceFormat::Text);

        let outcome = extract(&args);
        assert_eq!(outcome.code.as_deref(), Some(codes::EXTRACTION_TOO_LARGE));
        assert!(outcome.text.is_empty(), "超限时不能返回半份正文");
    }

    #[test]
    fn exactly_at_the_limit_is_accepted() {
        // 边界要两头都定住：正好等于上限不算超限。
        let exact = vec![b'a'; MAX_TEXT_BYTES as usize];
        let (_dir, path) = write_temp("exact.txt", &exact);
        let (args, _file) = worker_args_for(&path, SourceFormat::Text);

        let outcome = extract(&args);
        assert!(matches!(
            outcome.status,
            ExtractionStatus::Ok | ExtractionStatus::Partial
        ));
        assert_eq!(
            outcome.text.chars().count(),
            super::super::limits::MAX_EXTRACTED_CHARS
        );
    }

    #[test]
    fn a_wrong_protocol_version_is_refused() {
        let (_dir, path) = write_temp("a.txt", b"hello");
        let (mut args, _file) = worker_args_for(&path, SourceFormat::Text);
        args.version = PROTOCOL_VERSION + 1;

        let outcome = extract(&args);
        assert_eq!(
            outcome.code.as_deref(),
            Some(codes::EXTRACTION_WORKER_FAILED)
        );
    }

    #[test]
    fn a_garbage_pdf_reports_a_code_instead_of_panicking() {
        let (_dir, path) = write_temp("bad.pdf", &[0xFFu8; 512]);
        let (args, _file) = worker_args_for(&path, SourceFormat::Pdf);

        let outcome = extract(&args);
        assert!(matches!(outcome.status, ExtractionStatus::Failed));
        assert!(outcome.code.is_some());
    }

    #[test]
    fn the_evidence_locator_points_into_the_returned_text() {
        let (_dir, path) = write_temp("a.txt", "abcdef".as_bytes());
        let (args, _file) = worker_args_for(&path, SourceFormat::Text);

        let outcome = extract(&args);
        let first = outcome.evidence.first().expect("应当有证据");
        // 规格 6.4：locator 必须对应真实提取结果——这里的「字符范围」
        // 必须真的落在返回的正文里，而不是一个编造的页码。
        assert!(first.locator.starts_with("chars:0-"), "{}", first.locator);
        assert!(outcome.text.starts_with("abcdef"));
    }
}
