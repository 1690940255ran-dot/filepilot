//! T05：计划生成、编辑与一次性确认的命令。
//!
//! 与 `commands.rs` 分开是因为这一组围绕**计划**这一件事，
//! 而 `commands.rs` 里的那些围绕扫描与会话。两组的失败模式与测试方式都不同。
//!
//! 三条贯穿始终的约束：
//! 1. 前端只提交 **id 与编辑内容**，路径、指纹、版本都由后端持有；
//! 2. 计划必须**落库**（规格 8.1），不存在「只在内存里的计划」；
//! 3. 确认令牌**只有后端**能签发与消费（规格 7.4）。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::types::{
    Plan, PlanItemEdit, PlanItemOrigin, PlanStatus, Risk, ValidationReport,
};
use crate::domain::{errors::codes, errors::AppError, IpcResult};
use crate::planner::{build_rule_plan, validate_plan as validate_plan_core, PlanBuild};
use crate::rules::RuleKind;
use crate::safety::confirmation::now_unix_ms;
use crate::safety::path::validate_relative_path;
use crate::storage::repositories::{load_plan, save_plan, set_plan_digest};

/// 从一次扫描生成整理计划。
///
/// 规格 5.2：`create_plan(ruleKind)` 与 `create_plan(analysisId)`。
///
/// 两条路只走一条：规则模式不联网、不做内容提取，断网与未配置模型时都能用；
/// AI 模式则把 `start_analysis` 落库的建议原样交给同一个规划器。
///
/// **两者都给或都不给都是错的**，所以这里收成 `Option` 并显式拒绝——
/// 与其猜「他到底想要哪个」，不如让他说清楚。猜错的代价是用户拿到一份
/// 来源不明却看起来正常的计划。
#[tauri::command]
pub fn create_plan(
    state: State<'_, AppState>,
    scan_id: String,
    rule_kind: Option<RuleKind>,
    analysis_id: Option<String>,
) -> IpcResult<PlanBuild> {
    // scanId -> 该次扫描的记录与所属根。前端只提交 id，路径由后端查表得到。
    let Some(records) = state.scan_records(&scan_id) else {
        return IpcResult::err(AppError::new(
            codes::SOURCE_MISSING,
            "找不到该扫描结果；请重新扫描",
        ));
    };
    let Some(root_id) = state.scan_root_id(&scan_id) else {
        return IpcResult::err(AppError::new(
            codes::SOURCE_MISSING,
            "该扫描没有关联的根目录",
        ));
    };
    let Some(root) = state.root(&root_id) else {
        return IpcResult::err(AppError::new(
            codes::ROOT_NOT_AUTHORIZED,
            "该根目录的授权已失效，请重新选择文件夹",
        ));
    };

    let build = match (rule_kind, analysis_id.as_deref()) {
        (Some(kind), None) => match build_rule_plan(
            &root,
            &scan_id,
            kind,
            &records,
            std::time::SystemTime::now(),
        ) {
            Ok(build) => build,
            Err(error) => return IpcResult::err(error),
        },
        (None, Some(analysis_id)) => {
            match plan_from_analysis(state.db(), analysis_id, &scan_id, &root, &records) {
                Ok(build) => build,
                Err(error) => return IpcResult::err(error),
            }
        }
        (Some(_), Some(_)) => {
            return IpcResult::err(AppError::new(
                codes::INVALID_ENDPOINT,
                "规则模式与 AI 模式只能选一个：请只给出 ruleKind 或 analysisId。",
            ))
        }
        (None, None) => {
            return IpcResult::err(AppError::new(
                codes::INVALID_ENDPOINT,
                "缺少生成计划的依据：请给出 ruleKind（规则）或 analysisId（AI）。",
            ))
        }
    };

    // 规格 8.1：计划必须落库。新建时 expectedRevision 为 None，
    // 计划已存在会被存储层拒绝（不会静默覆盖）。
    if let Err(error) = save_plan(state.db(), &build.plan, None) {
        return IpcResult::err(error);
    }

    IpcResult::ok(build)
}

/// 用一次已完成的分析结果生成计划。
///
/// ## 为什么要核对 `scanId`
///
/// 把 A 扫描的分析用在 B 上，会生成一份「建议来自 A、文件来自 B」的计划。
/// 那份计划**看起来完全正常**——有文件名、有目标路径、有理由——直到执行时
/// 才会发现对不上，而那时用户已经在确认对话框上点了「执行」。
///
/// 所以这里的三道核对（属于同一次扫描、已完成、能解析出来）都不是形式：
/// 它们各自挡住一种「看起来正常但其实错位」的计划。
pub fn plan_from_analysis(
    db: &crate::storage::db::Database,
    analysis_id: &str,
    scan_id: &str,
    root: &crate::safety::root::ApprovedRoot,
    records: &[crate::domain::types::FileRecord],
) -> Result<PlanBuild, AppError> {
    let row = crate::storage::repositories::load_analysis(db, analysis_id)?.ok_or_else(|| {
        AppError::new(codes::SOURCE_MISSING, "找不到这次分析的结果；请重新分析。")
    })?;

    if row.scan_id != scan_id {
        return Err(AppError::new(
            codes::STALE_PLAN,
            "这次分析针对的是另一次扫描结果。请对当前扫描重新分析。",
        ));
    }

    if row.status != "completed" {
        return Err(AppError::new(
            codes::STALE_PLAN,
            match row.status.as_str() {
                "running" => "这次分析还在进行中，请等它完成。".to_owned(),
                "failed" => "这次分析失败了，请重新分析。".to_owned(),
                other => format!("这次分析没有可用的结果（状态：{other}）。"),
            },
        ));
    }

    let proposals: Vec<crate::domain::types::Proposal> =
        serde_json::from_str(row.proposal_json.as_deref().unwrap_or("[]")).map_err(|_| {
            AppError::new(
                codes::INTERNAL,
                "分析结果读不出来（格式异常）。请重新分析。",
            )
        })?;

    // 库里存的是 camelCase 文本，转回枚举。读不出来说明记录被外部改过，
    // 那不该猜一个默认值继续——计划里的 `mode` 会因此说谎。
    let mode: crate::domain::types::Mode =
        serde_json::from_value(serde_json::Value::String(row.mode.clone())).map_err(|_| {
            AppError::new(
                codes::INTERNAL,
                format!("分析记录里的模式无法识别（{}）。", row.mode),
            )
        })?;

    crate::planner::build::build_plan(&crate::planner::build::PlanRequest {
        root,
        scan_id,
        mode,
        records,
        proposals,
        created_at: std::time::SystemTime::now(),
    })
}

/// 读取计划。查不到返回 `None`——与 `get_task` 同理，查询不到不是错误。
#[tauri::command]
pub fn get_plan(state: State<'_, AppState>, plan_id: String) -> IpcResult<Option<Plan>> {
    match load_plan(state.db(), &plan_id) {
        Ok(plan) => IpcResult::ok(plan),
        Err(error) => IpcResult::err(error),
    }
}

/// 提交计划编辑，使用乐观锁。
///
/// 规格 5.2：`update_plan(planId, expectedRevision, ops)`；冲突返回 `STALE_PLAN`。
///
/// 这里只做**结构性**校验（项是否存在、扩展名是否被改、组件是否合法），
/// 不做完整校验：「这一项现在会不会撞车」属于 `validate_plan` 的职责，
/// 用户需要能保存一个暂时还有冲突的中间状态。
#[tauri::command]
pub fn update_plan(
    state: State<'_, AppState>,
    plan_id: String,
    expected_revision: u32,
    edits: Vec<PlanItemEdit>,
) -> IpcResult<Plan> {
    let db = state.db();

    let mut plan = match load_plan(db, &plan_id) {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            return IpcResult::err(AppError::new(
                codes::STALE_PLAN,
                "计划不存在；它可能已被删除或属于上一次运行的应用会话",
            ))
        }
        Err(error) => return IpcResult::err(error),
    };

    // 乐观锁：前端看到的版本必须与库里的一致。
    // 不一致说明别处已经改过，直接拒绝，而不是「以最后一次写入为准」。
    if plan.revision != expected_revision {
        return IpcResult::err(AppError::new(
            codes::STALE_PLAN,
            format!(
                "计划已被修改（当前版本 {}，你基于版本 {}）；请重新预览",
                plan.revision, expected_revision
            ),
        ));
    }

    // 先把「每条编辑对应第几项」算出来，让索引只借用 `plan.items` 一小段作用域。
    // 直接在循环里既查索引又可变借用会被借用检查器拒绝（同一份 items
    // 不能同时存在不可变与可变借用）。
    let positions: Vec<usize> = {
        let index: std::collections::HashMap<&str, usize> = plan
            .items
            .iter()
            .enumerate()
            .map(|(position, item)| (item.id.as_str(), position))
            .collect();

        let mut resolved = Vec::with_capacity(edits.len());
        for edit in &edits {
            match index.get(edit.item_id.as_str()) {
                Some(&position) => resolved.push(position),
                None => {
                    return IpcResult::err(AppError::new(
                        codes::INVALID_PATH,
                        format!("编辑引用了计划中不存在的项：{}", edit.item_id),
                    ))
                }
            }
        }
        resolved
    };

    for (edit, position) in edits.iter().zip(positions) {
        let item = &mut plan.items[position];

        if let Some(selected) = edit.selected {
            item.selected = selected;
        }

        if let Some(target) = &edit.target {
            // 规格 7.1：v0.1 不允许改变扩展名。在这里挡一道是为了让错误尽早出现，
            // 而不是等用户点了「确认」才报。
            if extension_of(&item.source) != extension_of(target) {
                return IpcResult::err(AppError::new(codes::INVALID_PATH, "不能改变文件扩展名"));
            }
            if let Err(error) = validate_relative_path(target) {
                return IpcResult::err(error);
            }
            item.target = target.clone();
            item.origin = PlanItemOrigin::User;
        }
    }

    // 版本**恰好 +1**（T04 审查确认的不变量），并回到草稿态：
    // 编辑过的计划不能再被当成「已校验」。
    plan.revision = match expected_revision.checked_add(1) {
        Some(revision) => revision,
        None => return IpcResult::err(AppError::new(codes::STALE_PLAN, "计划版本已达到上限")),
    };
    plan.status = PlanStatus::Draft;

    if let Err(error) = save_plan(db, &plan, Some(expected_revision)) {
        return IpcResult::err(error);
    }

    IpcResult::ok(plan)
}

/// 校验计划并签发一次性确认令牌。
///
/// 规格 7.4：令牌**只有后端**能签发，最多 5 分钟有效、只能用一次。
/// 令牌绑定 `planId` + `revision` + `digest`，任何一项对不上都会被拒绝。
///
/// 注意本命令**不**消费令牌：消费发生在本阶段之后的执行命令里（T06）。
/// 因此这里签发出来的令牌在本阶段只会过期，不会被执行——这是预期的，
/// 规格 T05 明确要求「execute_plan 尚未接通前按钮明确不可执行」。
#[tauri::command]
pub fn validate_plan(
    state: State<'_, AppState>,
    plan_id: String,
) -> IpcResult<Option<ValidationReport>> {
    let db = state.db();

    let plan = match load_plan(db, &plan_id) {
        Ok(Some(plan)) => plan,
        Ok(None) => return IpcResult::ok(None),
        Err(error) => return IpcResult::err(error),
    };

    // 根授权是运行期状态：应用重启后必须重新选目录（规格 3.3）。
    let Some(root) = state.root(&plan.root_id) else {
        return IpcResult::err(AppError::new(
            codes::ROOT_NOT_AUTHORIZED,
            "该根目录的授权已失效，请重新选择文件夹",
        ));
    };

    let mut report = match validate_plan_core(&plan, &root) {
        Ok(report) => report,
        Err(error) => return IpcResult::err(error),
    };

    // 摘要落库：执行阶段要拿它和确认时的摘要比对。
    if let Err(error) = set_plan_digest(db, &plan.id, plan.revision, &report.digest) {
        return IpcResult::err(error);
    }

    // **只有真的可执行才签发令牌**。可执行项为 0 或存在阻断项时，
    // 报告里不能带一个「看起来可用」的令牌——那会让前端以为可以执行。
    let blocked = report
        .issues
        .iter()
        .any(|issue| issue.severity == Risk::Block);
    if report.executable_count > 0 && !blocked {
        match state
            .confirmations()
            .issue(&plan.id, plan.revision, &report.digest, now_unix_ms())
        {
            Ok(issued) => {
                if let Err(error) = crate::storage::execution::persist_confirmation(
                    db,
                    &plan,
                    &report.digest,
                    &issued,
                ) {
                    return IpcResult::err(error);
                }
                report.validation_token = Some(issued.token);
                report.expires_at = Some(issued.expires_at);
            }
            Err(error) => return IpcResult::err(error),
        }
    }

    IpcResult::ok(Some(report))
}

/// 取相对路径最后一段的扩展名（小写，含前导点）。
///
/// 与 `scanner::walk` 的口径一致：无扩展名返回空串，
/// 前导点开头的隐藏文件名不算扩展名（`.gitignore` 没有扩展名）。
fn extension_of(relative: &[String]) -> String {
    let Some(name) = relative.last() else {
        return String::new();
    };
    match name.rfind('.') {
        Some(0) | None => String::new(),
        Some(index) => name[index..].to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_extraction_matches_the_scanner_convention() {
        assert_eq!(extension_of(&["a.txt".to_owned()]), ".txt");
        assert_eq!(
            extension_of(&["目录".to_owned(), "B.PDF".to_owned()]),
            ".pdf"
        );
        assert_eq!(extension_of(&["no_ext".to_owned()]), "");
        // 前导点是隐藏文件标记，不是扩展名
        assert_eq!(extension_of(&[".gitignore".to_owned()]), "");
        assert_eq!(extension_of(&[]), "");
    }
}
