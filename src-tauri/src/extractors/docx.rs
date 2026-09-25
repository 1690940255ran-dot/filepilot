//! DOCX 文本提取（规格 6.2 第三行）。
//!
//! | 项目 | 规格要求 |
//! |---|---|
//! | 方法 | **仅读取文档正文 XML** |
//! | 大小 | 压缩文件 ≤ 20 MiB、解压条目累计 ≤ 50 MiB、压缩比 ≤ 100 |
//! | 安全 | 禁止 XML 外部实体和宏 |
//!
//! ## 只读 `word/document.xml`
//!
//! 一份 DOCX 里还有样式、主题、页眉页脚、图片、嵌入对象……规格说的是
//! 「仅读取文档正文 XML」，因此这里只认 `word/document.xml` 一个条目。
//! 这既是一条功能边界，也是一条**攻击面**边界：不去解压的条目，
//! 就没有解析器会去碰它。
//!
//! 宏在 DOCX 里是 `word/vbaProject.bin`（那是 `.docm` 的形状）。
//! 我们不读它——不是因为「读了会执行」，而是因为**读取本身就是它存在的意义**。
//!
//! ## 三条压缩包限制各防什么
//!
//! * **压缩文件 ≤ 20 MiB**：把一份 2 GiB 的文件塞进内存之前先挡住。
//! * **解压条目累计 ≤ 50 MiB**：这是防炸弹的主闸。压缩包只有 20 MiB，
//!   解压出来可能是几十 GiB——限制输入大小管不到解压之后。
//! * **单个条目压缩比 ≤ 100**：累计上限是「总量」的闸，这一条是「局部」的闸。
//!   一个 1 KiB 的条目解出 100 MiB，总量上看不出来，但它是同一个手法。
//!
//! 三条都要在**读之前**用条目元数据判断，而不是「读出来看看有多大」——
//! 后者等于已经把钱花掉了再决定要不要省。

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use super::limits::{MAX_DOCX_BYTES, MAX_DOCX_COMPRESSION_RATIO, MAX_DOCX_UNCOMPRESSED_BYTES};
use crate::domain::errors::codes;

/// 正文所在的条目。规格：仅读取文档正文 XML。
const BODY_ENTRY: &str = "word/document.xml";

/// 提取成功的结果。
///
/// 派生 `PartialEq` 是为了让测试能直接比较整个结果（`assert_eq!(extract(..), Err(..))`）。
/// 没有它，测试就只能拆开逐个字段断言，失败信息里也看不到完整形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxText {
    pub text: String,
    pub truncated: bool,
}

pub type DocxResult = Result<DocxText, &'static str>;

/// 从字节里提取正文。
pub fn extract(bytes: &[u8]) -> DocxResult {
    if bytes.len() as u64 > MAX_DOCX_BYTES {
        return Err(codes::EXTRACTION_TOO_LARGE);
    }

    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| codes::EXTRACTION_CORRUPT)?;

    // 先审一遍整包的账，再决定读不读。
    audit_entries(&mut archive)?;

    let mut body = String::new();
    {
        let mut entry = archive
            .by_name(BODY_ENTRY)
            .map_err(|_| codes::EXTRACTION_CORRUPT)?;
        // 上限已经审过，这里再给一个「读不出来就是损坏」的出口：
        // 审计用的是条目头里声明的长度，而声明与实际不符本身就是一种损坏。
        entry
            .read_to_string(&mut body)
            .map_err(|_| codes::EXTRACTION_CORRUPT)?;
    }

    let text = collect_text(&body)?;
    if text.trim().is_empty() {
        // 正文 XML 在，但里面一个字都没有：那是一份空文档。
        // 与「没有正文条目」分开——前者是正常的空文档，后者不是一份 Word 文档。
        return Ok(DocxText {
            text: String::new(),
            truncated: false,
        });
    }

    let (text, truncated) = super::text::truncate(&text);
    Ok(DocxText { text, truncated })
}

/// 检查整个压缩包的账：条目累计大小与单个条目的压缩比。
///
/// **在解压任何条目之前**跑完。顺序很重要：先解压再检查，等于
/// 「先把 50 GiB 读进内存，然后发现它太大了」。
fn audit_entries<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<(), &'static str> {
    let mut total_uncompressed: u64 = 0;

    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|_| codes::EXTRACTION_CORRUPT)?;

        let uncompressed = entry.size();
        let compressed = entry.compressed_size();

        // 压缩比为 0（`compressed == 0` 而 `uncompressed > 0`）时比值是无穷，
        // 直接按超限处理。这类条目在正常 DOCX 里不该出现——
        // 唯一能造出它的方式就是刻意构造。
        if compressed == 0 {
            if uncompressed > 0 {
                return Err(codes::EXTRACTION_TOO_LARGE);
            }
            continue;
        }
        if uncompressed / compressed > MAX_DOCX_COMPRESSION_RATIO {
            return Err(codes::EXTRACTION_TOO_LARGE);
        }

        total_uncompressed = total_uncompressed.saturating_add(uncompressed);
        if total_uncompressed > MAX_DOCX_UNCOMPRESSED_BYTES {
            return Err(codes::EXTRACTION_TOO_LARGE);
        }
    }

    Ok(())
}

/// 从 `word/document.xml` 里取出正文。
///
/// 只关心两类元素：
/// * `t`（`w:t`）—— 一段文字；
/// * `p`（`w:p`）—— 一个段落，转成一个换行。
///
/// 用**本地名**而不是「去掉前缀的字符串」：命名空间的前缀在 XML 里
/// 是可以任意取的，只有 URI 才是身份。写死 `w:` 会在换一个生成器时
/// 静默地什么都提不到——而「静默地什么都提不到」正是最难查的一类问题。
fn collect_text(xml: &str) -> Result<String, &'static str> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut out = String::new();
    let mut inside_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if start.local_name().as_ref() == "t" {
                    inside_text = true;
                }
            }
            Ok(Event::End(end)) => {
                match end.local_name().as_ref() {
                    "t" => inside_text = false,
                    // 换行留在段落**结束**时给：这样「空段落」与
                    // 「有内容的段落」走的是同一条路径，不会出现
                    // 「<w:p/> 不算一段」这种解释不一致。
                    "p" => out.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::Empty(empty)) => {
                if empty.local_name().as_ref() == "p" {
                    out.push('\n');
                }
            }
            Ok(Event::Text(text)) => {
                if inside_text {
                    // quick-xml 的 `xml10_content` 已完成实体解码和 XML 1.0
                    // 换行规范化；再次 unescape 会把正文中的 `&` 当成新实体起点。
                    out.push_str(&text.xml10_content());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if inside_text {
                    if let Some(character) = reference
                        .resolve_char_ref()
                        .map_err(|_| codes::EXTRACTION_CORRUPT)?
                    {
                        out.push(character);
                    } else {
                        let name = reference.into_inner();
                        out.push(match name.as_ref() {
                            "amp" => '&',
                            "lt" => '<',
                            "gt" => '>',
                            "apos" => '\'',
                            "quot" => '"',
                            // 不解析 DTD，因此任何自定义实体都不能有来源；
                            // 把它当正文会静默改变文档含义。
                            _ => return Err(codes::EXTRACTION_CORRUPT),
                        });
                    }
                }
            }
            // 规格 6.2：「禁止 XML 外部实体」。
            //
            // quick-xml **默认就不解析** DTD 与外部实体，所以这条在这里
            // 是「不允许它出现在我们的文档里」而不是「拒绝解析它」。
            // 差别在于：如果哪天有人给 reader 开了实体解析，
            // 这一条会立刻把带 DTD 的文件挡下来，而不是安静地放过去。
            Ok(Event::DocType(_)) => return Err(codes::EXTRACTION_CORRUPT),
            // CData 只是另一种写法，内容仍然是正文。
            Ok(Event::CData(data)) => {
                if inside_text {
                    out.push_str(&data.xml10_content());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(codes::EXTRACTION_CORRUPT),
        }
    }

    Ok(normalise(&out))
}

/// 把空白整理成人类可读的形状。
///
/// 段内连续空白（Word 用多个空格做对齐）压成一个空格，
/// 连续空行压成一个空行。不这样做的话，正文会是一堆看不出结构的空白，
/// 而下游要拿它去分类——噪声会直接变成错误的建议。
fn normalise(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut pending_space = false;
    let mut consecutive_newlines = 0usize;

    for c in raw.chars() {
        if c == '\n' {
            // 行首的空白不要
            while out.ends_with(' ') {
                out.pop();
            }
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                out.push('\n');
            }
            pending_space = false;
            continue;
        }
        consecutive_newlines = 0;
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }

    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// 造一份 DOCX。`body` 是 `word/document.xml` 的内容。
    fn docx_with(body: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file(BODY_ENTRY, options).expect("建条目");
            writer.write_all(body.as_bytes()).expect("写正文");
            writer
                .start_file("[Content_Types].xml", options)
                .expect("建条目");
            writer.write_all(b"<Types/>").expect("写内容类型");
            writer.finish().expect("收尾");
        }
        buffer.into_inner()
    }

    /// 一份最小的合法正文 XML。
    fn body_xml(paragraphs: &[&str]) -> String {
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

    #[test]
    fn a_simple_document_yields_its_paragraphs() {
        let bytes = docx_with(&body_xml(&["第一段", "第二段"]));
        let result = extract(&bytes).expect("应当提取成功");

        assert!(result.text.contains("第一段"), "{:?}", result.text);
        assert!(result.text.contains("第二段"), "{:?}", result.text);
        assert!(!result.truncated);
    }

    #[test]
    fn paragraph_boundaries_become_line_breaks() {
        let bytes = docx_with(&body_xml(&["甲", "乙"]));
        let text = extract(&bytes).expect("应当成功").text;

        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            vec!["甲", "乙"],
            "两个段落应当分成两行，否则下游看到的是「甲乙」一个词"
        );
    }

    #[test]
    fn an_empty_document_is_empty_not_unsupported() {
        // 「有正文条目但没写字」是正常的空文档，与「没有正文条目」不同。
        let bytes = docx_with(&body_xml(&[]));
        let result = extract(&bytes).expect("空文档应当成功");
        assert!(result.text.is_empty());
    }

    #[test]
    fn a_zip_without_a_body_entry_is_corrupt() {
        // 有 zip 结构但不是 Word 文档（或者正文被删掉了）。
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            writer
                .start_file("random.txt", SimpleFileOptions::default())
                .expect("建条目");
            writer.write_all(b"not a document").expect("写");
            writer.finish().expect("收尾");
        }
        assert_eq!(
            extract(&buffer.into_inner()),
            Err(codes::EXTRACTION_CORRUPT)
        );
    }

    #[test]
    fn a_non_zip_file_is_corrupt() {
        let garbage: Vec<u8> = (0u8..=255).cycle().take(2048).collect();
        assert_eq!(extract(&garbage), Err(codes::EXTRACTION_CORRUPT));
    }

    #[test]
    fn a_document_type_declaration_is_refused() {
        // 规格 6.2：禁止 XML 外部实体。
        let body = r#"<?xml version="1.0"?>
<!DOCTYPE foo [ <!ENTITY xxe SYSTEM "file:///C:/Windows/win.ini"> ]>
<w:document xmlns:w="x"><w:body><w:p><w:t>&xxe;</w:t></w:p></w:body></w:document>"#;
        let bytes = docx_with(body);

        assert_eq!(
            extract(&bytes),
            Err(codes::EXTRACTION_CORRUPT),
            "带 DOCTYPE 的正文必须被拒绝，而不是把实体当普通文本读过去"
        );
    }

    #[test]
    fn a_compression_bomb_is_rejected_by_the_ratio_check() {
        // 一个极小的条目解出极大的内容：压缩比远高于 100。
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file(BODY_ENTRY, options).expect("建条目");
            // 1 MiB 的同一个字节，deflate 之后几十字节，压缩比远超 100。
            writer.write_all(&vec![b'a'; 1024 * 1024]).expect("写炸弹");
            writer.finish().expect("收尾");
        }

        assert_eq!(
            extract(&buffer.into_inner()),
            Err(codes::EXTRACTION_TOO_LARGE),
            "压缩比超限必须在**读之前**被拦住"
        );
    }

    #[test]
    fn the_audit_runs_before_any_entry_is_read() {
        // 这条断言的是**顺序**：炸弹文件里，正文条目本身也是炸弹。
        // 如果先读后查，测试会先耗尽内存或者至少慢得明显；
        // 先查后读则立刻返回 TOO_LARGE。
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file(BODY_ENTRY, options).expect("建条目");
            writer.write_all(&vec![b'x'; 8 * 1024 * 1024]).expect("写");
            writer.finish().expect("收尾");
        }
        let bytes = buffer.into_inner();
        assert!(
            bytes.len() < 64 * 1024,
            "样本本身必须很小，否则测的就不是压缩比了：{} 字节",
            bytes.len()
        );

        assert_eq!(extract(&bytes), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn a_file_over_the_container_limit_is_rejected_before_parsing() {
        let huge = vec![b'z'; (MAX_DOCX_BYTES + 1) as usize];
        assert_eq!(extract(&huge), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn xml_entities_in_the_body_are_decoded() {
        // `&amp;` 这类转义是正文的一部分，不能原样留在文本里。
        let bytes = docx_with(&body_xml(&["A &amp; B"]));
        let text = extract(&bytes).expect("应当成功").text;
        assert_eq!(text, "A & B");
    }

    #[test]
    fn a_prefix_other_than_w_is_still_understood() {
        // 命名空间的前缀可以任意取，身份由 URI 决定。
        // 写死 `w:` 会在换一个生成器时静默地什么都提不到。
        let body = r#"<d:document xmlns:d="http://example.com"><d:body><d:p><d:t>前缀不同</d:t></d:p></d:body></d:document>"#;
        let bytes = docx_with(body);
        let text = extract(&bytes).expect("应当成功").text;
        assert_eq!(text, "前缀不同");
    }

    #[test]
    fn whitespace_is_normalised_for_readability() {
        let bytes = docx_with(&body_xml(&["  前后有空格  ", "对齐用的      多个空格"]));
        let text = extract(&bytes).expect("应当成功").text;

        assert_eq!(text, "前后有空格\n对齐用的 多个空格");
    }
}
