//! 撤销执行的原子准入与幂等绑定。

use rusqlite::{params, OptionalExtension};

use crate::domain::errors::{codes, AppError};
use crate::domain::time::rfc3339_from_unix_ms;
use crate::domain::types::UndoReport;
use crate::storage::db::{map_db_error, Database};

fn write_error(error: rusqlite::Error) -> AppError {
    let mut mapped = map_db_error(error);
    mapped.code = codes::JOURNAL_WRITE_FAILED.to_owned();
    mapped
}

fn selected_json(selected: &[String]) -> Result<String, AppError> {
    let mut ids = selected.to_vec();
    ids.sort();
    ids.dedup();
    serde_json::to_string(&ids)
        .map_err(|error| AppError::internal(format!("序列化撤销选择失败: {error}")))
}

pub fn ensure_globally_unblocked(db: &Database) -> Result<(), AppError> {
    let connection = db.connection();
    let blocked: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM runs WHERE status IN ('queued','running','recoveryRequired')
             ) OR EXISTS(
                 SELECT 1 FROM operations
                 WHERE status IN ('prepared','ambiguous') AND resolution='open'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(map_db_error)?;
    if blocked {
        return Err(AppError::new(
            codes::RECOVERY_REQUIRED,
            "存在未完成或需核对的执行，请先处理历史记录",
        ));
    }
    Ok(())
}

pub fn replay(
    db: &Database,
    request_id: &str,
    undo_plan_id: &str,
    original_run_id: &str,
    digest: &str,
    token_hash: &str,
    selected: &[String],
) -> Result<Option<String>, AppError> {
    let wanted = selected_json(selected)?;
    let connection = db.connection();
    let saved = connection
        .query_row(
            "SELECT r.id, u.undoPlanId, u.originalRunId, u.digest, u.tokenHash, u.selectedJson
             FROM runs r JOIN undo_requests u ON u.requestId = r.requestId
             WHERE r.requestId = ?1",
            [request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_db_error)?;

    let Some((run_id, saved_plan, saved_original, saved_digest, saved_token, saved_selected)) =
        saved
    else {
        return Ok(None);
    };
    if saved_plan != undo_plan_id
        || saved_original != original_run_id
        || saved_digest != digest
        || saved_token != token_hash
        || saved_selected != wanted
    {
        return Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "requestId 已绑定另一份撤销计划或选择集合",
        ));
    }
    Ok(Some(run_id))
}

pub fn check_confirmation(
    db: &Database,
    undo_plan_id: &str,
    original_run_id: &str,
    digest: &str,
    token_hash: &str,
    now_ms: i64,
) -> Result<(), AppError> {
    let connection = db.connection();
    let saved = connection
        .query_row(
            "SELECT originalRunId, digest, tokenHash, expiresAt, consumedAt
             FROM undo_plans WHERE id = ?1",
            [undo_plan_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(map_db_error)?
        .ok_or_else(|| AppError::new(codes::STALE_PLAN, "撤销计划不存在，请重新预览"))?;
    if saved.0 != original_run_id || saved.1 != digest || saved.2 != token_hash {
        return Err(AppError::new(codes::STALE_PLAN, "撤销确认与当前计划不匹配"));
    }
    if saved.4.is_some() {
        return Err(AppError::new(codes::TOKEN_USED, "撤销确认已经使用"));
    }
    if rfc3339_from_unix_ms(now_ms) >= saved.3 {
        return Err(AppError::new(codes::TOKEN_EXPIRED, "撤销确认已过期"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn admit(
    db: &Database,
    plan_id: &str,
    undo_plan_id: &str,
    original_run_id: &str,
    digest: &str,
    token_hash: &str,
    selected: &[String],
    request_id: &str,
    now_ms: i64,
) -> Result<String, AppError> {
    let selected_json = selected_json(selected)?;
    let now = rfc3339_from_unix_ms(now_ms);
    db.with_transaction(|transaction| {
        let blocked: bool = transaction
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM runs WHERE status IN ('queued','running','recoveryRequired')
                 ) OR EXISTS(
                     SELECT 1 FROM operations
                     WHERE status IN ('prepared','ambiguous') AND resolution='open'
                 )",
                [],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if blocked {
            return Err(AppError::new(
                codes::RECOVERY_REQUIRED,
                "存在未完成或需核对的执行，请先处理历史记录",
            ));
        }

        let saved = transaction
            .query_row(
                "SELECT originalRunId, digest, tokenHash, expiresAt, consumedAt
                 FROM undo_plans WHERE id = ?1",
                [undo_plan_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or_else(|| AppError::new(codes::STALE_PLAN, "撤销计划不存在，请重新预览"))?;
        if saved.0 != original_run_id || saved.1 != digest || saved.2 != token_hash {
            return Err(AppError::new(codes::STALE_PLAN, "撤销确认与当前计划不匹配"));
        }
        if saved.4.is_some() {
            return Err(AppError::new(codes::TOKEN_USED, "撤销确认已经使用"));
        }
        if now >= saved.3 {
            return Err(AppError::new(codes::TOKEN_EXPIRED, "撤销确认已过期"));
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        transaction
            .execute(
                "INSERT INTO runs(id, planId, requestId, direction, status, startedAt)
                 VALUES (?1, ?2, ?3, 'undo', 'running', ?4)",
                params![run_id, plan_id, request_id, now],
            )
            .map_err(write_error)?;
        transaction
            .execute(
                "INSERT INTO undo_requests
                 (requestId, undoPlanId, originalRunId, tokenHash, digest, selectedJson)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    request_id,
                    undo_plan_id,
                    original_run_id,
                    token_hash,
                    digest,
                    selected_json
                ],
            )
            .map_err(write_error)?;
        let consumed = transaction
            .execute(
                "UPDATE undo_plans SET consumedAt = ?2
                 WHERE id = ?1 AND consumedAt IS NULL",
                params![undo_plan_id, now],
            )
            .map_err(write_error)?;
        if consumed != 1 {
            return Err(AppError::new(codes::TOKEN_USED, "撤销确认已经使用"));
        }
        Ok(run_id)
    })
}

pub fn finish_report(
    db: &Database,
    request_id: &str,
    report: &UndoReport,
    now_ms: i64,
) -> Result<(), AppError> {
    let json = serde_json::to_string(report)
        .map_err(|error| AppError::internal(format!("序列化撤销报告失败: {error}")))?;
    let finished_at = rfc3339_from_unix_ms(now_ms);
    db.with_transaction(|transaction| {
        let run_changed = transaction
            .execute(
                "UPDATE runs SET status = ?2, finishedAt = ?3
                 WHERE requestId = ?1 AND direction = 'undo'",
                params![request_id, report.status.as_text(), finished_at],
            )
            .map_err(write_error)?;
        let changed = transaction
            .execute(
                "UPDATE undo_requests SET reportJson = ?2 WHERE requestId = ?1",
                params![request_id, json],
            )
            .map_err(write_error)?;
        if run_changed != 1 || changed != 1 {
            return Err(AppError::new(
                codes::JOURNAL_WRITE_FAILED,
                "撤销请求绑定不存在",
            ));
        }
        Ok(())
    })
}

pub fn load_report(db: &Database, run_id: &str) -> Result<Option<UndoReport>, AppError> {
    let connection = db.connection();
    let json = connection
        .query_row(
            "SELECT u.reportJson FROM undo_requests u
             JOIN runs r ON r.requestId = u.requestId WHERE r.id = ?1",
            [run_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(map_db_error)?
        .flatten();
    json.map(|value| {
        serde_json::from_str(&value)
            .map_err(|error| AppError::internal(format!("读取撤销报告失败: {error}")))
    })
    .transpose()
}

pub fn original_run_id(db: &Database, run_id: &str) -> Result<Option<String>, AppError> {
    let connection = db.connection();
    connection
        .query_row(
            "SELECT u.originalRunId FROM undo_requests u
             JOIN runs r ON r.requestId = u.requestId WHERE r.id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_db_error)
}
