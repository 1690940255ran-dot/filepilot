//! PNG / JPEG 的图片解析（规格 6.2 第四行）。
//!
//! | 项目 | 规格要求 |
//! |---|---|
//! | 方法 | **解码后**系统 OCR |
//! | 大小 | 文件 ≤ 10 MiB |
//! | 上限 | ≤ 2000 万像素；**超限拒绝** |
//! | 失败 | 语言包缺失明确提示 |
//!
//! ## 为什么要自己解码，而不是把原始字节丢给 OCR
//!
//! 两个理由，都指向同一件事：**像素上限只有在解码之后才谈得上**。
//!
//! 1. 一个 2 MiB 的 PNG 可以解出 4 亿像素——按文件大小限制根本挡不住。
//!    规格写的是「≤20 百万像素」，那是解码后的尺寸。
//! 2. 先解码再判超限，等于「钱已经花掉了」：内存已经被那张巨图占住了。
//!    所以顺序是**先读文件头拿到尺寸 → 判超限 → 再解码像素**。
//!
//! ## 「解码后系统 OCR」的落点
//!
//! 解码得到像素 → 转成 BGRA8 → 交给 `platform::ocr`。
//! 全程不进临时文件，也不出现任何网络调用——规格要求
//! 「Windows OCR 适配只在本机运行，不调用云端视觉接口」，
//! 这一条在本模块里是**构造性**的：代码里就没有那条路径。

use image::ImageReader;
use std::io::Cursor;

use super::limits::{MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS};
use crate::domain::errors::codes;

/// 提取成功的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageText {
    pub text: String,
    pub truncated: bool,
    /// 图片的像素尺寸，用作证据。
    pub width: u32,
    pub height: u32,
}

pub type ImageResult = Result<ImageText, &'static str>;

/// 从字节里读出图片并做 OCR。
pub fn extract(bytes: &[u8]) -> ImageResult {
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(codes::EXTRACTION_TOO_LARGE);
    }

    // —— 第一步：只读文件头，拿到尺寸 ——
    //
    // `into_dimensions` 不会解码像素，因此一张 4 亿像素的图在这里只会
    // 让我们知道「它有 4 亿像素」，而不是真的去分配几个 GiB。
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| codes::EXTRACTION_CORRUPT)?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| codes::EXTRACTION_CORRUPT)?;

    if width == 0 || height == 0 {
        return Err(codes::EXTRACTION_CORRUPT);
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_IMAGE_PIXELS {
        // 规格：「超限拒绝」。不缩图、不截取——缩图会让用户以为
        // 「它读了我的图」，而实际上读的是一个我们替他改过的版本。
        return Err(codes::EXTRACTION_TOO_LARGE);
    }

    // —— 第二条上限：**引擎自己的边长** ——
    //
    // 规格 6.2 的上限是「≤20 百万像素」，但系统 OCR 还有一条独立的**边长**
    // 限制（实测本机 `OcrEngine::MaxImageDimension()` = 10000）。两条不是一回事：
    // 一张 10001×1900 的图只有 1900 万像素，上面那关过得去，却会被引擎拒绝。
    //
    // 而引擎拒绝的表现是「识别失败」，一路映射下来会变成
    // `EXTRACTION_CORRUPT`——那等于告诉用户「文件坏了」。文件没坏，只是太宽了，
    // 用户该做的动作（缩小图片）与「文件损坏」完全不同。
    //
    // 所以在这里先挡住，给出正确的码。取不到上限（比如这台机器没有 OCR）
    // 就跳过这一条：那种情况下后面会以 `OCR_UNAVAILABLE` 如实报出来。
    if let Some(limit) = crate::platform::ocr::max_image_dimension() {
        if width > limit || height > limit {
            return Err(codes::EXTRACTION_TOO_LARGE);
        }
    }

    // —— 第二步：确认 OCR 能用再解码 ——
    //
    // 顺序放在解码之前是有意的：这台机器根本没有中文 OCR 时，
    // 解一遍像素纯属白费（大图要几十 MiB 内存），而结论不会因此改变。
    let availability = crate::platform::ocr::availability();
    if !availability.is_usable() {
        return Err(codes::OCR_UNAVAILABLE);
    }

    // —— 第三步：解码并转成 OCR 要的像素布局 ——
    let decoded = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| codes::EXTRACTION_CORRUPT)?
        .decode()
        .map_err(|_| codes::EXTRACTION_CORRUPT)?;
    let bgra = decoded.to_rgba8();
    // WinRT 要的是 BGRA8；`to_rgba8` 给的是 RGBA8，交换前两个通道。
    //
    // 用 `as_chunks_mut::<4>` 而不是 `chunks_exact_mut(4)`：前者在**类型上**
    // 就保证了每块恰好 4 字节，因此块内下标不带边界检查。
    let mut buffer = bgra.into_raw();
    let (pixels, remainder) = buffer.as_chunks_mut::<4>();
    debug_assert!(remainder.is_empty(), "RGBA 字节数必然是 4 的整数倍");
    for pixel in pixels {
        pixel.swap(0, 2);
    }

    // —— 第四步：系统 OCR ——
    let text = crate::platform::ocr::recognize_bgra(&buffer, width, height)
        .map_err(|_| codes::EXTRACTION_CORRUPT)?;

    let trimmed = text.trim();
    if trimmed.is_empty() {
        // 「图里没有文字」不是失败：它是一张照片、一张纯色图、或者一张
        // 只有图形的图。规格要求「无文字或不可用时明确提示」——
        // 提示的方式就是 unsupported，而不是拿一段空正文冒充成功。
        return Err(codes::UNSUPPORTED_FORMAT);
    }

    let (text, truncated) = super::text::truncate(trimmed);
    Ok(ImageText {
        text,
        truncated,
        width,
        height,
    })
}

/// 图片的证据片段。
///
/// 与文本类一样，locator 说的是「这段片段在本次提取结果里的字符范围」，
/// 再补一条像素尺寸——用户据此判断「它到底看的是哪张图」。
pub fn evidence(result: &ImageText) -> Vec<crate::domain::types::Evidence> {
    use crate::domain::types::Evidence;

    let mut evidence = vec![Evidence {
        locator: format!("pixels:{}x{}", result.width, result.height),
        excerpt: format!(
            "{} × {} 像素（{:.1} 万像素）",
            result.width,
            result.height,
            (f64::from(result.width) * f64::from(result.height)) / 10_000.0
        ),
    }];

    let excerpt: String = result
        .text
        .chars()
        .take(super::limits::MAX_EVIDENCE_CHARS)
        .collect();
    if !excerpt.trim().is_empty() {
        evidence.push(Evidence {
            locator: format!("chars:0-{}", excerpt.chars().count()),
            excerpt: if result.truncated {
                format!("{excerpt}…（已截断）")
            } else {
                excerpt
            },
        });
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一张指定尺寸的 PNG。`text` 只用来让图片有点内容。
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut buffer = image::RgbImage::new(width, height);
        for (_, _, pixel) in buffer.enumerate_pixels_mut() {
            *pixel = image::Rgb([255, 255, 255]);
        }
        let mut out = Cursor::new(Vec::new());
        buffer
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("编码 PNG");
        out.into_inner()
    }

    #[test]
    fn a_file_over_the_byte_limit_is_rejected() {
        let oversized = vec![0u8; (MAX_IMAGE_BYTES + 1) as usize];
        assert_eq!(extract(&oversized), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn an_image_over_the_pixel_limit_is_rejected_before_decoding() {
        // 规格 6.2：≤2000 万像素，超限拒绝。
        //
        // 关键在**解码之前**就拒绝：5000×5000 = 2500 万像素，
        // 真解码出来是 2500 万 × 3 字节 ≈ 75 MiB 的内存。
        // 这条用例能在这个尺寸上跑完，本身就证明了没有先解码。
        let oversized = png(5_000, 5_000);
        // 样本必须**远小于文件大小上限**，这条用例才说明得了问题：
        // 只有在「不是因为文件太大而被拒」的前提下，拒绝才归因于像素数。
        // （实测算出来约 370 KB——纯色 PNG 压得动，但没到 64 KB 那么夸张。）
        assert!(
            (oversized.len() as u64) < MAX_IMAGE_BYTES / 8,
            "样本必须远小于文件上限（{MAX_IMAGE_BYTES} 字节），\
             否则拒绝可能来自文件大小而不是像素数：{} 字节",
            oversized.len()
        );
        assert_eq!(extract(&oversized), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn an_image_over_the_engine_dimension_limit_is_rejected() {
        // 引擎有**边长**上限（实测本机 10000），而它与「总像素数」是两条
        // 不同的限制。
        //
        // 这条用一张 10001×1 的图：总像素只有一万，远低于 2000 万那一条，
        // 所以拒绝**只可能**来自边长检查。反过来说，如果这条检查被删掉，
        // 这张图会被送去解码、然后被引擎拒绝，最终报成
        // `EXTRACTION_CORRUPT`——「文件坏了」，而它只是太宽了。
        let wide = png(10_001, 1);
        assert!(
            (wide.len() as u64) < MAX_IMAGE_BYTES / 8,
            "样本要小到「不可能是因为文件太大被拒」：{} 字节",
            wide.len()
        );
        assert_eq!(extract(&wide), Err(codes::EXTRACTION_TOO_LARGE));
    }

    #[test]
    fn an_image_at_the_engine_dimension_limit_is_not_called_too_large() {
        // 边界另一侧：正好等于边长上限的图不该被这条检查挡住。
        // 它最终会因为「图里没有文字」报成 unsupported，但**不能**是
        // TOO_LARGE——否则说明比较写成了 `>=`。
        let limit = crate::platform::ocr::max_image_dimension().expect("引擎应当报告边长上限");
        let outcome = extract(&png(limit, 1));
        assert!(
            !matches!(outcome, Err(codes::EXTRACTION_TOO_LARGE)),
            "正好等于上限不该被当成超限：{outcome:?}"
        );
    }

    #[test]
    fn a_corrupt_image_is_reported_as_corrupt() {
        let garbage: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        assert_eq!(extract(&garbage), Err(codes::EXTRACTION_CORRUPT));
    }

    #[test]
    fn an_empty_byte_slice_is_corrupt_not_a_panic() {
        assert_eq!(extract(b""), Err(codes::EXTRACTION_CORRUPT));
    }

    #[test]
    fn a_blank_image_never_comes_back_as_invented_text() {
        // 一张纯白图里没有文字。允许两种结论，但**不许**有第三种：
        // * `UNSUPPORTED_FORMAT` —— 系统 OCR 可用，但它确实没读出文字；
        // * `OCR_UNAVAILABLE`   —— 这台机器没有 OCR，我们没去猜。
        //
        // 不许的是「返回一段正文」：那意味着我们在没有依据的情况下
        // 编了内容出来。这条断言在任何机器上都成立，不依赖装没装语言包。
        let outcome = extract(&png(32, 32));
        assert!(
            matches!(
                outcome,
                Err(codes::UNSUPPORTED_FORMAT) | Err(codes::OCR_UNAVAILABLE)
            ),
            "空白图不能产出正文：{outcome:?}"
        );
    }
}
