//! 启动恢复（规格 8.3）。
//!
//! 应用每次启动都要回答一个问题：**上次退出时有没有事情做到一半？**
//!
//! 三条不容妥协的规则：
//!
//! 1. **只信磁盘与日志，不信内存。** 上一次进程的内存已经不存在了，
//!    这里做的一切判断都来自 `runs` / `operations` 表和真实的文件系统。
//! 2. **日志读不出来就停住，不「重置数据库再试」。** 日志是唯一的审计依据，
//!    把它删掉换取「能启动」等于销毁证据：用户会看到应用恢复正常，
//!    而实际上他可能有文件已经被移动过、却再也没人知道移到了哪里。
//! 3. **根目录没重新授权时不能假装核对过了。** 换不回可用路径就没法核对，
//!    只能如实标记 `recoveryRequired` 等用户重新授权。

use crate::app_state::AppState;
use crate::domain::errors::{codes, AppError};
use crate::domain::time::rfc3339_from_unix_ms;
use crate::domain::types::{OpResolution, OpStatus, RunCounts, RunStatus};
use crate::recovery::{reconcile_run, ReconcileOutcome};
use crate::storage::db::Database;
use crate::storage::runs::{self, list_operations};

/// 启动核对的结果。
#[derive(Debug, Clone)]
pub struct StartupRecovery {
    /// 完成了核对的 run。
    pub reconciled: Vec<ReconcileOutcome>,
    /// 因为根目录未授权而**没能**核对的 run id。
    ///
    /// 它们已被标记为 `recoveryRequired`，等用户重新授权后再核对。
    pub awaiting_authorization: Vec<String>,
}

impl StartupRecovery {
    /// 是否有任何未决事项。
    pub fn is_clear(&self) -> bool {
        self.reconciled
            .iter()
            .all(|outcome| !outcome.blocks_new_runs)
            && self.awaiting_authorization.is_empty()
    }
}

/// 核对上次未走完的执行。
///
/// 在 `AppState` 建好、Tauri 窗口起来**之前**调用一次。
/// 顺序很重要：新任务能不能开始，取决于这里有没有把阻塞写进库里。
pub fn recover_unfinished_runs(state: &AppState, now_ms: i64) -> Result<StartupRecovery, AppError> {
    let db = state.db();

    // 读不出来就**直接失败**。规格 8.5：`DB_UNAVAILABLE` 的动作是
    // 「停止一切文件操作并告知用户」，不是「清库重来」。
    let unfinished = runs::unfinished_runs(db).map_err(as_unavailable)?;

    let mut reconciled = Vec::new();
    let mut awaiting_authorization = Vec::new();

    for run in unfinished {
        let Some(root) = root_for_run(state, db, &run)? else {
            // 根目录授权在上次退出时已经失效（规格 8.1：授权不跨会话）。
            // 无法核对，但**必须**把它挡住，否则用户以为一切正常，
            // 而库里躺着一次没走完的执行。
            block_run(db, &run.id)?;
            awaiting_authorization.push(run.id);
            continue;
        };

        match reconcile_run(db, &root, &run.id, now_ms) {
            Ok(outcome) => reconciled.push(outcome),
            Err(error) => {
                // 核对中途失败（比如磁盘查询出错）。**同样不能放行**：
                // 保守地留在 recoveryRequired，让用户重新选择文件夹后再来一次。
                block_run(db, &run.id)?;
                if error.code == codes::DB_UNAVAILABLE || error.code == codes::JOURNAL_WRITE_FAILED
                {
                    return Err(error);
                }
            }
        }
    }

    Ok(StartupRecovery {
        reconciled,
        awaiting_authorization,
    })
}

/// 把一次 run 标成 `recoveryRequired`。
///
/// 单独成函数是为了**不容许失败被忽略**：这几处调用都发生在
/// 「本来就已经出了问题」的路径上，再吞掉一个错误，用户就会看到
/// 一个状态自相矛盾的界面（阻塞提示没有出现，但任务又发起不了）。
fn block_run(db: &Database, run_id: &str) -> Result<(), AppError> {
    let already = runs::load_run(db, run_id)?
        .map(|row| row.status == "recoveryRequired")
        .unwrap_or(false);
    if already {
        // 幂等：每次启动都跑一遍，不该反复重写时间戳。
        return Ok(());
    }
    runs::finish_run(
        db,
        run_id,
        RunStatus::RecoveryRequired,
        &rfc3339_from_unix_ms(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        ),
    )
}

/// 一次 run 的根目录，如果它在本次会话里仍被授权。
fn root_for_run(
    state: &AppState,
    db: &Database,
    run: &runs::RunRow,
) -> Result<Option<crate::safety::root::ApprovedRoot>, AppError> {
    // 计划的根是通过 scan → plan 这条链记录下来的，这里顺着链读，
    // 而不是让调用方传进来——启动路径上没有任何人「知道」该用哪个根。
    let Some(plan) = crate::storage::repositories::load_plan(db, &run.plan_id)? else {
        return Ok(None);
    };
    Ok(state.root(&plan.root_id))
}

fn as_unavailable(error: AppError) -> AppError {
    if error.code == codes::INTERNAL {
        return AppError::new(
            codes::DB_UNAVAILABLE,
            format!(
                "执行日志无法读取，已停止一切文件操作。请修复数据库后再启动：{}",
                error.message
            ),
        );
    }
    error
}

/// 汇总一次核对的结果，用于启动时的一行日志。
pub fn summarise(outcome: &ReconcileOutcome) -> String {
    // `RunCounts` 是 `Copy`，直接取值即可；这里刻意不写 `.clone()`，
    // 免得读的人以为它背后有堆分配。
    let counts = outcome.counts;
    format!(
        "run {} ({})：applied={} failed={} ambiguous={} pending={} blocked={}",
        outcome.run_id,
        outcome.status.as_text(),
        counts.applied,
        counts.failed,
        counts.ambiguous,
        counts.pending,
        outcome.blocks_new_runs
    )
}

/// 「这一项被确认保留原状了吗」。
///
/// 放在这里而不是散在界面代码里：判断依据必须是**落库的**处置状态，
/// 不是前端手里的那一份副本。
pub fn is_settled(operation_status: OpStatus, resolution: OpResolution) -> bool {
    operation_status != OpStatus::Ambiguous || resolution == OpResolution::Acknowledged
}

/// 读一次 run 的实时计数，供界面轮询。
pub fn live_counts(db: &Database, run_id: &str) -> Result<RunCounts, AppError> {
    runs::count_operations(db, run_id)
}

/// 读一次 run 的全部操作，仅供诊断输出使用。
pub fn snapshot(db: &Database, run_id: &str) -> Result<Vec<runs::OperationRow>, AppError> {
    list_operations(db, run_id)
}
