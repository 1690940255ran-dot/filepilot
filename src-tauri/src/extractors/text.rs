//! TXT / MD 等纯文本的解析（规格 6.2 第一行）。
//!
//! | 项目 | 规格要求 |
//! |---|---|
//! | 编码 | 检测 BOM，UTF-8 / UTF-16，**有限**支持 GB18030 |
//! | 大小 | ≤ 10 MiB |
//! | 上限 | 提取前 12,000 个 Unicode 字符 |
//! | 失败 | 乱码**明确失败** |
//!
//! ## 「乱码明确失败」到底在防什么
//!
//! 最省事的写法是 `String::from_utf8_lossy`：它永远成功，坏字节变成 U+FFFD。
//! 那样一段 GB18030 中文会变成一串问号菱形，而下游会把这段「正文」
//! 送去给模型分类——用户得到的是一份看起来正常、内容完全错误的建议。
//!
//! 所以这里按 **BOM → UTF-8 严格 → UTF-16（按 NUL 分布判定）→ GB18030**
//! 的顺序逐个尝试，每一步都如实记录「用了哪个编码、有多少位置说不出来」，
//! 可疑占比超过阈值就报 `EXTRACTION_GARBLED` 而不是硬给一段正文。

use encoding_rs::GB18030;

use super::limits::{GARBLED_MARK_PERMILLE, MAX_EXTRACTED_CHARS};
use crate::domain::errors::codes;

/// 正文里用来表示「这个字节解不出字符」的替代字符。
const REPLACEMENT: char = '\u{FFFD}';

/// 私用区的上下界（BMP 内的部分）。
///
/// GB18030 对未定义的双字节序列会**兜底映射到私用区**，而不是产出替换字符。
/// 只看 U+FFFD 会把这些整段漏过去——它们看起来是「字符」，
/// 但没有任何一份真实文档会由成片的私用区构成。
const PRIVATE_USE_START: char = '\u{E000}';
const PRIVATE_USE_END: char = '\u{F8FF}';

/// 这次解码实际用了哪个编码。
///
/// 单独返回而不是只给字符串：`evidence` 里要写明依据（规格 6.4 要求
/// 证据必须来自**真实提取结果**），而「用了什么编码」正是文本类文件
/// 最有价值的一条依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// 带 BOM 的 UTF-8。
    Utf8Bom,
    /// 无 BOM 但整段都是合法 UTF-8。
    Utf8,
    /// 带 BOM 或缺 BOM 但按 NUL 分布判定为 UTF-16。
    Utf16Le,
    Utf16Be,
    /// 前几种都不成立时，按 GB18030 解（规格：有限支持）。
    Gb18030,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8Bom => "utf-8-bom",
            Encoding::Utf8 => "utf-8",
            Encoding::Utf16Le => "utf-16le",
            Encoding::Utf16Be => "utf-16be",
            Encoding::Gb18030 => "gb18030",
        }
    }
}

/// 解码成功的结果。
#[derive(Debug, Clone)]
pub struct DecodedText {
    pub text: String,
    pub encoding: Encoding,
    /// 「说不出来」的位置数量：替换字符 + 控制字符 + 私用区。
    ///
    /// 不是「有几个字符不认识」这么窄——见 [`is_suspicious`]。
    /// 阈值判断用它，界面上也用它解释「为什么这份文件被判成乱码」。
    pub suspicious: usize,
}

/// 解码后的结果或一个明确的失败码。
pub type DecodeResult = Result<DecodedText, &'static str>;

/// 把一段字节解成正文。
///
/// 返回的 `Err` 是错误码，不是错误消息：调用方（worker）要把码原样带回
/// 界面，而消息由界面侧按用户语言拼。
pub fn decode(bytes: &[u8]) -> DecodeResult {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return finish(rest, Encoding::Utf8Bom, decode_utf8_lossy);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return finish(rest, Encoding::Utf16Le, decode_utf16_le);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return finish(rest, Encoding::Utf16Be, decode_utf16_be);
    }

    // 无 BOM。先按 UTF-8 严格解——现代文件绝大多数走到这里就结束了，
    // 而严格解析会把 GB18030 的中文直接判死，正好进入下面的分支。
    if let Ok(text) = std::str::from_utf8(bytes) {
        // **不能**在这里硬写 `suspicious: 0`：合法 UTF-8 不等于可读正文。
        // 一段不含高位字节的二进制（或者夹着控制字符的日志残片）
        // 也是合法 UTF-8，而它的「可疑度」正是判它是不是文本的唯一依据。
        // 早先这里写死 0，于是同一份字节**换个编码路径**就会得到不同结论——
        // 那等于让「判定结果」取决于「我们碰巧走了哪条分支」。
        return Ok(DecodedText {
            suspicious: count_suspicious(text),
            text: text.to_owned(),
            encoding: Encoding::Utf8,
        });
    }

    // 无 BOM 的 UTF-16。中文 Windows 上「记事本另存为 Unicode」产出的
    // 就是这种文件，而它按 UTF-8 解会得到一堆 NUL 与乱码。
    if let Some(encoding) = sniff_utf16_without_bom(bytes) {
        let decoder = match encoding {
            Encoding::Utf16Le => decode_utf16_le,
            _ => decode_utf16_be,
        };
        return finish(bytes, encoding, decoder);
    }

    // 规格说的是「**有限**支持 GB18030」。这里不引入字符集探测：
    // 探测会把「这段字节像什么编码」变成一次猜测，而规格要求
    // 乱码明确失败——猜错的代价是一份内容错误的建议。
    finish(bytes, Encoding::Gb18030, decode_gb18030)
}

/// 按 NUL 的奇偶分布判断是不是无 BOM 的 UTF-16。
///
/// ## 为什么一个 NUL 就够
///
/// 走到这里的文件**已经不是合法 UTF-8 了**，而且它要么是 GB18030 中文、
/// 要么是 UTF-16。正常文本里几乎不会出现 NUL 字节；UTF-16 的 ASCII 部分
/// 却每两字节就有一个。
///
/// 第一版用的是「绝大多数 NUL 都落在同一侧」（≥75% 的字节对）。
/// 那对**纯英文**的 UTF-16 成立，对「中文内容 test」这种汉字没有 NUL、
/// 只有尾部 ASCII 有 NUL 的混合文本则不成立——而那恰恰是最常见的一种。
/// 所以判据改成「NUL 出现且两侧比例悬殊」。
fn sniff_utf16_without_bom(bytes: &[u8]) -> Option<Encoding> {
    // 太短的样本判断不可靠：一两个 NUL 的分布没有统计意义。
    if bytes.len() < 8 {
        return None;
    }
    let sample = &bytes[..bytes.len().min(512)];
    let mut nul_at_even = 0usize;
    let mut nul_at_odd = 0usize;
    for (index, byte) in sample.iter().enumerate() {
        if *byte == 0 {
            if index % 2 == 0 {
                nul_at_even += 1;
            } else {
                nul_at_odd += 1;
            }
        }
    }

    // 每侧至少两个，且是另一侧的四倍以上，才算「分布悬殊」。
    // 单个 NUL 可能是巧合（比如一段二进制残留），两个以上就很难是了。
    if nul_at_odd >= 2 && nul_at_odd >= nul_at_even * 4 {
        return Some(Encoding::Utf16Le);
    }
    if nul_at_even >= 2 && nul_at_even >= nul_at_odd * 4 {
        return Some(Encoding::Utf16Be);
    }
    None
}

fn finish(bytes: &[u8], encoding: Encoding, decoder: fn(&[u8]) -> DecodedText) -> DecodeResult {
    // 解码函数各自只负责「怎么把字节变成字符串」，不负责知道**自己是被
    // 哪个分支选中的**——同一个 UTF-8 解码既服务「带 BOM」也服务
    // 「无 BOM 但整段合法」。编码标签由调用方在这里统一盖上，
    // 免得两个地方对同一个事实各写一份。
    let mut decoded = decoder(bytes);
    decoded.encoding = encoding;

    let total = decoded.text.chars().count();
    if total == 0 {
        // 空文件不是乱码，它只是没有内容。交给上层判成 unsupported，
        // 在这里报 GARBLED 会把「空文件」说成「编码坏了」。
        return Ok(decoded);
    }

    // 样本太小时**只认「全是可疑字符」这一种结论**。
    //
    // 比例判据是统计性的，需要足够的样本才有意义。对一个 3 个字符的文件，
    // 「1 个可疑」既是 333‰ 也是「样本只有 3 个，说明不了任何事」。
    // 早先无条件套比例，结果是**短文件被一边倒地误杀**：一个三行笔记本
    // 只要有一个坏字节就成了「编码读不出来」，而用户拿它没有任何办法。
    //
    // 反过来，「整段没有一个字符是可信的」在小样本上是确定的结论，
    // 不需要统计也成立。
    if total < MIN_SAMPLE_FOR_RATIO {
        return if decoded.suspicious >= total {
            Err(codes::EXTRACTION_GARBLED)
        } else {
            Ok(decoded)
        };
    }

    if decoded.suspicious * 1000 > total * GARBLED_MARK_PERMILLE {
        return Err(codes::EXTRACTION_GARBLED);
    }
    Ok(decoded)
}

/// 比例判据生效所需的最小样本量（Unicode 标量个数）。
///
/// 取 64：一段正常中文正文随便就有几十个字，而一个「短到不足以做统计」
/// 的文件本来也不该因为一个坏字节被整份拒掉。
const MIN_SAMPLE_FOR_RATIO: usize = 64;

/// 这个字符算不算「说不出来」。
///
/// 三类都算，缺一不可——只数替换字符会被 GB18030 整段骗过去
/// （它把未定义序列映射进私用区，产出的是「合法字符」）。
fn is_suspicious(c: char) -> bool {
    if c == REPLACEMENT {
        return true;
    }
    // 制表、换行、回车是正常的排版字符，其余控制字符在正文里成片出现
    // 只可能是解码错了。
    if c.is_control() && c != '\t' && c != '\n' && c != '\r' {
        return true;
    }
    (PRIVATE_USE_START..=PRIVATE_USE_END).contains(&c)
}

fn count_suspicious(text: &str) -> usize {
    text.chars().filter(|c| is_suspicious(*c)).count()
}

fn decode_utf8_lossy(bytes: &[u8]) -> DecodedText {
    // 调用方已经确认它是合法 UTF-8（BOM 分支除外，那里也可能有坏字节），
    // 因此仍需按 lossy 处理并数出可疑字符。
    let text = String::from_utf8_lossy(bytes).into_owned();
    DecodedText {
        suspicious: count_suspicious(&text),
        text,
        encoding: Encoding::Utf8Bom,
    }
}

fn decode_utf16_le(bytes: &[u8]) -> DecodedText {
    decode_utf16(bytes, u16::from_le_bytes)
}

fn decode_utf16_be(bytes: &[u8]) -> DecodedText {
    decode_utf16(bytes, u16::from_be_bytes)
}

/// 手工按 UTF-16 解码，而不是用 `String::from_utf16_lossy` 直接吃 `&[u16]`。
///
/// 两个原因：
/// 1. 奇数字节长度是**可能的**（文件被截断），`from_utf16_lossy` 要求
///    长度成对，得先自己处理尾巴；直接丢弃那一个字节而不记录，
///    会让「文件被截断」变成一次静默的数据丢失。
/// 2. 需要数出可疑字符的个数，而 `from_utf16_lossy` 只给结果不给统计。
fn decode_utf16(bytes: &[u8], to_unit: fn([u8; 2]) -> u16) -> DecodedText {
    let (pairs, trailing_bytes) = bytes.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|pair| to_unit([pair[0], pair[1]]))
        .collect();
    let trailing = trailing_bytes.len();

    let mut text = String::with_capacity(units.len());
    let mut suspicious = 0usize;
    // `decode_utf16` 会自己处理代理对，遇到孤立代理产出 Err。
    for item in char::decode_utf16(units) {
        match item {
            Ok(c) => {
                if is_suspicious(c) {
                    suspicious += 1;
                }
                text.push(c);
            }
            Err(_) => {
                text.push(REPLACEMENT);
                suspicious += 1;
            }
        }
    }
    if trailing > 0 {
        // 尾巴上的半个码元算一次「说不出来」。
        text.push(REPLACEMENT);
        suspicious += 1;
    }

    DecodedText {
        text,
        // 具体是 LE 还是 BE 由调用方覆盖；这里给一个占位不参与判断。
        encoding: Encoding::Utf16Le,
        suspicious,
    }
}

fn decode_gb18030(bytes: &[u8]) -> DecodedText {
    let (cow, _, had_errors) = GB18030.decode(bytes);
    let text = cow.into_owned();
    // `had_errors` 为真时必有可疑字符，但反过来不成立（GB18030 自身
    // 就有映射到私用区与控制区的码位）。以数出来的个数为准，别信那个布尔量。
    let _ = had_errors;
    DecodedText {
        suspicious: count_suspicious(&text),
        text,
        encoding: Encoding::Gb18030,
    }
}

/// 按上限截断正文，返回（正文, 是否截断）。
///
/// 截断按 **Unicode 标量**计数（规格写的是「12,000 个 Unicode 字符」），
/// 不是字节。用 `chars()` 而不是 `len()`：中文一个字三字节，
/// 按字节截会把上限变成「4000 个汉字」，与规格不符。
pub fn truncate(text: &str) -> (String, bool) {
    // 用 `take` + 「还能不能取到下一个」判断截断，而不是自己数计数器：
    // 计数器的写法要处理「正好等于上限」这个边界（多取一次才知道有没有截断），
    // 而 `chars` 迭代器天然表达得了这件事。
    let mut chars = text.chars();
    let kept: String = chars.by_ref().take(MAX_EXTRACTED_CHARS).collect();
    let truncated = chars.next().is_some();
    (kept, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把一段字符串编成无 BOM 的 UTF-16LE 字节。
    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn plain_utf8_is_read_as_utf8() {
        let decoded = decode("你好，世界".as_bytes()).expect("UTF-8 应当成功");
        assert_eq!(decoded.text, "你好，世界");
        assert_eq!(decoded.encoding, Encoding::Utf8);
        assert_eq!(decoded.suspicious, 0);
    }

    #[test]
    fn a_utf8_bom_is_stripped_and_reported() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("报告".as_bytes());
        let decoded = decode(&bytes).expect("带 BOM 的 UTF-8 应当成功");

        assert_eq!(decoded.text, "报告", "BOM 不能留在正文里");
        assert_eq!(decoded.encoding, Encoding::Utf8Bom);
    }

    #[test]
    fn a_utf16_bom_is_honoured() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend_from_slice(&utf16le("会议纪要"));
        let decoded = decode(&bytes).expect("UTF-16LE 应当成功");

        assert_eq!(decoded.text, "会议纪要");
        assert_eq!(decoded.encoding, Encoding::Utf16Le);
    }

    #[test]
    fn a_utf16_file_without_a_bom_is_still_recognised() {
        // 中文 Windows 上「记事本 → 另存为 Unicode」就是这个形状。
        // 注意样本是**中英混合**：汉字没有 NUL、只有尾部 ASCII 有。
        // 第一版的嗅探判据（NUL 必须占满字节对的大多数）会漏掉这种，
        // 而那正是最常见的一种。
        let bytes = utf16le("中文内容 test");
        let decoded = decode(&bytes).expect("无 BOM 的 UTF-16LE 也应当认出来");

        assert_eq!(decoded.text, "中文内容 test");
        assert_eq!(decoded.encoding, Encoding::Utf16Le);
    }

    #[test]
    fn a_utf16be_file_without_a_bom_is_recognised_as_big_endian() {
        let bytes: Vec<u8> = "报告 done"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let decoded = decode(&bytes).expect("无 BOM 的 UTF-16BE 应当认出来");

        assert_eq!(decoded.text, "报告 done");
        assert_eq!(decoded.encoding, Encoding::Utf16Be);
    }

    #[test]
    fn gb18030_chinese_is_decoded_not_mangled() {
        let (encoded, _, _) = GB18030.encode("这是一份中文报告");
        let decoded = decode(&encoded).expect("GB18030 应当有限支持");

        assert_eq!(decoded.text, "这是一份中文报告");
        assert_eq!(decoded.encoding, Encoding::Gb18030);
        assert_eq!(decoded.suspicious, 0);
    }

    #[test]
    fn random_bytes_fail_loudly_instead_of_becoming_a_wall_of_replacement_chars() {
        // 规格 6.2：「乱码明确失败」。
        //
        // 这一段既不是合法 UTF-8，按 GB18030 解也几乎不产出 U+FFFD——
        // 它是靠**私用区与控制字符**被判出来的。只数替换字符的实现
        // 会在这里把一大段噪声当成正文返回。
        let bytes: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let error = decode(&bytes).expect_err("乱码必须明确失败");

        assert_eq!(error, codes::EXTRACTION_GARBLED);
    }

    #[test]
    fn a_single_control_character_does_not_condemn_a_normal_file() {
        // 阈值是占比，不是「出现即失败」：正常文档里夹一个不可映射字符
        // 不该整份被判成乱码。
        let mut text = "这是一份很长的正常中文报告，内容完全可读。".repeat(20);
        text.push('\u{0007}');
        let decoded = decode(text.as_bytes()).expect("个别控制字符不该让整份文件失败");

        assert_eq!(decoded.suspicious, 1);
    }

    #[test]
    fn an_empty_file_is_empty_not_garbled() {
        // 空文件不是乱码。把它报成 GARBLED 会让用户去查编码，
        // 而实际上他只是有一个空文件。
        let decoded = decode(b"").expect("空文件应当成功解码");
        assert!(decoded.text.is_empty());
        assert_eq!(decoded.suspicious, 0);
    }

    #[test]
    fn a_truncated_utf16_tail_is_counted_not_dropped_silently() {
        // 奇数字节长度：最后一个字节凑不成码元。
        //
        // 样本带 BOM，是为了**确定性地**走 UTF-16 分支——
        // 不带 BOM 的五字节输入按设计就嗅探不出来（样本太短），
        // 那种情况该测的是嗅探，不是这里的尾巴处理。
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend_from_slice(&utf16le("AB"));
        bytes.push(b'C');

        let decoded = decode(&bytes).expect("应当仍然能解出前半段");
        assert_eq!(decoded.text, "AB\u{FFFD}");
        assert_eq!(
            decoded.suspicious, 1,
            "半个码元必须被记成一次「说不出来」，不能悄悄丢掉"
        );
    }

    #[test]
    fn truncation_counts_unicode_scalars_not_bytes() {
        // 规格写的是「12,000 个 Unicode 字符」。用字节数截断会把上限
        // 悄悄变成「4000 个汉字」，而那与规格不符。
        let long: String = "中".repeat(MAX_EXTRACTED_CHARS + 50);
        let (kept, truncated) = truncate(&long);

        assert!(truncated, "超过上限必须标记截断");
        assert_eq!(kept.chars().count(), MAX_EXTRACTED_CHARS);
        assert_eq!(MAX_EXTRACTED_CHARS, 12_000);
    }

    #[test]
    fn a_short_text_is_not_marked_as_truncated() {
        let (kept, truncated) = truncate("短文本");
        assert_eq!(kept, "短文本");
        assert!(!truncated);
    }

    #[test]
    fn decoding_a_realistic_chinese_markdown_note_works() {
        let markdown = "# 项目笔记\n\n- 需求确认\n- 排期\n";
        let decoded = decode(markdown.as_bytes()).expect("应当成功");
        assert_eq!(decoded.text, markdown);
        assert_eq!(decoded.encoding, Encoding::Utf8);
    }

    #[test]
    fn a_short_binary_file_is_not_mistaken_for_utf16() {
        // 短的二进制片段不该被嗅探成 UTF-16：样本太短，判据不成立。
        let bytes = [0x01u8, 0x00, 0x9C, 0x8E];
        assert_eq!(sniff_utf16_without_bom(&bytes), None);
    }

    #[test]
    fn a_lone_nul_in_a_binary_file_is_not_enough_to_sniff_utf16() {
        // 只有一个 NUL 时不判 UTF-16：单个 NUL 可能是巧合。
        let mut bytes = vec![0x9Cu8; 16];
        bytes[5] = 0;
        assert_eq!(sniff_utf16_without_bom(&bytes), None);
    }
}
