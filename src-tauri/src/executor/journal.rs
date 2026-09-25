//! 执行日志。规格 8.2 的「持久化意图 → 移动 → 核对 → 持久化结果」。
//!
//! 这个模块存在的唯一理由是：**崩溃后必须能判定每个文件到底动了没有**。
//! 因此每一处写入都遵循同一条规则——
//! *写下事实，再去做那个事实描述的动作*。
//!
//! 反过来说，任何「先移动再记日志」的写法都会在两步之间留下一个
//! 无法判定的窗口：进程在那个瞬间死掉，恢复时就只能猜。规格 8.3 的整张决策表
//! 都建立在「日志里的 prepared 一定先于真实的 rename」这个前提上。
//!
//! 写入失败一律映射成 `JOURNAL_WRITE_FAILED`（规格 8.5：停止执行）。

use crate::domain::errors::{codes, AppError};
use crate::domain::types::{OpResolution, OpStatus, RelPath, RunStatus, UndoStatus};
use crate::storage::db::Database;
use crate::storage::runs::{
    self, append_event, finish_run, insert_created_dir, insert_operation, insert_run,
    set_operation_status_and_event, CreatedDirRow, OperationRow,
};

/// 事件里记录阶段用的常量。
///
/// 用常量而不是散落的字符串：恢复逻辑要靠这些值做判断，
/// 拼错一个字母就会让某个状态永远匹配不上。
pub const PHASE_PREPARED: &str = "prepared";
pub const PHASE_APPLIED: &str = "applied";
pub const PHASE_FAILED: &str = "failed";
pub const PHASE_SKIPPED: &str = "skipped";
pub const PHASE_AMBIGUOUS: &str = "ambiguous";
pub const PHASE_DIR_CREATED: &str = "dirCreated";

/// `begin` 的结果。
///
/// 用枚举而不是 `Option<String>`：两种情形的**语义完全不同**
/// （一个是「刚建好、去执行」，一个是「早就跑过、去读结果」），
/// 混用一个 None 很容易让调用方写反。
pub enum BeginOutcome {
    /// 新建成功，可以开始执行。
    Created(String),
    /// 该 requestId 已有 run，必须走幂等路径。
    Existing(String),
}

impl BeginOutcome {
    pub fn run_id(&self) -> &str {
        match self {
            BeginOutcome::Created(id) | BeginOutcome::Existing(id) => id,
        }
    }

    pub fn is_existing(&self) -> bool {
        matches!(self, BeginOutcome::Existing(_))
    }
}

pub struct Journal<'a> {
    db: &'a Database,
}

impl<'a> Journal<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// 开启一次执行。
    ///
    /// 返回 [`BeginOutcome::Existing`] 表示这个 `requestId` **已经有** run 了——
    /// 调用方应当走幂等路径返回已有结果，而不是再执行一遍。
    /// 判定交给数据库的唯一约束，不靠应用层「先查后写」（那中间有竞争窗口）。
    pub fn begin(
        &self,
        plan_id: &str,
        request_id: &str,
        direction: &str,
        now_ms: i64,
    ) -> Result<BeginOutcome, AppError> {
        if let Some(existing) = runs::find_run_by_request(self.db, request_id)? {
            if existing.plan_id != plan_id || existing.direction != direction {
                return Err(AppError::new(
                    codes::REQUEST_CONFLICT,
                    "请求已绑定另一计划或方向",
                ));
            }
            return Ok(BeginOutcome::Existing(existing.id));
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        let started_at = crate::domain::time::rfc3339_from_unix_ms(now_ms);

        match insert_run(
            self.db,
            &run_id,
            plan_id,
            request_id,
            direction,
            &started_at,
        ) {
            Ok(()) => Ok(BeginOutcome::Created(run_id)),
            // 并发重试：唯一约束撞上了，说明另一个请求已经建好 run。
            // 这不是错误，而是幂等语义在起作用。
            Err(error) if error.code == codes::REQUEST_CONFLICT => {
                match runs::find_run_by_request(self.db, request_id)? {
                    Some(row) if row.plan_id == plan_id && row.direction == direction => {
                        Ok(BeginOutcome::Existing(row.id))
                    }
                    // 撞了约束却查不到，说明不是「已存在」而是别的问题，如实上报
                    _ => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// 派发前写下意图。**这是整个安全链条的支点。**
    ///
    /// 它成功返回之后，即使进程立刻死掉，恢复流程也能看到「这一项被标记为
    /// 即将移动」，从而去磁盘上核对到底动了没有，而不是靠猜。
    pub fn record_intent(&self, row: &OperationRow) -> Result<(), AppError> {
        insert_operation(self.db, row)
    }

    /// 记录一个待创建的目录。
    ///
    /// 规格 8.2：目录创建只能逐级在批准根内进行。这条记录的意义是
    /// 「若创建成功但身份没落盘，不猜其归属，恢复时保留该目录」——
    /// 所以意图要在**创建之前**写。
    pub fn record_dir_intent(&self, row: &CreatedDirRow) -> Result<(), AppError> {
        insert_created_dir(self.db, row)
    }

    /// 意图已落盘，即将移动。
    pub fn mark_prepared(&self, operation_id: &str, now_ms: i64) -> Result<(), AppError> {
        self.set_status_and_event(
            operation_id,
            OpStatus::Prepared,
            PHASE_PREPARED,
            now_ms,
            None,
        )
    }

    /// 移动完成并已核对。
    pub fn mark_applied(&self, operation_id: &str, now_ms: i64) -> Result<(), AppError> {
        self.set_status_and_event(operation_id, OpStatus::Applied, PHASE_APPLIED, now_ms, None)
    }

    /// 明确失败，且**确定文件没有被改动**。
    pub fn mark_failed(
        &self,
        operation_id: &str,
        code: &str,
        message: &str,
        now_ms: i64,
    ) -> Result<(), AppError> {
        self.set_status_and_event(
            operation_id,
            OpStatus::Failed,
            PHASE_FAILED,
            now_ms,
            Some((code, message)),
        )
    }

    /// 因停止而没被派发。
    pub fn mark_skipped(&self, operation_id: &str, now_ms: i64) -> Result<(), AppError> {
        self.set_status_and_event(operation_id, OpStatus::Skipped, PHASE_SKIPPED, now_ms, None)
    }

    /// 无法判定是否被改动 —— 不猜，交给人。
    ///
    /// 规格 8.2：rename 成功但日志失败时进入这个状态，
    /// 任务随之变成 `recoveryRequired`，且**不显示普通失败后可随意重试的入口**。
    pub fn mark_ambiguous(
        &self,
        operation_id: &str,
        reason: &str,
        now_ms: i64,
    ) -> Result<(), AppError> {
        self.set_status_and_event(
            operation_id,
            OpStatus::Ambiguous,
            PHASE_AMBIGUOUS,
            now_ms,
            Some(("RECOVERY_REQUIRED", reason)),
        )
    }

    /// 记录目录已创建并取得身份。
    pub fn record_dir_created(
        &self,
        operation_id: &str,
        relative: &RelPath,
        identity: &str,
        created_by_run: bool,
        now_ms: i64,
    ) -> Result<(), AppError> {
        let row = CreatedDirRow {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: current_run_of(self.db, operation_id)?,
            relative_path: relative.clone(),
            directory_identity: identity.to_owned(),
            created_by_run,
            state: if created_by_run {
                "created"
            } else {
                "existing"
            }
            .to_owned(),
        };
        insert_created_dir(self.db, &row)?;
        self.append(operation_id, PHASE_DIR_CREATED, now_ms, Some(identity))
    }

    /// 收尾。
    pub fn finish(&self, run_id: &str, status: RunStatus, now_ms: i64) -> Result<(), AppError> {
        finish_run(
            self.db,
            run_id,
            status,
            &crate::domain::time::rfc3339_from_unix_ms(now_ms),
        )
    }

    // -----------------------------------------------------------------------

    fn set_status_and_event(
        &self,
        operation_id: &str,
        status: OpStatus,
        phase: &str,
        now_ms: i64,
        error: Option<(&str, &str)>,
    ) -> Result<(), AppError> {
        let payload = error.map(|(code, message)| {
            serde_json::json!({ "code": code, "message": message }).to_string()
        });
        set_operation_status_and_event(
            self.db,
            operation_id,
            status,
            error.map(|(code, _)| code),
            &uuid::Uuid::new_v4().to_string(),
            phase,
            &crate::domain::time::rfc3339_from_unix_ms(now_ms),
            payload.as_deref(),
        )
    }

    fn append(
        &self,
        operation_id: &str,
        phase: &str,
        now_ms: i64,
        payload_json: Option<&str>,
    ) -> Result<(), AppError> {
        append_event(
            self.db,
            &uuid::Uuid::new_v4().to_string(),
            operation_id,
            phase,
            &crate::domain::time::rfc3339_from_unix_ms(now_ms),
            payload_json,
        )
    }
}

/// 取一项操作所属的 runId。
fn current_run_of(db: &Database, operation_id: &str) -> Result<String, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT runId FROM operations WHERE id = ?1",
            [operation_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| AppError::internal(format!("找不到操作 {operation_id}: {error}")))
}

/// 供执行器构造一条待记录的操作。
pub fn new_operation(
    run_id: &str,
    item_id: &str,
    sequence: u32,
    source: &RelPath,
    target: &RelPath,
    expected_json: &str,
) -> OperationRow {
    OperationRow {
        id: uuid::Uuid::new_v4().to_string(),
        run_id: run_id.to_owned(),
        item_id: item_id.to_owned(),
        // 正向整理的操作不指回任何东西；只有撤销 run 才填这一列。
        original_operation_id: None,
        sequence,
        source: source.clone(),
        target: target.clone(),
        expected_json: expected_json.to_owned(),
        status: OpStatus::Pending,
        undo_status: UndoStatus::NotRequested,
        // 新派发的操作必然是「尚未处置」：处置只针对判不出来的项。
        resolution: OpResolution::Open,
        error_code: None,
    }
}

/// 供撤销执行器构造一条「反向移动」的操作。
///
/// `original_operation_id` 是**唯一**能回答「这一项撤过没有」的凭据
/// （规格 5.1）：不填它，重复撤销就只能靠内存记忆，而重启后就什么都想不起来。
pub fn new_undo_operation(
    run_id: &str,
    item_id: &str,
    original_operation_id: &str,
    sequence: u32,
    source: &RelPath,
    target: &RelPath,
    expected_json: &str,
) -> OperationRow {
    OperationRow {
        original_operation_id: Some(original_operation_id.to_owned()),
        ..new_operation(run_id, item_id, sequence, source, target, expected_json)
    }
}
