//! byType 规则：按扩展名映射到固定的五个分类（规格 6.3）。
//!
//! 分类集合是**规格固定的**，不能由用户或模型扩展：
//! 文档、图片、音视频、压缩包、其他。
//!
//! 映射表以「扩展名（含前导点，全小写）」为准。未知扩展名、以及没有扩展名的文件，
//! 一律进「其他」——这是规格明确要求的行为，不是兜底失败。

/// 文档类。包含文字文档与办公文档。
pub const DOCUMENT: &str = "文档";
/// 图片类。
pub const IMAGE: &str = "图片";
/// 音视频类。
pub const AUDIO_VIDEO: &str = "音视频";
/// 压缩包类。
pub const ARCHIVE: &str = "压缩包";
/// 其他类。未知扩展名与无扩展名文件都到这里。
pub const OTHER: &str = "其他";

/// 文档扩展名。**必须全部小写且带前导点**（有测试守着）。
const DOCUMENT_EXTENSIONS: &[&str] = &[
    ".csv",
    ".doc",
    ".docx",
    ".dot",
    ".dotx",
    ".epub",
    ".log",
    ".markdown",
    ".md",
    ".mobi",
    ".odp",
    ".ods",
    ".odt",
    ".pdf",
    ".pps",
    ".ppsx",
    ".ppt",
    ".pptx",
    ".rst",
    ".rtf",
    ".tex",
    ".tsv",
    ".txt",
    ".wps",
    ".xls",
    ".xlsm",
    ".xlsx",
];

/// 图片扩展名。
const IMAGE_EXTENSIONS: &[&str] = &[
    ".avif", ".bmp", ".gif", ".heic", ".heif", ".ico", ".jfif", ".jpe", ".jpeg", ".jpg", ".png",
    ".raw", ".svg", ".tif", ".tiff", ".webp",
];

/// 音视频扩展名。
///
/// 刻意**不收录 `.ts`**：它既是 MPEG 传输流，也是 TypeScript 源码的常见后缀。
/// 把一个开发者的源码目录搬进「音视频」比放进「其他」糟糕得多。
const AUDIO_VIDEO_EXTENSIONS: &[&str] = &[
    ".3gp", ".aac", ".avi", ".flac", ".flv", ".m4a", ".m4v", ".mkv", ".mov", ".mp3", ".mp4",
    ".mpeg", ".mpg", ".oga", ".ogg", ".opus", ".wav", ".webm", ".wma", ".wmv",
];

/// 压缩包扩展名。
const ARCHIVE_EXTENSIONS: &[&str] = &[
    ".7z", ".arj", ".bz2", ".cab", ".gz", ".lz4", ".lzma", ".rar", ".tar", ".tbz", ".tbz2", ".tgz",
    ".txz", ".xz", ".zip", ".zst",
];

/// 扩展名 → 分类。
///
/// `extension` 应当是小写、含前导点的形式（`scanner::walk` 的产出）；
/// 无扩展名传空串。传入大写或带空白的值不会被"聪明地"修正——
/// 宁可归入「其他」，也不要在分类上做隐式猜测。
pub fn category_for(extension: &str) -> &'static str {
    if DOCUMENT_EXTENSIONS.contains(&extension) {
        DOCUMENT
    } else if IMAGE_EXTENSIONS.contains(&extension) {
        IMAGE
    } else if AUDIO_VIDEO_EXTENSIONS.contains(&extension) {
        AUDIO_VIDEO
    } else if ARCHIVE_EXTENSIONS.contains(&extension) {
        ARCHIVE
    } else {
        OTHER
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn each_spec_category_is_covered_by_a_known_extension() {
        assert_eq!(category_for(".pdf"), DOCUMENT);
        assert_eq!(category_for(".png"), IMAGE);
        assert_eq!(category_for(".mp4"), AUDIO_VIDEO);
        assert_eq!(category_for(".zip"), ARCHIVE);
        assert_eq!(category_for(".xyz"), OTHER);
    }

    #[test]
    fn unknown_and_missing_extensions_fall_into_other() {
        for extension in ["", ".", ".whatever", ".tar.gz", ".PDF"] {
            assert_eq!(
                category_for(extension),
                OTHER,
                "{extension:?} 必须进「其他」，不做隐式归一化"
            );
        }
    }

    #[test]
    fn no_extension_appears_in_two_categories() {
        // 一条扩展名同时出现在两张表里时，分类结果会取决于判断顺序——
        // 这种缺陷在功能上"能用"，但行为不可解释。用集合检查把它挡在门口。
        let mut seen: HashSet<&str> = HashSet::new();
        for table in [
            DOCUMENT_EXTENSIONS,
            IMAGE_EXTENSIONS,
            AUDIO_VIDEO_EXTENSIONS,
            ARCHIVE_EXTENSIONS,
        ] {
            for extension in table {
                assert!(seen.insert(extension), "{extension} 出现在多张映射表里");
            }
        }
    }

    #[test]
    fn the_mapping_table_is_well_formed() {
        for table in [
            DOCUMENT_EXTENSIONS,
            IMAGE_EXTENSIONS,
            AUDIO_VIDEO_EXTENSIONS,
            ARCHIVE_EXTENSIONS,
        ] {
            for extension in table {
                assert!(
                    extension.starts_with('.'),
                    "{extension} 缺少前导点，永远匹配不到 walk 的产出"
                );
                assert_eq!(
                    *extension,
                    extension.to_lowercase(),
                    "{extension} 不是小写，永远匹配不到 walk 的产出"
                );
                assert!(extension.len() > 1, "{extension} 是空扩展名");
            }
        }
    }

    #[test]
    fn the_mapping_table_is_sorted_for_reviewable_diffs() {
        // 排序本身不影响正确性，但能让「新增一个扩展名」在 diff 里一眼可见，
        // 避免同一张表里出现两处相近但不同的写法。
        for table in [
            DOCUMENT_EXTENSIONS,
            IMAGE_EXTENSIONS,
            AUDIO_VIDEO_EXTENSIONS,
            ARCHIVE_EXTENSIONS,
        ] {
            let mut sorted = table.to_vec();
            sorted.sort_unstable();
            assert_eq!(sorted, table.to_vec(), "映射表必须保持字典序");
        }
    }

    #[test]
    fn typescript_source_is_not_mistaken_for_a_video_stream() {
        // `.ts` 被刻意排除：源码不该被搬进「音视频」
        assert_eq!(category_for(".ts"), OTHER);
    }
}
