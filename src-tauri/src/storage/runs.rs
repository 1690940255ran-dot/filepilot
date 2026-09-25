//! 执行记录（run / operation / event / created_dir）的读写。
//!
//! 规格 8.1：这些是**审计事实**，不是缓存。因此：
//! - 只追加，不改写历史（事件表没有 UPDATE 路径）；
//! - 状态变更走明确的函数，不提供「随便写一个状态」的入口；
//! - `requestId` 唯一约束把「同一请求重试不新增 run」交给数据库保证，
//!   而不是靠应用层先查后写（那中间有竞争窗口）。
//!
//! 与 `repositories.rs` 分开：那一组围绕扫描与计划，这一组围绕**执行**。
//! 两者的失败模式不同——执行侧的写入失败必须能阻断文件变更。

use rusqlite::{params, OptionalExtension};

use crate::domain::errors::AppError;
use crate::domain::types::{OpResolution, OpStatus, RelPath, RunCounts, RunStatus, UndoStatus};
use crate::storage::db::{map_db_error, Database};

/// `runs` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub id: String,
    pub plan_id: String,
    pub request_id: String,
    /// 目前只有 `apply`；`undo` 属 T09。
    pub direction: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

/// `operations` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRow {
    pub id: String,
    pub run_id: String,
    pub item_id: String,
    /// 被本项撤销的原操作（仅 `direction = 'undo'` 的 run 会填）。
    ///
    /// 规格 5.1：`undoRunId` 是 `direction='undo'` 的普通 run，
    /// 它的每一项都指回被撤销的那个操作——**这是「哪些已经撤了」的唯一凭据**，
    /// 不能靠「内存里记着刚才撤过谁」。
    pub original_operation_id: Option<String>,
    pub sequence: u32,
    pub source: RelPath,
    pub target: RelPath,
    /// 源文件指纹的 JSON，执行前要拿它核对。
    pub expected_json: String,
    pub status: OpStatus,
    pub undo_status: UndoStatus,
    /// 未决事实的人工处置结果（规格 8.2）。
    ///
    /// 与 `status` 是**两个维度**：`status = ambiguous` 说的是「磁盘事实判不出来」，
    /// `resolution = acknowledged` 说的是「用户已经看过并决定保留原状」。
    /// 合在一起会丢掉其中一个事实——要么掩盖掉「这一项其实没执行完」，
    /// 要么让用户每次启动都被同一件事拦住。
    pub resolution: OpResolution,
    pub error_code: Option<String>,
}

/// `operation_events` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    pub id: String,
    pub operation_id: String,
    /// `prepared` / `applied` / `reverted` / …
    pub phase: String,
    pub timestamp: String,
    pub payload_json: Option<String>,
}

/// `created_dirs` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedDirRow {
    pub id: String,
    pub run_id: String,
    pub relative_path: RelPath,
    pub directory_identity: String,
    /// 是否**由本次运行**创建。已有目录只验证，不宣称由本次创建（规格 8.2）。
    pub created_by_run: bool,
    pub state: String,
}

// ---------------------------------------------------------------------------
// runs
// ---------------------------------------------------------------------------

/// 建一条 run。
///
/// `requestId` 由唯一索引兜底：并发重试时只有一个能插入成功，
/// 另一个会撞约束。调用方应当捕获它并改用 [`find_run_by_request`] 的结果。
pub fn insert_run(
    db: &Database,
    run_id: &str,
    plan_id: &str,
    request_id: &str,
    direction: &str,
    started_at: &str,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO runs (id, planId, requestId, direction, status, startedAt, finishedAt)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5, NULL)",
                params![run_id, plan_id, request_id, direction, started_at],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 按 `requestId` 找已有的 run —— 幂等重试的入口。
pub fn find_run_by_request(db: &Database, request_id: &str) -> Result<Option<RunRow>, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT id, planId, requestId, direction, status, startedAt, finishedAt
             FROM runs WHERE requestId = ?1",
            [request_id],
            |row| {
                Ok(RunRow {
                    id: row.get(0)?,
                    plan_id: row.get(1)?,
                    request_id: row.get(2)?,
                    direction: row.get(3)?,
                    status: row.get(4)?,
                    started_at: row.get(5)?,
                    finished_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(map_db_error)
}

/// 按 id 读一条 run。
pub fn load_run(db: &Database, run_id: &str) -> Result<Option<RunRow>, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT id, planId, requestId, direction, status, startedAt, finishedAt
             FROM runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok(RunRow {
                    id: row.get(0)?,
                    plan_id: row.get(1)?,
                    request_id: row.get(2)?,
                    direction: row.get(3)?,
                    status: row.get(4)?,
                    started_at: row.get(5)?,
                    finished_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(map_db_error)
}

/// 写入终态。
pub fn finish_run(
    db: &Database,
    run_id: &str,
    status: RunStatus,
    finished_at: &str,
) -> Result<(), AppError> {
    let text = status_to_text(status);
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE runs SET status = ?2, finishedAt = ?3 WHERE id = ?1",
                params![run_id, text, finished_at],
            )
            .map_err(map_write_error)?;
        if changed == 0 {
            return Err(AppError::internal(format!("run {run_id} 不存在")));
        }
        Ok(())
    })
}

/// 列出最近的执行记录，最新的在前。
///
/// 规格 T07：history 页面要「从数据库恢复状态」，因此这里不返回内存里的任务，
/// 而是直接读 `runs` 表——应用重启后历史仍在，这正是它与运行期任务表的区别。
pub fn list_runs(db: &Database, limit: u32) -> Result<Vec<RunRow>, AppError> {
    // limit 由调用方给，但不能无限大：一次读几十万行会让界面卡死。
    let limit = i64::from(limit.clamp(1, MAX_LISTED_RUNS));

    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT id, planId, requestId, direction, status, startedAt, finishedAt
             FROM runs ORDER BY startedAt DESC, rowid DESC LIMIT ?1",
        )
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([limit], |row| {
            Ok(RunRow {
                id: row.get(0)?,
                plan_id: row.get(1)?,
                request_id: row.get(2)?,
                direction: row.get(3)?,
                status: row.get(4)?,
                started_at: row.get(5)?,
                finished_at: row.get(6)?,
            })
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;
    Ok(rows)
}

/// 一次最多列出多少条执行记录。
pub const MAX_LISTED_RUNS: u32 = 200;

/// 所有仍处于非终态的 run。
///
/// 规格 8.3：启动时发现这类记录要标记 `recoveryRequired`——
/// 一个「running」的 run 只可能是上次进程没走完。
pub fn unfinished_runs(db: &Database) -> Result<Vec<RunRow>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT id, planId, requestId, direction, status, startedAt, finishedAt
             FROM runs WHERE status IN ('queued', 'running') ORDER BY startedAt",
        )
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([], |row| {
            Ok(RunRow {
                id: row.get(0)?,
                plan_id: row.get(1)?,
                request_id: row.get(2)?,
                direction: row.get(3)?,
                status: row.get(4)?,
                started_at: row.get(5)?,
                finished_at: row.get(6)?,
            })
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// operations
// ---------------------------------------------------------------------------

/// 写入一条操作。重复调用（同一 runId + itemId）不覆盖已有的状态。
///
/// 用 `INSERT OR IGNORE` 而不是 UPSERT：这个函数只在派发前调用一次，
/// 若同一项被重复派发，应当保留**第一次**的事实，而不是把状态冲掉。
pub fn insert_operation(db: &Database, row: &OperationRow) -> Result<(), AppError> {
    let source = serde_json::to_string(&row.source).map_err(serialize_error)?;
    let target = serde_json::to_string(&row.target).map_err(serialize_error)?;

    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT OR IGNORE INTO operations
                   (id, runId, itemId, originalOperationId, sequence, sourceJson, targetJson,
                    expectedJson, status, undoStatus, resolution, errorCode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    row.id,
                    row.run_id,
                    row.item_id,
                    row.original_operation_id,
                    i64::from(row.sequence),
                    source,
                    target,
                    row.expected_json,
                    op_status_to_text(row.status),
                    undo_status_to_text(row.undo_status),
                    op_resolution_to_text(row.resolution),
                    row.error_code,
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 更新一项操作的状态。
///
/// **不允许从终态改回非终态**：`applied` 一旦写下就是事实，重跑不能把它抹掉。
/// 这条由 SQL 的 `WHERE status NOT IN (...)` 保证，避免并发或重复调用破坏审计。
pub fn set_operation_status(
    db: &Database,
    operation_id: &str,
    status: OpStatus,
    error_code: Option<&str>,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE operations SET status = ?2, errorCode = ?3
                 WHERE id = ?1 AND status NOT IN ('applied')",
                params![operation_id, op_status_to_text(status), error_code],
            )
            .map_err(map_write_error)?;
        Ok(changed)
    })
    .map(|_| ())
}

/// 原子地更新操作状态并追加对应事件；任一步失败则两者一起回滚。
#[allow(clippy::too_many_arguments)]
pub fn set_operation_status_and_event(
    db: &Database,
    operation_id: &str,
    status: OpStatus,
    error_code: Option<&str>,
    event_id: &str,
    phase: &str,
    timestamp: &str,
    payload_json: Option<&str>,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE operations SET status = ?2, errorCode = ?3
                 WHERE id = ?1 AND status NOT IN ('applied')",
                params![operation_id, op_status_to_text(status), error_code],
            )
            .map_err(map_write_error)?;
        if changed != 1 {
            return Err(AppError::new(
                crate::domain::errors::codes::JOURNAL_WRITE_FAILED,
                "操作状态未更新，拒绝追加不一致的审计事件",
            ));
        }
        transaction
            .execute(
                "INSERT INTO operation_events (id, operationId, phase, timestamp, payloadJson)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![event_id, operation_id, phase, timestamp, payload_json],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 读一次 run 下的全部操作，按派发顺序。
pub fn list_operations(db: &Database, run_id: &str) -> Result<Vec<OperationRow>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT id, runId, itemId, originalOperationId, sequence, sourceJson, targetJson,
                    expectedJson, status, undoStatus, resolution, errorCode
             FROM operations WHERE runId = ?1 ORDER BY sequence",
        )
        .map_err(map_db_error)?;

    let raw = statement
        .query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;

    raw.into_iter()
        .map(
            |(
                id,
                run_id,
                item_id,
                original_operation_id,
                sequence,
                source,
                target,
                expected,
                status,
                undo,
                resolution,
                error,
            )| {
                Ok(OperationRow {
                    id,
                    run_id,
                    item_id,
                    original_operation_id,
                    sequence: u32::try_from(sequence)
                        .map_err(|_| AppError::internal("操作序号超出范围"))?,
                    source: serde_json::from_str(&source).map_err(deserialize_error)?,
                    target: serde_json::from_str(&target).map_err(deserialize_error)?,
                    expected_json: expected,
                    status: op_status_from_text(&status)?,
                    undo_status: undo_status_from_text(&undo)?,
                    resolution: op_resolution_from_text(&resolution)?,
                    error_code: error,
                })
            },
        )
        .collect()
}

/// 记下一项未决事实的**人工处置**：保留现状并确认知晓（规格 8.2）。
///
/// 三条刻意的约束，每一条都对应一种「看起来能工作、实际会出事」的写法：
///
/// 1. **只接受 `ambiguous`** —— 对 `applied` 写「已确认保留」没有意义，
///    对 `failed` 写则会把「这次没做成、可以重来」悄悄变成「就这样吧」。
/// 2. **不碰 `status`** —— 原始判定是事实，确认知晓只是加了一个态度，
///    把 status 改成别的值就等于篡改审计记录。
/// 3. **`changed == 0` 必须报错** —— 幂等重放（同一份确认提交两次）走
///    [`find_operation`] 的早返回，不应该走到这里；走到这里说明 id 不存在、
///    状态不是 ambiguous，或已被别的路径改过。静默吞掉会让界面显示「已确认」
///    而库里一字未改。这与 `set_operation_status` 不同：那里「没改动」
///    是正常的（`applied` 不可改写是有意的保护）。
pub fn acknowledge_operation(db: &Database, operation_id: &str) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE operations SET resolution = 'acknowledged'
                 WHERE id = ?1 AND status = 'ambiguous'",
                params![operation_id],
            )
            .map_err(map_write_error)?;
        if changed == 0 {
            return Err(AppError::new(
                crate::domain::errors::codes::REQUEST_CONFLICT,
                format!("操作 {operation_id} 当前不是待确认的歧义状态，无法标记为已确认"),
            ));
        }
        Ok(())
    })
}

/// 按 id 读一项操作。找不到返回 `None`。
pub fn find_operation(db: &Database, operation_id: &str) -> Result<Option<OperationRow>, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT id, runId, itemId, originalOperationId, sequence, sourceJson, targetJson,
                    expectedJson, status, undoStatus, resolution, errorCode
             FROM operations WHERE id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            },
        )
        .optional()
        .map_err(map_db_error)?
        .map(
            |(
                id,
                run_id,
                item_id,
                original_operation_id,
                sequence,
                source,
                target,
                expected,
                status,
                undo,
                resolution,
                error,
            )| {
                Ok::<_, AppError>(OperationRow {
                    id,
                    run_id,
                    item_id,
                    original_operation_id,
                    sequence: u32::try_from(sequence)
                        .map_err(|_| AppError::internal("操作序号超出范围"))?,
                    source: serde_json::from_str(&source).map_err(deserialize_error)?,
                    target: serde_json::from_str(&target).map_err(deserialize_error)?,
                    expected_json: expected,
                    status: op_status_from_text(&status)?,
                    undo_status: undo_status_from_text(&undo)?,
                    resolution: op_resolution_from_text(&resolution)?,
                    error_code: error,
                })
            },
        )
        .transpose()
}

/// 统计一次 run 的各状态数量。
pub fn count_operations(db: &Database, run_id: &str) -> Result<RunCounts, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare("SELECT status, COUNT(*) FROM operations WHERE runId = ?1 GROUP BY status")
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([run_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;

    let mut counts = RunCounts::default();
    for (status, count) in rows {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        match status.as_str() {
            "applied" => counts.applied = count,
            "failed" => counts.failed = count,
            "skipped" => counts.skipped = count,
            // 歧义项单独计数：它既不是「做成了」也不是「还没轮到」，
            // 界面要给它一句完全不同的话，所以不能并进 pending。
            "ambiguous" => counts.ambiguous = count,
            // pending / prepared 都还没有「完成」，归入 pending 以免界面上凭空少了几项。
            _ => counts.pending += count,
        }
    }
    Ok(counts)
}

// ---------------------------------------------------------------------------
// operation_events
// ---------------------------------------------------------------------------

/// 追加一个事件。
///
/// 事件表**只增不改**：它是崩溃恢复唯一的依据，改写它就等于篡改历史。
pub fn append_event(
    db: &Database,
    event_id: &str,
    operation_id: &str,
    phase: &str,
    timestamp: &str,
    payload_json: Option<&str>,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO operation_events (id, operationId, phase, timestamp, payloadJson)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![event_id, operation_id, phase, timestamp, payload_json],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 读一项操作的事件，按写入顺序。
pub fn list_events(db: &Database, operation_id: &str) -> Result<Vec<EventRow>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT id, operationId, phase, timestamp, payloadJson
             FROM operation_events WHERE operationId = ?1 ORDER BY rowid",
        )
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([operation_id], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                operation_id: row.get(1)?,
                phase: row.get(2)?,
                timestamp: row.get(3)?,
                payload_json: row.get(4)?,
            })
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// created_dirs
// ---------------------------------------------------------------------------

pub fn insert_created_dir(db: &Database, row: &CreatedDirRow) -> Result<(), AppError> {
    let path = serde_json::to_string(&row.relative_path).map_err(serialize_error)?;
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO created_dirs
                   (id, runId, relativePathJson, directoryIdentity, createdByRun, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    row.id,
                    row.run_id,
                    path,
                    row.directory_identity,
                    i64::from(row.created_by_run),
                    row.state,
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

pub fn list_created_dirs(db: &Database, run_id: &str) -> Result<Vec<CreatedDirRow>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT id, runId, relativePathJson, directoryIdentity, createdByRun, state
             FROM created_dirs WHERE runId = ?1 ORDER BY rowid",
        )
        .map_err(map_db_error)?;

    let raw = statement
        .query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;

    raw.into_iter()
        .map(|(id, run_id, path, identity, created_by_run, state)| {
            Ok(CreatedDirRow {
                id,
                run_id,
                relative_path: serde_json::from_str(&path).map_err(deserialize_error)?,
                directory_identity: identity,
                created_by_run: created_by_run != 0,
                state,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 撤销（规格 8.4、T09）
// ---------------------------------------------------------------------------

/// `undo_plans` 的一行。
///
/// 规格 8.1：撤销计划**必须持久化**，不能借前端本地状态替代——
/// 否则界面一刷新，「用户确认过的那份撤销清单」就没了凭据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoPlanRow {
    pub id: String,
    pub original_run_id: String,
    pub digest: String,
    pub items_json: String,
    pub token_hash: String,
    pub expires_at: String,
    pub consumed_at: Option<String>,
}

/// 写入一份撤销计划（含摘要与一次性令牌哈希）。
pub fn insert_undo_plan(db: &Database, row: &UndoPlanRow) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO undo_plans
                   (id, originalRunId, digest, itemsJson, tokenHash, expiresAt, consumedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![
                    row.id,
                    row.original_run_id,
                    row.digest,
                    row.items_json,
                    row.token_hash,
                    row.expires_at,
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 按 id 读一份撤销计划。
pub fn load_undo_plan(db: &Database, id: &str) -> Result<Option<UndoPlanRow>, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT id, originalRunId, digest, itemsJson, tokenHash, expiresAt, consumedAt
             FROM undo_plans WHERE id = ?1",
            [id],
            |row| {
                Ok(UndoPlanRow {
                    id: row.get(0)?,
                    original_run_id: row.get(1)?,
                    digest: row.get(2)?,
                    items_json: row.get(3)?,
                    token_hash: row.get(4)?,
                    expires_at: row.get(5)?,
                    consumed_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(map_db_error)
}

/// 消费一次性令牌。
///
/// 用**条件 UPDATE + 受影响行数**判定，而不是「先查再改」：
/// 后者在两个请求同时提交时会双双通过检查，于是同一份确认被用两次。
/// 这里由数据库保证「只有一个能把 `consumedAt` 从 NULL 改成时间」。
///
/// 返回 `false` 表示令牌不可用（不存在、哈希不符、已用过或已过期）。
pub fn consume_undo_plan(
    db: &Database,
    id: &str,
    token_hash: &str,
    now: &str,
) -> Result<bool, AppError> {
    let changed = db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE undo_plans SET consumedAt = ?3
                 WHERE id = ?1 AND tokenHash = ?2 AND consumedAt IS NULL AND expiresAt > ?3",
                params![id, token_hash, now],
            )
            .map_err(map_write_error)?;
        Ok(changed)
    })?;
    Ok(changed == 1)
}

/// 已撤销过的原操作 id 集合。
///
/// 判据是**库里的事实**：存在一条 `direction='undo'` 的操作指回它，
/// 且那条反向操作已经 `applied`。这样即便中途崩过、或者用户换了台机器重开，
/// 「这一项撤过没有」依然答得出来——不依赖任何内存记忆。
pub fn undone_original_ids(db: &Database, original_run_id: &str) -> Result<Vec<String>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT o.id
             FROM operations o
             WHERE o.runId = ?1
               AND EXISTS (
                   SELECT 1 FROM operations u
                   JOIN runs r ON r.id = u.runId
                   WHERE u.originalOperationId = o.id
                     AND r.direction = 'undo'
                     AND u.status = 'applied'
               )",
        )
        .map_err(map_db_error)?;

    let ids = statement
        .query_map([original_run_id], |row| row.get::<_, String>(0))
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;

    Ok(ids)
}

/// 更新一项原操作的撤销状态。
pub fn set_undo_status(
    db: &Database,
    operation_id: &str,
    status: UndoStatus,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "UPDATE operations SET undoStatus = ?2 WHERE id = ?1",
                params![operation_id, undo_status_to_text(status)],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 反向移动开始前的原子提交：「undo prepared」同时落到反向操作与原操作上。
///
/// 规格 8.4 第 5 条：**先记 undo prepared，后记 undone**。
/// 两步都必须是原子的——只要出现「反向操作是 prepared、原操作还是 notRequested」
/// 这种组合，恢复流程就无法判断这次撤销到底走到哪了。
pub fn commit_undo_prepared(
    db: &Database,
    undo_operation_id: &str,
    original_operation_id: &str,
    event_id: &str,
    timestamp: &str,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "UPDATE operations SET status = 'prepared' WHERE id = ?1",
                [undo_operation_id],
            )
            .map_err(map_write_error)?;
        transaction
            .execute(
                "UPDATE operations SET undoStatus = 'prepared' WHERE id = ?1",
                [original_operation_id],
            )
            .map_err(map_write_error)?;

        for (operation_id, phase) in [
            (undo_operation_id, "prepared"),
            (original_operation_id, UNDO_PREPARED_EVENT),
        ] {
            transaction
                .execute(
                    "INSERT INTO operation_events (id, operationId, phase, timestamp, payloadJson)
                     VALUES (?1, ?2, ?3, ?4, NULL)",
                    params![
                        format!("{event_id}:{phase}"),
                        operation_id,
                        phase,
                        timestamp
                    ],
                )
                .map_err(map_write_error)?;
        }
        Ok(())
    })
}

/// 反向移动完成后的**同一事务**提交（规格 5.1）。
///
/// 「反向移动已完成」与「原操作 undoStatus = undone」必须一起落盘：
/// 分开写就会出现「文件已经回去了，日志还说没撤」的窗口，
/// 而那个窗口里的崩溃会让用户看到「未撤销」从而再撤一次。
pub fn commit_revert(
    db: &Database,
    undo_operation_id: &str,
    original_operation_id: &str,
    event_id: &str,
    timestamp: &str,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "UPDATE operations SET status = 'applied' WHERE id = ?1",
                [undo_operation_id],
            )
            .map_err(map_write_error)?;
        transaction
            .execute(
                "UPDATE operations SET undoStatus = 'undone' WHERE id = ?1",
                [original_operation_id],
            )
            .map_err(map_write_error)?;

        for (operation_id, phase) in [
            (undo_operation_id, "applied"),
            (original_operation_id, UNDO_DONE_EVENT),
        ] {
            transaction
                .execute(
                    "INSERT INTO operation_events (id, operationId, phase, timestamp, payloadJson)
                     VALUES (?1, ?2, ?3, ?4, NULL)",
                    params![
                        format!("{event_id}:{phase}"),
                        operation_id,
                        phase,
                        timestamp
                    ],
                )
                .map_err(map_write_error)?;
        }
        Ok(())
    })
}

/// 标记一项为「有冲突，保留现状」。
///
/// 只动 `undoStatus`，**不动 `status`**：这一项当初确实执行成功了
/// （`applied` 是事实），只是现在撤不回去。把 status 改掉会抹掉那个事实。
pub fn mark_undo_conflict(
    db: &Database,
    original_operation_id: &str,
    event_id: &str,
    timestamp: &str,
    reason: &str,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "UPDATE operations SET undoStatus = 'conflict' WHERE id = ?1",
                [original_operation_id],
            )
            .map_err(map_write_error)?;
        transaction
            .execute(
                "INSERT INTO operation_events (id, operationId, phase, timestamp, payloadJson)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event_id,
                    original_operation_id,
                    UNDO_CONFLICT_EVENT,
                    timestamp,
                    serde_json::json!({ "reason": reason }).to_string()
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 原操作被撤销时写在**原操作**上的事件名。
const UNDO_PREPARED_EVENT: &str = "undoPrepared";
/// 原操作被成功撤销时写在**原操作**上的事件名。
const UNDO_DONE_EVENT: &str = "undone";
/// 原操作撤销受阻时的事件名。
const UNDO_CONFLICT_EVENT: &str = "undoConflict";

// ---------------------------------------------------------------------------
// 内部
// ---------------------------------------------------------------------------

/// 写失败统一映射成 `JOURNAL_WRITE_FAILED`。
///
/// 规格 8.5：这个码的动作是「停止执行，先恢复数据库可用性」，
/// 因此它必须与「数据库整体不可用」(`DB_UNAVAILABLE`) 区分开——
/// 后者是打不开库，前者是库在但这次写入没成。
fn map_write_error(error: rusqlite::Error) -> AppError {
    let message = error.to_string();
    if message.contains("UNIQUE constraint failed") {
        // 让调用方（尤其是 insert_run）能识别出来，改用幂等路径
        return AppError::new(
            crate::domain::errors::codes::REQUEST_CONFLICT,
            format!("该请求已存在：{message}"),
        );
    }
    AppError::new(
        crate::domain::errors::codes::JOURNAL_WRITE_FAILED,
        format!("执行日志写入失败: {message}"),
    )
}

fn serialize_error(error: serde_json::Error) -> AppError {
    AppError::internal(format!("序列化失败: {error}"))
}

fn deserialize_error(error: serde_json::Error) -> AppError {
    AppError::internal(format!("反序列化失败: {error}"))
}

fn status_to_text(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Partial => "partial",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::RecoveryRequired => "recoveryRequired",
    }
}

pub fn run_status_from_text(value: &str) -> Result<RunStatus, AppError> {
    match value {
        "queued" => Ok(RunStatus::Queued),
        "running" => Ok(RunStatus::Running),
        "completed" => Ok(RunStatus::Completed),
        "partial" => Ok(RunStatus::Partial),
        "failed" => Ok(RunStatus::Failed),
        "cancelled" => Ok(RunStatus::Cancelled),
        "recoveryRequired" => Ok(RunStatus::RecoveryRequired),
        other => Err(AppError::internal(format!("未知的 run 状态 {other}"))),
    }
}

fn op_status_to_text(status: OpStatus) -> &'static str {
    match status {
        OpStatus::Pending => "pending",
        OpStatus::Prepared => "prepared",
        OpStatus::Applied => "applied",
        OpStatus::Failed => "failed",
        OpStatus::Skipped => "skipped",
        OpStatus::Ambiguous => "ambiguous",
    }
}

fn op_status_from_text(value: &str) -> Result<OpStatus, AppError> {
    match value {
        "pending" => Ok(OpStatus::Pending),
        "prepared" => Ok(OpStatus::Prepared),
        "applied" => Ok(OpStatus::Applied),
        "failed" => Ok(OpStatus::Failed),
        "skipped" => Ok(OpStatus::Skipped),
        "ambiguous" => Ok(OpStatus::Ambiguous),
        other => Err(AppError::internal(format!("未知的操作状态 {other}"))),
    }
}

fn undo_status_to_text(status: UndoStatus) -> &'static str {
    match status {
        UndoStatus::NotRequested => "notRequested",
        UndoStatus::Prepared => "prepared",
        UndoStatus::Undone => "undone",
        UndoStatus::Conflict => "conflict",
    }
}

fn undo_status_from_text(value: &str) -> Result<UndoStatus, AppError> {
    match value {
        "notRequested" => Ok(UndoStatus::NotRequested),
        "prepared" => Ok(UndoStatus::Prepared),
        "undone" => Ok(UndoStatus::Undone),
        "conflict" => Ok(UndoStatus::Conflict),
        other => Err(AppError::internal(format!("未知的撤销状态 {other}"))),
    }
}

fn op_resolution_to_text(resolution: OpResolution) -> &'static str {
    match resolution {
        OpResolution::Open => "open",
        OpResolution::Acknowledged => "acknowledged",
    }
}

fn op_resolution_from_text(value: &str) -> Result<OpResolution, AppError> {
    match value {
        "open" => Ok(OpResolution::Open),
        "acknowledged" => Ok(OpResolution::Acknowledged),
        other => Err(AppError::internal(format!("未知的处置状态 {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::repositories::count_rows;

    fn db() -> Database {
        Database::open_in_memory().expect("应能初始化内存库")
    }

    /// 建一条最小可用的 plan。
    ///
    /// `runs.planId` 有外键指向 `plans`，而 `plans` 又指向 `scans` 与 `roots`。
    /// 测试要跑真实的约束（而不是把它关掉），就得把这一小段依赖链补上——
    /// 这条链本身就是「执行必须从一个真实落库的计划出发」的体现。
    fn seed_plan(db: &Database, plan_id: &str) {
        let connection = db.connection();
        connection
            .execute_batch(&format!(
                "INSERT INTO roots (id, canonicalPath, volumeId, identity, sessionId)
                   VALUES ('root-1', 'C:\\资料', 'V', 'I', 'S');
                 INSERT INTO scans (id, rootId, status, recursive, startedAt)
                   VALUES ('scan-1', 'root-1', 'completed', 1, 't0');
                 INSERT INTO plans (id, scanId, rootId, revision, status, mode, createdAt)
                   VALUES ('{plan_id}', 'scan-1', 'root-1', 1, 'validated', 'rules', 't0');"
            ))
            .expect("应能建出计划依赖链");
    }

    fn op(id: &str, run: &str, item: &str, seq: u32) -> OperationRow {
        OperationRow {
            id: id.to_owned(),
            run_id: run.to_owned(),
            item_id: item.to_owned(),
            // 正向整理的操作不指回任何东西；撤销用例另走 new_undo_operation。
            original_operation_id: None,
            sequence: seq,
            source: vec![format!("{item}.txt")],
            target: vec!["文档".to_owned(), format!("{item}.txt")],
            expected_json: "{}".to_owned(),
            status: OpStatus::Pending,
            undo_status: UndoStatus::NotRequested,
            resolution: OpResolution::Open,
            error_code: None,
        }
    }

    #[test]
    fn a_request_id_maps_to_exactly_one_run() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(
            &db,
            "run-1",
            "plan-1",
            "req-1",
            "apply",
            "2026-09-17T00:00:00.000Z",
        )
        .expect("首次插入应成功");

        let found = find_run_by_request(&db, "req-1").expect("查询应成功");
        assert_eq!(found.map(|r| r.id), Some("run-1".to_owned()));

        // 同一 requestId 再插会撞唯一约束，并被映射成 REQUEST_CONFLICT
        let err = insert_run(
            &db,
            "run-2",
            "plan-1",
            "req-1",
            "apply",
            "2026-09-17T00:00:01.000Z",
        )
        .expect_err("重复 requestId 必须失败");
        assert_eq!(err.code, crate::domain::errors::codes::REQUEST_CONFLICT);

        // 而且不会多出一条 run
        assert_eq!(count_rows(&db, "runs").expect("计数"), 1);
    }

    #[test]
    fn applied_status_cannot_be_rewritten() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        insert_operation(&db, &op("op-1", "run-1", "item-1", 0)).expect("建操作");

        set_operation_status(&db, "op-1", OpStatus::Prepared, None).expect("置 prepared");
        set_operation_status(&db, "op-1", OpStatus::Applied, None).expect("置 applied");

        // 事后再想改回 failed 必须无效 —— 审计事实不能被抹掉
        set_operation_status(&db, "op-1", OpStatus::Failed, Some("SOME_CODE"))
            .expect("调用本身不报错");

        let ops = list_operations(&db, "run-1").expect("读操作");
        assert_eq!(ops[0].status, OpStatus::Applied, "applied 必须保持不变");
        assert!(ops[0].error_code.is_none(), "错误码也不该被写进去");
    }

    #[test]
    fn re_inserting_the_same_item_keeps_the_first_fact() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        insert_operation(&db, &op("op-1", "run-1", "item-1", 0)).expect("首次");
        insert_operation(&db, &op("op-2", "run-1", "item-1", 0)).expect("重复派发不报错");

        let ops = list_operations(&db, "run-1").expect("读操作");
        assert_eq!(ops.len(), 1, "同一 itemId 只应留一条");
        assert_eq!(ops[0].id, "op-1", "保留的是第一条");
    }

    #[test]
    fn events_are_append_only_and_ordered() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        insert_operation(&db, &op("op-1", "run-1", "item-1", 0)).expect("建操作");

        append_event(&db, "e1", "op-1", "prepared", "t1", None).expect("追加 prepared");
        append_event(
            &db,
            "e2",
            "op-1",
            "applied",
            "t2",
            Some("{\"fileId\":\"F\"}"),
        )
        .expect("追加 applied");

        let events = list_events(&db, "op-1").expect("读事件");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase, "prepared");
        assert_eq!(events[1].phase, "applied");
    }

    #[test]
    fn counts_give_ambiguous_its_own_bucket() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        for (i, status) in [
            OpStatus::Applied,
            OpStatus::Applied,
            OpStatus::Failed,
            OpStatus::Prepared,
            OpStatus::Ambiguous,
            OpStatus::Skipped,
        ]
        .into_iter()
        .enumerate()
        {
            let mut row = op(&format!("op-{i}"), "run-1", &format!("item-{i}"), i as u32);
            row.status = status;
            insert_operation(&db, &row).expect("建操作");
        }

        let counts = count_operations(&db, "run-1").expect("计数");
        assert_eq!(counts.applied, 2);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.skipped, 1);
        // 歧义项**单独一档**：界面要给它的是「去核对」，不是「还没轮到」。
        // 并进 pending 会让恢复提示永远不出现。
        assert_eq!(counts.ambiguous, 1, "ambiguous 有自己的计数位");
        assert_eq!(counts.pending, 1, "只有 prepared 还算「还没轮到」");
    }

    #[test]
    fn unfinished_runs_lists_only_non_terminal_ones() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        insert_run(&db, "run-2", "plan-1", "req-2", "apply", "t1").expect("建 run");
        finish_run(&db, "run-1", RunStatus::Completed, "t2").expect("收尾");

        let open = unfinished_runs(&db).expect("查询");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, "run-2");
    }

    #[test]
    fn created_dirs_round_trip() {
        let db = db();
        seed_plan(&db, "plan-1");
        insert_run(&db, "run-1", "plan-1", "req-1", "apply", "t0").expect("建 run");
        insert_created_dir(
            &db,
            &CreatedDirRow {
                id: "d1".to_owned(),
                run_id: "run-1".to_owned(),
                relative_path: vec!["文档".to_owned()],
                directory_identity: "12345".to_owned(),
                created_by_run: true,
                state: "created".to_owned(),
            },
        )
        .expect("记录目录");

        let dirs = list_created_dirs(&db, "run-1").expect("读目录");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].relative_path, vec!["文档".to_owned()]);
        assert!(dirs[0].created_by_run);
    }

    #[test]
    fn list_runs_returns_newest_first_and_respects_the_limit() {
        let db = db();
        seed_plan(&db, "plan-1");
        for (id, request, started) in [
            ("run-1", "req-1", "2026-09-17T00:00:00.000Z"),
            ("run-2", "req-2", "2026-09-17T01:00:00.000Z"),
            ("run-3", "req-3", "2026-09-17T02:00:00.000Z"),
        ] {
            insert_run(&db, id, "plan-1", request, "apply", started).expect("建 run");
        }

        let all = list_runs(&db, 10).expect("应能列出");
        assert_eq!(all.len(), 3);
        assert_eq!(
            all[0].id, "run-3",
            "最新的必须排在最前：用户先看到最近做的事"
        );

        let limited = list_runs(&db, 2).expect("应能列出");
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, "run-3");

        // limit 为 0 会被钳到 1，而不是返回空列表——
        // 空列表会让界面显示「还没有任何记录」，那是误导。
        let clamped = list_runs(&db, 0).expect("应能列出");
        assert_eq!(clamped.len(), 1);
    }

    #[test]
    fn list_runs_on_an_empty_database_is_empty_not_an_error() {
        let db = db();
        assert!(list_runs(&db, 10).expect("空库也应正常返回").is_empty());
    }
}
