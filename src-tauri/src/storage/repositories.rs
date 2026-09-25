//! 仓储层：domain 类型 ↔ SQLite 行。
//!
//! 只有两个方向的转换，没有业务规则：业务规则在 `planner` 与后续的
//! `executor` / `recovery` 里。这里坚持三件事：
//!
//! 1. **路径与指纹存 JSON**，因为它们是组件数组与结构化对象，不是字符串；
//! 2. **枚举存 camelCase 文本**，与 IPC 线上取值一致，靠 serde 转换而不是
//!    手写映射表（手写表迟早会和类型定义脱节）；
//! 3. **多行写入永远在一个事务里**：计划与其计划项必须同生共死。
//!
//! 计划写入使用**乐观锁**（规格 8.1）：调用方必须给出它认为的当前版本，
//! 版本不符返回 `STALE_PLAN`，而不是"后写覆盖先写"。

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Transaction};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ai::provider::{ProviderConfig, ProviderKind};
use crate::domain::errors::{codes, AppError};
use crate::domain::types::{
    ExtractionStatus, FileRecord, Fingerprint, Id, Mode, Plan, PlanAction, PlanItem,
    PlanItemOrigin, PlanStatus, RelPath,
};
use crate::storage::db::{map_db_error, Database};

/// 根目录授权的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootRow {
    pub id: String,
    pub canonical_path: String,
    pub volume_id: String,
    pub identity: String,
    /// 授权会话标识。规格 8.1 要求记录它：重启后需要重新授权同一根目录。
    pub session_id: String,
}

/// 一次扫描的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRow {
    pub id: String,
    pub root_id: String,
    pub status: String,
    pub recursive: bool,
    pub started_at: String,
}

/// 写入根目录授权。
pub fn insert_root(db: &Database, row: &RootRow) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO roots (id, canonicalPath, volumeId, identity, sessionId) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.id,
                    row.canonical_path,
                    row.volume_id,
                    row.identity,
                    row.session_id
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 将一次新的原生选择重新绑定到所有身份相同的历史根。
///
/// 返回匹配行，由调用方把这些旧 rootId 重新登记进本次内存授权。比较卷内目录身份，
/// 不能只凭路径字符串授权一个后来替换到同一路径的新目录。
pub fn reauthorize_matching_roots(
    db: &Database,
    volume_id: &str,
    identity: &str,
    session_id: &str,
) -> Result<Vec<RootRow>, AppError> {
    db.with_transaction(|transaction| {
        let rows = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, canonicalPath, volumeId, identity, sessionId
                     FROM roots WHERE volumeId = ?1 AND identity = ?2 ORDER BY rowid",
                )
                .map_err(map_db_error)?;
            let collected = statement
                .query_map(params![volume_id, identity], |row| {
                    Ok(RootRow {
                        id: row.get(0)?,
                        canonical_path: row.get(1)?,
                        volume_id: row.get(2)?,
                        identity: row.get(3)?,
                        session_id: row.get(4)?,
                    })
                })
                .map_err(map_db_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_db_error)?;
            collected
        };

        if !rows.is_empty() {
            transaction
                .execute(
                    "UPDATE roots SET sessionId = ?3 WHERE volumeId = ?1 AND identity = ?2",
                    params![volume_id, identity, session_id],
                )
                .map_err(map_write_error)?;
        }
        Ok(rows)
    })
}

/// 登记一次扫描的开始。
pub fn insert_scan(db: &Database, row: &ScanRow) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO scans (id, rootId, status, recursive, startedAt, finishedAt, truncated) \
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0)",
                params![
                    row.id,
                    row.root_id,
                    row.status,
                    i64::from(row.recursive),
                    row.started_at
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 记录扫描结束。
pub fn finish_scan(
    db: &Database,
    scan_id: &str,
    status: &str,
    finished_at: &str,
    truncated: bool,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE scans SET status = ?2, finishedAt = ?3, truncated = ?4 WHERE id = ?1",
                params![scan_id, status, finished_at, i64::from(truncated)],
            )
            .map_err(map_write_error)?;
        if changed == 0 {
            return Err(AppError::new(
                codes::INVALID_PATH,
                format!("扫描 {scan_id} 不存在，无法记录结束状态"),
            ));
        }
        Ok(())
    })
}

/// 批量写入扫描结果。整体一个事务：要么全部落库，要么一条都没有。
pub fn insert_files(db: &Database, records: &[FileRecord]) -> Result<usize, AppError> {
    db.with_transaction(|transaction| {
        let mut statement = transaction
            .prepare(
                "INSERT INTO files \
                 (id, scanId, relativePathJson, fingerprintJson, extension, extractionStatus, skipCode) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(map_db_error)?;

        for record in records {
            statement
                .execute(params![
                    record.id,
                    record.scan_id,
                    to_json(&record.relative_path)?,
                    to_json(&record.fingerprint)?,
                    record.extension,
                    enum_to_text(&record.extraction_status)?,
                    record.skip_code,
                ])
                .map_err(map_write_error)?;
        }
        Ok(records.len())
    })
}

/// 读回一次扫描的全部文件，按写入顺序。
///
/// `rootId` 不在 files 表里（规格 8.1 只存 scanId），这里用 join 从 scans 取回，
/// 而不是再存一份副本——两份副本迟早会不一致。
pub fn list_files(db: &Database, scan_id: &str) -> Result<Vec<FileRecord>, AppError> {
    let connection = db.connection();
    let mut statement = connection
        .prepare(
            "SELECT f.id, f.scanId, s.rootId, f.relativePathJson, f.fingerprintJson, \
                    f.extension, f.extractionStatus, f.skipCode \
             FROM files AS f JOIN scans AS s ON s.id = f.scanId \
             WHERE f.scanId = ?1 ORDER BY f.rowid",
        )
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([scan_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(map_db_error)?;

    let mut records = Vec::new();
    for row in rows {
        let (id, scan_id, root_id, path_json, fingerprint_json, extension, status, skip_code) =
            row.map_err(map_db_error)?;
        records.push(FileRecord {
            id,
            scan_id,
            root_id,
            relative_path: from_json::<RelPath>(&path_json)?,
            extension,
            fingerprint: from_json::<Fingerprint>(&fingerprint_json)?,
            extraction_status: enum_from_text::<ExtractionStatus>(&status)?,
            skip_code,
        });
    }
    Ok(records)
}

/// 读取某个 scan 所属的 rootId。
pub fn file_root_id(db: &Database, scan_id: &str) -> Result<Option<String>, AppError> {
    let connection = db.connection();
    connection
        .query_row("SELECT rootId FROM scans WHERE id = ?1", [scan_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(map_db_error)
}

/// 保存计划（含计划项），使用乐观锁。
///
/// - `expected_revision = None`：新建。计划已存在则报错，不覆盖。
/// - `expected_revision = Some(r)`：更新。库里的版本必须恰好是 `r`，
///   否则返回 `STALE_PLAN`——这正是"编辑使旧确认失效"所依赖的机制。
///
/// 更新时会把 `digest` 清空：旧摘要绑定的是旧版本，留着它等于允许用
/// 旧校验结果确认新计划。
pub fn save_plan(
    db: &Database,
    plan: &Plan,
    expected_revision: Option<u32>,
) -> Result<(), AppError> {
    let mode = enum_to_text(&plan.mode)?;
    let status = enum_to_text(&plan.status)?;

    db.with_transaction(|transaction| {
        let current: Option<(i64, String, String, String, String)> = transaction
            .query_row(
                "SELECT revision, scanId, rootId, mode, createdAt FROM plans WHERE id = ?1",
                [&plan.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .map_err(map_db_error)?;

        match (current, expected_revision) {
            (None, None) => {
                if plan.revision != 1 {
                    return Err(AppError::new(
                        codes::STALE_PLAN,
                        format!("新计划必须从版本 1 开始，收到版本 {}", plan.revision),
                    ));
                }
                transaction
                    .execute(
                        "INSERT INTO plans (id, scanId, rootId, revision, status, digest, mode, createdAt) \
                         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
                        params![
                            plan.id,
                            plan.scan_id,
                            plan.root_id,
                            i64::from(plan.revision),
                            status,
                            mode,
                            plan.created_at
                        ],
                    )
                    .map_err(map_write_error)?;
            }
            (None, Some(_)) => {
                return Err(AppError::new(
                    codes::STALE_PLAN,
                    format!("计划 {} 不存在，无法按期望版本更新", plan.id),
                ))
            }
            (Some(_), None) => {
                return Err(AppError::new(
                    codes::STALE_PLAN,
                    format!("计划 {} 已存在，新建操作被拒绝", plan.id),
                ))
            }
            (Some((current, scan_id, root_id, stored_mode, created_at)), Some(expected)) => {
                if current != i64::from(expected) {
                    return Err(AppError::new(
                        codes::STALE_PLAN,
                        format!(
                            "计划 {} 的当前版本是 {current}，与期望版本 {expected} 不一致，请重新加载",
                            plan.id
                        ),
                    ));
                }
                if plan.scan_id != scan_id
                    || plan.root_id != root_id
                    || mode != stored_mode
                    || plan.created_at != created_at
                {
                    return Err(AppError::new(
                        codes::STALE_PLAN,
                        "计划的 scanId、rootId、mode 或 createdAt 与已保存计划不一致，拒绝更新不可变身份",
                    ));
                }
                // 版本必须**递增**。只检查「期望版本 == 当前版本」还不够：
                // 调用方一旦忘记升版本，内容变了、版本没变，两个并发编辑就会互相覆盖
                // （后写的期望值也对得上）。规格 7.2 要求「编辑后 revision 加一」，
                // 这条不变量由存储层兜住，而不是指望每个调用方都记得。
                if i64::from(plan.revision) != current + 1 {
                    return Err(AppError::new(
                        codes::STALE_PLAN,
                        format!(
                            "更新计划必须严格递增且恰好加一：当前 {current}，提交的是 {}",
                            plan.revision
                        ),
                    ));
                }
                transaction
                    .execute(
                        "UPDATE plans SET revision = ?2, status = ?3, digest = NULL \
                         WHERE id = ?1",
                        params![plan.id, i64::from(plan.revision), status],
                    )
                    .map_err(map_write_error)?;
                transaction
                    .execute("DELETE FROM plan_items WHERE planId = ?1", [&plan.id])
                    .map_err(map_write_error)?;
            }
        }

        insert_plan_items(transaction, &plan.id, &plan.items)?;
        Ok(())
    })
}

fn insert_plan_items(
    transaction: &Transaction<'_>,
    plan_id: &str,
    items: &[PlanItem],
) -> Result<(), AppError> {
    let mut statement = transaction
        .prepare(
            "INSERT INTO plan_items \
             (id, planId, fileId, sourceJson, targetJson, action, selected, origin, expectedJson, reason) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .map_err(map_db_error)?;

    for item in items {
        statement
            .execute(params![
                item.id,
                plan_id,
                item.file_id,
                to_json(&item.source)?,
                to_json(&item.target)?,
                enum_to_text(&item.action)?,
                i64::from(item.selected),
                enum_to_text(&item.origin)?,
                to_json(&item.expected)?,
                item.reason,
            ])
            .map_err(map_write_error)?;
    }
    Ok(())
}

/// 读回计划与它的计划项。
///
/// 计划项按写入顺序返回——规划器就是按源相对路径的稳定顺序写入的，
/// 因此读回来仍然是预览里那个顺序。
pub fn load_plan(db: &Database, plan_id: &str) -> Result<Option<Plan>, AppError> {
    let connection = db.connection();

    let header: Option<(String, String, i64, String, String, String)> = connection
        .query_row(
            "SELECT scanId, rootId, revision, status, mode, createdAt FROM plans WHERE id = ?1",
            [plan_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_db_error)?;

    let Some((scan_id, root_id, revision, status, mode, created_at)) = header else {
        return Ok(None);
    };

    let mut statement = connection
        .prepare(
            "SELECT id, fileId, sourceJson, targetJson, action, selected, origin, expectedJson, reason \
             FROM plan_items WHERE planId = ?1 ORDER BY rowid",
        )
        .map_err(map_db_error)?;

    let rows = statement
        .query_map([plan_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(map_db_error)?;

    let mut items = Vec::new();
    for row in rows {
        let (id, file_id, source, target, action, selected, origin, expected, reason) =
            row.map_err(map_db_error)?;
        items.push(PlanItem {
            id,
            file_id,
            source: from_json(&source)?,
            target: from_json(&target)?,
            action: enum_from_text::<PlanAction>(&action)?,
            selected: selected != 0,
            origin: enum_from_text::<PlanItemOrigin>(&origin)?,
            reason,
            expected: from_json::<Fingerprint>(&expected)?,
        });
    }

    Ok(Some(Plan {
        id: plan_id.to_owned(),
        root_id,
        scan_id,
        revision: u32::try_from(revision)
            .map_err(|_| AppError::new(codes::DB_UNAVAILABLE, "计划版本号超出取值范围"))?,
        mode: enum_from_text::<Mode>(&mode)?,
        status: enum_from_text::<PlanStatus>(&status)?,
        items,
        created_at,
    }))
}

/// 记录一次校验产生的摘要。
///
/// 摘要与（planId, revision）绑定：版本对不上时拒绝写入，
/// 否则会出现"摘要属于版本 2、计划已经是版本 3"的错配。
pub fn set_plan_digest(
    db: &Database,
    plan_id: &str,
    revision: u32,
    digest: &str,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        let changed = transaction
            .execute(
                "UPDATE plans SET digest = ?3 WHERE id = ?1 AND revision = ?2",
                params![plan_id, i64::from(revision), digest],
            )
            .map_err(map_write_error)?;
        if changed == 0 {
            return Err(AppError::new(
                codes::STALE_PLAN,
                format!("计划 {plan_id} 的版本不是 {revision}，摘要未写入"),
            ));
        }
        Ok(())
    })
}

/// 读取计划当前绑定的摘要。
///
/// 编辑计划会把摘要清空（见 [`save_plan`]），因此界面可以用「摘要是否存在」
/// 判断「这份计划是否还有效的校验结果」。
pub fn plan_digest(db: &Database, plan_id: &str) -> Result<Option<String>, AppError> {
    let connection = db.connection();
    connection
        .query_row("SELECT digest FROM plans WHERE id = ?1", [plan_id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .optional()
        .map_err(map_db_error)
        .map(|value| value.flatten())
}

/// 读取计划的当前版本号。
pub fn plan_revision(db: &Database, plan_id: &str) -> Result<Option<u32>, AppError> {
    let connection = db.connection();
    let revision: Option<i64> = connection
        .query_row(
            "SELECT revision FROM plans WHERE id = ?1",
            [plan_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_db_error)?;
    match revision {
        None => Ok(None),
        Some(value) => Ok(Some(u32::try_from(value).map_err(|_| {
            AppError::new(codes::DB_UNAVAILABLE, "计划版本号超出取值范围")
        })?)),
    }
}

// ---------------------------------------------------------------------------
// 转换辅助
// ---------------------------------------------------------------------------

fn to_json<T: Serialize>(value: &T) -> Result<String, AppError> {
    serde_json::to_string(value)
        .map_err(|error| AppError::internal(format!("序列化持久化字段失败: {error}")))
}

fn from_json<T: DeserializeOwned>(text: &str) -> Result<T, AppError> {
    serde_json::from_str(text).map_err(|error| {
        AppError::new(
            codes::DB_UNAVAILABLE,
            format!("数据库中的 JSON 字段无法解析: {error}"),
        )
    })
}

/// 枚举 → camelCase 文本。
///
/// 走 serde 而不是手写 `match`：手写表会在新增枚举变体时悄悄漏掉一支，
/// 而 serde 与 IPC 契约用的是同一份定义。
fn enum_to_text<T: Serialize>(value: &T) -> Result<String, AppError> {
    match serde_json::to_value(value)
        .map_err(|error| AppError::internal(format!("序列化枚举失败: {error}")))?
    {
        serde_json::Value::String(text) => Ok(text),
        other => Err(AppError::internal(format!(
            "枚举的序列化结果不是字符串: {other}"
        ))),
    }
}

fn enum_from_text<T: DeserializeOwned>(text: &str) -> Result<T, AppError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned())).map_err(|error| {
        AppError::new(
            codes::DB_UNAVAILABLE,
            format!("数据库中的枚举值 {text:?} 无法识别: {error}"),
        )
    })
}

/// 写入类错误的映射：约束冲突是**数据问题**，不该报成"数据库不可用"。
fn map_write_error(error: rusqlite::Error) -> AppError {
    if let rusqlite::Error::SqliteFailure(inner, _) = &error {
        if inner.code == ErrorCode::ConstraintViolation {
            return AppError::new(
                codes::INVALID_PATH,
                format!("数据违反唯一性或外键约束: {error}"),
            );
        }
    }
    map_db_error(error)
}

// ---------------------------------------------------------------------------
// 提供商配置（T12）
// ---------------------------------------------------------------------------

/// 保存（或更新）一份提供商配置。
///
/// **这里没有接收密钥的参数**，这不是疏忽：规格 3.3 要求「配置只存
/// credentialRef」，而让这个函数连密钥都拿不到，「把密钥写进 SQLite」
/// 这件事在这条路径上就无从表达。密钥进系统凭据存储，见
/// [`crate::platform::credentials`]。
///
/// 同一个 id 再写一次是**更新**（`ON CONFLICT`），因为用户改端点或模型时
/// 用的就是同一条配置。
pub fn save_provider(db: &Database, config: &ProviderConfig) -> Result<(), AppError> {
    let kind = enum_to_text(&config.kind)?;
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO providers (id, kind, endpoint, model, credentialRef) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET \
                   kind = excluded.kind, endpoint = excluded.endpoint, \
                   model = excluded.model, credentialRef = excluded.credentialRef",
                params![
                    config.id,
                    kind,
                    config.endpoint,
                    config.model,
                    config.credential_ref
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 按 id 读一份提供商配置。不存在返回 `None`。
pub fn load_provider(db: &Database, id: &str) -> Result<Option<ProviderConfig>, AppError> {
    db.with_transaction(|transaction| {
        let row = transaction
            .query_row(
                "SELECT id, kind, endpoint, model, credentialRef FROM providers WHERE id = ?1",
                params![id],
                provider_from_row,
            )
            .optional()
            .map_err(map_db_error)?;
        Ok(row)
    })
}

/// 列出全部提供商配置，按写入顺序。
///
/// 顺序稳定是为了让设置页每次打开时看到同样的排列——一个会自己换位置的
/// 列表会让人怀疑自己点错了。
pub fn list_providers(db: &Database) -> Result<Vec<ProviderConfig>, AppError> {
    db.with_transaction(|transaction| {
        let mut statement = transaction
            .prepare(
                "SELECT id, kind, endpoint, model, credentialRef FROM providers ORDER BY rowid",
            )
            .map_err(map_db_error)?;
        let rows = statement
            .query_map([], provider_from_row)
            .map_err(map_db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    })
}

fn provider_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConfig> {
    let kind_text: String = row.get(1)?;
    // 枚举映射失败要在这一层转成 rusqlite 错误，因为 `query_map` 的闭包
    // 只能返回 `rusqlite::Result`。真正的原因会被 `enum_from_text` 写成
    // 一句可读的话，再包进 `FromSqlConversionFailure` 带出去。
    let kind = enum_from_text::<ProviderKind>(&kind_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.message,
            )),
        )
    })?;

    Ok(ProviderConfig {
        id: row.get(0)?,
        kind,
        endpoint: row.get(2)?,
        model: row.get(3)?,
        credential_ref: row.get(4)?,
    })
}

// ---------------------------------------------------------------------------
// 分析结果（T13）
// ---------------------------------------------------------------------------

/// 一次分析的一行。
///
/// 规格 8.1 把它列成「id、scanId、mode、providerId、status、proposalJson、
/// inputFingerprintsJson、promptVersion」，并特意写了一句：
/// 「**不保存正文**」——正文只留在会话缓存里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRow {
    pub id: Id,
    pub scan_id: Id,
    pub mode: String,
    pub provider_id: Option<String>,
    pub status: String,
    /// 模型给出的建议（JSON），不含绝对路径。
    pub proposal_json: Option<String>,
    /// 这次分析所依据的那批文件的指纹。
    ///
    /// 冻结它是为了事后能回答「这次分析是基于哪一版文件」——
    /// 用户在分析之后又改了文件时，那个问题会变得很实际。
    pub input_fingerprints_json: Option<String>,
    pub prompt_version: Option<String>,
}

/// 登记一次分析的开始。
pub fn insert_analysis(db: &Database, row: &AnalysisRow) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO analyses \
                   (id, scanId, mode, providerId, status, proposalJson, \
                    inputFingerprintsJson, promptVersion) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    row.id,
                    row.scan_id,
                    row.mode,
                    row.provider_id,
                    row.status,
                    row.proposal_json,
                    row.input_fingerprints_json,
                    row.prompt_version,
                ],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 记录一次分析的结束（成功或失败）。
///
/// 成功时把 `proposalJson` 与 `promptVersion` 一起写入——两者必须同时落库：
/// 只有建议而没有产生它的提示词版本，事后就无法解释「为什么这次的建议
/// 粒度不一样」。
pub fn finish_analysis(
    db: &Database,
    id: &str,
    status: &str,
    proposal_json: Option<&str>,
) -> Result<(), AppError> {
    db.with_transaction(|transaction| {
        transaction
            .execute(
                "UPDATE analyses SET status = ?1, proposalJson = ?2 WHERE id = ?3",
                params![status, proposal_json, id],
            )
            .map_err(map_write_error)?;
        Ok(())
    })
}

/// 读一次分析。
pub fn load_analysis(db: &Database, id: &str) -> Result<Option<AnalysisRow>, AppError> {
    db.with_transaction(|transaction| {
        let row = transaction
            .query_row(
                "SELECT id, scanId, mode, providerId, status, proposalJson, \
                        inputFingerprintsJson, promptVersion \
                 FROM analyses WHERE id = ?1",
                params![id],
                |row| {
                    Ok(AnalysisRow {
                        id: row.get(0)?,
                        scan_id: row.get(1)?,
                        mode: row.get(2)?,
                        provider_id: row.get(3)?,
                        status: row.get(4)?,
                        proposal_json: row.get(5)?,
                        input_fingerprints_json: row.get(6)?,
                        prompt_version: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(map_db_error)?;
        Ok(row)
    })
}

/// 供测试与诊断使用：直接查一张表的行数。
pub fn count_rows(db: &Database, table: &str) -> Result<i64, AppError> {
    // 表名不能参数化，因此只允许白名单内的名字，避免把外部输入拼进 SQL。
    // 表名不能参数化，因此只允许白名单内的名字，避免把外部输入拼进 SQL。
    // 白名单必须跟 migration 同步：漏掉的表会让调用方拿到「不允许统计」，
    // 而那看起来像是权限问题，实际只是这里忘了加。
    const ALLOWED: &[&str] = &[
        "roots",
        "scans",
        "files",
        "plans",
        "plan_items",
        "runs",
        "operations",
        "operation_events",
        "created_dirs",
    ];
    if !ALLOWED.contains(&table) {
        return Err(AppError::internal(format!("不允许统计表 {table}")));
    }
    let connection: std::sync::MutexGuard<'_, Connection> = db.connection();
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(map_db_error)
}

/// 供测试与诊断使用：列出某张表的列名。
///
/// 存在的理由是让「**密钥没有进数据库**」这条断言可以真的去查表结构，
/// 而不是靠读代码相信它。与 [`count_rows`] 一样，表名只接受白名单——
/// `PRAGMA` 的参数不能参数化，拼进去的必须是已知值。
pub fn column_names(db: &Database, table: &str) -> Result<Vec<String>, AppError> {
    const ALLOWED: &[&str] = &["providers", "settings", "plans", "runs"];
    if !ALLOWED.contains(&table) {
        return Err(AppError::internal(format!("不允许查询表 {table} 的列名")));
    }

    let connection: std::sync::MutexGuard<'_, Connection> = db.connection();
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(map_db_error)?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(map_db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_db_error)?;
    Ok(names)
}
