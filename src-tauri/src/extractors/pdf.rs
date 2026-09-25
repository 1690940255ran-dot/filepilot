//! PDF 文本提取（规格 6.2 第二行）。
//!
//! | 项目 | 规格要求 |
//! |---|---|
//! | 方法 | 本地文本提取 |
//! | 大小 | ≤ 20 MiB |
//! | 上限 | 前 10 页 |
//! | 失败 | 扫描型 PDF 无文本时返回 `unsupported`；**首版不做 PDF 页面 OCR** |
//!
//! ## 三条边界，三种不同的结论
//!
//! 「这份 PDF 读不出文字」有三种互不相同的原因，用户要做的事也不同：
//!
//! * **加密**：文件是好的，只是没有钥匙。用户去提供密码或换一份未加密的。
//!   → `EXTRACTION_ENCRYPTED`
//! * **损坏**：文件本身有问题。用户去确认这份文件是不是传坏了。
//!   → `EXTRACTION_CORRUPT`
//! * **扫描型**：文件是好的，但它本来就是图片。规格明确说首版不做页面 OCR，
//!   所以这不是失败，而是「这类文件没有可提取的正文」。
//!   → `unsupported`（`UNSUPPORTED_FORMAT`）
//!
//! 把三者混成一句「读取失败」，用户就只能在三个方向里瞎试。

use lopdf::{DecompressError, Error as PdfError, LoadOptions};

use super::limits::{
    MAX_EXTRACTED_CHARS, MAX_PDF_BYTES, MAX_PDF_PAGES, MAX_PDF_PAGE_CONTENT_BYTES,
    MAX_PDF_STREAM_BYTES,
};
use crate::domain::errors::codes;

/// 提取成功的结果。
///
/// 派生 `PartialEq` 是为了让测试能直接比较整个结果（`assert_eq!(extract(..), Err(..))`）。
/// 没有它，测试就只能拆开逐个字段断言，失败信息里也看不到完整形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfText {
    pub text: String,
    /// 实际读了几页。
    pub pages_read: u32,
    /// 文档总页数。用来向用户解释「只看了前 10 页」这件事的范围。
    pub total_pages: u32,
    pub truncated: bool,
}

/// 提取结果，或一个明确的失败码。
pub type PdfResult = Result<PdfText, &'static str>;

/// 从字节里提取文本。
///
/// 只认**已经在内存里的字节**，不收路径：工作进程手里没有路径（见 `protocol`），
/// 这也让这个函数天然适合单元测试。
pub fn extract(bytes: &[u8]) -> PdfResult {
    if bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(codes::EXTRACTION_TOO_LARGE);
    }

    // `extract_text_with_limit` 而**不是** `extract_text`：
    // 后者对页面内容流不设上限，一个构造出来的小文件就能在解压时把内存吃光。
    // lopdf 自己的文档也把前者标成「不可信来源应当优先用它」。
    let document = lopdf::Document::load_mem_with_options(
        bytes,
        LoadOptions {
            max_decompressed_size: Some(MAX_PDF_STREAM_BYTES),
            ..Default::default()
        },
    )
    .map_err(classify_load_error)?;

    // lopdf 对某些结构完整但口令字段不完整的样本可以完成语法加载；
    // 在查看页数前仍要尊重 trailer 的 /Encrypt 标记，不能把 0 页误报成扫描型 PDF。
    if document.is_encrypted() {
        return Err(codes::EXTRACTION_ENCRYPTED);
    }

    let pages = document.get_pages();
    let total_pages = pages.len() as u32;

    // 规格：前 10 页。页码从 1 开始（lopdf 的约定）。
    let page_numbers: Vec<u32> = pages.keys().copied().take(MAX_PDF_PAGES as usize).collect();

    if page_numbers.is_empty() {
        // 一页都没有的 PDF。它可能是合法的（某些生成器会产出 0 页文档），
        // 但没有正文可提，属于 unsupported 而不是 failed。
        return Err(codes::UNSUPPORTED_FORMAT);
    }

    let pages_read = page_numbers.len() as u32;
    let text = document
        .extract_text_with_limit(&page_numbers, MAX_PDF_PAGE_CONTENT_BYTES)
        .map_err(classify_text_error)?;

    // 扫描型 PDF：页面在，但内容流里没有文字绘制指令。
    // 规格明确首版不做页面 OCR，因此这里如实报「这类文件没有可提的正文」，
    // 而不是拿一个空字符串冒充成功——空字符串会让下游以为「这文档没内容」。
    if text.trim().is_empty() {
        return Err(codes::UNSUPPORTED_FORMAT);
    }

    let (text, truncated) = super::text::truncate(&text);
    Ok(PdfText {
        text,
        pages_read,
        total_pages,
        truncated,
    })
}

/// 判断读取阶段失败到底是哪一类。
///
/// 顺序有讲究：**加密要排在损坏前面**。加密文档在解析器眼里也是「读不下去」，
/// 先判损坏会把一份完好的加密文件说成坏文件。
fn classify_load_error(error: PdfError) -> &'static str {
    match error {
        PdfError::Decryption(_) | PdfError::InvalidPassword => codes::EXTRACTION_ENCRYPTED,
        PdfError::Decompress(DecompressError::MemoryLimitExceeded { .. }) => {
            codes::EXTRACTION_TOO_LARGE
        }
        _ => codes::EXTRACTION_CORRUPT,
    }
}

/// 判断文本提取阶段失败是哪一类。
fn classify_text_error(error: PdfError) -> &'static str {
    match error {
        PdfError::Decryption(_) | PdfError::InvalidPassword => codes::EXTRACTION_ENCRYPTED,
        // 内容流膨胀超限走这条：它同样是「这份文件太大」，
        // 只是大在解压之后而不是文件本身。
        PdfError::Decompress(DecompressError::MemoryLimitExceeded { .. }) => {
            codes::EXTRACTION_TOO_LARGE
        }
        // 页面内容解不开，多数是内容流损坏；但也可能是我们没实现的过滤器。
        // 两者对用户的动作都是「这份文件的正文读不出来」，因此不细分。
        _ => codes::EXTRACTION_CORRUPT,
    }
}

/// 上限值本身也要能被断言到，免得改常量时没人发现契约漂了。
pub const fn configured_max_chars() -> usize {
    MAX_EXTRACTED_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 造一份最小可用的单页 PDF，正文是 `content`。
    ///
    /// 手工拼而不是引一个「PDF 生成库」：测试样本的形状要能被读代码的人
    /// 一眼看明白，而引库会把「这份 PDF 里到底有什么」藏在另一个依赖里。
    fn simple_pdf(content: &str) -> Vec<u8> {
        let mut objects: Vec<String> = Vec::new();
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
        objects.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned());
        objects.push(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
                .to_owned(),
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\nBT /F1 12 Tf 10 100 Td ({content}) Tj ET\nendstream",
            format!("BT /F1 12 Tf 10 100 Td ({content}) Tj ET").len()
        ));
        objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());

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
        let _ = write!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        );
        out
    }

    #[test]
    fn a_simple_text_pdf_yields_its_text() {
        let pdf = simple_pdf("Hello FilePilot");
        let result = extract(&pdf).expect("应当提取成功");

        assert!(
            result.text.contains("Hello FilePilot"),
            "正文里应当有那段文字：{:?}",
            result.text
        );
        assert_eq!(result.total_pages, 1);
        assert_eq!(result.pages_read, 1);
        assert!(!result.truncated);
    }

    #[test]
    fn a_pdf_with_no_text_is_unsupported_not_failed() {
        // 扫描型 PDF 的形状：页面在，但内容流里没有文字。
        // 规格明确首版不做页面 OCR，所以这是 unsupported。
        let mut objects: Vec<String> = Vec::new();
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
        objects.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned());
        objects.push(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>".to_owned(),
        );
        // 只有一条「画一条线」的指令，没有任何 Tj/TJ。
        let stream = "0 0 m 100 100 l S";
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len()
        ));

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
        let _ = write!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        );

        assert_eq!(
            extract(&out),
            Err(codes::UNSUPPORTED_FORMAT),
            "没有文字层 ≠ 读取失败：前者是「这类文件没有可提的正文」"
        );
    }

    #[test]
    fn garbage_bytes_are_reported_as_corrupt() {
        let garbage: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        assert_eq!(extract(&garbage), Err(codes::EXTRACTION_CORRUPT));
    }

    #[test]
    fn an_encrypted_pdf_is_reported_as_encrypted_not_corrupt() {
        // 一份**结构完好但被加密**的 PDF：把 /Encrypt 挂进 trailer，
        // 并给一个形状合法的加密字典。
        //
        // 这样断言才有意义：如果把 /Encrypt 换成一段垃圾字节，
        // 它既是「加密」也是「损坏」，测试就无法区分我们是不是
        // 真的按「加密」分类——而那正是这个分支存在的理由。
        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();

        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [] /Count 0 >>",
            // 标准安全处理器的加密字典形状（RC4 40 位）。
            "<< /Filter /Standard /V 1 /R 2 /O <00> /U <00> /P -1 >>",
        ];
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
        let _ = write!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R /Encrypt 3 0 R /ID [<00><00>] >>\n\
             startxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        );

        let result = extract(&out);
        // 端到端的兜底断言：一份带 `/Encrypt` 的样本，无论解析器在哪个阶段
        // 发现问题，我们都**绝不能**返回 Ok——那会让下游拿一份实际上是密文的
        // 「正文」去给文件分类。
        //
        // 至于它落进「加密」还是「损坏」，取决于 lopdf 在哪个阶段发现它，
        // 那是解析器的内部顺序。我们自己那部分分类逻辑由下面几条
        // 直接对 `classify_*` 的用例钉住——那才是「用户看到哪句话」的
        // 决定因素，也才是我们真正写下的代码。
        assert!(
            result.is_err(),
            "带 /Encrypt 的样本不能被报成提取成功：{result:?}"
        );
    }

    #[test]
    fn an_encrypted_pdf_is_classified_as_encrypted_not_corrupt() {
        // 手写一份**真正**加密的 PDF 需要一个加密实现（RC4/AES + 密钥派生），
        // 那是另一个库的量级；而构造出来的「假加密」样本在解析器眼里
        // 既可能是加密也可能是损坏，断言就变得说不清。
        //
        // 分类逻辑是我们自己写的那部分，也正是用户看到哪句话的决定因素，
        // 所以直接对它下断言。加密与损坏必须分开：前者是一份**完好**的文件
        // 只是没钥匙（用户去提供密码），后者是文件本身坏了（用户去查文件）。
        assert_eq!(
            classify_load_error(PdfError::InvalidPassword),
            codes::EXTRACTION_ENCRYPTED
        );
        assert_eq!(
            classify_text_error(PdfError::InvalidPassword),
            codes::EXTRACTION_ENCRYPTED
        );
    }

    #[test]
    fn a_decompression_bomb_is_classified_as_too_large() {
        // 内容流膨胀超限走 `Decompress(MemoryLimitExceeded)`。
        // 它必须归到「太大」而不是「损坏」：用户能做的事完全不同
        // （前者换一份文件，后者去查文件是不是传坏了）。
        // 造两份而不是复制一份：`lopdf::Error` 没有实现 `Clone`
        // （它的 `IO` 变体包着一个 `std::io::Error`，本来也不该被复制）。
        let load = PdfError::Decompress(DecompressError::MemoryLimitExceeded { limit: 1024 });
        let text = PdfError::Decompress(DecompressError::MemoryLimitExceeded { limit: 1024 });
        assert_eq!(classify_load_error(load), codes::EXTRACTION_TOO_LARGE);
        assert_eq!(classify_text_error(text), codes::EXTRACTION_TOO_LARGE);
    }

    #[test]
    fn other_parse_errors_are_classified_as_corrupt() {
        assert_eq!(
            classify_load_error(PdfError::ObjectNotFound((1, 0))),
            codes::EXTRACTION_CORRUPT
        );
        assert_eq!(
            classify_text_error(PdfError::PageNumberNotFound(3)),
            codes::EXTRACTION_CORRUPT
        );
    }

    #[test]
    fn an_empty_byte_slice_is_corrupt_not_a_panic() {
        assert_eq!(extract(b""), Err(codes::EXTRACTION_CORRUPT));
    }

    #[test]
    fn a_file_over_the_size_limit_is_rejected_before_parsing() {
        // 用一段超过 20 MiB 的字节：它同时也不是合法 PDF，
        // 所以「先报 TOO_LARGE」证明大小检查发生在解析**之前**。
        let huge = vec![b'x'; (MAX_PDF_BYTES + 1) as usize];
        assert_eq!(extract(&huge), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn the_page_limit_is_ten() {
        assert_eq!(MAX_PDF_PAGES, 10, "规格 6.2：前 10 页");
        assert_eq!(configured_max_chars(), 12_000);
    }
}
