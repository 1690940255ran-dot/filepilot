//! T09：撤销的命令。
//!
//! 规格 8.4 把撤销定义成「另一个可预览、可确认、可失败的操作」，
//! 因此命令也只有三个，形状与整理前的那组完全对应：
//!
//! | 命令 | 对应整理的哪一步 |
//! |---|---|
//! | `preview_undo` | `create_plan` + `validate_plan`：给出清单、算摘要、签一次性令牌 |
//! | `execute_undo` | `execute_plan`：校验令牌与摘要，然后动手 |
//! | `get_undo_report` | `get_run`：按 runId 查回结果，页面切换不丢 |
//!
//! 刻意**不提供**「一键全部撤销」：规格 8.4 第 3 条明确要求有冲突的项
//! 默认不选中、允许用户只确认无冲突的子集。给一个跳过预览的入口，
//! 等于让用户在没看到冲突的情况下把文件搬回去。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use tauri::{Emitter, Manager, State};

use crate::app_state::AppState;
use crate::domain::errors::AppError;
use crate::domain::types::{RunStatus, TaskError, TaskStatus, UndoPreview, UndoReport};
use crate::domain::IpcResult;
use crate::executor::undo::{
    execute_undo_observed as run_undo, preview_undo as build_undo_preview, replay_undo_request,
    report_for_undo_run, UndoObserver,
};
use crate::safety::confirmation::now_unix_ms;
use crate::storage::repositories::load_plan;
use crate::storage::runs::load_run;

/// 生成一份撤销预览。
///
/// 每一次调用都会重新核对磁盘并重新签发令牌——用户看到的必须是**现在**的事实。
#[tauri::command]
pub fn preview_undo(state: State<'_, AppState>, run_id: String) -> IpcResult<UndoPreview> {
    let db = state.db();

    // 撤销要把文件搬回原处，「原处」只能由批准根定义。这里顺着
    // plan → rootId 这条链取根，而不是让前端传路径——
    // 前端持有的是 rootId，它换不回路径也就伪造不出授权（规格 3.3）。
    let root = match root_for(state.inner(), &run_id) {
        Ok(root) => root,
        Err(error) => return IpcResult::err(error),
    };

    match build_undo_preview(db, &root, &run_id, now_unix_ms()) {
        Ok(preview) => IpcResult::ok(preview),
        Err(error) => IpcResult::err(error),
    }
}

/// 执行一次撤销。
///
/// `selected` 是用户勾选的**原操作 id**。只有预览时判定为「可撤销」的项
/// 才允许出现在里面——执行器会重算一遍并逐项校验资格。
#[tauri::command]
pub async fn execute_undo(
    app: tauri::AppHandle,
    undo_plan_id: String,
    original_run_id: String,
    digest: String,
    undo_token: String,
    selected: Vec<String>,
    request_id: String,
) -> IpcResult<UndoReport> {
    match tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        execute_undo_inner(
            app.clone(),
            &state,
            undo_plan_id,
            original_run_id,
            digest,
            undo_token,
            selected,
            request_id,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(error) => IpcResult::err(AppError::internal(format!("撤销线程异常: {error}"))),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_undo_inner(
    app: tauri::AppHandle,
    state: &AppState,
    undo_plan_id: String,
    original_run_id: String,
    digest: String,
    undo_token: String,
    selected: Vec<String>,
    request_id: String,
) -> IpcResult<UndoReport> {
    let db = state.db();

    match replay_undo_request(
        db,
        &undo_plan_id,
        &original_run_id,
        &digest,
        &undo_token,
        &selected,
        &request_id,
    ) {
        Ok(Some(report)) => return IpcResult::ok(report),
        Ok(None) => {}
        Err(error) => return IpcResult::err(error),
    }

    let root = match root_for(state, &original_run_id) {
        Ok(root) => root,
        Err(error) => return IpcResult::err(error),
    };

    // 全局执行锁：撤销同样在改动用户文件，必须与整理任务互斥（规格 5.2）。
    let _guard = match state.begin_execution(&original_run_id) {
        Ok(guard) => guard,
        Err(error) => return IpcResult::err(error),
    };

    let task_id = state.start_task(Some(selected.len() as u32));
    let cancelled = state
        .cancellation_token(&task_id)
        .expect("刚登记的撤销任务必须带取消令牌");
    let observer = UndoAppObserver {
        app,
        task_id: task_id.clone(),
        cancelled,
        seq: AtomicU32::new(0),
        total: selected.len() as u32,
    };
    observer.emit(TaskStatus::Running, 0);

    match run_undo(
        db,
        &root,
        &undo_plan_id,
        &digest,
        &undo_token,
        &selected,
        &request_id,
        now_unix_ms(),
        &observer,
    ) {
        Ok(report) => {
            let task_status = match report.status {
                RunStatus::Completed => TaskStatus::Completed,
                RunStatus::Cancelled => TaskStatus::Cancelled,
                RunStatus::RecoveryRequired => TaskStatus::RecoveryRequired,
                _ => TaskStatus::Partial,
            };
            observer.emit(task_status, observer.total);
            state.finish_task_with_status(&task_id, task_status);
            IpcResult::ok(report)
        }
        Err(error) => {
            state.fail_task(&task_id, TaskError::from(&error));
            observer.emit(TaskStatus::Failed, 0);
            IpcResult::err(error)
        }
    }
}

struct UndoAppObserver {
    app: tauri::AppHandle,
    task_id: String,
    cancelled: Arc<AtomicBool>,
    seq: AtomicU32,
    total: u32,
}

impl UndoAppObserver {
    fn emit(&self, status: TaskStatus, processed: u32) {
        let _ = self.app.emit(
            crate::commands::TASK_PROGRESS_CHANNEL,
            serde_json::json!({
                "taskId": self.task_id,
                "seq": self.seq.fetch_add(1, Ordering::SeqCst) + 1,
                "status": status,
                "processed": processed,
                "total": self.total,
            }),
        );
    }
}

impl UndoObserver for UndoAppObserver {
    fn should_stop(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        self.emit(TaskStatus::Running, processed);
    }
}

/// 按 runId 读回一次撤销的结果。
///
/// 规格 5.1：`undoRunId` 是 `direction='undo'` 的普通 run，可以用 `get_run` 查询。
/// 这个命令是它的撤销版视图——把「已撤销 / 有冲突 / 未处理」的分项直接给出来，
/// 免得前端为了显示一句话还得自己去翻 operations。
#[tauri::command]
pub fn get_undo_report(
    state: State<'_, AppState>,
    run_id: String,
) -> IpcResult<Option<UndoReport>> {
    let db = state.db();
    let row = match load_run(db, &run_id) {
        Ok(Some(row)) => row,
        Ok(None) => return IpcResult::ok(None),
        Err(error) => return IpcResult::err(error),
    };
    if row.direction != "undo" {
        return IpcResult::err(AppError::new(
            crate::domain::errors::codes::INVALID_PATH,
            "这条记录不是撤销任务",
        ));
    }
    match report_for_undo_run(db, &run_id) {
        Ok(report) => IpcResult::ok(Some(report)),
        Err(error) => IpcResult::err(error),
    }
}

/// 找一次 run 所属的批准根。
///
/// 与启动恢复同一条链：`runs.planId` → `plans.rootId` → 本次会话里已授权的根。
/// 未授权时**如实报错**，而不是拿一个空根去核对——那会把「没权限看」
/// 伪装成「没什么可看的」。
fn root_for(state: &AppState, run_id: &str) -> Result<crate::safety::root::ApprovedRoot, AppError> {
    let db = state.db();
    let run = load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {run_id}")))?;
    let plan = load_plan(db, &run.plan_id)?
        .ok_or_else(|| AppError::internal("执行记录关联的计划不存在"))?;

    state.root(&plan.root_id).ok_or_else(|| {
        AppError::new(
            crate::domain::errors::codes::ROOT_NOT_AUTHORIZED,
            "这个文件夹在本会话里还没有重新授权，请先重新选择它再撤销",
        )
    })
}
