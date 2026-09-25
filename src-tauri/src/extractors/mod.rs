//! 内容提取（规格 6.2）。
//!
//! ## 为什么是一个独立进程
//!
//! 规格写得很直接：**Tauri 前端隔离不等于解析器隔离**，P5 必须验证
//! 真实的工作进程边界；若无法可靠限制某个解析器，就关掉那个格式的内容解析，
//! 「不能在 UI 进程直接执行不可信解析」。
//!
//! 用户文件内容是不可信数据。一个 PDF 解析器崩溃一次，代价不该是
//! 整个应用连同用户还没保存的界面状态一起消失——那是把「读一份坏文件」
//! 变成了「丢掉正在做的事」。
//!
//! 因此本模块分成两半：
//!
//! * **界面进程这一半**（本文件）：打开文件、算指纹、派生工作进程、收取结果；
//! * **工作进程那一半**（`worker` 模块 + `src/bin/extract_worker.rs`）：
//!   真正去碰那些解析库。
//!
//! 两半之间只走 `protocol` 定义的那几个字段，而且**不传路径**。

pub mod docx;
pub mod image;
pub mod limits;
pub mod pdf;
pub mod protocol;
pub mod service;
pub mod text;
pub mod worker;

pub use limits::{
    EXTRACT_CONCURRENCY, EXTRACT_TIMEOUT_MS, MAX_EVIDENCE, MAX_EXTRACTED_CHARS,
    WORKER_MEMORY_LIMIT_BYTES,
};
pub use protocol::{SourceFormat, WorkerArgs, WorkerOutcome, PROTOCOL_VERSION};
pub use service::{extract_batch, ExtractObserver, ExtractRequest, NoopExtractObserver};

/// 由扩展名判断该用哪个解析器（规格 6.2 的表格）。
///
/// 判不出来返回 `None`，调用方据此走「**其他**：不提取内容，
/// 仅文件名/扩展名/时间可进入建议」那条路。
///
/// 扩展名一律小写、带前导点——这是 `scanner::walk::extension_of` 的产出，
/// 不是这里自己转的。两处各转一次，早晚会出现「扫描说是 .pdf、
/// 提取却按其他处理」这种不一致。
pub fn format_for(extension: &str) -> Option<SourceFormat> {
    match extension {
        ".txt" | ".md" | ".markdown" | ".rst" | ".log" | ".csv" | ".tsv" => {
            Some(SourceFormat::Text)
        }
        ".pdf" => Some(SourceFormat::Pdf),
        ".docx" => Some(SourceFormat::Docx),
        // 规格 6.2 第四行：PNG/JPEG 走「解码后系统 OCR」。
        // 只有这两种——GIF/BMP/TIFF 等都不在规格的表里，
        // 多收一种就多一份解析用户文件的代码，而规格没要求。
        ".png" | ".jpg" | ".jpeg" => Some(SourceFormat::Image),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extensions_map_to_a_parser() {
        assert_eq!(format_for(".txt"), Some(SourceFormat::Text));
        assert_eq!(format_for(".md"), Some(SourceFormat::Text));
        assert_eq!(format_for(".pdf"), Some(SourceFormat::Pdf));
        assert_eq!(format_for(".docx"), Some(SourceFormat::Docx));
    }

    #[test]
    fn unknown_and_empty_extensions_have_no_parser() {
        // 规格 6.2 的最后一行：其他类型不提取内容。
        // 这里返回 None 而不是「兜底用文本解析」——把 .exe 当文本读
        // 除了浪费时间，还会给下游一段纯噪声。
        for extension in ["", ".exe", ".zip", ".ts", ".PDF"] {
            assert_eq!(format_for(extension), None, "{extension} 不该有解析器");
        }
    }

    #[test]
    fn old_doc_and_xls_are_deliberately_not_parsed() {
        // .doc 是 OLE 复合文档、.xls 同理，都不是 ZIP 容器。
        // 按 DOCX 去解它们会得到一个「损坏」的结论，而它们其实完好——
        // 那会把用户引到「我的文件坏了」这个错误方向。
        for extension in [".doc", ".xls", ".ppt"] {
            assert_eq!(format_for(extension), None, "{extension} 不应被当成 docx");
        }
    }
}
