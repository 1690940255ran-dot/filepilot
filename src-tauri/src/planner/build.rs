//! 计划构建（规格 7.1、T04）。
//!
//! 输入：一次扫描的 `FileRecord` + 一批建议（`Proposal`）。
//! 输出：一份确定性的 `Plan`——相同输入必然得到**相同的目标方案**。
//!
//! ## 处理顺序（顺序本身就是行为的一部分）
//!
//! 1. 只保留可整理的记录（`skip_code` 为 `None`）；
//! 2. 把所有源路径登记为「已占用」（规格 7.1 第 9 条，防止循环交换）；
//! 3. 建议**按源相对路径稳定排序**（规格 7.1 第 7 条：冲突候选按这个顺序分配）；
//! 4. 逐条：校验分类与文件名 → 判断 noop / 仅改大小写 → **先算完整指纹** →
//!    再分配目标名；
//! 5. 任一步失败：这一项标记为**不可选**（`selected = false`，动作退化为 noop），
//!    并附一条 `Warning` 说明原因；其余项照常进入计划。
//!
//! 「先算指纹再分配名字」是有意的：指纹失败时不该占用一个目标名，
//! 否则其他项会白白让出 `名称 (2)`，预览里出现一批解释不通的空洞。
//!
//! ## 严重度的约定（与 `planner::validate` 共用）
//!
//! - 本模块的失败项是 `Warning`：**这一项不能执行**，不影响其他项；
//! - 校验阶段对**选中项**的问题是 `Block`：选中了却不能执行；
//! - 全局问题是 `Block`。
//!
//! 这样前端只需要一条规则判断「能不能进入执行」：是否存在 `Block`。

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use uuid::Uuid;

use crate::domain::errors::{codes, AppError};
use crate::domain::time::civil_from_unix_ns;
use crate::domain::types::{
    FileRecord, Issue, Mode, Plan, PlanAction, PlanItem, PlanItemOrigin, PlanStatus, Proposal,
    RelPath, Risk,
};
use crate::planner::naming::{self, TargetLedger};
use crate::rules::{self, RuleKind};
use crate::safety::path::validate_component;
use crate::safety::root::ApprovedRoot;

/// 分类目录的最大层数。规格 7.1 第 6 条：`category` 至多两层。
pub const MAX_CATEGORY_DEPTH: usize = 2;

/// 理由的最大长度。规格 2.2：理由最多 120 个中文字符。
const MAX_REASON_CHARS: usize = 120;

/// 建议指向的文件在扫描阶段就被跳过了（例如未完成的下载）。
pub const CODE_SKIPPED_AT_SCAN: &str = "SKIPPED_AT_SCAN";
/// 同一文件出现了多条建议。
pub const CODE_DUPLICATE_PROPOSAL: &str = "DUPLICATE_PROPOSAL";
/// 目标与当前位置相同，无需移动。
pub const CODE_NOOP: &str = "NOOP";
/// 模型对这一项的把握不够，默认不选中（规格 4.2）。
pub const CODE_LOW_CONFIDENCE: &str = "LOW_CONFIDENCE";

/// 规格 4.2 的默认选中门槛。
///
/// 它是「默认」而不是「能不能做」：低于它仍然会出现在预览里、理由也写清楚，
/// 用户想勾就能勾。**把它当成「模型说准不准」来展示是错的**——规格明确说
/// confidence 只是排序信号，不是准确率。
const DEFAULT_SELECT_THRESHOLD: f64 = 0.80;

/// 这一项该不该**默认不选中**；返回理由。
///
/// 规格 4.2：「低于 0.80、**依据不足**、解析失败或存在冲突的行默认不选中」。
///
/// 这里管前两条。解析失败在构建时就走不到 `Planned::Move`（没有正文的
/// 文件不会有建议），冲突要等 `validate_plan`——那一层才知道目标位置上
/// 有没有东西。
fn withheld_reason(proposal: &crate::domain::types::Proposal, mode: Mode) -> Option<String> {
    if proposal.confidence < DEFAULT_SELECT_THRESHOLD {
        return Some(format!(
            "模型对这项的把握是 {:.0}%，低于 {:.0}%",
            proposal.confidence * 100.0,
            DEFAULT_SELECT_THRESHOLD * 100.0
        ));
    }

    // 「依据不足」只对 **AI 模式**成立。
    //
    // 规则模式按扩展名分类，它本来就没有、也不需要正文引用——用同一条
    // 规则去要求它，结果是**规则模式的每一项都不被选中**：界面看起来
    // 完全正常，只是没有任何东西会被整理。这个 bug 是「依据不足」这四个字
    // 直接照搬到两处才出现的。
    if mode != Mode::Rules && proposal.evidence_locator.is_none() {
        return Some("模型没有给出依据".to_owned());
    }

    None
}
/// v0.1 不支持仅改变大小写的重命名（规格 7.1 第 9 条）。
pub const CODE_CASE_ONLY_RENAME: &str = "CASE_ONLY_RENAME";

/// 构建计划所需的输入。
///
/// `created_at` 由调用方注入而不是在内部取 `SystemTime::now()`：
/// 时间是可观测输出的一部分，测试需要能固定它。
pub struct PlanRequest<'a> {
    pub root: &'a ApprovedRoot,
    pub scan_id: &'a str,
    pub mode: Mode,
    pub records: &'a [FileRecord],
    pub proposals: Vec<Proposal>,
    pub created_at: SystemTime,
}

/// 构建结果。
///
/// T05 起它是 `create_plan` 的返回值，因此成为契约类型：
/// 前端要同时拿到 `plan`（渲染表格）与 `issues`（说明哪些项不可选、为什么）。
/// **不派生 `Deserialize`**：它只由后端产出，前端不该有能力提交一个 PlanBuild 回来。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct PlanBuild {
    pub plan: Plan,
    /// 构建期间发现的问题。`Warning` 表示「这一项不可选，原因如下」。
    pub issues: Vec<Issue>,
}

/// 由规则生成一份计划。
///
/// 这是规格 5.2 `create_plan(ruleKind)` 的实现入口：规则模式不联网、
/// 不做内容提取，断网与无模型时都能走到这里。
pub fn build_rule_plan(
    root: &ApprovedRoot,
    scan_id: &str,
    kind: RuleKind,
    records: &[FileRecord],
    created_at: SystemTime,
) -> Result<PlanBuild, AppError> {
    let outcome = rules::suggest(kind, records);

    let mut build = build_plan(&PlanRequest {
        root,
        scan_id,
        mode: Mode::Rules,
        records,
        proposals: outcome.proposals,
        created_at,
    })?;

    // 规则阶段自己的失败（例如时间戳不可表示）也要出现在预览里。
    for failure in outcome.failures {
        build.issues.push(Issue {
            code: failure.error.code.clone(),
            severity: Risk::Warning,
            item_id: None,
            message: format!(
                "{}：{}",
                failure.relative_path.join("\\"),
                failure.error.message
            ),
        });
    }

    Ok(build)
}

/// 由一批建议构建计划。
pub fn build_plan(request: &PlanRequest<'_>) -> Result<PlanBuild, AppError> {
    let usable: HashMap<&str, &FileRecord> = request
        .records
        .iter()
        .filter(|record| record.skip_code.is_none())
        .map(|record| (record.id.as_str(), record))
        .collect();

    // 全部记录（含被跳过的）：用来把「这个 id 根本不属于本次扫描」与
    // 「这个文件被扫描跳过了」分开报告——两者的用户动作完全不同。
    let all: HashMap<&str, &FileRecord> = request
        .records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();

    let mut ledger = TargetLedger::new(request.root);
    for record in usable.values() {
        ledger.reserve_source(&record.relative_path);
    }

    // 稳定排序：按源相对路径。规格 7.1 第 7 条要求冲突候选按这个顺序分配，
    // 因此「谁拿到 名称 (2)」是可复现的，不取决于建议到达的顺序。
    let mut proposals: Vec<&Proposal> = request.proposals.iter().collect();
    proposals.sort_by(|left, right| {
        let left_path = usable.get(left.file_id.as_str()).map(|r| &r.relative_path);
        let right_path = usable.get(right.file_id.as_str()).map(|r| &r.relative_path);
        left_path
            .cmp(&right_path)
            .then_with(|| left.file_id.cmp(&right.file_id))
    });

    let origin = origin_of(request.mode);
    let mut items = Vec::with_capacity(proposals.len());
    let mut issues = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();

    for proposal in proposals {
        let Some(record) = usable.get(proposal.file_id.as_str()) else {
            // 两种情况必须分开说：一种是文件确实被扫描跳过了（用户去处理那个文件），
            // 另一种是这个 id 根本不属于本次扫描（建议本身有问题）。
            // 把它们混成一句「文件不存在」会让用户以为扫描漏了文件。
            match all.get(proposal.file_id.as_str()) {
                Some(skipped) => issues.push(Issue {
                    code: CODE_SKIPPED_AT_SCAN.to_owned(),
                    severity: Risk::Warning,
                    item_id: None,
                    message: format!(
                        "{} 在扫描阶段已被跳过（{}），不参与整理计划",
                        skipped.relative_path.join("\\"),
                        skipped.skip_code.as_deref().unwrap_or("原因未记录")
                    ),
                }),
                None => issues.push(Issue {
                    code: codes::MODEL_INVALID_OUTPUT.to_owned(),
                    severity: Risk::Warning,
                    item_id: None,
                    message: format!(
                        "建议引用了本次扫描中不存在的文件 id {}，已忽略",
                        proposal.file_id
                    ),
                }),
            }
            continue;
        };

        if !seen.insert(proposal.file_id.as_str()) {
            issues.push(item_warning(
                &record.id,
                CODE_DUPLICATE_PROPOSAL,
                format!(
                    "{} 有多条建议，只采用第一条",
                    record.relative_path.join("\\")
                ),
            ));
            continue;
        }

        let (item, mut item_issues) = plan_one(request, &mut ledger, record, proposal, origin);
        issues.append(&mut item_issues);
        items.push(item);
    }

    let plan = Plan {
        id: Uuid::new_v4().to_string(),
        root_id: request.root.id().to_string(),
        scan_id: request.scan_id.to_owned(),
        // 新建计划从 1 开始。任何编辑都会 +1 并使旧校验与旧确认失效（T05）。
        revision: 1,
        mode: request.mode,
        status: PlanStatus::Draft,
        items,
        created_at: rfc3339(request.created_at)?,
    };

    Ok(PlanBuild { plan, issues })
}

/// 处理单条建议，返回一个计划项与它自己的问题。
///
/// 无论成功还是失败都返回一个计划项：规格 2.2 要求预览逐行可见，
/// 「这个文件为什么没被整理」必须能显示在它自己那一行上。
fn plan_one(
    request: &PlanRequest<'_>,
    ledger: &mut TargetLedger<'_>,
    record: &FileRecord,
    proposal: &Proposal,
    origin: PlanItemOrigin,
) -> (PlanItem, Vec<Issue>) {
    let source = &record.relative_path;
    let file_name = source.last().map(String::as_str).unwrap_or_default();
    let (_source_stem, source_extension) = crate::safety::path::split_file_name(file_name);

    // 先取 id：问题与计划项必须引用**同一个** itemId，
    // 否则界面无法把「为什么不可选」贴到对应那一行上。
    let item_id = Uuid::new_v4().to_string();

    let outcome = plan_target(request, ledger, record, proposal, source_extension);

    match outcome {
        Ok(Planned::Noop) => (
            PlanItem {
                id: item_id.clone(),
                file_id: record.id.clone(),
                source: source.clone(),
                target: source.clone(),
                action: PlanAction::Noop,
                // 规格 6.3：目标就是现有路径 → noop、不可选、不进入执行。
                selected: false,
                origin,
                reason: "目标位置与当前位置相同，无需移动".to_owned(),
                expected: record.fingerprint.clone(),
            },
            vec![item_info(
                &item_id,
                CODE_NOOP,
                "目标与当前位置相同，无需移动".to_owned(),
            )],
        ),
        Ok(Planned::Move { target, expected }) => {
            // 规格 4.2：「模型自报 confidence 只是排序信号，不显示为
            // 『准确率』。**低于 0.80、依据不足、解析失败或存在冲突的行
            // 默认不选中**；较高分也必须由用户最终确认。」
            //
            // 这里管前两条。后两条在别处：解析失败在构建时就走不到
            // `Planned::Move`，冲突要等 `validate_plan`。
            let withheld = withheld_reason(proposal, request.mode);

            (
                PlanItem {
                    id: item_id.clone(),
                    file_id: record.id.clone(),
                    source: source.clone(),
                    target,
                    action: PlanAction::Move,
                    selected: withheld.is_none(),
                    origin,
                    // 不可选时把**为什么**写进理由里：用户在预览页看到这一行
                    // 没被勾上，得能一眼看出是他自己的设置、还是模型的把握不够。
                    reason: match &withheld {
                        Some(why) => truncate_reason(format!("默认不选中：{why}")),
                        None => truncate_reason(proposal.reason.clone()),
                    },
                    expected,
                },
                withheld
                    .map(|why| {
                        vec![item_info(
                            &item_id,
                            CODE_LOW_CONFIDENCE,
                            format!("{why}。确认无误后可以手动勾选。"),
                        )]
                    })
                    .unwrap_or_default(),
            )
        }
        Err(error) => (
            PlanItem {
                id: item_id.clone(),
                file_id: record.id.clone(),
                source: source.clone(),
                // 不可选项保持原位：既不改名也不搬动。
                target: source.clone(),
                action: PlanAction::Noop,
                selected: false,
                origin,
                reason: truncate_reason(format!("不可选：{}", error.message)),
                // 只带初扫指纹（sha256 为 None）。它不可执行，也不参与摘要，
                // 因此不需要为了「看起来完整」去补一个没算过的哈希。
                expected: record.fingerprint.clone(),
            },
            vec![item_warning(&item_id, &error.code, error.message)],
        ),
    }
}

/// 单条建议的规划结果。
enum Planned {
    /// 目标与当前位置相同，无需移动。
    Noop,
    Move {
        target: RelPath,
        expected: crate::domain::types::Fingerprint,
    },
}

fn plan_target(
    request: &PlanRequest<'_>,
    ledger: &mut TargetLedger<'_>,
    record: &FileRecord,
    proposal: &Proposal,
    source_extension: &str,
) -> Result<Planned, AppError> {
    let source = &record.relative_path;

    validate_category(&proposal.category)?;

    // 建议的 stem 为空说明建议不完整。**不**退回原名——那是替模型猜意图。
    let stem = proposal.stem.as_str();
    if stem.is_empty() {
        return Err(AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            "建议没有给出文件名主体，无法确定目标名称",
        ));
    }
    validate_component(stem)?;

    let mut desired: RelPath = proposal.category.clone();
    desired.push(format!("{stem}{source_extension}"));

    if desired == *source {
        return Ok(Planned::Noop);
    }

    // 规格 7.1 第 9 条：不支持大小写单独重命名。这类「目标」在 Windows 上
    // 就是同一个名字，执行器无法可靠区分，v0.1 直接拒绝而不是赌它能成立。
    if naming::comparison_key(&desired) == naming::comparison_key(source) {
        return Err(AppError::new(
            CODE_CASE_ONLY_RENAME,
            format!(
                "v0.1 不支持仅改变大小写的重命名（{} → {}）",
                source.join("\\"),
                desired.join("\\")
            ),
        ));
    }

    // 规格 T04：计划进入可选状态前必须绑定**完整**指纹。
    // 顺序上先算指纹再分配名字，失败时不占用目标名。
    let source_path = request.root.resolve_existing_within(source)?;
    let expected =
        crate::scanner::snapshot::fingerprint(&source_path, request.root.volume_id(), true)?;

    let target = ledger.allocate(&proposal.category, stem, source_extension, source)?;

    Ok(Planned::Move { target, expected })
}

/// 分类目录校验：至多两层，每层都是合法的 Windows 目录名。
///
/// 允许空数组：它的含义是「放在根目录下」，是一个合法意图
/// （此时若主体名也不变，就会得到 noop）。
fn validate_category(category: &[String]) -> Result<(), AppError> {
    if category.len() > MAX_CATEGORY_DEPTH {
        return Err(AppError::new(
            codes::INVALID_PATH,
            format!(
                "分类目录最多 {MAX_CATEGORY_DEPTH} 层，收到 {} 层",
                category.len()
            ),
        ));
    }
    for component in category {
        validate_component(component)?;
    }
    Ok(())
}

fn origin_of(mode: Mode) -> PlanItemOrigin {
    match mode {
        Mode::Rules => PlanItemOrigin::Rule,
        Mode::AiLocal | Mode::AiCloud => PlanItemOrigin::Ai,
    }
}

/// 把 `SystemTime` 转成规格 5.1 要求的 UTC RFC3339 字符串。
fn rfc3339(time: SystemTime) -> Result<String, AppError> {
    let ns = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i128,
        Err(error) => -(error.duration().as_nanos() as i128),
    };
    civil_from_unix_ns(ns)
        .map(|value| value.to_rfc3339_utc())
        .ok_or_else(|| AppError::new(codes::INVALID_TIMESTAMP, "计划创建时间超出可表示的日期范围"))
}

/// 理由文案截断。规格 2.2：最多 120 个中文字符。
///
/// 用**字符数**而不是字节数：中文字符在 UTF-8 里占 3 字节，按字节截断会切碎字符。
fn truncate_reason(reason: String) -> String {
    if reason.chars().count() <= MAX_REASON_CHARS {
        return reason;
    }
    let mut truncated: String = reason.chars().take(MAX_REASON_CHARS - 1).collect();
    truncated.push('…');
    truncated
}

fn item_warning(item_id: &str, code: &str, message: String) -> Issue {
    Issue {
        code: code.to_owned(),
        severity: Risk::Warning,
        item_id: Some(item_id.to_owned()),
        message,
    }
}

fn item_info(item_id: &str, code: &str, message: String) -> Issue {
    Issue {
        code: code.to_owned(),
        severity: Risk::Info,
        item_id: Some(item_id.to_owned()),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{ExtractionStatus, Fingerprint};

    fn record(id: &str, name: &str, extension: &str) -> FileRecord {
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
                modified_ns: "0".to_owned(),
                sha256: None,
            },
            extraction_status: ExtractionStatus::Pending,
            skip_code: None,
        }
    }

    fn proposal(confidence: f64, evidence: Option<&str>) -> crate::domain::types::Proposal {
        crate::domain::types::Proposal {
            file_id: "f1".to_owned(),
            category: vec!["学习".to_owned()],
            stem: "资料".to_owned(),
            reason: "按内容归类".to_owned(),
            confidence,
            evidence_locator: evidence.map(str::to_owned),
        }
    }

    #[test]
    fn a_low_confidence_suggestion_is_not_selected_by_default() {
        // 规格 4.2：「低于 0.80……的行默认不选中」。
        assert!(
            withheld_reason(&proposal(0.79, Some("chars:0-10")), Mode::AiLocal).is_some(),
            "0.79 应当不选中"
        );
        assert!(withheld_reason(&proposal(0.5, Some("chars:0-10")), Mode::AiLocal).is_some());
        // 边界：正好 0.80 是**选中**的——规格说的是「低于 0.80」。
        assert!(
            withheld_reason(&proposal(0.80, Some("chars:0-10")), Mode::AiLocal).is_none(),
            "0.80 不该被拦"
        );
        assert!(withheld_reason(&proposal(0.95, Some("chars:0-10")), Mode::AiLocal).is_none());
    }

    #[test]
    fn an_ai_suggestion_without_evidence_is_not_selected_by_default() {
        let reason = withheld_reason(&proposal(0.9, None), Mode::AiCloud);
        assert!(reason.is_some(), "AI 模式没给出处 → 依据不足");
        assert!(reason.expect("有理由").contains("依据"));
    }

    #[test]
    fn a_rules_suggestion_needs_no_evidence() {
        // **规则模式按扩展名分类**，它本来就没有、也不需要正文引用。
        //
        // 用同一条「依据不足」去要求它，结果是**规则模式的每一项都不被
        // 选中**——界面看起来完全正常，只是没有任何东西会被整理。
        // 这条测试就是守着这个：它不报错，只是什么都不发生。
        assert!(
            withheld_reason(&proposal(1.0, None), Mode::Rules).is_none(),
            "规则模式不需要正文引用"
        );
        assert!(withheld_reason(&proposal(1.0, Some("chars:0-10")), Mode::Rules).is_none());
        // 但低把握仍然不选中：规则给出的建议也可能带低分。
        assert!(withheld_reason(&proposal(0.3, None), Mode::Rules).is_some());
    }

    #[test]
    fn category_validation_enforces_the_two_level_limit() {
        assert!(validate_category(&[]).is_ok(), "空分类表示放在根目录下");
        assert!(validate_category(&["学习".to_owned()]).is_ok());
        assert!(validate_category(&["学习".to_owned(), "数学".to_owned()]).is_ok());
        assert_eq!(
            validate_category(&["a".to_owned(), "b".to_owned(), "c".to_owned()])
                .unwrap_err()
                .code,
            codes::INVALID_PATH
        );
    }

    #[test]
    fn category_components_are_validated_as_windows_names() {
        assert_eq!(
            validate_category(&["..".to_owned()]).unwrap_err().code,
            codes::INVALID_PATH
        );
        assert_eq!(
            validate_category(&["CON".to_owned()]).unwrap_err().code,
            codes::RESERVED_NAME
        );
    }

    #[test]
    fn reasons_are_truncated_by_characters_not_bytes() {
        let long = "中".repeat(200);
        let truncated = truncate_reason(long);
        assert_eq!(truncated.chars().count(), MAX_REASON_CHARS);
        assert!(truncated.ends_with('…'));
        assert!(truncated.chars().all(|c| c == '中' || c == '…'));
    }

    #[test]
    fn short_reasons_are_left_alone() {
        assert_eq!(
            truncate_reason("按文件类型分类".to_owned()),
            "按文件类型分类"
        );
    }

    #[test]
    fn origin_follows_the_mode() {
        assert_eq!(origin_of(Mode::Rules), PlanItemOrigin::Rule);
        assert_eq!(origin_of(Mode::AiLocal), PlanItemOrigin::Ai);
        assert_eq!(origin_of(Mode::AiCloud), PlanItemOrigin::Ai);
    }

    #[test]
    fn rfc3339_conversion_matches_the_known_epoch() {
        assert_eq!(
            rfc3339(SystemTime::UNIX_EPOCH).expect("纪元应可表示"),
            "1970-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn the_record_helper_has_no_skip_code() {
        assert!(record("a", "a.txt", ".txt").skip_code.is_none());
    }
}
