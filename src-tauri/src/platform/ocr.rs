//! 系统 OCR 适配层（规格 6.2：PNG/JPEG 走「解码后系统 OCR」）。
//!
//! ## 只在本机跑，不碰云端
//!
//! 用的是 Windows 自带的 WinRT OCR（`Windows.Media.Ocr`）。
//! 规格点名要求「**不调用云端视觉接口**」，因此这里：
//!
//! * 不出现任何 URL、HTTP 客户端或提供商配置；
//! * 唯一的输入是内存里的像素数据——连临时文件都不落。
//!
//! 这一条是**构造性**的：代码里根本没有那条路径，不是运行时拦下来的。
//!
//! ## 「不可用」必须说得出原因
//!
//! OCR 不可用有三种互不相同的原因，用户能做的事也不同：
//!
//! | 原因 | 用户能做什么 |
//! |---|---|
//! | 系统没有装任何 OCR 语言 | 去「设置 → 时间和语言 → 语言」里添加语言的 OCR 组件 |
//! | 装的语言里没有中文 | 添加中文（简体）的 OCR 组件 |
//! | 这版 Windows 不支持 WinRT OCR | 换一台机器，或者只用规则模式 |
//!
//! 因此 [`availability`] 返回的是**结构化的原因**，不是一句「不可用」。
//! 设置页直接拿它显示，用户才知道下一步点哪里。

use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;

/// OCR 的可用状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrAvailability {
    /// 可用，且给出了系统里可用于识别的语言标签（如 `zh-Hans-CN`）。
    Available { languages: Vec<String> },
    /// 系统没装任何 OCR 识别语言。
    NoRecognizerLanguage,
    /// 装了识别语言，但没有收录中文——本产品的目标用户需要中文。
    NoChineseLanguage { languages: Vec<String> },
    /// 这版系统上 WinRT OCR 起不来（调用失败）。
    Unsupported { reason: String },
}

impl OcrAvailability {
    /// 能不能真的调用识别。
    pub fn is_usable(&self) -> bool {
        matches!(self, OcrAvailability::Available { .. })
    }

    /// 转成可以跨进程传的报告。
    pub fn to_report(&self) -> crate::extractors::protocol::OcrAvailabilityReport {
        let (status, languages) = match self {
            OcrAvailability::Available { languages } => ("available", languages.clone()),
            OcrAvailability::NoRecognizerLanguage => ("noRecognizerLanguage", Vec::new()),
            OcrAvailability::NoChineseLanguage { languages } => {
                ("noChineseLanguage", languages.clone())
            }
            OcrAvailability::Unsupported { .. } => ("unsupported", Vec::new()),
        };
        crate::extractors::protocol::OcrAvailabilityReport {
            status: status.to_owned(),
            languages,
            message: self.message(),
        }
    }

    /// 给用户看的一句话。**必须包含下一步动作**，否则用户只能干瞪眼。
    pub fn message(&self) -> String {
        match self {
            OcrAvailability::Available { languages } => {
                format!("系统 OCR 可用（{}）", languages.join("、"))
            }
            OcrAvailability::NoRecognizerLanguage => {
                "这台电脑还没有安装任何 OCR 识别语言。请在「设置 → 时间和语言 → \
                 语言和区域」里为某个语言添加「可选语言功能 → 光学字符识别」。\
                 不装也能用：规则整理与按文件名/时间分类不依赖 OCR。"
                    .to_owned()
            }
            OcrAvailability::NoChineseLanguage { languages } => format!(
                "系统 OCR 只装了 {}，没有中文。请在「设置 → 时间和语言 → 语言和区域」\
                 里给「中文（简体）」添加「可选语言功能 → 光学字符识别」。\
                 不装也能用：规则整理不依赖 OCR。",
                languages.join("、")
            ),
            OcrAvailability::Unsupported { reason } => {
                format!("这台系统上无法使用 OCR（{reason}）。图片将只用文件名与元信息参与建议。")
            }
        }
    }
}

/// 判断 OCR 是否可用。
///
/// 每次都真的去问系统，不缓存结果：用户完全可能在我们运行期间去装了语言包，
/// 而**缓存一个「不可用」会让他在装完之后仍然用不了**，还找不到原因。
pub fn availability() -> OcrAvailability {
    if let Err(reason) = ensure_initialized() {
        return OcrAvailability::Unsupported { reason };
    }

    let languages = match OcrEngine::AvailableRecognizerLanguages() {
        Ok(list) => {
            let mut collected = Vec::new();
            for index in 0..list.Size().unwrap_or(0) {
                if let Ok(language) = list.GetAt(index) {
                    if let Ok(tag) = language.LanguageTag() {
                        collected.push(tag.to_string());
                    }
                }
            }
            collected
        }
        Err(error) => {
            return OcrAvailability::Unsupported {
                reason: format!("无法枚举识别语言: {error}"),
            }
        }
    };

    if languages.is_empty() {
        return OcrAvailability::NoRecognizerLanguage;
    }
    // 中文的标签形如 `zh-Hans-CN` / `zh-Hant-TW`，统一按前缀 `zh` 判。
    if !languages
        .iter()
        .any(|tag| tag.to_lowercase().starts_with("zh"))
    {
        return OcrAvailability::NoChineseLanguage { languages };
    }

    OcrAvailability::Available { languages }
}

/// 对一段 BGRA8 像素做 OCR，返回识别到的文本。
///
/// `pixels` 的长度必须正好是 `width * height * 4`——调用方（`extractors::image`）
/// 负责解码与像素上限校验；这里只负责把像素交给系统。
pub fn recognize_bgra(pixels: &[u8], width: u32, height: u32) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("图片尺寸为空".to_owned());
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| "图片尺寸过大，无法计算缓冲区长度".to_owned())?;
    if pixels.len() != expected {
        return Err(format!(
            "像素缓冲区长度不符：期望 {expected} 字节，收到 {}",
            pixels.len()
        ));
    }

    ensure_initialized()?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|error| format!("无法创建 OCR 引擎: {error}"))?;

    // 用 `CreateFromByteArray` + `CreateCopyFromBuffer` 直接造位图，
    // 而不是走 `BitmapDecoder` + 内存流：解码已经在调用方做过了
    // （那里要按像素上限先挡一道），再解一次既浪费又会让上限失去意义。
    let buffer = CryptographicBuffer::CreateFromByteArray(pixels)
        .map_err(|error| format!("无法创建像素缓冲区: {error}"))?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )
    .map_err(|error| format!("无法构造位图: {error}"))?;

    // 同步阻塞等结果。
    //
    // 这里**自己轮询 `Status()`**，而不是用 `windows_future::Async::join()`：
    // `windows 0.62` 配套的 `windows-future 0.3` 把 `Async` trait 收成了
    // 私有实现细节（`use r#async::*;` 不是 `pub use`），旧版的
    // `IAsyncOperation::get()` 也已经移除。剩下的公开表面只有
    // `Status()` / `GetResults()` / `SetCompleted()`——用前两个拼出阻塞等待
    // 是唯一不依赖私有 API 的写法。
    //
    // **不在这里设超时**：外层（`extractors::service` → 作业对象）已经有
    // 15 秒上限，超时会把整个工作进程连同这里一起终止。
    // 在里层再叠一层超时只会让两处数字打架，而外层那个才是真正有效的那道。
    use windows_future::AsyncStatus;

    let operation = engine
        .RecognizeAsync(&bitmap)
        .map_err(|error| format!("无法发起 OCR 识别: {error}"))?;

    loop {
        match operation.Status() {
            Ok(AsyncStatus::Completed) => break,
            Ok(AsyncStatus::Started) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(AsyncStatus::Canceled) => return Err("OCR 识别被取消".to_owned()),
            Ok(AsyncStatus::Error) => return Err("OCR 识别出错".to_owned()),
            // 枚举里还有未列出的状态值（WinRT 允许扩展）。既不认识就不猜，
            // 直接去取结果——取不到会得到一个明确的错误。
            Ok(_) => break,
            Err(error) => return Err(format!("无法读取 OCR 状态: {error}")),
        }
    }

    let result = operation
        .GetResults()
        .map_err(|error| format!("OCR 识别失败: {error}"))?;

    let text = result
        .Text()
        .map_err(|error| format!("无法读取识别结果: {error}"))?;
    Ok(collapse_cjk_spaces(&text.to_string()))
}

/// 一个可以在折叠范围内被吞掉的空白。
///
/// 刻意**不含换行**：`\n` 携带行结构，不该被当作词间空白处理。
fn is_collapsible_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\u{3000}')
}

/// 一个「不写空格」的文字系统中的字符。
///
/// **刻意不含韩文（U+D7AF 之前的 Hangul）**：韩文用空格分词，
/// 把它的空格吞掉会破坏词边界，那是把一种语言的错误修法套到另一种上。
fn is_cjk_character(character: char) -> bool {
    matches!(
        character,
        '\u{3000}'..='\u{303F}'      // CJK 标点（、。「」等）
            | '\u{3040}'..='\u{30FF}' // 平假名 / 片假名
            | '\u{31F0}'..='\u{31FF}' // 片假名扩展
            | '\u{3400}'..='\u{4DBF}' // 表意文字扩展 A
            | '\u{4E00}'..='\u{9FFF}' // 表意文字基本区
            | '\u{F900}'..='\u{FAFF}' // 兼容表意文字
            | '\u{FF01}'..='\u{FF60}' // 全角标点与全角字母
            | '\u{FFE0}'..='\u{FFE6}' // 全角符号
            | '\u{20000}'..='\u{2FA1F}' // 扩展 B 及以后
    )
}

/// 合并 Windows OCR 在汉字之间硬切出来的空格。
///
/// ## 为什么这不是「显示问题」
///
/// Windows OCR 按字符切分 CJK 的词边界，于是「会议纪要」会变成
/// `会 议 纪 要`。这份正文**是要拿去检索的**：用户在结果里搜「会议」，
/// 在 `会 议 纪 要` 里一个都搜不到——而 T11 的验收原文写的正是
/// 「OCR 有文字时返回可搜索片段」。所以必须把这类空格去掉。
///
/// 判据是「两侧**都**是 CJK」，这样真正起分隔作用的空格会被保住：
///
/// | 输入 | 输出 | 为什么 |
/// |---|---|---|
/// | `会 议 纪 要` | `会议纪要` | 汉字之间不写空格 |
/// | `项目进度报告 2026` | 原样 | 那个空格在分隔中文与数字，有用 |
/// | `Hello World` | 原样 | 拉丁字母不在折叠范围内 |
/// | `안녕 하세요` | 原样 | 韩文用空格分词，动了就错 |
///
/// 连续多个空白会被并成一个，行首与行尾的空白一律丢掉。
pub fn collapse_cjk_spaces(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            // 行结构保留：OCR 的空格折叠只发生在行内。
            output.push('\n');
        }
        collapse_spaces_in_line(line.trim_end_matches('\r'), &mut output);
    }
    output
}

/// 折叠**一行之内**的空白。见 [`collapse_cjk_spaces`]。
fn collapse_spaces_in_line(line: &str, output: &mut String) {
    // 空白先挂起，等看到它后面那个字符再决定留不留——
    // 「要不要这个空格」取决于它的右邻居，而流式处理时右邻居还没到。
    let mut pending_space = false;
    let mut previous: Option<char> = None;

    for character in line.chars() {
        if is_collapsible_space(character) {
            pending_space = true;
            continue;
        }

        if pending_space {
            let joins_cjk = previous
                .is_some_and(|previous| is_cjk_character(previous) && is_cjk_character(character));
            // 行首的待决空白（`previous` 为 `None`）不补出去。
            if !joins_cjk && previous.is_some() {
                output.push(' ');
            }
            pending_space = false;
        }

        output.push(character);
        previous = Some(character);
    }
    // 行尾的待决空白直接丢掉：它不携带信息。
}

/// 引擎能处理的最大边长（像素）。系统有硬上限，超过会直接报错。
///
/// 实测本机返回 `Some(10000)`。这是一条**独立于「总像素数」**的限制：
/// `extractors::image` 用的是它，见那里的注释。
///
/// ## 为什么这个值可以缓存（而公寓初始化不能）
///
/// [`ensure_initialized`] 每次都要调——公寓是**线程级**的，见它的文档。
/// 但「引擎能处理多大边长」是**引擎的属性，不随线程变化**，所以问一次
/// 就够。缓存它还有第二个好处：每次调 `OcrEngine::MaxImageDimension()`
/// 都是一次 WinRT 调用，而 `extractors::image` 的每条用例都会经过这里
/// ——并行跑测试时，那是一片同时压向 WinRT 的调用。
pub fn max_image_dimension() -> Option<u32> {
    static LIMIT: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();

    *LIMIT.get_or_init(|| {
        // 这一次会在**调用它的那个线程**上初始化公寓，而紧接着的引擎调用
        // 就在同一线程上，所以是对的。
        ensure_initialized().ok()?;
        OcrEngine::MaxImageDimension().ok()
    })
}

/// WinRT 公寓初始化。
///
/// ## 为什么**不能**缓存成进程级的一次性初始化
///
/// `RoInitialize` 初始化的是**调用线程**的公寓模型，不是进程的。
/// 第一版把它放进 `OnceLock` 缓存，结果是：第一个线程初始化过之后，
/// 第二个线程看到缓存是 `Ok` 就跳过了初始化，然后**在没有公寓的线程上**
/// 直接调 WinRT——进程当场段错误（实测 `exit 139`，卡在第二条用例上）。
///
/// 所以每次都调。它足够便宜（已初始化时立即返回 `S_FALSE`），
/// 而且**只有每次都调才是对的**：线程是调用方的事，我们无从知道
/// 下一个调用会落在哪个线程上。
fn ensure_initialized() -> Result<(), String> {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    // SAFETY: `RoInitialize` 只影响调用线程的公寓模型；
    // 重复初始化返回 `S_FALSE` 或 `RPC_E_CHANGED_MODE`，都不影响使用。
    match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        Ok(()) => Ok(()),
        Err(error) => {
            // 已经初始化过、或本线程已是别的公寓模型：都不是致命问题，
            // 只是「不需要再初始化」。真正的失败是别的原因。
            let code = error.code().0;
            // S_FALSE = 0x00000001（已初始化）；RPC_E_CHANGED_MODE = 0x80010106
            if code == 0x0000_0001 || code == 0x8001_0106u32 as i32 {
                Ok(())
            } else {
                Err(format!("WinRT 初始化失败: {error}"))
            }
        }
    }
}

/// 把一段字符串裁成便于作为证据展示的片段。
pub fn excerpt(text: &str, limit: usize) -> String {
    let collected: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        format!("{collected}…")
    } else {
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这里**只测不碰 WinRT 的纯逻辑**。
    //
    // `availability()` 与 `recognize_bgra()` 会在本模块外被调用一次，
    // 而 WinRT 的公寓模型**绑定线程**：libtest 每个用例跑在自己的线程上，
    // 第一个线程初始化公寓后退出，第二个线程再去调 WinRT 就是
    // `STATUS_ACCESS_VIOLATION`（段错误，实测）。
    //
    // 因此真实调用只走**工作进程**（`extract_worker` 的 `ocrAvailability` 模式），
    // 由 `tests/extract.rs` 从外部验证——那也是它在生产里的唯一去处。
    // 界面进程从不直接调 WinRT：OCR 是一种解析，界面进程不该碰解析器。

    #[test]
    fn every_unavailable_reason_tells_the_user_what_to_do_next() {
        // 三类原因的下一步完全不同（装语言包 / 装中文语言包 / 换机器）。
        // 一句「OCR 不可用」会让用户只能干瞪眼。
        let cases = [
            (OcrAvailability::NoRecognizerLanguage, "设置"),
            (
                OcrAvailability::NoChineseLanguage {
                    languages: vec!["en-US".to_owned()],
                },
                "中文（简体）",
            ),
            (
                OcrAvailability::Unsupported {
                    reason: "测试用原因".to_owned(),
                },
                "测试用原因",
            ),
        ];

        for (status, expected) in cases {
            let message = status.message();
            assert!(
                message.contains(expected),
                "{status:?} 的提示必须包含「{expected}」：{message}"
            );
            assert!(!status.is_usable(), "{status:?} 不该被判成可用");
        }
    }

    #[test]
    fn the_available_message_lists_the_languages() {
        let status = OcrAvailability::Available {
            languages: vec!["zh-Hans-CN".to_owned()],
        };
        assert!(status.is_usable());
        assert!(status.message().contains("zh-Hans-CN"));
    }

    #[test]
    fn the_report_carries_the_same_message_as_the_status() {
        // 报告与状态必须说同一句话：界面拿的是报告，两边不一致
        // 会让「设置页显示的原因」和「实际原因」对不上。
        for status in [
            OcrAvailability::Available {
                languages: vec!["zh-Hans-CN".to_owned()],
            },
            OcrAvailability::NoRecognizerLanguage,
            OcrAvailability::NoChineseLanguage {
                languages: vec!["en-US".to_owned()],
            },
            OcrAvailability::Unsupported {
                reason: "x".to_owned(),
            },
        ] {
            let report = status.to_report();
            assert_eq!(report.message, status.message());
            assert_eq!(report.is_usable(), status.is_usable());
        }
    }

    #[test]
    fn recognizing_rejects_a_buffer_whose_length_does_not_match() {
        // 长度不符必须先被挡住：直接交给系统会得到一个含糊的错误，
        // 而「缓冲区小了一圈」在我们这一侧是能算出来的事实。
        // 这条在触到 WinRT 之前就返回了，所以可以安全地放在单测里。
        let error = recognize_bgra(&[0u8; 16], 4, 4).expect_err("长度不符必须拒绝");
        assert!(error.contains("长度不符"), "{error}");
    }

    #[test]
    fn recognizing_rejects_an_empty_image() {
        assert!(recognize_bgra(&[], 0, 0).is_err());
    }

    #[test]
    fn the_excerpt_is_bounded_and_marks_truncation() {
        let long: String = "字".repeat(300);
        let short = excerpt(&long, 160);
        assert_eq!(short.chars().count(), 161, "160 个字符 + 省略号");
        assert!(short.ends_with('…'));

        assert_eq!(excerpt("短", 160), "短", "没超长就不该加省略号");
    }

    // ---- 汉字之间的空格折叠（T11 实测发现的缺陷）----

    #[test]
    fn the_spaces_ocr_puts_between_chinese_characters_are_collapsed() {
        // 这是本机 OCR 的**真实**输出形状，不是构造出来的：
        // 一张写着「会议纪要」的图会被读成 `会 议 纪 要`。
        assert_eq!(collapse_cjk_spaces("会 议 纪 要"), "会议纪要");
        assert_eq!(collapse_cjk_spaces("项 目 进 度 报 告"), "项目进度报告");
    }

    #[test]
    fn a_collapsed_result_is_actually_searchable() {
        // 折叠的**目的**是检索，所以断言落在检索上，而不是落在字符串相等上：
        // 就算哪天折叠规则变了，只要「搜得到」这条成立，用户就是好的。
        let raw = "会 议 纪 要";
        assert!(!raw.contains("会议"), "前提：折叠之前是搜不到的");

        let collapsed = collapse_cjk_spaces(raw);
        assert!(collapsed.contains("会议"), "{collapsed}");
        assert!(collapsed.contains("纪要"), "{collapsed}");
    }

    #[test]
    fn a_space_between_chinese_and_digits_survives() {
        // 折叠规则是「两侧**都**是 CJK」。这个空格在分隔中文与数字，
        // 去掉它会把 `报告 2026` 粘成 `报告2026`，那是另一种错误。
        assert_eq!(
            collapse_cjk_spaces("项目进度报告 2026"),
            "项目进度报告 2026"
        );
    }

    #[test]
    fn latin_word_spacing_is_untouched() {
        assert_eq!(collapse_cjk_spaces("Hello World"), "Hello World");
        assert_eq!(collapse_cjk_spaces("Hello   World"), "Hello World");
    }

    #[test]
    fn korean_keeps_its_word_spacing() {
        // 反向断言：韩文用空格分词，折叠范围一旦扩到 Hangul 就会破坏它。
        // 这条防的是「顺手把所有非 ASCII 都当成 CJK」。
        assert_eq!(collapse_cjk_spaces("안녕 하세요"), "안녕 하세요");
    }

    #[test]
    fn mixed_chinese_and_latin_keeps_only_the_meaningful_spaces() {
        assert_eq!(
            collapse_cjk_spaces("中 文 English 混 排"),
            "中文 English 混排"
        );
    }

    #[test]
    fn full_width_punctuation_counts_as_chinese() {
        // OCR 也会把全角标点单独切开：`会 议 ， 记 要`。
        // 标点不算 CJK 的话，逗号两侧的空格会被留下，
        // 「纪要」两个字仍然隔着东西，检索照样失效。
        assert_eq!(collapse_cjk_spaces("会 议 ， 记 要"), "会议，记要");
    }

    #[test]
    fn a_full_width_space_also_collapses() {
        assert_eq!(collapse_cjk_spaces("会\u{3000}议"), "会议");
    }

    #[test]
    fn leading_and_trailing_spaces_are_dropped() {
        assert_eq!(collapse_cjk_spaces("  会 议  "), "会议");
    }

    #[test]
    fn line_breaks_are_preserved() {
        // 行结构不是词间空白，折叠不该把它压平——
        // 否则多行图片的正文会变成不分段的一长串。
        assert_eq!(collapse_cjk_spaces("会 议\n记 要"), "会议\n记要");
        assert_eq!(collapse_cjk_spaces("会 议\r\n记 要"), "会议\n记要");
    }

    #[test]
    fn an_empty_string_stays_empty() {
        assert_eq!(collapse_cjk_spaces(""), "");
        assert_eq!(collapse_cjk_spaces("   "), "");
    }

    #[test]
    fn the_engine_reports_a_sane_dimension_limit() {
        // 这条会**真的调 WinRT**，而它能放在单测里是有原因的：
        // `ensure_initialized` 每次都调（见上方的长注释）——T11 那次的
        // 段错误正是「把初始化缓存成进程级 + 第二个线程」造成的，
        // 修掉之后任何线程调用都是安全的。
        //
        // 边长上限是引擎的属性，不依赖语言包。它的**具体值**不写死
        // （别的机器可能不同），但必须是合理的正数：`extractors::image`
        // 拿它当第二条上限用，一个 0 会让所有图片都被拒。
        let limit = max_image_dimension().expect("引擎应当报告边长上限");
        assert!(limit >= 1000, "边长上限 {limit} 小得不合理");
    }
}
