//! 计划校验与摘要（规格 7.2）。
//!
//! 校验回答两个不同的问题，代码里也分成两段：
//!
//! 1. **这份计划现在能不能执行**——逐项检查路径合法性、目标是否被抢占、
//!    源文件是否还是计划里记录的那个文件；
//! 2. **这份计划到底是什么**——对固定字段做规范序列化再取 SHA-256，
//!    得到 `digest`。用户确认的是这个摘要，执行器只认这个摘要。
//!
//! ## 与 T05 的边界
//!
//! 规格把 `validationToken` 的签发与消费放在预览确认阶段（T05，
//! `safety/confirmation.rs`）。本模块因此**只产出报告与摘要**，
//! `validation_token` 与 `expires_at` 一律为 `None`——一个「已经填好的假 token」
//! 会让后续阶段的接口看起来已经实现，实际上没有任何一次性语义。

use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::domain::errors::{codes, AppError};
use crate::domain::types::{Issue, Plan, PlanAction, PlanItem, PlanStatus, Risk, ValidationReport};
use crate::planner::naming::{self, comparison_key, TargetLedger};
use crate::safety::path::{validate_existing_relative_path, validate_relative_path};
use crate::safety::root::ApprovedRoot;

/// 摘要格式的版本前缀。
///
/// 摘要的输入串一旦改变，旧摘要就再也对不上旧计划。加版本前缀是为了让
/// 「格式变了」这件事在代码里显式可见，而不是靠比对结果变化去猜。
const DIGEST_FORMAT: &str = "filepilot.plan.digest.v1";

/// 计划选中项为空。规格 7.2：选中项不得为空，这是**全局**问题。
pub const CODE_EMPTY_SELECTION: &str = "EMPTY_SELECTION";
/// 一个 noop 项被选中了。noop 不进入执行（规格 6.3）。
pub const CODE_NOOP_SELECTED: &str = "NOOP_SELECTED";
/// 本批中有两项指向同一个目标。
pub const CODE_DUPLICATE_TARGET: &str = "DUPLICATE_TARGET";
/// 计划里有两条计划项共用同一个 itemId。
pub const CODE_DUPLICATE_ITEM_ID: &str = "DUPLICATE_ITEM_ID";
/// 两个选中项引用了同一个源文件（按路径或 fileId）。
pub const CODE_DUPLICATE_SOURCE: &str = "DUPLICATE_SOURCE";
/// 用户编辑目标时改变了扩展名；v0.1 禁止修改扩展名。
pub const CODE_EXTENSION_CHANGED: &str = "EXTENSION_CHANGED";
/// 目标目录当前不存在，执行时会逐级创建（提示，不阻断）。
pub const CODE_TARGET_DIR_WILL_BE_CREATED: &str = "TARGET_DIR_WILL_BE_CREATED";

/// 校验一份计划。
///
/// 只读：不改文件、不改计划本身。校验通过不等于已获授权——
/// 授权是用户确认与一次性令牌的事（T05）。
///
/// `executable_count` 的含义是「**当前状态下真的能执行的项数**」：
/// 存在任何全局阻断问题时它一定是 0（规格 7.2：「全局问题始终阻断」），
/// 因此前端只要看这一个数字就能判断能不能进入执行。
pub fn validate_plan(plan: &Plan, root: &ApprovedRoot) -> Result<ValidationReport, AppError> {
    let mut issues = Vec::new();
    check_globals(plan, root, &mut issues);
    let globally_blocked = issues
        .iter()
        .any(|issue| issue.severity == Risk::Block && issue.item_id.is_none());

    // 目标重复是**批次一致性**问题：两项指向同一个目标时无法判断谁该让路，
    // 因此涉及的所有项一律阻断，而不是「先到先得」。
    let duplicated = duplicated_targets(plan);
    let (duplicated_sources, duplicated_file_ids) = duplicated_sources(plan);

    // 「目标现在是否已被占用」与规划阶段用的是**同一份**判断逻辑
    // （`TargetLedger`），避免两处实现慢慢分叉。
    let mut ledger = TargetLedger::new(root);

    let mut executable_count = 0u32;
    for item in plan.items.iter().filter(|item| item.selected) {
        let before = count_item_blocks(&issues, &item.id);
        check_item(
            root,
            &mut ledger,
            item,
            &duplicated,
            &duplicated_sources,
            &duplicated_file_ids,
            &mut issues,
        );
        if count_item_blocks(&issues, &item.id) == before {
            executable_count += 1;
        }
    }

    if globally_blocked {
        executable_count = 0;
    }

    Ok(ValidationReport {
        plan_id: plan.id.clone(),
        revision: plan.revision,
        digest: compute_digest(plan, root),
        executable_count,
        issues,
        // T05 负责签发；这里**不**制造一个看起来可用的假令牌。
        validation_token: None,
        expires_at: None,
    })
}

/// 被两个以上选中项同时指向的目标（比较键）。
fn duplicated_targets(plan: &Plan) -> std::collections::HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in plan.items.iter().filter(|item| item.selected) {
        *counts.entry(comparison_key(&item.target)).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(key, _)| key)
        .collect()
}

/// 被两个以上选中项引用的源路径 / fileId。
fn duplicated_sources(plan: &Plan) -> (HashSet<String>, HashSet<String>) {
    let mut paths: HashMap<String, usize> = HashMap::new();
    let mut file_ids: HashMap<String, usize> = HashMap::new();
    for item in plan.items.iter().filter(|item| item.selected) {
        *paths.entry(comparison_key(&item.source)).or_default() += 1;
        *file_ids.entry(item.file_id.clone()).or_default() += 1;
    }
    (
        paths
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect(),
        file_ids
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect(),
    )
}

fn count_item_blocks(issues: &[Issue], item_id: &str) -> usize {
    issues
        .iter()
        .filter(|issue| issue.item_id.as_deref() == Some(item_id) && issue.severity == Risk::Block)
        .count()
}

/// 全局问题。规格 7.2：全局问题始终阻断，不因为「大部分项没问题」而放行。
fn check_globals(plan: &Plan, root: &ApprovedRoot, issues: &mut Vec<Issue>) {
    if plan.root_id != root.id().to_string() {
        issues.push(global(
            codes::ROOT_CHANGED,
            "计划的根目录与当前授权的根目录不是同一个，请重新生成计划",
        ));
    }
    if plan.status != PlanStatus::Draft {
        issues.push(global(
            codes::STALE_PLAN,
            "这份计划已经进入执行流程，不能再次校验",
        ));
    }
    if plan.revision == 0 {
        issues.push(global(
            codes::STALE_PLAN,
            "计划版本无效（revision 从 1 开始）",
        ));
    }
    if let Err(error) = root.verify_still_valid() {
        issues.push(global(&error.code, &error.message));
    }
    if !plan.items.iter().any(|item| item.selected) {
        issues.push(global(
            CODE_EMPTY_SELECTION,
            "计划里没有任何选中的操作项，不能执行；请至少选择一项",
        ));
    }

    // itemId 是**问题归属**（`Issue.itemId`）与**摘要排序**的依据，执行阶段
    // 还会把它写进 `operations`，而那张表上有 `unique(runId, itemId)`。
    // 两条计划项共用一个 id 时，"这条问题属于哪一行"就无法回答，
    // 因此这里直接判为不可执行，而不是让它带着歧义继续往下走。
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for item in &plan.items {
        *seen.entry(item.id.as_str()).or_default() += 1;
    }
    if let Some((id, count)) = seen.iter().find(|(_, count)| **count > 1) {
        issues.push(global(
            CODE_DUPLICATE_ITEM_ID,
            &format!("计划里有 {count} 条计划项共用同一个 itemId（{id}），无法归属问题与执行记录"),
        ));
    }
}

/// 单项检查。
///
/// 规格 7.2：未选中的坏项可以保留在预览、不阻止有效选中项，因此这里只处理选中项。
/// 每一项的问题只作用于它自己——「这一项不能执行」不等于「整批不能执行」。
fn check_item(
    root: &ApprovedRoot,
    ledger: &mut TargetLedger<'_>,
    item: &PlanItem,
    duplicated: &std::collections::HashSet<String>,
    duplicated_sources: &HashSet<String>,
    duplicated_file_ids: &HashSet<String>,
    issues: &mut Vec<Issue>,
) {
    let block = |issues: &mut Vec<Issue>, code: &str, message: String| {
        issues.push(Issue {
            code: code.to_owned(),
            severity: Risk::Block,
            item_id: Some(item.id.clone()),
            message,
        });
    };

    // ---- 动作本身是否自洽 ----
    if item.action == PlanAction::Noop {
        block(
            issues,
            CODE_NOOP_SELECTED,
            "该项的目标与当前位置相同（noop），不应被选中执行".to_owned(),
        );
    }
    if item.source == item.target {
        block(
            issues,
            codes::INVALID_PATH,
            "源路径与目标路径相同，不需要移动".to_owned(),
        );
    }

    // ---- 路径合法性 ----
    if let Err(error) = validate_existing_relative_path(&item.source) {
        block(
            issues,
            &error.code,
            format!("源路径不合法：{}", error.message),
        );
    }
    if let Err(error) = validate_relative_path(&item.target) {
        block(
            issues,
            &error.code,
            format!("目标路径不合法：{}", error.message),
        );
    }
    let source_extension = item
        .source
        .last()
        .map(|name| crate::safety::path::split_file_name(name).1);
    let target_extension = item
        .target
        .last()
        .map(|name| crate::safety::path::split_file_name(name).1);
    if source_extension.is_some()
        && target_extension.is_some()
        && source_extension != target_extension
    {
        block(
            issues,
            CODE_EXTENSION_CHANGED,
            "v0.1 不允许修改文件扩展名；请保留源扩展名（含大小写）".to_owned(),
        );
    }
    if let Err(error) = naming::check_total_path_length(root, &item.target) {
        block(issues, &error.code, error.message);
    }

    // ---- 指纹必须绑定内容哈希（规格 5.1） ----
    if item.expected.sha256.is_none() {
        block(
            issues,
            codes::SOURCE_CHANGED,
            "计划没有绑定文件内容哈希，拒绝进入执行".to_owned(),
        );
    }

    // ---- 目标侧 ----
    if let Some((_, dir)) = item.target.split_last() {
        match naming::check_target_parent_chain(root, dir) {
            Err(error) => block(issues, &error.code, error.message),
            Ok(()) => {
                if matches!(naming::observe(root, dir), Ok(naming::Observed::Missing)) {
                    issues.push(Issue {
                        code: CODE_TARGET_DIR_WILL_BE_CREATED.to_owned(),
                        severity: Risk::Info,
                        item_id: Some(item.id.clone()),
                        message: format!("目标目录 {} 尚不存在，执行时会创建", dir.join("\\")),
                    });
                }
            }
        }
    }

    if duplicated.contains(&comparison_key(&item.target)) {
        block(
            issues,
            CODE_DUPLICATE_TARGET,
            format!(
                "目标位置 {} 同时被本批的多个选中项指向，必须先解决冲突",
                item.target.join("\\")
            ),
        );
    }

    if duplicated_sources.contains(&comparison_key(&item.source))
        || duplicated_file_ids.contains(&item.file_id)
    {
        block(
            issues,
            CODE_DUPLICATE_SOURCE,
            format!(
                "源文件 {} 被多个选中项重复引用，同一批中只能执行一次",
                item.source.join("\\")
            ),
        );
    }

    // 目标当前是否已被占用。规格 7.1 第 8 条：确认之后才出现的冲突要报错停止，
    // **不自动另取名字**——那个新名字没有被用户确认过。
    //
    // 这里只关心「被占用」这一种结果；路径本身是否合法已在上面的
    // `validate_relative_path` 里单独报过，重复报会让预览出现两条同义问题。
    if matches!(ledger.target_exists_on_disk(&item.target), Ok(true)) {
        block(
            issues,
            codes::TARGET_EXISTS,
            format!("目标位置 {} 现在已存在同名项", item.target.join("\\")),
        );
    }

    // ---- 源侧：预览之后文件是否变过 ----
    if let Err(error) = verify_source_unchanged(root, item) {
        block(issues, &error.code, error.message);
    }
}

/// 核对源文件是否仍是计划里记录的那一个。
///
/// 只比对**身份、大小、修改时间**，不重算内容哈希：哈希在生成计划时算过、
/// 执行前还会再算一次（规格 8.2 的「重新计算并核对源身份/哈希」）。
/// 在校验阶段对 10,000 个文件重算哈希，代价与收益不成比例；而「预览后文件被改」
/// 这类问题里，大小或时间变化的占绝大多数，三者都不变却内容变了的，
/// 执行阶段的内容哈希一定会拦住。
fn verify_source_unchanged(root: &ApprovedRoot, item: &PlanItem) -> Result<(), AppError> {
    let path = root.resolve_existing_within(&item.source)?;
    let actual = crate::scanner::snapshot::fingerprint(&path, root.volume_id(), false)?;

    if actual.volume_id != item.expected.volume_id || actual.file_id != item.expected.file_id {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "源文件已被替换成另一个文件，请重新扫描后再生成计划",
        ));
    }
    if actual.size != item.expected.size || actual.modified_ns != item.expected.modified_ns {
        return Err(AppError::new(
            codes::SOURCE_CHANGED,
            "源文件在生成计划之后被修改过，请重新扫描后再生成计划",
        ));
    }
    Ok(())
}

fn global(code: &str, message: &str) -> Issue {
    Issue {
        code: code.to_owned(),
        severity: Risk::Block,
        item_id: None,
        message: message.to_owned(),
    }
}

/// 计算计划摘要。
///
/// 规格 7.2 规定了参与计算的字段。规范序列化用「长度前缀 + 内容 + 换行」：
/// 只用分隔符拼接时，`["a", "b"]` 与 `["a\u{1f}b"]` 这类输入可能拼出同一个字符串；
/// 长度前缀让每个字段的边界由内容自己决定，不存在歧义。
///
/// 长度按 **UTF-8 字节**计（内容本身是 UTF-8，用字节数最不容易算错）。
pub fn compute_digest(plan: &Plan, root: &ApprovedRoot) -> String {
    let canonical = canonical_form(
        plan,
        root.volume_id(),
        &root.identity().file_id_string(),
        &root.canonical().to_string_lossy().to_lowercase(),
    );

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 摘要的规范输入串。
///
/// 与 [`compute_digest`] 分开是为了能在单元测试里直接检查「哪些字段参与、
/// 顺序如何」，而不必在测试中构造一个真实的 [`ApprovedRoot`]。
fn canonical_form(
    plan: &Plan,
    root_volume_id: &str,
    root_file_id: &str,
    root_path: &str,
) -> String {
    let mut canonical = String::new();

    push_field(&mut canonical, DIGEST_FORMAT);
    push_field(&mut canonical, &plan.id);
    push_field(&mut canonical, &plan.revision.to_string());

    // root：卷身份 + 根的卷内文件身份 + 规范真实路径。
    // 加上文件身份是刻意的加强——同一路径上的目录被换成另一个目录时，
    // 只比路径的摘要不会变化。
    push_field(&mut canonical, root_volume_id);
    push_field(&mut canonical, root_file_id);
    push_field(&mut canonical, root_path);

    let mut selected: Vec<&PlanItem> = plan.items.iter().filter(|item| item.selected).collect();
    // 规格 7.2：选中项**按 itemId 排序**。排序让摘要与计划项在数组里的
    // 物理顺序无关——同一个计划被重新序列化一次不应该让用户的确认失效。
    selected.sort_by(|a, b| a.id.cmp(&b.id));

    for item in selected {
        push_field(&mut canonical, &item.id);
        push_field(
            &mut canonical,
            match item.action {
                PlanAction::Move => "move",
                PlanAction::Noop => "noop",
            },
        );
        push_field(&mut canonical, &item.source.join("\\"));
        push_field(&mut canonical, &item.target.join("\\"));
        push_field(&mut canonical, &item.expected.volume_id);
        push_field(&mut canonical, &item.expected.file_id);
        push_field(&mut canonical, &item.expected.size);
        push_field(&mut canonical, &item.expected.modified_ns);
        push_field(
            &mut canonical,
            item.expected.sha256.as_deref().unwrap_or(""),
        );
    }

    canonical
}

fn push_field(out: &mut String, value: &str) {
    out.push_str(&value.len().to_string());
    out.push(':');
    out.push_str(value);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Fingerprint, Mode, PlanItemOrigin, RelPath};

    fn item(id: &str, source: &[&str], target: &[&str], selected: bool) -> PlanItem {
        PlanItem {
            id: id.to_owned(),
            file_id: format!("file-{id}"),
            source: RelPath::from(source.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()),
            target: RelPath::from(target.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()),
            action: PlanAction::Move,
            selected,
            origin: PlanItemOrigin::Rule,
            reason: String::new(),
            expected: Fingerprint {
                volume_id: "V".to_owned(),
                file_id: format!("F-{id}"),
                size: "1".to_owned(),
                modified_ns: "0".to_owned(),
                sha256: Some("a".repeat(64)),
            },
        }
    }

    fn plan(items: Vec<PlanItem>) -> Plan {
        Plan {
            id: "plan-1".to_owned(),
            root_id: "root-1".to_owned(),
            scan_id: "scan-1".to_owned(),
            revision: 1,
            mode: Mode::Rules,
            status: PlanStatus::Draft,
            items,
            created_at: "2026-09-17T00:00:00.000Z".to_owned(),
        }
    }

    fn canonical(plan: &Plan) -> String {
        canonical_form(plan, "V", "ROOT-FID", "c:\\users\\me\\资料")
    }

    #[test]
    fn canonical_serialization_is_unambiguous_across_field_boundaries() {
        let mut a = String::new();
        push_field(&mut a, "ab");
        push_field(&mut a, "c");

        let mut b = String::new();
        push_field(&mut b, "a");
        push_field(&mut b, "bc");

        assert_ne!(a, b, "长度前缀必须让字段边界无歧义");
    }

    #[test]
    fn the_same_plan_always_produces_the_same_canonical_form() {
        let p = plan(vec![item("i1", &["a.txt"], &["文档", "a.txt"], true)]);
        assert_eq!(canonical(&p), canonical(&p));
    }

    #[test]
    fn changing_any_bound_field_changes_the_canonical_form() {
        let base = plan(vec![item("i1", &["a.txt"], &["文档", "a.txt"], true)]);

        let mut renamed = base.clone();
        renamed.items[0].target = RelPath::from(vec!["文档".to_owned(), "b.txt".to_owned()]);
        assert_ne!(canonical(&base), canonical(&renamed), "target 必须参与摘要");

        let mut reselected = base.clone();
        reselected.items[0].selected = false;
        assert_ne!(
            canonical(&base),
            canonical(&reselected),
            "选中集合必须参与摘要"
        );

        let mut revised = base.clone();
        revised.revision = 2;
        assert_ne!(
            canonical(&base),
            canonical(&revised),
            "revision 必须参与摘要"
        );

        let mut rehashed = base.clone();
        rehashed.items[0].expected.sha256 = Some("b".repeat(64));
        assert_ne!(
            canonical(&base),
            canonical(&rehashed),
            "expected 必须参与摘要"
        );

        let mut moved = base.clone();
        moved.items[0].action = PlanAction::Noop;
        assert_ne!(canonical(&base), canonical(&moved), "操作类型必须参与摘要");
    }

    #[test]
    fn changing_the_root_identity_changes_the_digest_input() {
        let p = plan(vec![item("i1", &["a.txt"], &["文档", "a.txt"], true)]);
        let same = canonical_form(&p, "V", "ROOT-FID", "c:\\users\\me\\资料");
        let replaced = canonical_form(&p, "V", "OTHER-FID", "c:\\users\\me\\资料");
        assert_ne!(same, replaced, "根目录被换成另一个目录时必须改变摘要输入");
    }

    #[test]
    fn item_order_in_the_vector_does_not_matter() {
        // 规格 7.2：按 itemId 排序。同一个计划换个数组顺序不该让确认失效。
        let first = plan(vec![
            item("i1", &["a.txt"], &["文档", "a.txt"], true),
            item("i2", &["b.txt"], &["文档", "b.txt"], true),
        ]);
        let second = plan(vec![
            item("i2", &["b.txt"], &["文档", "b.txt"], true),
            item("i1", &["a.txt"], &["文档", "a.txt"], true),
        ]);
        assert_eq!(canonical(&first), canonical(&second));
    }

    #[test]
    fn unselected_items_do_not_participate() {
        let with = plan(vec![
            item("i1", &["a.txt"], &["文档", "a.txt"], true),
            item("i2", &["b.txt"], &["文档", "b.txt"], false),
        ]);
        let without = plan(vec![item("i1", &["a.txt"], &["文档", "a.txt"], true)]);
        assert_eq!(
            canonical(&with),
            canonical(&without),
            "未选中项不属于用户确认的范围"
        );
    }
}
