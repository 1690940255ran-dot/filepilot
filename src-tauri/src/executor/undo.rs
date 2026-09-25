//! 撤销。规格 8.4。
//!
//! **撤销是另一个可预览、可确认、可失败的操作**，不是执行程序异常后的
//! 无条件补偿。这句话决定了本模块的两个基本形状：
//!
//! 1. 它有自己的预览、摘要与一次性确认 —— 与整理前的那次确认同构，
//!    因为用户同样在看一份「将要改动我的文件」的清单；
//! 2. 它**只**处理原操作确实 `applied` 的项，且每一项都要重新核对磁盘。
//!    整理之后用户完全可能又动过那些文件，此时「把它搬回去」是错的。
//!
//! ## 与恢复（`recovery`）的分工
//!
//! 恢复回答的是「上次执行到底做成了什么」（规格 8.3 决策表），
//! 撤销回答的是「能不能把它退回去」（规格 8.4）。
//! 两者都只依据磁盘事实，但**判定方向相反**：
//! 恢复看「文件到目标了吗」，撤销看「文件还在目标、而原处是空的吗」。
//!
//! ## 规格 8.4 的八条，落在代码哪里
//!
//! | 条 | 实现 |
//! |---|---|
//! | 1 逆序、只取 applied | `ordered_candidates` |
//! | 2 三项核对 | `classify` |
//! | 3 冲突默认不选中 | `UndoItem.selected` 由 `UndoOutcome` 决定 |
//! | 4 原父目录消失即冲突 | `classify` 的 `ParentMissing` |
//! | 5 先记 undo prepared 后记 undone | `commit_undo_prepared` → rename → `commit_revert` |
//! | 6 中断后按「源目标互换」核对 | 复用同一套 `classify`（撤销操作的 source/target 本就是互换的） |
//! | 7 只清本次创建且已空的目录 | `clean_created_dirs` |
//! | 8 重复撤销只回报已完成 | `UndoOutcome::AlreadyUndone` |

use sha2::{Digest, Sha256};

use crate::domain::errors::{codes, AppError};
use crate::domain::time::rfc3339_from_unix_ms;
use crate::domain::types::{
    Fingerprint, Issue, RelPath, RunStatus, UndoItem, UndoOutcome, UndoPreview, UndoReport,
    UndoStatus,
};
use crate::executor::journal::{new_undo_operation, Journal};
use crate::safety::fingerprint::{move_no_replace_with, MoveFailPoints};
use crate::safety::root::ApprovedRoot;
use crate::storage::db::Database;
use crate::storage::runs::{self, append_event, CreatedDirRow, OperationRow};
use crate::storage::runs::{
    commit_revert, commit_undo_prepared, insert_undo_plan, list_created_dirs, load_run,
    load_undo_plan, mark_undo_conflict, undone_original_ids, UndoPlanRow,
};

/// 摘要的版本串。
///
/// 与 `filepilot.run.state.v*` 分开：那两个说的是「执行留下的事实」，
/// 这个说的是「撤销预览那一刻的事实」。合用一个版本串会让两类变更
/// 互相触发对方的重算，而它们本就不该耦合。
const UNDO_DIGEST_VERSION: &str = "filepilot.undo.state.v1";

/// 一次性令牌的有效期：5 分钟，与整理确认保持一致。
const UNDO_TOKEN_TTL_MS: i64 = 5 * 60 * 1000;

pub trait UndoObserver {
    fn should_stop(&self) -> bool;
    fn on_progress(&self, processed: u32, total: u32);
}

struct NoopUndoObserver;

impl UndoObserver for NoopUndoObserver {
    fn should_stop(&self) -> bool {
        false
    }

    fn on_progress(&self, _processed: u32, _total: u32) {}
}

/// 撤销预览的分类结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Ready,
    Conflict,
    AlreadyUndone,
}

impl Verdict {
    fn outcome(self) -> UndoOutcome {
        match self {
            Verdict::Ready => UndoOutcome::Ready,
            Verdict::Conflict => UndoOutcome::Conflict,
            Verdict::AlreadyUndone => UndoOutcome::AlreadyUndone,
        }
    }
}

/// 分类时给出的一句话，以及可选的冲突原因。
struct Classified {
    verdict: Verdict,
    message: String,
}

/// 生成一份撤销预览，并把摘要与一次性令牌落库。
///
/// 传入的 `root` 必须是原 run 所在的那个批准根：撤销要把文件搬回去，
/// 而「搬回哪里」这件事只能由根来决定。
pub fn preview_undo(
    db: &Database,
    root: &ApprovedRoot,
    original_run_id: &str,
    now_ms: i64,
) -> Result<UndoPreview, AppError> {
    let run = load_run(db, original_run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {original_run_id}")))?;

    // 撤销一个撤销没有意义，而且会把 source/target 再翻一次。
    // 与其支持这种绕圈，不如明确拒绝——用户要的「撤回去」就是撤销那个整理操作。
    if run.direction != "apply" {
        return Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "只能撤销一次整理操作；这条记录本身是撤销，不能再撤一次",
        ));
    }

    ensure_not_blocked(db, original_run_id)?;

    let items = plan_items(db, root, original_run_id)?;
    let digest = undo_digest(&items);

    let ready_count = count(&items, UndoOutcome::Ready);
    let conflict_count = count(&items, UndoOutcome::Conflict);
    let already_undone_count = count(&items, UndoOutcome::AlreadyUndone);

    // 令牌只存哈希（规格 7.2 对整理确认的同样要求）：
    // 库被读走也不能反推出一个可用的令牌。
    let token = crate::safety::confirmation::random_token()?;
    let plan_id = uuid::Uuid::new_v4().to_string();
    let expires_at = rfc3339_from_unix_ms(now_ms + UNDO_TOKEN_TTL_MS);

    insert_undo_plan(
        db,
        &UndoPlanRow {
            id: plan_id.clone(),
            original_run_id: original_run_id.to_owned(),
            digest: digest.clone(),
            items_json: serde_json::to_string(&items)
                .map_err(|error| AppError::internal(format!("序列化撤销清单失败: {error}")))?,
            token_hash: hash_token(&token),
            expires_at: expires_at.clone(),
            consumed_at: None,
        },
    )?;

    Ok(UndoPreview {
        undo_plan_id: plan_id,
        original_run_id: original_run_id.to_owned(),
        digest,
        items,
        ready_count,
        conflict_count,
        already_undone_count,
        undo_token: Some(token),
        expires_at: Some(expires_at),
    })
}

/// 执行一次撤销。
///
/// `selected` 是用户勾选的**原操作 id**。只有当前判定为 `Ready` 的项才可以被选中：
/// 预览之后磁盘可能又变了，此时拿旧清单来执行是危险的——所以这里重算一遍，
/// 既校验摘要，也校验每一项的资格。
#[allow(clippy::too_many_arguments)]
pub fn execute_undo(
    db: &Database,
    root: &ApprovedRoot,
    undo_plan_id: &str,
    expected_digest: &str,
    undo_token: &str,
    selected: &[String],
    request_id: &str,
    now_ms: i64,
) -> Result<UndoReport, AppError> {
    execute_undo_observed(
        db,
        root,
        undo_plan_id,
        expected_digest,
        undo_token,
        selected,
        request_id,
        now_ms,
        &NoopUndoObserver,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn execute_undo_observed(
    db: &Database,
    root: &ApprovedRoot,
    undo_plan_id: &str,
    expected_digest: &str,
    undo_token: &str,
    selected: &[String],
    request_id: &str,
    now_ms: i64,
    observer: &dyn UndoObserver,
) -> Result<UndoReport, AppError> {
    let plan = load_undo_plan(db, undo_plan_id)?
        .ok_or_else(|| AppError::internal("撤销计划不存在，请重新预览"))?;

    let original_run_id = plan.original_run_id.clone();
    if let Some(existing) = replay_undo_request(
        db,
        undo_plan_id,
        &original_run_id,
        expected_digest,
        undo_token,
        selected,
        request_id,
    )? {
        return Ok(existing);
    }
    let run = load_run(db, &original_run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {original_run_id}")))?;
    if run.direction != "apply" {
        return Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "只能撤销一次整理操作",
        ));
    }

    ensure_not_blocked(db, &original_run_id)?;
    crate::storage::undo_execution::check_confirmation(
        db,
        undo_plan_id,
        &original_run_id,
        expected_digest,
        &hash_token(undo_token),
        now_ms,
    )?;

    // 重新分类。摘要在**写任何文件之前**整批校验（同一份确认不能
    // 「前几项按旧事实、后几项按新事实」地被执行）。
    let items = plan_items(db, root, &original_run_id)?;
    let actual_digest = undo_digest(&items);
    if actual_digest != expected_digest || actual_digest != plan.digest {
        return Err(AppError::new(
            codes::STALE_PLAN,
            "撤销预览之后文件状态发生了变化，请重新预览再确认",
        ));
    }

    // 只有当前是 Ready 的项才允许被选中。
    let selectable: std::collections::HashSet<&str> = items
        .iter()
        .filter(|item| item.outcome == UndoOutcome::Ready)
        .map(|item| item.operation_id.as_str())
        .collect();
    let ready_total = selectable.len();
    for operation_id in selected {
        if !selectable.contains(operation_id.as_str()) {
            return Err(AppError::new(
                codes::UNDO_CONFLICT,
                "选中的项当前不满足撤销条件（原位置被占用、内容已改动或尚未完成整理），请重新预览",
            ));
        }
    }
    if ready_total > 0 && selected.is_empty() {
        return Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "没有选中任何可撤销的项",
        ));
    }

    // 在一个数据库事务中完成全局恢复闸门、令牌消费、run 与 requestId 参数绑定。
    let journal = Journal::new(db);
    let run_id = crate::storage::undo_execution::admit(
        db,
        &run.plan_id,
        undo_plan_id,
        &original_run_id,
        expected_digest,
        &hash_token(undo_token),
        selected,
        request_id,
        now_ms,
    )?;

    let mut report_items: Vec<UndoItem> = Vec::new();
    let mut warnings: Vec<Issue> = Vec::new();
    let mut reverted = 0u32;
    let mut conflicted = 0u32;
    let mut already_undone = 0u32;
    let mut untouched = 0u32;
    let mut needs_recovery = false;
    let mut cancelled = false;
    let total = selected.len() as u32;
    let mut processed = 0u32;

    // 先为所有选中项落 pending 意图。这样首项失败或用户取消时，后续项也能明确
    // 写成 skipped，而不是完全没有审计记录。
    let mut undo_operations = std::collections::HashMap::new();
    for (sequence, item) in items.iter().enumerate() {
        if item.outcome != UndoOutcome::Ready || !selected.iter().any(|id| id == &item.operation_id)
        {
            continue;
        }
        let original = runs::find_operation(db, &item.operation_id)?
            .ok_or_else(|| AppError::internal("原操作不存在"))?;
        let operation = new_undo_operation(
            &run_id,
            &original.item_id,
            &original.id,
            sequence as u32,
            &original.target,
            &original.source,
            &original.expected_json,
        );
        journal.record_intent(&operation)?;
        undo_operations.insert(item.operation_id.clone(), operation);
    }

    // 逆序执行（规格 8.4 第 1 条）。
    for (sequence, item) in items.iter().enumerate() {
        match item.outcome {
            UndoOutcome::AlreadyUndone => {
                already_undone += 1;
                report_items.push(item.clone());
                continue;
            }
            UndoOutcome::Conflict => {
                conflicted += 1;
                // 冲突项也写一条审计事件：用户下次预览时应当能看出
                // 「这一项上次就已经判过冲突」，而不是一片空白。
                mark_undo_conflict(
                    db,
                    &item.operation_id,
                    &uuid::Uuid::new_v4().to_string(),
                    &rfc3339_from_unix_ms(now_ms),
                    &item.message,
                )?;
                report_items.push(item.clone());
                continue;
            }
            UndoOutcome::Ready => {}
        }

        let is_selected = selected.iter().any(|id| id == &item.operation_id);
        if !is_selected {
            untouched += 1;
            report_items.push(item.clone());
            continue;
        }
        if observer.should_stop() {
            cancelled = true;
            mark_unprocessed_skipped(
                &journal,
                &items,
                sequence,
                selected,
                &undo_operations,
                now_ms,
            )?;
            append_unprocessed(
                &items,
                sequence,
                selected,
                &mut report_items,
                &mut untouched,
            );
            break;
        }

        let original = runs::find_operation(db, &item.operation_id)?
            .ok_or_else(|| AppError::internal("原操作不存在"))?;
        let expected: Fingerprint = serde_json::from_str(&original.expected_json)
            .map_err(|error| AppError::internal(format!("操作缺少指纹信息: {error}")))?;

        // 撤销操作的 source/target 与原来**互换**：搬回去。
        let undo_operation = undo_operations
            .get(&item.operation_id)
            .ok_or_else(|| AppError::internal("撤销意图记录不存在"))?;
        commit_undo_prepared(
            db,
            &undo_operation.id,
            &original.id,
            &uuid::Uuid::new_v4().to_string(),
            &rfc3339_from_unix_ms(now_ms),
        )?;

        let outcome = move_no_replace_with(
            root,
            &undo_operation.source,
            &undo_operation.target,
            &expected,
            MoveFailPoints::Undo,
            |_handle| Ok(()),
        );

        match outcome {
            Ok(_receipt) => {
                // 「文件已搬回」与「原操作 undoStatus = undone」在**同一事务**提交。
                match commit_revert(
                    db,
                    &undo_operation.id,
                    &original.id,
                    &uuid::Uuid::new_v4().to_string(),
                    &rfc3339_from_unix_ms(now_ms),
                ) {
                    Ok(()) => {
                        reverted += 1;
                        processed += 1;
                        observer.on_progress(processed, total);
                        report_items.push(UndoItem {
                            outcome: UndoOutcome::AlreadyUndone,
                            selected: true,
                            message: "已撤销：文件已搬回原位置，内容未变。".to_owned(),
                            ..item.clone()
                        });
                    }
                    Err(error) => {
                        // 磁盘已经变了、日志没落上。这与正向执行的同类情形一样危险，
                        // 必须进 recoveryRequired 并停止，不能让用户当作普通失败重试。
                        needs_recovery = true;
                        conflicted += 1;
                        report_items.push(UndoItem {
                            outcome: UndoOutcome::Conflict,
                            selected: false,
                            message: format!(
                                "文件已搬回原位置，但撤销日志未能写入（{}）。请先恢复数据库可用性，\
                                 再用恢复流程核对，不要盲目重试。",
                                error.message
                            ),
                            ..item.clone()
                        });
                        append_unprocessed(
                            &items,
                            sequence + 1,
                            selected,
                            &mut report_items,
                            &mut untouched,
                        );
                        mark_unprocessed_skipped(
                            &journal,
                            &items,
                            sequence + 1,
                            selected,
                            &undo_operations,
                            now_ms,
                        )?;
                        break;
                    }
                }
            }
            Err(error) => {
                if error.code == codes::RECOVERY_REQUIRED {
                    // 改名的结果不确定：交给恢复流程按「源目标互换」核对。
                    needs_recovery = true;
                    conflicted += 1;
                    journal.mark_ambiguous(&undo_operation.id, &error.message, now_ms)?;
                    report_items.push(UndoItem {
                        outcome: UndoOutcome::Conflict,
                        selected: false,
                        message: format!(
                            "撤销过程中状态不确定：{}。请用恢复流程核对。",
                            error.message
                        ),
                        ..item.clone()
                    });
                    append_unprocessed(
                        &items,
                        sequence + 1,
                        selected,
                        &mut report_items,
                        &mut untouched,
                    );
                    mark_unprocessed_skipped(
                        &journal,
                        &items,
                        sequence + 1,
                        selected,
                        &undo_operations,
                        now_ms,
                    )?;
                    break;
                }

                // 确定没动过：如实记成冲突并保留现状（规格 8.4 第 3 条）。
                conflicted += 1;
                processed += 1;
                observer.on_progress(processed, total);
                let message = conflict_message(&error);
                journal.mark_failed(&undo_operation.id, &error.code, &message, now_ms)?;
                mark_undo_conflict(
                    db,
                    &original.id,
                    &uuid::Uuid::new_v4().to_string(),
                    &rfc3339_from_unix_ms(now_ms),
                    &message,
                )?;
                report_items.push(UndoItem {
                    outcome: UndoOutcome::Conflict,
                    selected: false,
                    message,
                    ..item.clone()
                });
                append_unprocessed(
                    &items,
                    sequence + 1,
                    selected,
                    &mut report_items,
                    &mut untouched,
                );
                mark_unprocessed_skipped(
                    &journal,
                    &items,
                    sequence + 1,
                    selected,
                    &undo_operations,
                    now_ms,
                )?;
                break;
            }
        }
    }

    // 目录清理（规格 8.4 第 7 条）。失败只记警告——文件已经安全回去了，
    // 把「空目录没删掉」说成撤销失败是错的。
    //
    // **没有撤回过任何项时不做清理**，两个理由：
    // 1. 语义上：分类目录只可能因为「文件被搬回原位」而变空。一项都没搬，
    //    就没有「本次造成的空目录」这回事。
    // 2. 结构上：审计事件要挂在一条**真实操作**上（`operation_events` 有外键），
    //    而这时 undo run 里一条操作都没有。硬要写就只能挂在一个不存在的
    //    operationId 上——那会被外键直接拒掉，而且**静默**。
    if !needs_recovery && reverted > 0 {
        warnings.extend(clean_created_dirs(
            db,
            root,
            &original_run_id,
            &run_id,
            now_ms,
        )?);
    }

    let status = if needs_recovery {
        RunStatus::RecoveryRequired
    } else if cancelled {
        RunStatus::Cancelled
    } else if conflicted > 0 || untouched > 0 {
        // **绝不把部分撤销标成全部成功**：有任一项没回去，就是 partial。
        RunStatus::Partial
    } else {
        RunStatus::Completed
    };
    let report = UndoReport {
        run_id,
        original_run_id,
        status,
        reverted,
        conflicted,
        already_undone,
        untouched,
        items: report_items,
        warnings,
    };
    crate::storage::undo_execution::finish_report(db, request_id, &report, now_ms)?;
    Ok(report)
}

fn mark_unprocessed_skipped(
    journal: &Journal<'_>,
    items: &[UndoItem],
    from: usize,
    selected: &[String],
    operations: &std::collections::HashMap<String, OperationRow>,
    now_ms: i64,
) -> Result<(), AppError> {
    for item in items.iter().skip(from) {
        if !selected.iter().any(|id| id == &item.operation_id) {
            continue;
        }
        if let Some(operation) = operations.get(&item.operation_id) {
            journal.mark_skipped(&operation.id, now_ms)?;
        }
    }
    Ok(())
}

fn append_unprocessed(
    items: &[UndoItem],
    from: usize,
    selected: &[String],
    report_items: &mut Vec<UndoItem>,
    untouched: &mut u32,
) {
    for item in items.iter().skip(from) {
        let was_selected = selected.iter().any(|id| id == &item.operation_id);
        if item.outcome == UndoOutcome::Ready && was_selected {
            *untouched += 1;
            report_items.push(UndoItem {
                selected: false,
                message: "前一项撤销失败，本项未处理，文件保持原位。".to_owned(),
                ..item.clone()
            });
        } else {
            report_items.push(item.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn replay_undo_request(
    db: &Database,
    undo_plan_id: &str,
    original_run_id: &str,
    digest: &str,
    undo_token: &str,
    selected: &[String],
    request_id: &str,
) -> Result<Option<UndoReport>, AppError> {
    let existing = crate::storage::undo_execution::replay(
        db,
        request_id,
        undo_plan_id,
        original_run_id,
        digest,
        &hash_token(undo_token),
        selected,
    )?;
    existing
        .map(|run_id| report_for_undo_run(db, &run_id))
        .transpose()
}

/// 读一份已存在的撤销 run 的结果。
///
/// 两个用途：`execute_undo` 的幂等返回（同一个 requestId 再点一次），
/// 以及前端按 runId 查回结果（规格 5.1：undoRunId 用 get_run 查得到）。
pub fn report_for_undo_run(db: &Database, run_id: &str) -> Result<UndoReport, AppError> {
    if let Some(report) = crate::storage::undo_execution::load_report(db, run_id)? {
        return Ok(report);
    }
    let row = load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到撤销记录 {run_id}")))?;
    let status = runs::run_status_from_text(&row.status)?;
    let operations = runs::list_operations(db, run_id)?;

    let mut reverted = 0u32;
    let mut conflicted = 0u32;
    let mut items = Vec::new();
    for operation in &operations {
        let outcome = match operation.status {
            crate::domain::types::OpStatus::Applied => {
                reverted += 1;
                UndoOutcome::AlreadyUndone
            }
            crate::domain::types::OpStatus::Pending
            | crate::domain::types::OpStatus::Prepared
            | crate::domain::types::OpStatus::Skipped => UndoOutcome::Conflict,
            crate::domain::types::OpStatus::Failed | crate::domain::types::OpStatus::Ambiguous => {
                conflicted += 1;
                UndoOutcome::Conflict
            }
        };
        items.push(UndoItem {
            operation_id: operation.original_operation_id.clone().unwrap_or_default(),
            item_id: operation.item_id.clone(),
            source: operation.source.clone(),
            target: operation.target.clone(),
            outcome,
            selected: operation.status == crate::domain::types::OpStatus::Applied,
            message: "这是上一次撤销留下的结果。".to_owned(),
        });
    }

    Ok(UndoReport {
        run_id: run_id.to_owned(),
        original_run_id: crate::storage::undo_execution::original_run_id(db, run_id)?
            .unwrap_or_else(|| row.plan_id.clone()),
        status,
        reverted,
        conflicted,
        already_undone: 0,
        untouched: 0,
        items,
        warnings: Vec::new(),
    })
}

/// 撤销前的阻塞闸门（规格 8.2）。
///
/// 「存在 recoveryRequired 或 ambiguous 未解决操作时，后端禁止新执行/撤销任务。」
/// 这条规则在撤销上的意义尤其直接：**上一次连文件去了哪儿都还没确定，
/// 现在却要基于那份记录把文件搬回去**——等于拿一个自己都没算清的账去改用户的文件。
///
/// 允许的例外只有一种：用户已经对未决项做过处置（`acknowledged`）。
/// 那代表他看过并接受了现状，此时账是清楚的——「这两份都留着」。
fn ensure_not_blocked(db: &Database, original_run_id: &str) -> Result<(), AppError> {
    crate::storage::undo_execution::ensure_globally_unblocked(db)?;
    let run = load_run(db, original_run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {original_run_id}")))?;
    if run.status == "recoveryRequired" {
        return Err(AppError::new(
            codes::RECOVERY_REQUIRED,
            "这次整理还有未决项没核对完，请先在「恢复」页处理，再撤销",
        ));
    }

    let unresolved = runs::list_operations(db, original_run_id)?
        .into_iter()
        .filter(|operation| {
            operation.status == crate::domain::types::OpStatus::Ambiguous
                && operation.resolution != crate::domain::types::OpResolution::Acknowledged
        })
        .count();
    if unresolved > 0 {
        return Err(AppError::new(
            codes::RECOVERY_REQUIRED,
            format!("这次整理还有 {unresolved} 项无法自动判定，请先在「恢复」页核对，再撤销"),
        ));
    }
    Ok(())
}

/// 按逆序给出待处理的项，并逐项分类。
///
/// 只取 `applied` 的项：`skipped` 从未被派发，`failed` 确定没动，
/// `pending`/`prepared`/`ambiguous` 属于恢复流程的管辖范围——
/// 撤销在这些状态上下手，等于在「不知道文件在哪」的前提下搬东西。
fn plan_items(
    db: &Database,
    root: &ApprovedRoot,
    original_run_id: &str,
) -> Result<Vec<UndoItem>, AppError> {
    let operations = runs::list_operations(db, original_run_id)?;
    let undone: std::collections::HashSet<String> = undone_original_ids(db, original_run_id)?
        .into_iter()
        .collect();

    let mut candidates: Vec<&OperationRow> = operations
        .iter()
        .filter(|operation| operation.status == crate::domain::types::OpStatus::Applied)
        .collect();
    // 规格 8.4 第 1 条：按原 sequence **逆序**。
    // 整理时是先建目录再往深处放，撤销倒着走才能自然地把目录掏空。
    candidates.sort_by_key(|operation| std::cmp::Reverse(operation.sequence));

    let mut items = Vec::with_capacity(candidates.len());
    for operation in candidates {
        // 两条判据都要看：`undoStatus == undone` 是本次流程写下的，
        // 而 `undone` 集合来自「库里存在一条已应用的反向操作」。
        // 后者能覆盖「写 undone 之前崩掉、恢复流程补齐」的情形。
        let already = operation.undo_status == UndoStatus::Undone || undone.contains(&operation.id);

        let classified = if already {
            Classified {
                verdict: Verdict::AlreadyUndone,
                message: "这一项此前已经撤销过，文件已经在原位置，不会再搬动它。".to_owned(),
            }
        } else {
            classify(root, operation)?
        };

        items.push(UndoItem {
            operation_id: operation.id.clone(),
            item_id: operation.item_id.clone(),
            source: operation.target.clone(),
            target: operation.source.clone(),
            outcome: classified.verdict.outcome(),
            // 规格 8.4 第 3 条：冲突项默认不选中；已撤销的也没有可做的动作。
            selected: classified.verdict == Verdict::Ready,
            message: classified.message,
        });
    }
    Ok(items)
}

/// 单项的磁盘核对（规格 8.4 第 2、3、4 条）。
fn classify(root: &ApprovedRoot, operation: &OperationRow) -> Result<Classified, AppError> {
    let expected: Fingerprint = serde_json::from_str(&operation.expected_json)
        .map_err(|error| AppError::internal(format!("操作缺少指纹信息: {error}")))?;

    // 撤销的方向与整理相反：文件现在应当在**原操作的目标**位置。
    let at_target = fingerprint_at(root, &operation.target, &expected.volume_id)?;
    let at_source = fingerprint_at(root, &operation.source, &expected.volume_id)?;

    let Some(now) = at_target else {
        return Ok(Classified {
            verdict: Verdict::Conflict,
            message: "整理后的文件已经不在目标位置了，无法安全地把它搬回去。\
                      请先用恢复流程核对这一项的实际去向。"
                .to_owned(),
        });
    };
    if !same_thing(&now, &expected) {
        return Ok(Classified {
            verdict: Verdict::Conflict,
            message: "目标位置的文件已经不是当初整理的那个（身份或内容已改变）。\
                      规格要求不擅自搬动整理后被修改的文件，因此保留现状。"
                .to_owned(),
        });
    }

    if at_source.is_some() {
        return Ok(Classified {
            verdict: Verdict::Conflict,
            message: "原位置已经有了别的文件。撤销不会覆盖它，两个文件都保留。\
                      请先把它移走或改名，再重新预览撤销。"
                .to_owned(),
        });
    }

    // 规格 8.4 第 4 条：原父目录被外部删掉了，v0.1 不擅自重建用户的目录。
    // 建回去看起来「贴心」，但那是在用户的目录结构里凭空造东西——
    // 用户当初删掉那个目录可能正是有意的。
    if let Some(parent) = parent_of(&operation.source).filter(|path| !path.is_empty()) {
        // 这是数据库里记录的既有原目录，不是新生成的分类名称。
        // 只应用防逃逸校验，不能再套用“新名称 80 UTF-16”限制。
        let parent_path = root.resolve_existing_within(&parent)?;
        if std::fs::symlink_metadata(&parent_path).is_err() {
            return Ok(Classified {
                verdict: Verdict::Conflict,
                message: format!(
                    "原来所在的目录 {} 已经不在了。v0.1 不会替用户重建目录，\
                     请先手动建好它再重新预览撤销。",
                    parent.join("\\")
                ),
            });
        }
    }

    Ok(Classified {
        verdict: Verdict::Ready,
        message: "可以撤销：文件仍在整理后的位置且内容未变，原位置是空的。".to_owned(),
    })
}

/// 清理本次整理**自己创建**、身份一致且已经空了的分类目录。
///
/// 规格 8.4 第 7 条的三条限制都体现在这里：
/// * 只碰 `created_by_run` 为真的目录——用户原有的目录一个都不动；
/// * 只做**非递归**删除——里面还有用户的文件时，`remove_dir` 自己就会失败；
/// * 失败只记警告——文件已经安全回去了，那是另一码事。
fn clean_created_dirs(
    db: &Database,
    root: &ApprovedRoot,
    original_run_id: &str,
    undo_run_id: &str,
    now_ms: i64,
) -> Result<Vec<Issue>, AppError> {
    let mut warnings = Vec::new();

    // 逆序：先删深的，再删浅的。正序删的话父目录永远不空。
    let mut dirs: Vec<CreatedDirRow> = list_created_dirs(db, original_run_id)?;
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.relative_path.len()));

    for dir in dirs {
        if !dir.created_by_run || dir.state != "created" {
            continue;
        }

        match crate::safety::fingerprint::remove_created_directory(
            root,
            &dir.relative_path,
            &dir.directory_identity,
        ) {
            Ok(true) => {
                append_event(
                    db,
                    &uuid::Uuid::new_v4().to_string(),
                    // 事件表有外键指向 operations，必须挂在一条真实操作上。
                    &host_operation_id(db, undo_run_id)?,
                    DIR_REMOVED_EVENT,
                    &rfc3339_from_unix_ms(now_ms),
                    Some(
                        &serde_json::json!({
                            "runId": original_run_id,
                            "path": dir.relative_path.join("\\"),
                        })
                        .to_string(),
                    ),
                )?;
            }
            Ok(false) => {}
            Err(error) => warnings.push(dir_warning(&dir, &error.message)),
        }
    }

    Ok(warnings)
}

/// 「本次创建的目录已被清理」这条审计事件的名字。
const DIR_REMOVED_EVENT: &str = "undoRemovedDirectory";

/// 取这次 undo run 下第一条操作，用来挂事件。
///
/// 与 `recovery` 里的做法同源：`operation_events.operationId` 有外键，
/// 合成的 `<runId>#dir` 这种 id 会被直接拒掉，而且**静默**——
/// 看起来审计过了，实际什么都没写。
fn host_operation_id(db: &Database, run_id: &str) -> Result<String, AppError> {
    runs::list_operations(db, run_id)?
        .first()
        .map(|operation| operation.id.clone())
        .ok_or_else(|| AppError::internal("撤销记录没有任何操作，无法挂载审计事件"))
}

fn dir_warning(dir: &CreatedDirRow, reason: &str) -> Issue {
    Issue {
        code: codes::UNDO_CONFLICT.to_owned(),
        severity: crate::domain::types::Risk::Warning,
        item_id: None,
        message: format!("目录 {} 未能清理：{}", dir.relative_path.join("\\"), reason),
    }
}

/// 把一次「确定没动过」的失败翻译成用户能行动的话。
fn conflict_message(error: &AppError) -> String {
    match error.code.as_str() {
        codes::TARGET_EXISTS => {
            "原位置在这期间出现了同名文件。为了不覆盖它，这一项保留现状。".to_owned()
        }
        codes::SOURCE_MISSING => {
            "整理后的文件已经不在目标位置了，无法搬回。请用恢复流程核对该项去向。".to_owned()
        }
        codes::SOURCE_CHANGED => {
            "文件内容在整理之后被修改过。规格要求不擅自搬动被修改的文件，因此保留现状。".to_owned()
        }
        _ => format!("撤销未能完成（{}）：{}", error.code, error.message),
    }
}

/// 原路径的父目录；已经在根下第一层时返回空数组（表示根本身）。
fn parent_of(relative: &[String]) -> Option<RelPath> {
    if relative.is_empty() {
        return None;
    }
    Some(relative[..relative.len() - 1].to_vec())
}

/// 撤销预览的摘要。
///
/// 覆盖每一项的**操作 id、撤销状态与当前判定**，因此：
/// * 任何一项从 Ready 变成 Conflict，摘要就变；
/// * 多撤了一项、少撤了一项，摘要也变。
///
/// **刻意不包含 `selected`**：用户正是要在预览之后调整勾选，
/// 把勾选算进摘要会让「取消勾选一项」变成「确认过期」，逼用户重新预览一遍，
/// 而他手上的信息一点没变。勾选的合法性由「只能选当前 Ready 的项」来保证。
pub fn undo_digest(items: &[UndoItem]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(UNDO_DIGEST_VERSION.as_bytes());
    for item in items {
        hasher.update(item.operation_id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(outcome_text(item.outcome).as_bytes());
        hasher.update(b"\x1f");
        hasher.update(item.source.join("\\").as_bytes());
        hasher.update(b"\x1f");
        hasher.update(item.target.join("\\").as_bytes());
        hasher.update(b"\x1e");
    }
    format!("{:x}", hasher.finalize())
}

fn outcome_text(outcome: UndoOutcome) -> &'static str {
    match outcome {
        UndoOutcome::Ready => "ready",
        UndoOutcome::Conflict => "conflict",
        UndoOutcome::AlreadyUndone => "alreadyUndone",
    }
}

fn count(items: &[UndoItem], outcome: UndoOutcome) -> u32 {
    items.iter().filter(|item| item.outcome == outcome).count() as u32
}

pub(crate) fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"filepilot.undo.token.v1");
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 两个指纹是不是「同一个东西，且内容没变」。
///
/// 与 `recovery` 里同名函数的判据一致，但**不能合并**：
/// 那边是「核对上次执行的结果」，这边是「核对能不能撤销」。
/// 将来一边要加放宽条件时，合并会让另一边悄悄跟着变。
fn same_thing(now: &Fingerprint, expected: &Fingerprint) -> bool {
    if now.volume_id != expected.volume_id
        || now.file_id != expected.file_id
        || now.size != expected.size
        || now.modified_ns != expected.modified_ns
    {
        return false;
    }
    match (&now.sha256, &expected.sha256) {
        (Some(a), Some(b)) => a == b,
        // 没有内容哈希就**不当作匹配**——否则「无法确认」会变成默认通过。
        _ => false,
    }
}

fn fingerprint_at(
    root: &ApprovedRoot,
    relative: &[String],
    volume_id: &str,
) -> Result<Option<Fingerprint>, AppError> {
    crate::safety::fingerprint::guarded_fingerprint_at(root, relative, volume_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, outcome: UndoOutcome) -> UndoItem {
        UndoItem {
            operation_id: id.to_owned(),
            item_id: format!("item-{id}"),
            source: vec!["文档".to_owned(), "a.txt".to_owned()],
            target: vec!["a.txt".to_owned()],
            outcome,
            selected: outcome == UndoOutcome::Ready,
            message: String::new(),
        }
    }

    #[test]
    fn the_digest_changes_when_any_verdict_changes() {
        let base = vec![
            item("op-1", UndoOutcome::Ready),
            item("op-2", UndoOutcome::Ready),
        ];
        let flipped = vec![
            item("op-1", UndoOutcome::Ready),
            item("op-2", UndoOutcome::Conflict),
        ];
        assert_ne!(
            undo_digest(&base),
            undo_digest(&flipped),
            "一项从可撤销变成冲突，摘要必须跟着变，否则用户能拿旧确认提交"
        );
    }

    #[test]
    fn the_digest_ignores_which_items_the_user_ticked() {
        // 勾选是用户看完之后才做的调整，磁盘事实一点没变。
        // 把它算进摘要会让「取消勾选一项」变成「确认过期」。
        let mut picked = item("op-1", UndoOutcome::Ready);
        picked.selected = true;
        let mut unpicked = item("op-1", UndoOutcome::Ready);
        unpicked.selected = false;
        assert_eq!(undo_digest(&[picked]), undo_digest(&[unpicked]));
    }

    #[test]
    fn a_different_file_with_the_same_name_is_not_the_same_thing() {
        let now = Fingerprint {
            volume_id: "V".to_owned(),
            file_id: "A".to_owned(),
            size: "1".to_owned(),
            modified_ns: "0".to_owned(),
            sha256: Some("x".to_owned()),
        };
        let other = Fingerprint {
            file_id: "B".to_owned(),
            ..now.clone()
        };
        assert!(!same_thing(&other, &now));
        assert!(same_thing(&now, &now));
    }

    #[test]
    fn a_missing_hash_never_counts_as_a_match() {
        let with = Fingerprint {
            volume_id: "V".to_owned(),
            file_id: "A".to_owned(),
            size: "1".to_owned(),
            modified_ns: "0".to_owned(),
            sha256: Some("x".to_owned()),
        };
        let without = Fingerprint {
            sha256: None,
            ..with.clone()
        };
        assert!(!same_thing(&without, &with));
    }

    #[test]
    fn the_parent_of_a_top_level_file_is_the_root_itself() {
        assert_eq!(parent_of(&["a.txt".to_owned()]), Some(vec![]));
        assert_eq!(
            parent_of(&["文档".to_owned(), "a.txt".to_owned()]),
            Some(vec!["文档".to_owned()])
        );
        assert_eq!(parent_of(&[]), None);
    }
}
