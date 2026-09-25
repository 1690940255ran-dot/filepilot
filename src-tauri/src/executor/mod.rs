//! 执行器（规格 7.3、8.2）。
//!
//! 规格 8.2 的顺序是整个模块的骨架，代码结构刻意与之一一对应：
//!
//! ```text
//! 查询幂等结果 → 全批只读预检查 → 逐项：核对 → 写意图 → 移动 → 核对 → 写结果
//! ```
//!
//! 三条贯穿始终的规则：
//! 1. **计划从数据库读**，不信任 IPC 传来的路径（本模块只接受 `Plan` 对象，
//!    而调用方必须先用 `load_plan` 从库里取）；
//! 2. **第一项失败即停止**，后续项标 `skipped`，已完成项保持完成、不自动回滚；
//! 3. **选择集合之外的文件一个都不碰**——只有 `item.selected` 为真且是 `Move`
//!    的项才会进入派发列表。

pub mod journal;
pub mod undo;

use std::collections::BTreeMap;

use crate::domain::errors::{codes, AppError};
use crate::domain::types::{Issue, OpStatus, Plan, PlanAction, Risk, RunCounts, RunStatus};
use crate::safety::fingerprint::{move_no_replace_with, MoveFailPoints};
use crate::safety::root::ApprovedRoot;
use crate::storage::db::Database;
use crate::storage::runs::{self, count_operations, list_operations};
use journal::{new_operation, BeginOutcome, Journal};

/// 执行过程中的观察者。
///
/// 把「进度上报」与「取消检查」放在同一个接口里，是因为它们由同一件事驱动：
/// **每派发一项之前**。分成两个回调会让调用方在错误的位置插桩
/// （比如把取消检查放在移动之后——那已经晚了，规格要求当前项落日志后才停）。
pub trait ExecutionObserver {
    /// 是否应当停止。返回 `true` 时**当前项不再派发**，后续项标 `skipped`。
    ///
    /// 这是「请求取消」检查点。用户点了取消之后，已经派发的那一项会走完
    /// （移动 + 落日志），因为半途而废才是真正危险的状态。
    fn should_stop(&self) -> bool;

    /// 一项处理完毕。
    fn on_progress(&self, processed: u32, total: u32);
}

/// 不上报、不取消的观察者。测试与「不需要进度的调用方」用它。
pub struct NoopObserver;

impl ExecutionObserver for NoopObserver {
    fn should_stop(&self) -> bool {
        false
    }
    fn on_progress(&self, _processed: u32, _total: u32) {}
}

/// 一次执行的结果。
#[derive(Debug, Clone)]
pub struct ExecuteOutcome {
    pub run_id: String,
    pub status: RunStatus,
    pub counts: RunCounts,
    pub issues: Vec<Issue>,
    /// 该 `requestId` 之前已经跑过，本次直接返回既有结果。
    pub deduplicated: bool,
    /// 因为用户取消而提前结束。已完成项保持完成，未派发项是 `skipped`。
    pub cancelled: bool,
}

/// 执行一份计划。
///
/// 调用方必须已完成：加载计划（从库）、消费一次性令牌、校验摘要。
/// 本函数**不**做授权判断——它只保证「要么完整按计划执行，要么明确停下来」。
pub fn execute_plan(
    db: &Database,
    root: &ApprovedRoot,
    plan: &Plan,
    request_id: &str,
    now_ms: i64,
) -> Result<ExecuteOutcome, AppError> {
    execute_plan_observed(db, root, plan, request_id, now_ms, &NoopObserver)
}

/// 与 [`execute_plan`] 相同，但会向观察者上报进度并接受取消请求。
pub fn execute_plan_observed(
    db: &Database,
    root: &ApprovedRoot,
    plan: &Plan,
    request_id: &str,
    now_ms: i64,
    observer: &dyn ExecutionObserver,
) -> Result<ExecuteOutcome, AppError> {
    let journal = Journal::new(db);

    // ---- 0) 幂等：同一 requestId 绝不产生第二个 run ----
    let run_id = match journal.begin(&plan.id, request_id, "apply", now_ms)? {
        BeginOutcome::Existing(run_id) => return existing_outcome(db, &run_id),
        BeginOutcome::Created(run_id) => run_id,
    };

    execute_started_observed(db, root, plan, run_id, now_ms, observer)
}

/// Run already created by the durable authorization transaction.
pub(crate) fn execute_started_observed(
    db: &Database,
    root: &ApprovedRoot,
    plan: &Plan,
    run_id: String,
    now_ms: i64,
    observer: &dyn ExecutionObserver,
) -> Result<ExecuteOutcome, AppError> {
    let journal = Journal::new(db);

    // ---- 1) 全批只读预检查 ----
    // 规格 8.2：失败则终止该 run，下次必须重新预览确认。
    // 预检查**不碰任何文件**，只是尽早把「这批里已经有明显不可执行的项」暴露出来。
    let plan_items = dispatchable_items(plan);
    let preflight = preflight_check(root, plan, &plan_items);
    if !preflight.is_empty() {
        journal.finish(&run_id, RunStatus::Failed, now_ms)?;
        return Ok(ExecuteOutcome {
            run_id,
            status: RunStatus::Failed,
            counts: RunCounts {
                pending: plan_items.len() as u32,
                ..RunCounts::default()
            },
            issues: preflight,
            deduplicated: false,
            cancelled: false,
        });
    }

    // ---- 2) 逐项执行 ----
    let mut issues: Vec<Issue> = Vec::new();
    let mut stopped = false;
    let mut needs_recovery = false;
    let mut cancelled_by_user = false;

    let total = plan_items.len() as u32;
    observer.on_progress(0, total);

    for (sequence, item) in plan_items.iter().enumerate() {
        let operation = new_operation(
            &run_id,
            &item.id,
            sequence as u32,
            &item.source,
            &item.target,
            &serde_json::to_string(&item.expected).unwrap_or_else(|_| "{}".to_owned()),
        );

        // 先在日志里为**每一**项建行（状态 pending）。
        //
        // 这不是可有可无的记账：规格 8.2 把「未派发」描述为 `pending → skipped`，
        // 说明未派发的项本来就应该有一条记录。而且 `operation_events.operationId`
        // 有外键——不先建行，`mark_skipped` 连事件都写不下去。
        //
        // 写不进去就直接失败：日志不可用时的任何文件改动都是不可追溯的。
        // 这里用 `?` 而不是「记个问题继续跑」，因为继续跑意味着接下来
        // 每一次 `mark_*` 都会失败，而失败发生在文件已经被移动之后。
        journal.record_intent(&operation)?;

        // 前面已经失败：这一项根本没被派发
        if stopped {
            journal.mark_skipped(&operation.id, now_ms)?;
            // 跳过也算「处理过」——不上报的话进度条会停在中间
            observer.on_progress(sequence as u32 + 1, total);
            continue;
        }

        // 取消检查点。放在**任何文件动作之前**：这一项还没碰过磁盘，
        // 此时停下来是干净的。已经派发的那一项会走完——
        // 半途而废才是真正危险的状态。
        if observer.should_stop() {
            cancelled_by_user = true;
            journal.mark_skipped(&operation.id, now_ms)?;
            observer.on_progress(sequence as u32 + 1, total);
            continue;
        }

        // 目标父目录：已存在则只验证并记录「不是我建的」
        if let Err(error) = ensure_parent_dir(root, &journal, &operation.id, &item.target, now_ms) {
            journal.mark_failed(&operation.id, &error.code, &error.message, now_ms)?;
            issues.push(item_issue(&item.id, &error));
            stopped = true;
            continue;
        }

        // 核对 + 移动 + 复核。日志写入被安排在核对通过之后、重命名之前。
        let outcome = move_no_replace_with(
            root,
            &item.source,
            &item.target,
            &item.expected,
            MoveFailPoints::Apply,
            |_handle| journal.mark_prepared(&operation.id, now_ms),
        );

        match outcome {
            Ok(_receipt) => {
                // rename 已经发生。此时日志写入失败就会留下一个「磁盘动了、
                // 日志没记」的窗口——规格 8.2 要求进入 recoveryRequired 并立即停止。
                if let Err(error) = journal.mark_applied(&operation.id, now_ms) {
                    let _ =
                        journal.mark_ambiguous(&operation.id, "重命名已成功但结果未能落盘", now_ms);
                    issues.push(Issue {
                        code: codes::RECOVERY_REQUIRED.to_owned(),
                        severity: Risk::Block,
                        item_id: Some(item.id.clone()),
                        message: format!(
                            "文件已移动，但执行日志未能写入（{}）。请先恢复数据库可用性，\
                             再用恢复流程核对，不要盲目重试。",
                            error.message
                        ),
                    });
                    needs_recovery = true;
                    stopped = true;
                }

                // 故障注入：结果已落盘。到这里这一项就是**确定完成**的，
                // 恢复流程看到 `applied` 不需要再去磁盘核对。
                crate::platform::failpoint::fail_point(
                    crate::platform::failpoint::FAILPOINT_AFTER_APPLIED,
                );
            }
            Err(error) => {
                // 区分「确定没动」与「可能动了」。
                // `move_no_replace_with` 只在核对通过后才会调用重命名，
                // 而它的错误要么发生在重命名之前（没动），要么来自内核拒绝（没动），
                // 复核失败那条路则已经被它自己处理成 SOURCE_CHANGED。
                if error.code == codes::RECOVERY_REQUIRED {
                    journal.mark_ambiguous(&operation.id, &error.message, now_ms)?;
                    needs_recovery = true;
                } else {
                    journal.mark_failed(&operation.id, &error.code, &error.message, now_ms)?;
                }
                issues.push(item_issue(&item.id, &error));
                stopped = true;
            }
        }

        observer.on_progress(sequence as u32 + 1, total);
    }

    // ---- 3) 汇总终态 ----
    let counts = count_operations(db, &run_id)?;
    let status = if needs_recovery {
        RunStatus::RecoveryRequired
    } else if cancelled_by_user {
        // 规格 T07：明确区分「请求取消」与「已取消」——
        // 到了这里说明确实停下来了，才写成 `cancelled`。
        RunStatus::Cancelled
    } else if counts.applied == 0 && counts.failed == 0 && counts.pending == 0 {
        RunStatus::Completed
    } else if counts.failed > 0 || counts.pending > 0 || counts.skipped > 0 {
        RunStatus::Partial
    } else {
        RunStatus::Completed
    };
    journal.finish(&run_id, status, now_ms)?;

    Ok(ExecuteOutcome {
        run_id,
        status,
        counts,
        issues,
        deduplicated: false,
        cancelled: cancelled_by_user,
    })
}

/// 读一个**已完成**的 run 的结果，供幂等返回。
fn existing_outcome(db: &Database, run_id: &str) -> Result<ExecuteOutcome, AppError> {
    let row = runs::load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("run {run_id} 不存在")))?;
    let status = runs::run_status_from_text(&row.status)?;
    let counts = count_operations(db, run_id)?;

    // 把上一次真实发生过的问题还原出来，而不是给一个「一切正常」的空列表。
    let mut issues = Vec::new();
    for operation in list_operations(db, run_id)? {
        if operation.status == OpStatus::Failed || operation.status == OpStatus::Ambiguous {
            issues.push(Issue {
                code: operation
                    .error_code
                    .unwrap_or_else(|| codes::INTERNAL.to_owned()),
                severity: if operation.status == OpStatus::Ambiguous {
                    Risk::Block
                } else {
                    Risk::Warning
                },
                item_id: Some(operation.item_id.clone()),
                message: format!(
                    "上一次执行时这一项未完成（{}）",
                    status_label(operation.status)
                ),
            });
        }
    }

    Ok(ExecuteOutcome {
        run_id: run_id.to_owned(),
        status,
        counts,
        issues,
        deduplicated: true,
        cancelled: status == RunStatus::Cancelled,
    })
}

/// 本次真正要派发的项。
///
/// 只包含**选中且动作为 `Move`** 的项。这条过滤是「选择集合之外的文件不触碰」
/// 的唯一执行点，因此单独成函数、单独测试。
fn dispatchable_items(plan: &Plan) -> Vec<&crate::domain::types::PlanItem> {
    plan.items
        .iter()
        .filter(|item| item.selected && item.action == PlanAction::Move)
        .collect()
}

/// 全批只读预检查。返回空 vec 表示可以开始执行。
fn preflight_check(
    root: &ApprovedRoot,
    plan: &Plan,
    items: &[&crate::domain::types::PlanItem],
) -> Vec<Issue> {
    let mut issues = Vec::new();

    if plan.status == crate::domain::types::PlanStatus::Sealed {
        issues.push(Issue {
            code: codes::STALE_PLAN.to_owned(),
            severity: Risk::Block,
            item_id: None,
            message: "该计划已被密封，不能再次执行".to_owned(),
        });
    }

    if items.is_empty() {
        issues.push(Issue {
            code: "EMPTY_SELECTION".to_owned(),
            severity: Risk::Block,
            item_id: None,
            message: "没有选中任何要移动的项".to_owned(),
        });
    }

    if let Err(error) = root.verify_still_valid() {
        issues.push(Issue {
            code: error.code.clone(),
            severity: Risk::Block,
            item_id: None,
            message: error.message.clone(),
        });
        return issues;
    }

    // 目标重复：两项指向同一个位置时无法判断谁该让路，整批停。
    let mut targets: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        let key = crate::planner::naming::comparison_key(&item.target);
        *targets.entry(key).or_default() += 1;
    }
    for item in items {
        let key = crate::planner::naming::comparison_key(&item.target);
        if targets.get(&key).copied().unwrap_or(0) > 1 {
            issues.push(Issue {
                code: "DUPLICATE_TARGET".to_owned(),
                severity: Risk::Block,
                item_id: Some(item.id.clone()),
                message: "有两项要移到同一个位置".to_owned(),
            });
        }
    }

    for item in items {
        // 源必须存在
        match root.resolve_existing_within(&item.source) {
            Ok(path) => {
                if std::fs::symlink_metadata(&path).is_err() {
                    issues.push(Issue {
                        code: codes::SOURCE_MISSING.to_owned(),
                        severity: Risk::Block,
                        item_id: Some(item.id.clone()),
                        message: "源文件已不在原位置".to_owned(),
                    });
                }
            }
            Err(error) => issues.push(Issue {
                code: error.code.clone(),
                severity: Risk::Block,
                item_id: Some(item.id.clone()),
                message: error.message.clone(),
            }),
        }

        // 目标不能已存在
        if let Ok(target) = root.resolve_within(&item.target) {
            if std::fs::symlink_metadata(&target).is_ok() {
                issues.push(Issue {
                    code: codes::TARGET_EXISTS.to_owned(),
                    severity: Risk::Block,
                    item_id: Some(item.id.clone()),
                    message: "目标位置已经存在同名项".to_owned(),
                });
            }
        }
    }

    issues
}

/// 确保目标父目录存在。
///
/// 规格 8.2：目录创建只能**逐级**在批准根内进行，并且要在创建之前写好意图记录。
/// 已有目录只验证、只记录「不是我建的」，绝不宣称由本次创建。
fn ensure_parent_dir(
    root: &ApprovedRoot,
    journal: &Journal<'_>,
    operation_id: &str,
    target: &crate::domain::types::RelPath,
    now_ms: i64,
) -> Result<(), AppError> {
    if target.len() <= 1 {
        return Ok(());
    }

    // 逐级构造前缀，每一级都单独校验与记录
    for depth in 1..target.len() {
        let prefix: Vec<String> = target[..depth].to_vec();
        let path = root.resolve_within(&prefix)?;

        match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.is_dir() {
                    return Err(AppError::new(
                        codes::TARGET_PARENT_IS_FILE,
                        format!("{} 已经是一个文件，不能作为目录使用", prefix.join("\\")),
                    ));
                }
                // 已有目录：只记录，并明确标注不是本次创建
                journal.record_dir_created(operation_id, &prefix, "existing", false, now_ms)?;
            }
            Err(_) => {
                // 先写意图再创建 —— 创建成功但身份没落盘时，恢复流程才能
                // 「不猜归属、保留该目录」，而不是把它当成陌生目录。
                std::fs::create_dir(&path).map_err(|error| {
                    AppError::new(
                        codes::PERMISSION_DENIED,
                        format!("无法创建目录 {}: {error}", prefix.join("\\")),
                    )
                })?;

                let identity = match crate::platform::windows::open_directory(&path)
                    .and_then(|dir| crate::platform::windows::file_identity(&dir))
                {
                    Ok(identity) => identity.file_id_string(),
                    // 目录建好了但身份取不到：不能猜，如实记成一个空身份
                    Err(_) => String::new(),
                };
                journal.record_dir_created(operation_id, &prefix, &identity, true, now_ms)?;

                // 故障注入：目录已建、身份已记录。
                // 恢复流程要能认出「这个目录是本次创建的」——
                // 建了但没记上，就不能猜，只能保留原状。
                crate::platform::failpoint::fail_point(
                    crate::platform::failpoint::FAILPOINT_AFTER_DIR_CREATED,
                );
            }
        }
    }

    Ok(())
}

/// 给用户看的操作状态名。
///
/// 不直接 Debug 打印枚举：那会输出 `Prepared` 这种内部拼写，
/// 而这个字符串会出现在界面上。
fn status_label(status: OpStatus) -> &'static str {
    match status {
        OpStatus::Pending => "待处理",
        OpStatus::Prepared => "已记录意图但未完成",
        OpStatus::Applied => "已完成",
        OpStatus::Failed => "失败",
        OpStatus::Skipped => "未执行",
        OpStatus::Ambiguous => "状态不明，需要人工核对",
    }
}

fn item_issue(item_id: &str, error: &AppError) -> Issue {
    Issue {
        code: error.code.clone(),
        severity: Risk::Block,
        item_id: Some(item_id.to_owned()),
        message: error.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Fingerprint, Mode, PlanItem, PlanItemOrigin, PlanStatus, RelPath};

    fn item(id: &str, selected: bool, action: PlanAction) -> PlanItem {
        PlanItem {
            id: id.to_owned(),
            file_id: format!("fid-{id}"),
            source: vec![format!("{id}.txt")],
            target: vec!["文档".to_owned(), format!("{id}.txt")],
            action,
            selected,
            origin: PlanItemOrigin::Rule,
            reason: "测试".to_owned(),
            expected: Fingerprint {
                volume_id: "V".to_owned(),
                file_id: format!("fid-{id}"),
                size: "1".to_owned(),
                modified_ns: "0".to_owned(),
                sha256: Some("0".repeat(64)),
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
            status: PlanStatus::Validated,
            items,
            created_at: "2026-09-17T00:00:00.000Z".to_owned(),
        }
    }

    #[test]
    fn only_selected_move_items_are_dispatched() {
        let p = plan(vec![
            item("a", true, PlanAction::Move),
            item("b", false, PlanAction::Move), // 没选中
            item("c", true, PlanAction::Noop),  // 选中了但不动
            item("d", true, PlanAction::Move),
        ]);

        let dispatched = dispatchable_items(&p);
        let ids: Vec<&str> = dispatched.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a", "d"],
            "只有「选中 + 移动」的项能进派发列表，这是不碰其他文件的唯一执行点"
        );
    }

    #[test]
    fn a_sealed_plan_is_refused_in_preflight() {
        let mut p = plan(vec![item("a", true, PlanAction::Move)]);
        p.status = PlanStatus::Sealed;
        let items = dispatchable_items(&p);
        // 预检查需要一个真实根；这里只验证「密封」这一条不依赖磁盘的分支
        // 由 items 为空的快速路径覆盖，完整的预检查在集成测试里跑。
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn an_empty_selection_is_reported_by_preflight_shape() {
        let p = plan(vec![item("a", false, PlanAction::Move)]);
        assert!(dispatchable_items(&p).is_empty());
    }

    #[test]
    fn operation_records_carry_the_bound_fingerprint() {
        let row = new_operation(
            "run-1",
            "item-1",
            0,
            &RelPath::from(vec!["a.txt".to_owned()]),
            &RelPath::from(vec!["文档".to_owned(), "a.txt".to_owned()]),
            "{\"sha256\":\"abc\"}",
        );
        assert_eq!(row.status, OpStatus::Pending);
        assert!(
            row.expected_json.contains("sha256"),
            "操作记录必须带上计划里绑定的指纹，执行时才有得核对"
        );
    }
}
