//! T08：崩溃恢复的命令。
//!
//! 规格 8.2 给出的动作只有两个，命令也**只有两个**（外加一个补跑）：
//!
//! | 命令 | 做什么 |
//! |---|---|
//! | `get_recovery` | 读一次 run 的实时核对结果，不改任何东西 |
//! | `acknowledge_recovery` | 用户核对后确认「保留现状」，写审计事件 |
//! | `recover_pending_runs` | 补跑启动核对（用户重新授权根目录后重试） |
//! | `recovery_status` | 首页问「现在能不能开新任务」 |
//!
//! 刻意**不提供**「自动修复」「重置日志」这类入口：规格 8.3 的决策表
//! 只在一种情形下允许应用自己动手（目标在、身份与内容都对得上 → 记为已应用），
//! 其余情形都要人来判断。给一个「一键修复」按钮，等于把猜测包装成功能。

use tauri::State;

use crate::app_state::AppState;
use crate::domain::types::{
    OpResolution, OpStatus, RecoveryItem, RecoveryReport, RunCounts, RunStatus,
};
use crate::domain::{errors::AppError, IpcResult};
use crate::recovery::startup::recover_unfinished_runs;
use crate::recovery::{acknowledge_items, reconcile_run, ReconcileOutcome};
use crate::safety::confirmation::now_unix_ms;
use crate::storage::repositories::load_plan;
use crate::storage::runs::{
    count_operations, list_operations, list_runs, load_run, MAX_LISTED_RUNS,
};

/// 读一次执行的实时恢复报告。
///
/// 每次调用都**重新核对磁盘**，不用内存里的旧判定：用户可能刚在文件管理器里
/// 手动移动了文件，而这份报告存在的全部意义就是如实反映「现在是什么样」。
#[tauri::command]
pub fn get_recovery(state: State<'_, AppState>, run_id: String) -> IpcResult<RecoveryReport> {
    match refresh(&state, &run_id) {
        Ok(report) => IpcResult::ok(report),
        Err(error) => IpcResult::err(error),
    }
}

/// 用户核对之后确认「保留现状并确认知晓」。
///
/// `state_digest` 由前端从 `get_recovery` 拿到并原样带回。它不是签名，
/// 也不防篡改——它防的是**过期**：用户在屏幕上核对的是一份快照，
/// 如果核对与提交之间磁盘事实变了，那份快照就不再是他确认过的东西。
#[tauri::command]
pub fn acknowledge_recovery(
    state: State<'_, AppState>,
    run_id: String,
    state_digest: String,
    reason: String,
) -> IpcResult<RecoveryReport> {
    let db = state.db();
    match acknowledge_items(db, &run_id, &state_digest, &reason, now_unix_ms()) {
        // 用刚算出来的结果直接组装，不再读一遍：中间的窗口里事实可能又变了，
        // 那时返回的就是与用户刚才那一下操作无关的另一份报告。
        Ok(outcome) => match report_from(outcome, &state) {
            Ok(report) => IpcResult::ok(report),
            Err(error) => IpcResult::err(error),
        },
        Err(error) => IpcResult::err(error),
    }
}

/// 补跑启动核对。
///
/// 用户在本次会话里重新授权了根目录之后用它——启动时没条件核对的 run
/// 这时才核准得上。幂等：重复调用不会重复写审计事件。
#[tauri::command]
pub fn recover_pending_runs(state: State<'_, AppState>) -> IpcResult<Vec<RecoveryReport>> {
    let startup = match recover_unfinished_runs(&state, now_unix_ms()) {
        Ok(startup) => startup,
        Err(error) => return IpcResult::err(error),
    };

    let mut ids: Vec<String> = startup
        .reconciled
        .iter()
        .map(|outcome| outcome.run_id.clone())
        .collect();
    ids.extend(startup.awaiting_authorization);

    let mut reports = Vec::with_capacity(ids.len());
    for run_id in &ids {
        match refresh(&state, run_id) {
            Ok(report) => reports.push(report),
            Err(error) => return IpcResult::err(error),
        }
    }
    IpcResult::ok(reports)
}

/// 首页要知道的恢复概况：现在能不能开新任务，不能的话是卡在哪些 run 上。
#[tauri::command]
pub fn recovery_status(state: State<'_, AppState>) -> IpcResult<RecoveryStatus> {
    let db = state.db();
    let runs = match list_runs(db, MAX_LISTED_RUNS) {
        Ok(runs) => runs,
        Err(error) => return IpcResult::err(error),
    };

    let mut blocked_runs = Vec::new();
    for row in &runs {
        match unresolved_count(db, &row.id) {
            Ok(0) => {}
            Ok(count) => blocked_runs.push(BlockedRun {
                run_id: row.id.clone(),
                plan_id: row.plan_id.clone(),
                unresolved: count,
            }),
            Err(error) => return IpcResult::err(error),
        }
    }

    IpcResult::ok(RecoveryStatus {
        blocked: !blocked_runs.is_empty(),
        blocked_runs,
    })
}

/// 首页要知道的恢复概况。
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    ts_rs::TS,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RecoveryStatus {
    /// 还有任何未处置的未决项时为 true —— 此时不能开始新任务（规格 8.2）。
    pub blocked: bool,
    pub blocked_runs: Vec<BlockedRun>,
}

/// 一个还没处理干净的 run。
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    ts_rs::TS,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct BlockedRun {
    pub run_id: String,
    pub plan_id: String,
    /// 还有几项没有被核对处置。
    pub unresolved: u32,
}

// ---------------------------------------------------------------------------

/// 重新核对一次 run，并组装出面向界面的报告。
fn refresh(state: &AppState, run_id: &str) -> Result<RecoveryReport, AppError> {
    let db = state.db();
    let run = load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {run_id}")))?;

    let Some(root) = authorized_root(state, db, &run.plan_id)? else {
        return unverified_report(db, run_id, &run.plan_id);
    };

    let outcome = reconcile_run(db, &root, run_id, now_unix_ms())?;
    report_from(outcome, state)
}

/// 根目录没授权时的报告。
///
/// 核不了磁盘，就**如实说核不了**：返回一份「没有未决项」的空报告
/// 会让界面显示一切正常，而库里其实躺着一次没走完的执行。
fn unverified_report(
    db: &crate::storage::db::Database,
    run_id: &str,
    plan_id: &str,
) -> Result<RecoveryReport, AppError> {
    let operations = list_operations(db, run_id)?;
    let unresolved = operations
        .iter()
        .filter(|op| !is_settled(op.status, op.resolution))
        .count();

    Ok(RecoveryReport {
        run_id: run_id.to_owned(),
        plan_id: plan_id.to_owned(),
        // 无法核对就不是「完成」也不是「失败」，只能是「需要恢复」。
        status: RunStatus::RecoveryRequired,
        state_digest: crate::commands_execute::state_digest(&operations),
        items: operations
            .iter()
            .filter(|op| !is_settled(op.status, op.resolution))
            .map(|op| RecoveryItem {
                operation_id: op.id.clone(),
                item_id: op.item_id.clone(),
                source: op.source.clone(),
                target: op.target.clone(),
                status: op.status,
                resolution: op.resolution,
                message: "该文件夹的授权已失效，无法核对磁盘。请重新选择这个文件夹后再看结果。"
                    .to_owned(),
            })
            .collect(),
        counts: count_operations(db, run_id)?,
        blocks_new_runs: unresolved > 0,
        root_authorized: false,
    })
}

/// 按 plan 找回本次会话授权的根。
///
/// 这是**唯一**把 rootId 变成可用路径的入口（`AppState::root`）。
/// 找不到就返回 `None`，调用方必须把它当成「核不了」，
/// 绝不能退化成「用某个默认路径继续核对」——那会把核对结果变成猜测。
fn authorized_root(
    state: &AppState,
    db: &crate::storage::db::Database,
    plan_id: &str,
) -> Result<Option<crate::safety::root::ApprovedRoot>, AppError> {
    let Some(plan) = load_plan(db, plan_id)? else {
        return Ok(None);
    };
    Ok(state.root(&plan.root_id))
}

fn report_from(outcome: ReconcileOutcome, state: &AppState) -> Result<RecoveryReport, AppError> {
    let db = state.db();
    let root_authorized = authorized_root(state, db, &outcome.plan_id)?.is_some();
    let plan_id = outcome.plan_id;

    Ok(RecoveryReport {
        run_id: outcome.run_id,
        plan_id,
        status: outcome.status,
        state_digest: outcome.state_digest,
        items: outcome
            .items
            .into_iter()
            .map(|item| RecoveryItem {
                operation_id: item.operation_id,
                item_id: item.item_id,
                source: item.source,
                target: item.target,
                // `reconcile_run` 只把 ambiguous 的项放进 items，
                // 这里的字面量不是猜测，而是那个过滤条件的回声。
                status: OpStatus::Ambiguous,
                resolution: item.resolution,
                message: item.message,
            })
            .collect(),
        counts: outcome.counts,
        blocks_new_runs: outcome.blocks_new_runs,
        root_authorized,
    })
}

/// 一项是否已被处置（规格 8.2：只有全部处置完才解除阻塞）。
///
/// 与 `recovery::startup::is_settled` 同义，这里只作用于单行，避免再多一次遍历。
fn is_settled(status: OpStatus, resolution: OpResolution) -> bool {
    status != OpStatus::Ambiguous || resolution == OpResolution::Acknowledged
}

fn unresolved_count(db: &crate::storage::db::Database, run_id: &str) -> Result<u32, AppError> {
    let run = load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {run_id}")))?;
    let operations = list_operations(db, run_id)?;
    let unresolved = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OpStatus::Pending | OpStatus::Prepared | OpStatus::Ambiguous
            ) && op.resolution != OpResolution::Acknowledged
        })
        .count() as u32;
    if unresolved == 0
        && matches!(
            run.status.as_str(),
            "queued" | "running" | "recoveryRequired"
        )
    {
        return Ok(1);
    }
    Ok(unresolved)
}

/// 供启动路径使用：把核对结果压成日志。
///
/// 启动阶段拿不到界面，唯一能让用户事后追查的就是这几行输出。
pub fn log_startup(startup: &crate::recovery::startup::StartupRecovery) {
    for outcome in &startup.reconciled {
        eprintln!("启动核对 {}", crate::recovery::startup::summarise(outcome));
    }
    for run_id in &startup.awaiting_authorization {
        eprintln!("启动核对：run {run_id} 需要先重新授权文件夹才能核对");
    }
    if startup.is_clear() {
        eprintln!("启动核对：没有未决的执行");
    }
}

/// 空计数，供 `RecoveryReport` 的默认值使用（测试与降级路径）。
impl Default for RecoveryStatus {
    fn default() -> Self {
        Self {
            blocked: false,
            blocked_runs: Vec::new(),
        }
    }
}

/// `RunCounts` 的零值别名，让调用处读起来更清楚。
pub fn no_counts() -> RunCounts {
    RunCounts::default()
}
