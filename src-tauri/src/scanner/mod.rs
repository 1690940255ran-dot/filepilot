//! 扫描器。
//!
//! 规格 3.2：**只枚举批准范围、采集快照与跳过原因，不修改内容**。
//! 本模块没有任何写盘调用，这一点由 INV-01 的测试守着。

pub mod snapshot;
pub mod walk;

use uuid::Uuid;

use crate::domain::types::{ExtractionStatus, FileRecord, Fingerprint};
use crate::domain::AppError;
use crate::safety::root::ApprovedRoot;

pub use walk::{skip_codes, WalkOptions};

/// 扫描参数。默认值来自规格 6.1。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanOptions {
    pub recursive: bool,
    pub max_files: u32,
    pub max_depth: u32,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            max_files: 10_000,
            max_depth: 20,
        }
    }
}

impl From<ScanOptions> for WalkOptions {
    fn from(o: ScanOptions) -> Self {
        WalkOptions {
            recursive: o.recursive,
            max_files: o.max_files,
            max_depth: o.max_depth,
        }
    }
}

/// 一次扫描的结果。
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub scan_id: Uuid,
    pub root_id: Uuid,
    pub records: Vec<FileRecord>,
    /// 规格 6.1：达到数量/深度上限时为 true，界面必须要求用户缩小范围。
    pub truncated: bool,
}

impl ScanOutcome {
    /// 可参与整理的文件数（未跳过的）。
    pub fn usable_count(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.skip_code.is_none())
            .count()
    }

    /// 被跳过的文件数。
    pub fn skipped_count(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.skip_code.is_some())
            .count()
    }
}

/// 扫描一个已授权的根目录。
///
/// 全程只读：调用前后文件树的字节内容必须完全一致（规格 T02 的验收要求）。
pub fn scan(root: &ApprovedRoot, options: &ScanOptions) -> Result<ScanOutcome, AppError> {
    scan_cancellable(root, options, &|| false)?.ok_or_else(|| {
        AppError::new(
            crate::domain::errors::codes::INTERNAL,
            "不可取消扫描意外进入取消状态",
        )
    })
}

/// 可取消扫描。取消时丢弃不完整枚举结果，返回 `Ok(None)`。
pub fn scan_cancellable(
    root: &ApprovedRoot,
    options: &ScanOptions,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<ScanOutcome>, AppError> {
    // 每次扫描前重新核对根身份。规格 3.3：根可能在这期间被替换。
    root.verify_still_valid()?;

    let Some(walk_result) = walk::walk_cancellable(
        root.canonical(),
        root.volume_id(),
        &WalkOptions::from(*options),
        cancelled,
    )?
    else {
        return Ok(None);
    };

    let scan_id = Uuid::new_v4();
    let mut records = Vec::with_capacity(walk_result.entries.len());

    for entry in walk_result.entries {
        // 被跳过的条目同样保留一条记录（带 skip_code），这样界面能向用户
        // 解释「为什么这个文件没被处理」，而不是让它凭空消失。
        // 下游靠 `skip_code.is_some()` 区分可执行项，因此不会误用它的指纹。
        records.push(FileRecord {
            id: Uuid::new_v4().to_string(),
            scan_id: scan_id.to_string(),
            root_id: root.id().to_string(),
            relative_path: entry.relative_path,
            extension: entry.extension,
            fingerprint: Fingerprint {
                volume_id: root.volume_id().to_owned(),
                file_id: entry.file_id,
                size: entry.size.to_string(),
                modified_ns: entry.modified_ns,
                // 规格 6.1：初扫不算哈希，进入选中计划前才算
                sha256: None,
            },
            extraction_status: ExtractionStatus::Pending,
            skip_code: entry.skip_code.map(str::to_owned),
        });
    }

    Ok(Some(ScanOutcome {
        scan_id,
        root_id: root.id(),
        records,
        truncated: walk_result.truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_options_defaults_match_the_spec() {
        let d = ScanOptions::default();
        assert_eq!(d.max_files, 10_000, "规格 6.1 默认 10000 个文件");
        assert_eq!(d.max_depth, 20, "规格 6.1 默认深度 20");
        assert!(d.recursive);
    }

    #[test]
    fn counts_split_usable_and_skipped() {
        let outcome = ScanOutcome {
            scan_id: Uuid::new_v4(),
            root_id: Uuid::new_v4(),
            records: vec![],
            truncated: false,
        };
        assert_eq!(outcome.usable_count(), 0);
        assert_eq!(outcome.skipped_count(), 0);
    }

    #[test]
    fn cancellation_discards_partial_scan_results() {
        let tmp = tempfile::tempdir().expect("建临时目录");
        std::fs::write(tmp.path().join("a.txt"), b"x").expect("写文件");
        // 这里只验证 walk 层的取消语义；根授权的真实 NTFS 测试在集成测试中覆盖。
        let result =
            walk::walk_cancellable(tmp.path(), "unused", &WalkOptions::default(), &|| true)
                .expect("取消不应成为错误");
        assert!(result.is_none(), "取消后不能返回不完整扫描结果");
    }
}
