//! 规格 T10 验收：内容解析工作进程（规格 6.2）。
//!
//! 这个文件测的不是「解析函数算得对」——那些在 `src/extractors/*` 的单测里。
//! 它测的是**跨进程那条链路**：界面进程把一支只读句柄交给工作进程，
//! 后者把正文读回来，而任何一个环节出问题都只影响那一个文件。
//!
//! ## 关于「资源超限后进程被终止」与「访问未授权路径被限制」
//!
//! 这两条硬保证的**证明在 `src/platform/child_process.rs` 的单测里**，
//! 因为它们需要跑真实子进程去试探边界，而那种试探放在集成测试里
//! 会让「跑一次全套」慢上几十秒：
//!
//! * `a_job_with_a_one_process_limit_rejects_grandchildren` —— 作业限 1 进程时
//!   子进程开不了子进程（由**内核**执行）；
//! * `terminating_the_job_kills_the_process_inside_it` —— 超时终止真的结束进程；
//! * `a_handle_that_was_not_listed_is_not_inherited` —— 没列进清单的句柄，
//!   子进程读不到；
//! * `the_child_receives_only_the_handles_we_listed` —— 列进去的才能用。
//!
//! 这里补的是**端到端**那一段：真的走 `extract_batch`，真的把文件读出来。

#![cfg(windows)]

mod support;

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use filepilot_lib::domain::types::{ExtractionStatus, RelPath};
use filepilot_lib::extractors::{
    extract_batch, ExtractObserver, ExtractRequest, NoopExtractObserver,
};
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn make_root() -> (tempfile::TempDir, ApprovedRoot) {
    let tmp = support::test_root();
    let approved = approve_root(tmp.path()).expect("临时根应被授权");
    (tmp, approved)
}

/// 写一个文件并返回它的相对路径组件。
fn put(root: &ApprovedRoot, relative: &[&str], contents: &[u8]) -> RelPath {
    support::write_file(root.canonical(), relative, contents);
    relative.iter().map(|part| (*part).to_owned()).collect()
}

fn request(file_id: &str, relative: RelPath) -> ExtractRequest {
    ExtractRequest {
        file_id: file_id.to_owned(),
        relative_path: relative,
    }
}

/// 记进度的观察者：既收集进度，也用来做取消。
struct Progress {
    seen: std::sync::Mutex<Vec<u32>>,
    /// 处理完多少项之后开始要求停止。默认大到等于不停。
    stop_after_processed: AtomicU32,
}

impl Progress {
    fn new() -> Self {
        Self {
            seen: std::sync::Mutex::new(Vec::new()),
            stop_after_processed: AtomicU32::new(u32::MAX),
        }
    }

    /// 处理完 `processed` 项之后开始要求停止。
    ///
    /// `should_stop` 是在**派发下一项之前**被问的，所以判据必须是
    /// 「已经处理完几项」，而不是「还剩几项」——后者会在第一项派发之前
    /// 就把闸门关上，一条都跑不完。
    fn stop_after(&self, processed: u32) {
        self.stop_after_processed.store(processed, Ordering::SeqCst);
    }

    fn observed(&self) -> Vec<u32> {
        self.seen.lock().expect("进度锁").clone()
    }

    fn processed(&self) -> u32 {
        // `seen` 的第一条是初始的 0，所以「已处理数」是长度减一。
        (self.seen.lock().expect("进度锁").len() as u32).saturating_sub(1)
    }
}

impl ExtractObserver for Progress {
    fn on_progress(&self, processed: u32, _total: u32) {
        self.seen.lock().expect("进度锁").push(processed);
    }

    fn should_stop(&self) -> bool {
        self.processed() >= self.stop_after_processed.load(Ordering::SeqCst)
    }
}

/// 造一份最小可用的单页 PDF，正文是 `content`。
fn text_pdf(content: &str) -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
         /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_owned(),
        {
            let stream = format!("BT /F1 12 Tf 10 100 Td ({content}) Tj ET");
            format!(
                "<< /Length {} >>\nstream\n{stream}\nendstream",
                stream.len()
            )
        },
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    assemble_pdf(&objects, None)
}

/// 一页都没有的 PDF。
fn empty_pdf() -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [] /Count 0 >>".to_owned(),
    ];
    assemble_pdf(&objects, None)
}

/// 一份**结构完好但被加密**的 PDF。
///
/// 手写一份真正加密的 PDF 需要一个加密实现（RC4/AES + 密钥派生），
/// 那是另一个库的量级。这份样本验证的是**我们的分类与最终行为**：
/// 无论解析器在哪个阶段发现问题，都绝不能返回一份「正文」。
fn encrypted_pdf() -> Vec<u8> {
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [] /Count 0 >>".to_owned(),
        "<< /Filter /Standard /V 1 /R 2 /O <00> /U <00> /P -1 >>".to_owned(),
    ];
    assemble_pdf(&objects, Some("3 0 R"))
}

fn assemble_pdf(objects: &[String], encrypt: Option<&str>) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{body}\nendobj\n", index + 1);
    }
    let xref_at = out.len();
    let _ = write!(out, "xref\n0 {}\n", objects.len() + 1);
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        let _ = writeln!(out, "{offset:010} 00000 n ");
    }
    let trailer = match encrypt {
        Some(reference) => format!(
            "<< /Size {} /Root 1 0 R /Encrypt {reference} /ID [<00><00>] >>",
            objects.len() + 1
        ),
        None => format!("<< /Size {} /Root 1 0 R >>", objects.len() + 1),
    };
    let _ = write!(out, "trailer\n{trailer}\nstartxref\n{xref_at}\n%%EOF\n");
    out
}

/// 造一份 DOCX。`body` 是 `word/document.xml` 的内容。
fn docx_with(body: &str) -> Vec<u8> {
    use zip::write::SimpleFileOptions;

    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file("word/document.xml", options)
            .expect("建条目");
        writer.write_all(body.as_bytes()).expect("写正文");
        writer.finish().expect("收尾");
    }
    buffer.into_inner()
}

fn docx_body(paragraphs: &[&str]) -> String {
    let mut body = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
    );
    for text in paragraphs {
        body.push_str(&format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>"));
    }
    body.push_str("</w:body></w:document>");
    body
}

/// 一个极小的条目解出极大内容的压缩包（压缩炸弹）。
fn docx_bomb() -> Vec<u8> {
    use zip::write::SimpleFileOptions;

    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file("word/document.xml", options)
            .expect("建条目");
        writer
            .write_all(&vec![b'a'; 4 * 1024 * 1024])
            .expect("写炸弹");
        writer.finish().expect("收尾");
    }
    buffer.into_inner()
}

fn code_of(extraction: &filepilot_lib::domain::types::Extraction) -> String {
    extraction.code.clone().unwrap_or_default()
}

/// 这台机器是否**被要求保证**有可用的中文 OCR。
///
/// CI 装了中文 OCR 语言包之后会设 `FILEPILOT_REQUIRE_OCR=1`（见 `.github/workflows/ci.yml`）。
///
/// ## 为什么需要这个开关（CI-005）
///
/// 「本机没装中文语言包 → 跳过真实识别」这条分支是必要的（开发机不一定装），
/// 但它有个坏味道：**如果 CI 上语言包没装上或没生效，用例会安静地退回跳过，
/// 而 CI 依然全绿** —— 覆盖缺口就这么无声无息地回来了。
/// 这正是本项目警告过的那类「一批用例被整段跳过而 CI 依然全绿」。
///
/// 所以：**在能保证环境的地方，把跳过变成失败。**
/// 设了这个变量的机器上，一旦报 `OCR_UNAVAILABLE`，用例直接红 ——
/// 让人看见「语言包没装上」，而不是以为覆盖还在。
fn ocr_required() -> bool {
    std::env::var("FILEPILOT_REQUIRE_OCR").is_ok_and(|value| value == "1")
}

// ---------------------------------------------------------------------------
// 1. 文本：编码与边界
// ---------------------------------------------------------------------------

#[test]
fn a_plain_utf8_text_file_is_extracted() {
    let (_tmp, root) = make_root();
    // 注意断言的**是文件内容**，不是文件名：提取的是字节，不是路径。
    let relative = put(&root, &["会议纪要.txt"], "第一行\n第二行".as_bytes());

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results.len(), 1);
    let extraction = &results[0];
    assert_eq!(extraction.status, ExtractionStatus::Ok, "{:?}", extraction);
    assert!(
        extraction.text.contains("第一行") && extraction.text.contains("第二行"),
        "{:?}",
        extraction.text
    );
    assert!(!extraction.truncated);
    assert!(!extraction.evidence.is_empty(), "提取结果必须带证据");
}

#[test]
fn a_gb18030_chinese_file_is_decoded_not_mangled() {
    // 规格 6.2：UTF-8/UTF-16，**有限**支持 GB18030。
    // 中文 Windows 上老文件大量是这个编码。
    let (_tmp, root) = make_root();
    let (encoded, _, _) = encoding_rs::GB18030.encode("这是一份中文报告");
    let relative = put(&root, &["报告.txt"], &encoded);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Ok);
    assert_eq!(results[0].text, "这是一份中文报告");
}

#[test]
fn a_utf16_file_is_decoded() {
    let (_tmp, root) = make_root();
    let mut bytes = vec![0xFF, 0xFE];
    for unit in "记事本另存为 Unicode".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let relative = put(&root, &["unicode.txt"], &bytes);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Ok);
    assert_eq!(results[0].text, "记事本另存为 Unicode");
}

#[test]
fn garbage_bytes_fail_loudly_with_a_code() {
    // 规格 6.2：「乱码明确失败」。绝不能返回一段看起来正常的正文。
    let (_tmp, root) = make_root();
    let garbage: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let relative = put(&root, &["坏文件.txt"], &garbage);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Failed);
    assert_eq!(code_of(&results[0]), "EXTRACTION_GARBLED");
    assert!(results[0].text.is_empty(), "失败时不能给半份正文");
}

#[test]
fn a_text_file_over_the_size_limit_is_rejected() {
    let (_tmp, root) = make_root();
    let oversized = vec![b'a'; 10 * 1024 * 1024 + 16];
    let relative = put(&root, &["巨大.txt"], &oversized);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(code_of(&results[0]), "EXTRACTION_TOO_LARGE");
}

#[test]
fn long_text_is_truncated_to_the_documented_limit_and_says_so() {
    // 规格 6.2：提取前 12,000 个 Unicode 字符。
    let (_tmp, root) = make_root();
    let long: String = "中".repeat(13_500);
    let relative = put(&root, &["很长.txt"], long.as_bytes());

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(extraction.status, ExtractionStatus::Ok);
    assert_eq!(extraction.text.chars().count(), 12_000);
    assert!(extraction.truncated, "截断必须如实标记");
}

#[test]
fn a_missing_file_is_reported_as_source_missing() {
    let (_tmp, root) = make_root();
    let missing = vec!["不存在.txt".to_owned()];

    let results = extract_batch(&root, &[request("f1", missing)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Failed);
    assert_eq!(code_of(&results[0]), "SOURCE_MISSING");
}

// ---------------------------------------------------------------------------
// 2. PDF
// ---------------------------------------------------------------------------

#[test]
fn a_text_pdf_yields_its_text() {
    let (_tmp, root) = make_root();
    let relative = put(&root, &["报告.pdf"], &text_pdf("Hello FilePilot"));

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(extraction.status, ExtractionStatus::Ok, "{:?}", extraction);
    assert!(
        extraction.text.contains("Hello FilePilot"),
        "{:?}",
        extraction.text
    );
    // 证据里要说清「读了几页」，用户才知道上限有没有生效。
    assert!(
        extraction
            .evidence
            .iter()
            .any(|item| item.locator.starts_with("pages:")),
        "{:#?}",
        extraction.evidence
    );
}

#[test]
fn an_empty_pdf_is_unsupported_not_failed() {
    // 「这份 PDF 没有正文」与「这份 PDF 读坏了」是两件事，
    // 用户要做的事也不同。规格把前者定义为 unsupported。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["空.pdf"], &empty_pdf());

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Unsupported);
    assert_eq!(code_of(&results[0]), "UNSUPPORTED_FORMAT");
}

#[test]
fn an_encrypted_pdf_never_reports_success() {
    // 本任务最重要的一条 PDF 断言：**绝不能**把一份密文当正文交出去。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["加密.pdf"], &encrypted_pdf());

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_ne!(
        extraction.status,
        ExtractionStatus::Ok,
        "加密文件不能报成提取成功：{:?}",
        extraction
    );
    assert!(extraction.text.is_empty());
    assert!(!code_of(extraction).is_empty(), "必须给出明确的错误码");
}

#[test]
fn a_corrupt_pdf_is_reported_as_corrupt() {
    let (_tmp, root) = make_root();
    let relative = put(&root, &["坏的.pdf"], &[0xFFu8; 2048]);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Failed);
    assert_eq!(code_of(&results[0]), "EXTRACTION_CORRUPT");
}

// ---------------------------------------------------------------------------
// 3. DOCX
// ---------------------------------------------------------------------------

#[test]
fn a_docx_yields_its_paragraphs() {
    let (_tmp, root) = make_root();
    let relative = put(&root, &["纪要.docx"], &docx_with(&docx_body(&["甲", "乙"])));

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(extraction.status, ExtractionStatus::Ok, "{:?}", extraction);
    assert_eq!(
        extraction.text.lines().collect::<Vec<_>>(),
        vec!["甲", "乙"],
        "段落边界必须变成换行，否则下游看到的是「甲乙」一个词"
    );
}

#[test]
fn a_corrupt_docx_is_reported_as_corrupt() {
    let (_tmp, root) = make_root();
    // 有一段像 ZIP 的开头，但整体不是合法压缩包。
    let mut bytes = b"PK\x03\x04".to_vec();
    bytes.extend_from_slice(&[0xAB; 1024]);
    let relative = put(&root, &["坏.docx"], &bytes);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Failed);
    assert_eq!(code_of(&results[0]), "EXTRACTION_CORRUPT");
}

#[test]
fn a_docx_zip_bomb_is_rejected_before_it_can_expand() {
    // 规格 6.2：压缩比 ≤100、解压累计 ≤50 MiB。
    // 关键是**在解压之前**就挡住——「读出来再看有多大」等于钱已经花掉了。
    let (_tmp, root) = make_root();
    let bomb = docx_bomb();
    assert!(
        bomb.len() < 64 * 1024,
        "样本本身必须很小，否则测的不是压缩比：{} 字节",
        bomb.len()
    );
    let relative = put(&root, &["炸弹.docx"], &bomb);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(
        code_of(&results[0]),
        "EXTRACTION_TOO_LARGE",
        "{:?}",
        results[0]
    );
    assert!(results[0].text.is_empty());
}

#[test]
fn a_docx_with_a_doctype_is_refused() {
    // 规格 6.2：禁止 XML 外部实体。
    let (_tmp, root) = make_root();
    let body = r#"<?xml version="1.0"?>
<!DOCTYPE foo [ <!ENTITY xxe SYSTEM "file:///C:/Windows/win.ini"> ]>
<w:document xmlns:w="x"><w:body><w:p><w:t>&xxe;</w:t></w:p></w:body></w:document>"#;
    let relative = put(&root, &["带实体.docx"], &docx_with(body));

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(code_of(&results[0]), "EXTRACTION_CORRUPT");
}

// ---------------------------------------------------------------------------
// 4. 其他格式与批次行为
// ---------------------------------------------------------------------------

#[test]
fn an_unsupported_extension_has_no_text_and_is_not_a_failure() {
    // 规格 6.2 最后一行：其他类型不提取内容，只用文件名/扩展名/时间。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["程序.exe"], b"MZ\x90\x00binary");

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Unsupported);
    assert_eq!(code_of(&results[0]), "UNSUPPORTED_FORMAT");
}

#[test]
fn one_bad_file_does_not_stop_the_batch() {
    // 规格 T10：「单文件失败不拖垮扫描会话」。
    let (_tmp, root) = make_root();
    let good_first = put(&root, &["一.txt"], "第一个".as_bytes());
    let broken = put(
        &root,
        &["二.txt"],
        &(0u8..=255).cycle().take(8192).collect::<Vec<u8>>(),
    );
    let good_third = put(&root, &["三.txt"], "第三个".as_bytes());

    let results = extract_batch(
        &root,
        &[
            request("f1", good_first),
            request("f2", broken),
            request("f3", good_third),
        ],
        &NoopExtractObserver,
    );

    assert_eq!(results.len(), 3, "每个请求都要有一份结果");
    assert_eq!(results[0].status, ExtractionStatus::Ok);
    assert_eq!(results[1].status, ExtractionStatus::Failed);
    assert_eq!(
        results[2].status,
        ExtractionStatus::Ok,
        "前面一个文件坏了，后面的必须照常处理"
    );
    // 结果与请求一一对应：贴回界面时不能张冠李戴。
    assert_eq!(results[0].file_id, "f1");
    assert_eq!(results[2].file_id, "f3");
}

#[test]
fn progress_is_reported_for_every_item_including_the_initial_zero() {
    let (_tmp, root) = make_root();
    let requests: Vec<ExtractRequest> = (0..3)
        .map(|index| {
            let name = format!("文件{index}.txt");
            request(&format!("f{index}"), put(&root, &[&name], b"content"))
        })
        .collect();

    let observer = Progress::new();
    extract_batch(&root, &requests, &observer);

    let seen = observer.observed();
    assert_eq!(seen.first().copied(), Some(0), "进度必须从 0 开始");
    assert_eq!(seen.last().copied(), Some(3), "最后一条必须是总数");
    assert_eq!(seen, vec![0, 1, 2, 3], "进度不能跳号：{seen:?}");
}

#[test]
fn extraction_binds_the_full_source_fingerprint() {
    // 规格 6.2：提取时绑定完整源指纹，生成计划时要能与新快照比对。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["带指纹.txt"], "内容".as_bytes());

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let fingerprint = &results[0].source_fingerprint;
    assert_eq!(fingerprint.volume_id, root.volume_id());
    assert!(!fingerprint.file_id.is_empty(), "必须有卷内文件身份");
    assert!(
        fingerprint
            .sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64),
        "必须有完整内容哈希：{:?}",
        fingerprint.sha256
    );
    assert!(!fingerprint.size.is_empty());
    assert!(!fingerprint.modified_ns.is_empty());
}

#[test]
fn a_cancelled_batch_returns_one_result_per_request() {
    // 取消之后也要给出一份结果：否则调用方拿到的数组与请求数对不上，
    // 「哪个文件没提取」就无从谈起。
    let (_tmp, root) = make_root();
    let requests: Vec<ExtractRequest> = (0..4)
        .map(|index| {
            let name = format!("取消{index}.txt");
            request(&format!("f{index}"), put(&root, &[&name], b"content"))
        })
        .collect();

    let observer = Progress::new();
    observer.stop_after(1);
    let results = extract_batch(&root, &requests, &observer);

    assert_eq!(results.len(), 4, "取消也要每个请求一份结果");
    assert_eq!(
        results[0].status,
        ExtractionStatus::Ok,
        "已经做的那一项要保住（取消不回头撤销已完成的工作）"
    );

    // 注意**不能**断言「除第一项外全是失败」：并发是 2，第二项在闸门关上之前
    // 就已经派发了，而已经派发的那一项会走完——这正是「停止派发下一项，
    // 当前项走完」的语义。可控的是「有没有如实标记未派发项」。
    let not_dispatched: Vec<&filepilot_lib::domain::types::Extraction> = results
        .iter()
        .filter(|item| {
            item.evidence
                .iter()
                .any(|evidence| evidence.excerpt.contains("已取消"))
        })
        .collect();
    assert!(
        !not_dispatched.is_empty(),
        "没派发的项必须如实标成没做，不能假装成功：{:#?}",
        results
            .iter()
            .map(|item| (&item.file_id, item.status, item.code.clone()))
            .collect::<Vec<_>>()
    );
    for item in not_dispatched {
        assert_eq!(item.status, ExtractionStatus::Failed);
        assert!(item.code.is_some(), "未提取也要给一个码，界面才能解释");
        assert!(item.text.is_empty(), "没提过就不能有正文");
    }
}

// ---------------------------------------------------------------------------
// 5. 工作进程的输入面（负向）
// ---------------------------------------------------------------------------

/// 找到 `extract_worker` 可执行文件。
fn worker_binary() -> std::path::PathBuf {
    let mut directory = std::env::current_exe().expect("取测试可执行文件路径");
    directory.pop(); // -> target/debug/deps
    directory.pop(); // -> target/debug
    let path = directory.join(format!("extract_worker{}", std::env::consts::EXE_SUFFIX));
    assert!(path.is_file(), "找不到工作进程：{}", path.display());
    path
}

#[test]
fn the_worker_rejects_a_path_argument_instead_of_ignoring_it() {
    // 规格 T10 的负向要求：「工作进程尝试访问未授权路径必须被限制」。
    //
    // 这条从**命令行契约**上验证：`--path` 是不认识的参数，必须整体拒绝。
    // 若它能被「忽略掉然后照常跑」，那「顺手支持一下 --path」就只差一次提交，
    // 而那一刻「工作进程拿不到路径」这条保证就没了。
    let output = std::process::Command::new(worker_binary())
        .args([
            "--version",
            "1",
            "--handle",
            "1",
            "--file-id",
            "f",
            "--format",
            "text",
            "--path",
            r"C:\Windows\System32\drivers\etc\hosts",
        ])
        .output()
        .expect("应能运行工作进程");

    assert!(
        !output.status.success(),
        "带路径的调用必须失败，而不是被忽略"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--path"),
        "错误里要点名是哪个参数不认识：{stderr}"
    );
}

#[test]
fn the_worker_reports_a_usage_error_for_a_missing_handle() {
    // 「参数没传对」与「解析结果不理想」用**不同退出码**分开：
    // 两者排查看的是完全不同的地方。
    //
    // T11 起 `--mode` 也成了必填（多了一个 ocrAvailability 模式），
    // 所以这里必须**显式给出模式**：否则先撞上的是「缺少 --mode」，
    // 这条用例就测不到「提取模式缺句柄」那一支了。缺 mode 的情况
    // 由下面那条单独覆盖。
    let output = std::process::Command::new(worker_binary())
        .args([
            "--version",
            "1",
            "--mode",
            "extract",
            "--file-id",
            "f",
            "--format",
            "text",
        ])
        .output()
        .expect("应能运行工作进程");

    assert_eq!(output.status.code(), Some(64), "用法错误应当用 64 退出");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--handle"),
        "错误里要点名是哪个参数没给：{stderr}"
    );
}

#[test]
fn the_worker_reports_a_usage_error_for_a_missing_mode() {
    // 与上一条配对：模式同样是必填，缺了也要点名，而不是含糊地报一句失败。
    let output = std::process::Command::new(worker_binary())
        .args(["--version", "1", "--file-id", "f", "--format", "text"])
        .output()
        .expect("应能运行工作进程");

    assert_eq!(output.status.code(), Some(64), "用法错误应当用 64 退出");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--mode"),
        "错误里要点名是哪个参数没给：{stderr}"
    );
}

#[test]
fn the_worker_refuses_a_mismatched_protocol_version() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["版本.txt"], b"hello");

    let output = std::process::Command::new(worker_binary())
        .args([
            "--version",
            "999",
            "--handle",
            "0",
            "--file-id",
            "f",
            "--format",
            "text",
        ])
        .output()
        .expect("应能运行工作进程");

    assert!(
        !output.status.success(),
        "版本不符必须拒绝，而不是按旧格式解释新字段：{}",
        path.display()
    );
}

#[test]
fn the_extractor_never_modifies_the_user_file() {
    // 规格 INV-01：扫描、解析、分析都不得改变用户文件。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["只读.txt"], "不要改我".as_bytes());
    let path = support::join_under(root.canonical(), &["只读.txt"]);
    let before = std::fs::read(&path).expect("读原文件");
    let modified_before = std::fs::metadata(&path)
        .expect("取属性")
        .modified()
        .expect("取修改时间");

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    assert_eq!(results[0].status, ExtractionStatus::Ok);
    assert_eq!(
        std::fs::read(&path).expect("再读一次"),
        before,
        "提取前后字节必须完全一致"
    );
    assert_eq!(
        std::fs::metadata(&path)
            .expect("再取属性")
            .modified()
            .expect("取修改时间"),
        modified_before,
        "提取不得改动修改时间"
    );
}

#[test]
fn the_whole_tree_is_unchanged_by_extraction() {
    // 比单个文件更强的一条：整棵树的形状与字节都不变。
    let (_tmp, root) = make_root();
    put(&root, &["a.txt"], b"AAA");
    put(&root, &["子目录", "b.md"], b"# BBB");
    put(&root, &["c.pdf"], &text_pdf("CCC"));
    let before: BTreeMap<String, Vec<u8>> = support::snapshot_tree(root.canonical())
        .into_iter()
        .collect();

    let requests = vec![
        request("f1", vec!["a.txt".to_owned()]),
        request("f2", vec!["子目录".to_owned(), "b.md".to_owned()]),
        request("f3", vec!["c.pdf".to_owned()]),
    ];
    extract_batch(&root, &requests, &NoopExtractObserver);

    let after: BTreeMap<String, Vec<u8>> = support::snapshot_tree(root.canonical())
        .into_iter()
        .collect();
    assert_eq!(before, after, "提取不得改动任何文件");
}

// ---------------------------------------------------------------------------
// 6. 图片 OCR（T11，规格 6.2 第四行）
// ---------------------------------------------------------------------------

/// 造一张**声明了大尺寸、但不含真实像素数据**的 PNG。
///
/// 尺寸就在 IHDR 里，而这条用例要验的正是「读完头就该拒绝，别去解码」，
/// 所以它只需要是一个**结构合法**的 PNG：签名 + IHDR + 空的 IDAT + IEND。
///
/// 实测过：只给 IHDR、不给 IDAT/IEND，连 PIL 都会报 `UnidentifiedImageError`
/// ——「图不完整」和「图很大」是两件事，那样就测不到像素上限这条分支了。
///
/// 另一层价值：它**没有真实像素数据**。所以一旦实现被改成
/// 「先解码再判像素数」，这条用例会立刻变红，而不是安静地继续通过。
fn png_declaring(width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    // IHDR 的数据固定 13 字节：宽、高、8 位色深、真彩色、
    // 以及压缩/过滤/隔行方式各一个字节。
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(b"IHDR");
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_chunk(&mut out, &ihdr);

    // 一个**空的** zlib 流：`78 9C` 是 zlib 头，`03 00` 是
    // 「最后一个固定 Huffman 空块」，末 4 字节是 adler32(空)。
    let mut idat = Vec::new();
    idat.extend_from_slice(b"IDAT");
    idat.extend_from_slice(&[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
    push_chunk(&mut out, &idat);

    push_chunk(&mut out, b"IEND");

    out
}

/// 追加一个 PNG chunk：长度 + （类型名 + 数据） + CRC。
///
/// `chunk` 的前 4 字节是类型名，长度字段写的是**不含类型名**的数据长度，
/// 而 CRC 覆盖的是**类型名加数据**的整段。
fn push_chunk(out: &mut Vec<u8>, chunk: &[u8]) {
    out.extend_from_slice(&(chunk.len() as u32 - 4).to_be_bytes());
    out.extend_from_slice(chunk);
    out.extend_from_slice(&crc32(chunk).to_be_bytes());
}

/// PNG 各 chunk 用的 CRC-32（反射多项式 `0xEDB88320`）。
///
/// 自己算是因为：不引入图片编码依赖，就不必让集成测试去编码真实像素。
/// 算错的后果是明确的——PNG 读不出来，断言会直接红，不会静默放过。
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[test]
fn a_chinese_image_comes_back_as_searchable_text() {
    // T11 验收原文：「OCR 有文字时返回可搜索片段」。
    //
    // 夹具是一张真渲染出来的中文图（见 `tmp/make_ocr_fixture.py`），
    // 不是画出来的方块——方块验不出 OCR 到底认不认字。
    //
    // ## 为什么要分两条路（2026-09-27 起）
    //
    // 「OCR 能不能读中文」取决于**这台机器装没装中文 OCR 语言包**，
    // 不取决于代码。开发机装了，GitHub 的 windows-latest 没装 ——
    // 于是这条用例此前在 CI 上必然红，而红的原因**不是代码回归**。
    //
    // 分两条路之后，两种机器上都验一件真事：
    //   * 有语言包 → 验「读得出可检索的中文」（这条用例原本的意图）；
    //   * 没有语言包 → 验「如实报告 OCR_UNAVAILABLE、且不编造正文」——
    //     这同样是规格要求（「语言包缺失明确提示」）。
    //
    // **代价要说清**：CI 上因此验不到真正的识别结果。这个覆盖缺口
    // 如实记在 `docs/POST_RELEASE_TODO.md` 的 CI-005，**不当作已覆盖**。
    //
    // ## ⚠️ 「本机能不能读中文」必须从**提取结果**里读，不能直调 `availability()`
    //
    // 这是一个已经踩过的坑（2026-09-27，CI-004）：这里原本写的是
    // `filepilot_lib::platform::ocr::availability().is_usable()` ——
    // 那是**进程内直调 WinRT**，而 WinRT 的公寓模型**绑定线程**：
    // libtest 每条用例跑在自己的线程上，第一个线程初始化公寓后退出，
    // 第二个线程再调 WinRT 就是 `STATUS_ACCESS_VIOLATION`（段错误）。
    // 完整套件跑起来必崩，而单跑那条用例却正常 —— 正是这个原因。
    //
    // 本项目的架构本来就是为避开这件事设计的：`extract_batch` **起工作进程**
    // （见 `extractors::service`），所以这个测试文件**原本零进程内 WinRT 调用**。
    // 需要「本机 OCR 状态」时，正确答案是从**已经拿到的提取结果**里读
    // （`OCR_UNAVAILABLE`），或者问工作进程（`service::ocr_availability()`）——
    // 前者零额外开销，用前者。
    //
    // 规矩写在 `src/platform/ocr.rs` 的模块测试注释里：真实 WinRT 调用
    // 只走工作进程，测试从外部验证。
    let (_tmp, root) = make_root();
    let relative = put(
        &root,
        &["扫描件.png"],
        include_bytes!("fixtures/ocr-chinese.png"),
    );

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);
    let extraction = &results[0];

    if code_of(extraction) == "OCR_UNAVAILABLE" {
        assert!(
            !ocr_required(),
            "本机声明保证有中文 OCR（FILEPILOT_REQUIRE_OCR=1），却报 OCR_UNAVAILABLE。\n\
             这说明 CI 上的语言包没装上或没生效 —— 覆盖缺口会**静默**回来。\n\
             不要把它当成「环境问题」绕过：要么修好语言包，要么明确承认覆盖不在。\n\
             见 docs/POST_RELEASE_TODO.md 的 CI-005。"
        );
        println!("[跳过真实识别] 本机 OCR 不可用：只验「如实报告缺失」，不验识别结果");
        assert!(
            extraction.text.is_empty(),
            "OCR 不可用时必须给空正文，不能编：{:?}",
            extraction.text
        );
        return;
    }

    assert_eq!(extraction.status, ExtractionStatus::Ok, "{:?}", extraction);
    assert!(
        extraction.text.contains("会议"),
        "识别结果要能直接检索：{:?}",
        extraction.text
    );
    assert!(
        extraction.text.contains("2026"),
        "图里的数字也要读出来：{:?}",
        extraction.text
    );
    // 证据里要说清「看的是哪张图」，用户才知道结论从何而来。
    assert!(
        extraction
            .evidence
            .iter()
            .any(|item| item.locator.starts_with("pixels:")),
        "{:#?}",
        extraction.evidence
    );
}

#[test]
fn the_spaces_ocr_inserts_between_chinese_characters_are_gone() {
    // 端到端地钉住那处后处理：系统 OCR 交回来的是 `会 议 纪 要`，
    // 直接存下来的话，用户搜「会议」一个字都搜不到。
    //
    // 断言落在「汉字之间没有空格」这个**结构**上，而不是落在
    // 「必须等于会议纪要」上：OCR 模型会随系统更新而微调用字，
    // 但「汉字之间不该有空格」这条不变。
    let (_tmp, root) = make_root();
    let relative = put(
        &root,
        &["扫描件.png"],
        include_bytes!("fixtures/ocr-chinese.png"),
    );

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);
    let outcome = &results[0];
    let text = &outcome.text;

    // 没有 OCR 时这条断言会**真空为真**（空文本里当然不含任何模式），
    // 所以先确认这轮真的读到了字，否则等于没验。见 POST_RELEASE_TODO 的 CI-005。
    //
    // 「本机 OCR 可不可用」从**提取结果**里读，不直调 `availability()` ——
    // 理由见 `a_chinese_image_comes_back_as_searchable_text` 上方那段
    // （进程内直调 WinRT 会因公寓绑定线程而 `STATUS_ACCESS_VIOLATION`，CI-004）。
    if code_of(outcome) == "OCR_UNAVAILABLE" {
        assert!(
            !ocr_required(),
            "本机声明保证有中文 OCR（FILEPILOT_REQUIRE_OCR=1），却报 OCR_UNAVAILABLE —— \
             语言包没装上或没生效，这条检查会静默变成真空为真。见 CI-005。"
        );
        println!("[跳过空格折叠检查] 本机 OCR 不可用，读不到文本");
        return;
    }
    assert!(
        !text.trim().is_empty(),
        "OCR 可用却读不出任何文本，这条检查会变成真空为真：{text:?}"
    );

    for adjacent in ["会 议", "议 纪", "纪 要"] {
        assert!(
            !text.contains(adjacent),
            "汉字之间不该留着 OCR 切出来的空格（{adjacent}）：{text:?}"
        );
    }
}

#[test]
fn an_image_without_text_is_unsupported_and_never_invents_content() {
    // 「图里没有文字」不是失败：它是一张照片、一张纯色图、一张只有图形的图。
    // 允许两个结论，不许的是第三个——返回一段编出来的正文。
    let (_tmp, root) = make_root();
    let relative = put(
        &root,
        &["空白.png"],
        include_bytes!("fixtures/ocr-blank.png"),
    );

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    // `OCR_UNAVAILABLE` 也会落到 unsupported（这台机器没装 OCR）。
    assert_eq!(
        extraction.status,
        ExtractionStatus::Unsupported,
        "{:?}",
        extraction
    );
    assert!(
        extraction.text.is_empty(),
        "没有文字就必须给空正文，不能编：{:?}",
        extraction.text
    );
    // **两个码都允许**，与上面那句注释保持一致。
    //
    // 这里此前只接受 `UNSUPPORTED_FORMAT`，与注释直接矛盾：装了 OCR 但图上没字
    // 才是 UNSUPPORTED_FORMAT，没装 OCR 则是 OCR_UNAVAILABLE。
    // 2026-09-27 在 CI 上暴露：runner 没有中文 OCR 语言包，
    // 这条断言让整个 `tests/extract.rs` 变红，而它想守的不变量其实是
    // 上面那一句 —— **绝不编造正文**。
    let code = code_of(extraction);
    assert!(
        code == "UNSUPPORTED_FORMAT" || code == "OCR_UNAVAILABLE",
        "空白图只能给这两个码之一（前者=有 OCR 但没字，后者=本机没 OCR），实得 {code}"
    );
}

#[test]
fn an_image_over_the_pixel_limit_is_rejected_as_a_failure() {
    // 规格 6.2：≤2000 万像素，超限拒绝。
    // 4500×4500 = 2025 万像素，刚好越过上限。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["巨图.png"], &png_declaring(4_500, 4_500));

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(
        code_of(extraction),
        "EXTRACTION_TOO_LARGE",
        "{:?}",
        extraction
    );
    // 「这张图太大了」是本该能读却没读成，不是「这类格式没有正文」——
    // 与损坏的 PDF / DOCX 归一类（`Failed`）。
    assert_eq!(
        extraction.status,
        ExtractionStatus::Failed,
        "{:?}",
        extraction
    );
    assert!(extraction.text.is_empty());
}

#[test]
fn an_image_over_the_engine_dimension_limit_reports_too_large_not_corrupt() {
    // 引擎还有一条**边长**上限（实测本机 10000），它与「总像素数」是两条
    // 不同的限制：这张 10001×1 的图只有一万像素，远低于 2000 万那一条，
    // 所以拒绝只可能来自边长检查。
    //
    // 端到端跑它的意义在于：**如果只靠引擎拒绝**，用户拿到的会是
    // `EXTRACTION_CORRUPT`——「文件损坏」。而文件没坏，只是太宽，
    // 他该做的动作（缩小图片）与「换个文件」完全不同。
    // 所以这里断言的是**用户看到的码是对的**，不只是「它被拒了」。
    let (_tmp, root) = make_root();
    let relative = put(&root, &["超宽.png"], &png_declaring(10_001, 1));

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(
        code_of(extraction),
        "EXTRACTION_TOO_LARGE",
        "超长边必须报「太大」，而不是「损坏」：{:?}",
        extraction
    );
    assert_eq!(
        extraction.status,
        ExtractionStatus::Failed,
        "{:?}",
        extraction
    );
}

#[test]
fn a_corrupt_image_is_failed_not_unsupported() {
    // 这条守着一处真实的不一致：图片分支最初把所有错误码都映射成
    // `Unsupported`，于是同一个 `EXTRACTION_CORRUPT` 在 DOCX 上是 `Failed`、
    // 在 PNG 上是 `Unsupported`。用户会看到「不支持此格式」，
    // 而那份文件本该能读、只是坏了。
    let (_tmp, root) = make_root();
    let garbage: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
    let relative = put(&root, &["坏图.png"], &garbage);

    let results = extract_batch(&root, &[request("f1", relative)], &NoopExtractObserver);

    let extraction = &results[0];
    assert_eq!(
        code_of(extraction),
        "EXTRACTION_CORRUPT",
        "{:?}",
        extraction
    );
    assert_eq!(
        extraction.status,
        ExtractionStatus::Failed,
        "损坏的文件在哪个格式上都该是 Failed：{:?}",
        extraction
    );
}

#[test]
fn reading_an_image_leaves_the_file_and_the_tree_untouched() {
    // T11 验收原文：「文件内容和用户目录均未被修改」。
    let (_tmp, root) = make_root();
    put(
        &root,
        &["扫描件.png"],
        include_bytes!("fixtures/ocr-chinese.png"),
    );
    put(
        &root,
        &["空白.png"],
        include_bytes!("fixtures/ocr-blank.png"),
    );
    let before: BTreeMap<String, Vec<u8>> = support::snapshot_tree(root.canonical())
        .into_iter()
        .collect();

    let requests = vec![
        request("f1", vec!["扫描件.png".to_owned()]),
        request("f2", vec!["空白.png".to_owned()]),
    ];
    extract_batch(&root, &requests, &NoopExtractObserver);

    let after: BTreeMap<String, Vec<u8>> = support::snapshot_tree(root.canonical())
        .into_iter()
        .collect();
    assert_eq!(before, after, "OCR 不得改动任何文件");
}

#[test]
fn the_ocr_status_can_be_asked_for_from_the_settings_page() {
    // 设置页要显示 «OCR 可用状态和原因»，所以这个查询必须能从外部真的调到，
    // 而不是只有单测里能调到。
    //
    // 这里**不预设本机的结果**：装了中文语言包就是 available，
    // 没装就是 noChineseLanguage——两种都是正确答案。要验的是
    // 「每一种状态都带一句包含下一步动作的原因」，而不是「本机一定可用」。
    let report =
        filepilot_lib::extractors::service::ocr_availability().expect("查询 OCR 状态不该失败");

    assert!(
        matches!(
            report.status.as_str(),
            "available" | "noRecognizerLanguage" | "noChineseLanguage" | "unsupported"
        ),
        "未知状态会让界面没法解释：{report:?}"
    );
    assert!(
        !report.message.trim().is_empty(),
        "每种状态都要给用户一句原因：{report:?}"
    );
    if report.status == "available" {
        // 可用时必须说清能认哪些语言，否则用户无从判断中文行不行。
        assert!(!report.languages.is_empty(), "{report:?}");
    } else {
        // 不可用时的原因必须包含下一步动作，而不是一句「不可用」。
        assert!(
            report.message.contains("设置")
                || report.message.contains("语言包")
                || report.message.contains("中文"),
            "不可用的原因要给下一步动作：{report:?}"
        );
    }
}
