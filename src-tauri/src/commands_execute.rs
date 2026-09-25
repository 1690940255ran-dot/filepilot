//! T06：执行与执行报告的命令。
//!
//! 规格 5.2：`execute_plan(planId, requestId, validationToken)`。
//!
//! 这条命令是**唯一**会改动用户文件的地方（在当前阶段），因此每一层检查
//! 都不能省：
//! 1. 计划必须从数据库读——调用方给的任何路径都不采信；
//! 2. 一次性令牌必须由后端消费——伪造、过期、重用都会被拒（T05 已实现）；
//! 3. 摘要必须与库中记录一致——防止「确认的是 A、执行的是 B」；
//! 4. 执行本身遵循规格 8.2 的四步闸门。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, State};

use crate::app_state::AppState;

use crate::commands::TASK_PROGRESS_CHANNEL;
use crate::domain::types::{
    Issue, OpStatus, PlanAction, PlanStatus, Risk, RunReport, RunStatus, TaskError, TaskStatus,
};
use crate::domain::{errors::codes, errors::AppError, IpcResult};
use crate::executor::{execute_started_observed, ExecutionObserver};
use crate::planner::compute_digest;
use crate::safety::confirmation::now_unix_ms;
use crate::storage::repositories::{load_plan, plan_digest};
use crate::storage::runs::{count_operations, list_operations, load_run};

/// 把执行器的事件转成 `task-progress`。
///
/// 取消令牌与扫描共用一套：`cancel_task(taskId)` 对两者语义一致——
/// **只发出请求**，由正在跑的那一项自己走到安全点再停。
struct AppObserver {
    app: tauri::AppHandle,
    task_id: String,
    cancelled: Arc<AtomicBool>,
    seq: AtomicU32,
    total: u32,
}

impl AppObserver {
    fn emit(&self, status: TaskStatus, processed: u32) {
        let _ = self.app.emit(
            TASK_PROGRESS_CHANNEL,
            serde_json::json!({
                "taskId": self.task_id,
                // seq 单调递增，前端据此丢弃乱序事件
                "seq": self.seq.fetch_add(1, Ordering::SeqCst) + 1,
                "status": status,
                "processed": processed,
                "total": self.total,
            }),
        );
    }
}

impl ExecutionObserver for AppObserver {
    fn should_stop(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        self.emit(TaskStatus::Running, processed);
    }
}

/// 执行一份已确认的计划。
///
/// 幂等由 `requestId` 保证：同一个 id 重复调用不会产生第二个 run，
/// 也不会第二次移动文件（规格 T07「双击执行只生成一个 run」）。
#[tauri::command]
pub async fn execute_plan(
    app: tauri::AppHandle,
    plan_id: String,
    request_id: String,
    validation_token: String,
) -> IpcResult<RunReport> {
    match tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        execute_plan_inner(app.clone(), &state, plan_id, request_id, validation_token)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => IpcResult::err(AppError::internal(format!("执行线程异常: {error}"))),
    }
}

fn execute_plan_inner(
    app: tauri::AppHandle,
    state: &AppState,
    plan_id: String,
    request_id: String,
    validation_token: String,
) -> IpcResult<RunReport> {
    let db = state.db();

    match crate::storage::execution::replay(db, &request_id, &plan_id, &validation_token) {
        Ok(Some(id)) => {
            return match load_run(db, &id)
                .and_then(|r| r.ok_or_else(|| AppError::internal("执行记录不存在")))
                .and_then(|r| report_for(db, &r))
            {
                Ok(report) => IpcResult::ok(report),
                Err(error) => IpcResult::err(error),
            }
        }
        Ok(None) => {}
        Err(error) => return IpcResult::err(error),
    }
    // Lock before touching the token: TASK_BUSY must leave confirmation usable.
    let _execution_guard = match state.begin_execution(&plan_id) {
        Ok(guard) => guard,
        Err(error) => return IpcResult::err(error),
    };

    // ---- 1) 计划从库里读 ----
    let plan = match load_plan(db, &plan_id) {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            return IpcResult::err(AppError::new(
                codes::STALE_PLAN,
                "计划不存在；它可能已被删除或属于上一次运行的应用会话",
            ))
        }
        Err(error) => return IpcResult::err(error),
    };

    // ---- 2) 状态必须是「已校验」----
    // 草稿意味着用户还没点过「校验并获取确认」，密封则说明已经执行过了。
    if plan.status != PlanStatus::Validated {
        return IpcResult::err(AppError::new(
            codes::STALE_PLAN,
            format!(
                "计划当前状态为 {}，必须先校验并确认才能执行",
                status_text(plan.status)
            ),
        ));
    }

    // ---- 3) 根授权必须仍有效 ----
    let Some(root) = state.root(&plan.root_id) else {
        return IpcResult::err(AppError::new(
            codes::ROOT_NOT_AUTHORIZED,
            "该根目录的授权已失效，请重新选择文件夹",
        ));
    };

    // ---- 4) 摘要必须与确认时一致 ----
    let expected_digest = match plan_digest(db, &plan_id) {
        Ok(Some(digest)) => digest,
        Ok(None) => {
            return IpcResult::err(AppError::new(
                codes::STALE_PLAN,
                "该计划没有校验记录，请重新校验并确认",
            ))
        }
        Err(error) => return IpcResult::err(error),
    };
    let actual_digest = compute_digest(&plan, &root);
    if actual_digest != expected_digest {
        return IpcResult::err(AppError::new(
            codes::STALE_PLAN,
            "计划内容在确认之后发生了变化，请重新预览并确认",
        ));
    }

    // ---- 5) 消费一次性令牌 ----
    // 到这里为止都还没碰过任何文件。令牌消费失败就必须原地返回。
    let run_id = match crate::storage::execution::admit(
        db,
        &plan,
        &actual_digest,
        &request_id,
        &validation_token,
        now_unix_ms(),
    ) {
        Ok(id) => id,
        Err(error) => return IpcResult::err(error),
    };

    // ---- 7) 登记任务，让取消与进度都有地方挂 ----
    let total = plan
        .items
        .iter()
        .filter(|item| item.selected && item.action == PlanAction::Move)
        .count() as u32;
    let task_id = state.start_task(Some(total));
    let cancelled = state
        .cancellation_token(&task_id)
        .expect("刚登记的任务必须带取消令牌");

    let observer = AppObserver {
        app,
        task_id: task_id.clone(),
        cancelled: cancelled.clone(),
        seq: AtomicU32::new(0),
        total,
    };

    // ---- 8) 执行 ----
    let outcome = match execute_started_observed(
        db,
        &root,
        &plan,
        run_id.clone(),
        now_unix_ms(),
        &observer,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = crate::storage::runs::finish_run(
                db,
                &run_id,
                RunStatus::RecoveryRequired,
                &crate::domain::time::rfc3339_from_unix_ms(now_unix_ms()),
            );
            state.fail_task(&task_id, TaskError::from(&error));
            observer.emit(TaskStatus::Failed, 0);
            return IpcResult::err(error);
        }
    };

    // ---- 9) 收尾任务状态 ----
    // 幂等复用说明这次没有真的执行，但任务本身仍要有个终态，
    // 否则界面会一直停在「进行中」。
    let final_status = if outcome.cancelled {
        TaskStatus::Cancelled
    } else if outcome.status == RunStatus::Completed {
        TaskStatus::Completed
    } else {
        TaskStatus::Partial
    };
    observer.emit(final_status, total);
    state.finish_task_with_status(&task_id, final_status);

    // ---- 10) 汇总报告 ----
    match build_report(db, &outcome) {
        Ok(report) => IpcResult::ok(report),
        Err(error) => IpcResult::err(error),
    }
}

/// 读取一次执行的报告。
#[tauri::command]
pub fn get_run(state: State<'_, AppState>, run_id: String) -> IpcResult<Option<RunReport>> {
    let db = state.db();
    match load_run(db, &run_id) {
        Ok(Some(row)) => match report_for(db, &row) {
            Ok(report) => IpcResult::ok(Some(report)),
            Err(error) => IpcResult::err(error),
        },
        Ok(None) => IpcResult::ok(None),
        Err(error) => IpcResult::err(error),
    }
}

/// 列出最近的执行记录。
///
/// 规格 T07：history 页面要「从数据库恢复状态，页面刷新不重启任务」。
/// 这里直接读 `runs` 表而不是内存里的任务表——**这正是关键**：
/// 应用重启后内存里的任务没了，但历史还在。
#[tauri::command]
pub fn list_runs(state: State<'_, AppState>, limit: Option<u32>) -> IpcResult<Vec<RunReport>> {
    let db = state.db();
    let limit = limit.unwrap_or(DEFAULT_RUN_PAGE);

    let rows = match crate::storage::runs::list_runs(db, limit) {
        Ok(rows) => rows,
        Err(error) => return IpcResult::err(error),
    };

    let mut reports = Vec::with_capacity(rows.len());
    for row in &rows {
        match report_for(db, row) {
            Ok(report) => reports.push(report),
            Err(error) => return IpcResult::err(error),
        }
    }
    IpcResult::ok(reports)
}

/// 默认列出多少条历史。
const DEFAULT_RUN_PAGE: u32 = 50;

/// 把一行 run 组装成面向界面的报告。
///
/// `get_run` 与 `list_runs` 共用它，避免两处各写一遍「读操作、算计数、收问题」——
/// 那种重复迟早会让两个入口对同一次执行给出不一样的描述。
fn report_for(
    db: &crate::storage::db::Database,
    row: &crate::storage::runs::RunRow,
) -> Result<RunReport, AppError> {
    let status = crate::storage::runs::run_status_from_text(&row.status)?;
    let counts = count_operations(db, &row.id)?;
    let operations = list_operations(db, &row.id)?;

    let mut issues = Vec::new();
    for operation in &operations {
        if let Some(code) = &operation.error_code {
            issues.push(Issue {
                code: code.clone(),
                severity: if operation.status == OpStatus::Ambiguous {
                    Risk::Block
                } else {
                    Risk::Warning
                },
                item_id: Some(operation.item_id.clone()),
                message: format!("这一项未完成（{}）", operation.item_id),
            });
        }
    }

    Ok(RunReport {
        run_id: row.id.clone(),
        plan_id: row.plan_id.clone(),
        direction: row.direction.clone(),
        state_digest: state_digest(&operations),
        status,
        counts,
        issues,
    })
}

// ---------------------------------------------------------------------------

fn build_report(
    db: &crate::storage::db::Database,
    outcome: &crate::executor::ExecuteOutcome,
) -> Result<RunReport, AppError> {
    let operations = list_operations(db, &outcome.run_id)?;
    let row = load_run(db, &outcome.run_id)?;
    let plan_id = row
        .as_ref()
        .map(|row| row.plan_id.clone())
        .unwrap_or_default();

    Ok(RunReport {
        run_id: outcome.run_id.clone(),
        plan_id,
        // 这条路径只服务 `execute_plan`，方向必然是整理。
        direction: row
            .map(|row| row.direction)
            .unwrap_or_else(|| "apply".to_owned()),
        state_digest: state_digest(&operations),
        status: outcome.status,
        counts: outcome.counts,
        issues: outcome.issues.clone(),
    })
}

/// 对「当前操作事实」取摘要。
///
/// 规格 8.3 用它防止用户确认一份过期的恢复报告：如果两次读取之间
/// 有任何一个操作的状态**或处置**变了，摘要就会变，界面据此要求重新核对。
///
/// `resolution` 必须在摘要里：一批歧义项被逐条确认的过程中，
/// 摘要如果不变，用户就可能拿一份「只确认了第一项」时看到的摘要
/// 去提交「全部已核对」——那等于未经核对就解除了任务阻塞。
pub fn state_digest(operations: &[crate::storage::runs::OperationRow]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"filepilot.run.state.v2");
    for operation in operations {
        hasher.update(operation.id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(operation.item_id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(op_status_text(operation.status).as_bytes());
        hasher.update(b"\x1f");
        hasher.update(op_resolution_text(operation.resolution).as_bytes());
        hasher.update(b"\x1e");
    }
    format!("{:x}", hasher.finalize())
}

fn op_resolution_text(resolution: crate::domain::types::OpResolution) -> &'static str {
    use crate::domain::types::OpResolution::*;
    match resolution {
        Open => "open",
        Acknowledged => "acknowledged",
    }
}

fn op_status_text(status: crate::domain::types::OpStatus) -> &'static str {
    use crate::domain::types::OpStatus::*;
    match status {
        Pending => "pending",
        Prepared => "prepared",
        Applied => "applied",
        Failed => "failed",
        Skipped => "skipped",
        Ambiguous => "ambiguous",
    }
}

fn status_text(status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::Draft => "草稿",
        PlanStatus::Validated => "已校验",
        PlanStatus::Sealed => "已密封",
        PlanStatus::Archived => "已归档",
    }
}
