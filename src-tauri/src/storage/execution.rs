//! Durable confirmation and execution admission, committed as one transaction.
use super::db::{map_db_error, Database};
use crate::domain::{
    errors::{codes, AppError},
    types::Plan,
};
use crate::safety::confirmation::{hash_token, IssuedConfirmation};
use rusqlite::{params, OptionalExtension};

pub fn persist_confirmation(
    db: &Database,
    plan: &Plan,
    digest: &str,
    issued: &IssuedConfirmation,
) -> Result<(), AppError> {
    db.with_transaction(|tx| {
        let changed = tx.execute("UPDATE plans SET status='validated', digest=?3 WHERE id=?1 AND revision=?2 AND status IN ('draft','validated')",
            params![plan.id, plan.revision, digest]).map_err(map_db_error)?;
        if changed != 1 { return Err(AppError::new(codes::STALE_PLAN, "计划已改变，重新校验")); }
        tx.execute("INSERT INTO confirmations(tokenHash,planId,revision,digest,expiresAt) VALUES (?1,?2,?3,?4,?5)",
            params![hash_token(&issued.token),plan.id,plan.revision,digest,issued.expires_at]).map_err(map_db_error)?;
        Ok(())
    })
}

pub fn replay(
    db: &Database,
    request: &str,
    plan: &str,
    token: &str,
) -> Result<Option<String>, AppError> {
    let conn = db.connection();
    let row: Option<(String,String,Option<String>)> = conn.query_row(
        "SELECT r.id,r.planId,b.tokenHash FROM runs r LEFT JOIN execution_requests b ON b.requestId=r.requestId WHERE r.requestId=?1",
        [request], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(map_db_error)?;
    match row {
        Some((id, p, Some(hash))) if p == plan && hash == hash_token(token) => Ok(Some(id)),
        Some(_) => Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "requestId 已绑定其他执行参数",
        )),
        None => Ok(None),
    }
}

/// Caller holds the application execution lock. No filesystem mutations occur here.
pub fn admit(
    db: &Database,
    plan: &Plan,
    digest: &str,
    request: &str,
    token: &str,
    now_ms: i64,
) -> Result<String, AppError> {
    let now = crate::domain::time::rfc3339_from_unix_ms(now_ms);
    db.with_transaction(|tx| {
        let blocked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM runs WHERE status IN ('queued','running','recoveryRequired')) OR EXISTS(SELECT 1 FROM operations WHERE status IN ('prepared','ambiguous') AND resolution='open')",[],|r|r.get(0)).map_err(map_db_error)?;
        if blocked { return Err(AppError::new(codes::RECOVERY_REQUIRED,"存在未完成或需核对的执行，请先处理历史记录")); }
        let hash = hash_token(token);
        let saved: Option<(String,u32,String,String,Option<String>)> = tx.query_row(
            "SELECT planId,revision,digest,expiresAt,consumedAt FROM confirmations WHERE tokenHash=?1",[&hash],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(map_db_error)?;
        let Some((p,revision,d,expiry,consumed)) = saved else { return Err(AppError::new(codes::STALE_PLAN,"确认不存在")); };
        if consumed.is_some() { return Err(AppError::new(codes::TOKEN_USED,"确认已经使用")); }
        if now >= expiry { return Err(AppError::new(codes::TOKEN_EXPIRED,"确认已过期")); }
        if p != plan.id || revision != plan.revision || d != digest { return Err(AppError::new(codes::STALE_PLAN,"确认与计划不匹配")); }
        let changed = tx.execute("UPDATE plans SET status='sealed' WHERE id=?1 AND revision=?2 AND digest=?3 AND status='validated'",
            params![plan.id,plan.revision,digest]).map_err(map_db_error)?;
        if changed != 1 { return Err(AppError::new(codes::STALE_PLAN,"计划已改变或已经执行")); }
        let run = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO runs(id,planId,requestId,direction,status,startedAt) VALUES (?1,?2,?3,'apply','running',?4)",params![run,plan.id,request,now]).map_err(map_db_error)?;
        tx.execute("INSERT INTO execution_requests(requestId,tokenHash) VALUES (?1,?2)",params![request,hash]).map_err(map_db_error)?;
        tx.execute("UPDATE confirmations SET consumedAt=?2 WHERE tokenHash=?1",params![hash,now]).map_err(map_db_error)?;
        Ok(run)
    })
}
