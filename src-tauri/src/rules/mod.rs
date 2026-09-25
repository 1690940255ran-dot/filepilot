//! 规则模式（规格 6.3）。
//!
//! 规则模式是产品的**离线底座**：未配置模型、断网或模型失败时，
//! 用户仍然要能完成一次完整的整理。因此本模块有三条硬约束：
//!
//! 1. **不联网、不做内容提取**——只读 `FileRecord`（扩展名与修改时间）；
//! 2. **默认保留原文件名**——只决定「放进哪个目录」；
//! 3. **确定性**——同一个输入永远得到同一批建议，不依赖目录枚举顺序、时间或随机数。
//!
//! 建议以 `Proposal` 表达（规格 5.1 的公共结构），这样规则模式与 AI 模式
//! 走的是同一条下游管道：规划器不需要知道建议是谁产生的。

pub mod by_month;
pub mod by_type;

use serde::{Deserialize, Serialize};

use crate::domain::errors::AppError;
use crate::domain::types::{FileRecord, Proposal};

/// 规则种类。规格 5.2：`create_plan` 的 `ruleKind` 取值 `byType` / `byMonth`。
///
/// 这里只做 `serde` 而不生成 TypeScript/JSON Schema：T04 还没有接通
/// `create_plan` 命令，为一个当前无人消费的类型生成契约定义
/// 只会让 `contracts:check` 的比对结果与实际用法脱节。
/// T05 起它成为**契约类型**：前端要在界面上让用户选按什么规则整理，
/// 因此和 、 一样由 Rust 生成 TS 类型与 JSON Schema，
/// 而不是在前端另写一份（那会变成第二套真源）。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum RuleKind {
    /// 按扩展名分类：文档 / 图片 / 音视频 / 压缩包 / 其他。
    ByType,
    /// 按**修改**月份分类：`YYYY-MM`。
    ByMonth,
}

impl RuleKind {
    /// 规格 5.2 的线上取值。
    pub fn as_str(self) -> &'static str {
        match self {
            RuleKind::ByType => "byType",
            RuleKind::ByMonth => "byMonth",
        }
    }

    /// 解析线上取值。非法值返回 `None`，由调用方转成结构化错误。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "byType" => Some(RuleKind::ByType),
            "byMonth" => Some(RuleKind::ByMonth),
            _ => None,
        }
    }
}

/// 一条规则无法处理的文件，以及**具体原因**。
///
/// 之所以把失败单独收集而不是丢掉：界面必须能回答「这个文件为什么没被整理」。
/// 规格 2.3：解析失败要保留元信息并说明原因，不能假装成功。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFailure {
    pub file_id: String,
    /// 与文件相关的相对路径，供界面直接展示。
    pub relative_path: Vec<String>,
    pub error: AppError,
}

/// 规则模式的输出。
#[derive(Debug, Clone, Default)]
pub struct RuleOutcome {
    /// 建议。顺序与输入 `records` 的顺序一致（调用方已经按相对路径稳定排序）。
    pub proposals: Vec<Proposal>,
    pub failures: Vec<RuleFailure>,
}

/// 按规则种类为可整理的文件生成建议。
///
/// 被跳过的记录（`skip_code.is_some()`）不产生建议：它们在扫描阶段就已经
/// 被判定为不可整理，再给一条建议只会让预览出现无法执行的行。
pub fn suggest(kind: RuleKind, records: &[FileRecord]) -> RuleOutcome {
    let mut outcome = RuleOutcome::default();

    for record in records {
        if record.skip_code.is_some() {
            continue;
        }

        let category = match kind {
            RuleKind::ByType => Ok(by_type::category_for(&record.extension).to_owned()),
            RuleKind::ByMonth => by_month::category_for(&record.fingerprint.modified_ns),
        };

        match category {
            Ok(category) => outcome.proposals.push(Proposal {
                file_id: record.id.clone(),
                category: vec![category],
                // 规格 6.3：规则模式默认保留文件名。
                stem: stem_of(record).to_owned(),
                reason: reason_for(kind, record),
                // 规则是确定性的，不存在「把握不足」。这里不是模型自报的准确率，
                // 也不代表文件内容被理解过——只是说明这条建议完全由确定性规则得出。
                confidence: 1.0,
                // 规则模式不提取内容，因此没有可引用的证据定位。
                // 规格 6.4：evidenceLocator 必须对应真实提取结果，不许编造。
                evidence_locator: None,
            }),
            Err(error) => outcome.failures.push(RuleFailure {
                file_id: record.id.clone(),
                relative_path: record.relative_path.clone(),
                error,
            }),
        }
    }

    outcome
}

/// 从源相对路径取「不含扩展名的部分」，**大小写保持原样**。
///
/// 与 `scanner::walk` 的扩展名判定共用同一套边界（前导点不算扩展名），
/// 否则会出现「扫描认为无扩展名、规划器却切掉了 `.gitignore`」这种不一致。
pub fn stem_of(record: &FileRecord) -> &str {
    let name = record
        .relative_path
        .last()
        .map(String::as_str)
        .unwrap_or_default();
    crate::safety::path::split_file_name(name).0
}

/// 建议理由。规格 2.2：理由最多 120 个中文字符，并说明依据类型。
///
/// 规则模式的依据永远是**元信息**（扩展名或修改时间），绝不能写成
/// 「根据文档内容」——那会让用户以为文件被读过了。
fn reason_for(kind: RuleKind, record: &FileRecord) -> String {
    let name = record
        .relative_path
        .last()
        .map(String::as_str)
        .unwrap_or_default();
    match kind {
        RuleKind::ByType => format!("按文件类型分类（依据：文件名扩展名，来源 {name}）"),
        RuleKind::ByMonth => {
            let month = by_month::year_month_or_unknown(&record.fingerprint.modified_ns);
            format!("按文件最后修改月份分类（依据：修改时间 {month}）")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ExtractionStatus, Fingerprint};

    fn record(id: &str, name: &str, extension: &str, modified_ns: &str) -> FileRecord {
        FileRecord {
            id: id.to_owned(),
            scan_id: "scan".to_owned(),
            root_id: "root".to_owned(),
            relative_path: vec![name.to_owned()],
            extension: extension.to_owned(),
            fingerprint: Fingerprint {
                volume_id: "V".to_owned(),
                file_id: format!("F-{id}"),
                size: "1".to_owned(),
                modified_ns: modified_ns.to_owned(),
                sha256: None,
            },
            extraction_status: ExtractionStatus::Pending,
            skip_code: None,
        }
    }

    #[test]
    fn skipped_records_never_produce_suggestions() {
        let mut skipped = record("b", "b.tmp", ".tmp", "0");
        skipped.skip_code = Some("TEMP_DOWNLOAD".to_owned());

        let outcome = suggest(
            RuleKind::ByType,
            &[record("a", "a.pdf", ".pdf", "0"), skipped],
        );
        assert_eq!(outcome.proposals.len(), 1, "被跳过的记录不应产生建议");
        assert_eq!(outcome.proposals[0].file_id, "a");
    }

    #[test]
    fn rule_suggestions_keep_the_original_stem_and_have_no_evidence() {
        let outcome = suggest(RuleKind::ByType, &[record("a", "报告.PDF", ".pdf", "0")]);
        let proposal = &outcome.proposals[0];
        assert_eq!(proposal.stem, "报告", "扩展名不属于 stem");
        assert!(proposal.evidence_locator.is_none(), "规则模式没有内容证据");
        assert_eq!(proposal.category, vec!["文档".to_owned()]);
    }

    #[test]
    fn stem_of_handles_names_without_extension() {
        let r = record("a", ".gitignore", "", "0");
        assert_eq!(stem_of(&r), ".gitignore");
        let r = record("b", "archive.tar.gz", ".gz", "0");
        assert_eq!(stem_of(&r), "archive.tar");
    }

    #[test]
    fn rule_kind_round_trips_through_its_wire_names() {
        assert_eq!(RuleKind::ByType.as_str(), "byType");
        assert_eq!(RuleKind::parse("byMonth"), Some(RuleKind::ByMonth));
        assert_eq!(RuleKind::parse("bytype"), None, "线上取值区分大小写");
        assert_eq!(RuleKind::parse(""), None);
    }

    #[test]
    fn a_bad_timestamp_becomes_a_reported_failure_not_a_wrong_bucket() {
        // 修改时间不可表示时，byMonth 必须报错，而不是悄悄塞进「其他」或某个默认月份
        let outcome = suggest(
            RuleKind::ByMonth,
            &[record("a", "a.txt", ".txt", "999999999999999999999999")],
        );
        assert!(outcome.proposals.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].error.code,
            crate::domain::errors::codes::INVALID_TIMESTAMP
        );
    }
}
