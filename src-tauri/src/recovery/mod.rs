//! 崩溃恢复。规格 8.3 的决策表。
//!
//! 恢复的**唯一依据是磁盘事实**，不是内存记忆，也不是「日志里写了什么」——
//! 日志只能告诉我们「可能发生了什么」，最终判定必须落到「现在磁盘上是什么样」。
//!
//! 规格 8.3 的决策表：
//!
//! | 源位置 | 目标位置 | 处理 |
//! |---|---|---|
//! | 原文件仍在，快照匹配 | 不存在 | 记为未应用 |
//! | 不存在 | 原文件身份及快照匹配 | 记为已应用，写恢复事件，不重复移动 |
//! | 两处都有文件 | 任意 | ambiguous，保留两者 |
//! | 两处均不存在 | 无 | ambiguous，展示两处路径 |
//! | 任意一处身份不符或内容改变 | 任意 | ambiguous，禁止自动猜测 |
//!
//! 另外（规格同段）：**「识别出文件」与「满足执行条件」是两件事**。
//! 源文件的身份对得上、但内容被改过，仍然不能自动继续——报告里要把这两件事分开说，
//! 否则用户看到的「找到了文件」会被误读成「可以继续」。

pub mod startup;

use crate::domain::errors::{codes, AppError};
use crate::domain::time::rfc3339_from_unix_ms;
use crate::domain::types::{
    Fingerprint, OpResolution, OpStatus, RelPath, RunCounts, RunStatus, UndoStatus,
};
use crate::safety::root::ApprovedRoot;
use crate::storage::db::Database;
use crate::storage::runs::{
    self, acknowledge_operation, append_event, list_created_dirs, list_events, list_operations,
    set_operation_status_and_event, CreatedDirRow, OperationRow,
};

/// 单项的判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 源还在、目标不在：这一项确实没被执行。
    NotApplied,
    /// 目标在且身份与内容都对得上：这一项确实执行了。
    Applied,
    /// 无法判定。**绝不猜**。
    Ambiguous,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::NotApplied => "notApplied",
            Verdict::Applied => "applied",
            Verdict::Ambiguous => "ambiguous",
        }
    }
}

/// 单个未决项的核对结果。
#[derive(Debug, Clone)]
pub struct ReconciledItem {
    pub operation_id: String,
    pub item_id: String,
    pub source: RelPath,
    pub target: RelPath,
    pub verdict: Verdict,
    /// 用户是否已经对这件事做过决定。
    pub resolution: OpResolution,
    /// 为什么是这个判定。
    ///
    /// 这句话会直接出现在界面上，所以要写「用户能据以行动」的内容，
    /// 而不是 `source_fp.is_none()` 这种内部状态。
    pub message: String,
}

/// `reconcile_run` 的结果。
#[derive(Debug, Clone)]
pub struct ReconcileOutcome {
    pub run_id: String,
    pub plan_id: String,
    pub status: RunStatus,
    /// 当前未决事实的摘要；界面上「保留现状并确认知晓」要用它做校验。
    pub state_digest: String,
    pub items: Vec<ReconciledItem>,
    pub counts: RunCounts,
    /// 仍有未决项时为 true —— 规格要求此时**阻止新任务**。
    pub blocks_new_runs: bool,
}

/// 核对一次执行留下的状态。
///
/// **幂等**：多次调用得到同样的判定。这个函数只做两件事：
/// 读磁盘、把判定写回日志。已写下的 `applied` 不会被改写
/// （`set_operation_status` 的 SQL 自带这个约束）。
pub fn reconcile_run(
    db: &Database,
    root: &ApprovedRoot,
    run_id: &str,
    now_ms: i64,
) -> Result<ReconcileOutcome, AppError> {
    let run = runs::load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {run_id}")))?;

    let operations = list_operations(db, run_id)?;
    let mut items = Vec::new();

    for operation in &operations {
        // `applied` 是确定的事实，不需要也不应该再核对：
        // 磁盘与日志已经一致，再算一遍哈希只是浪费。
        // `skipped` 同理——它从未被派发过。
        if matches!(operation.status, OpStatus::Applied | OpStatus::Skipped) {
            continue;
        }

        let outcome = classify(root, operation)?;
        let target_status = outcome.verdict.into_status();

        if operation.status != target_status {
            let payload = serde_json::json!({ "verdict": outcome.verdict.as_str() }).to_string();
            set_operation_status_and_event(
                db,
                &operation.id,
                target_status,
                None,
                &uuid::Uuid::new_v4().to_string(),
                match outcome.verdict {
                    Verdict::Applied => "recoveredApplied",
                    Verdict::NotApplied => "recoveredNotApplied",
                    Verdict::Ambiguous => "recoveredAmbiguous",
                },
                &rfc3339_from_unix_ms(now_ms),
                Some(&payload),
            )?;
        }

        // 撤销 run 的核对（规格 8.4 第 6 条）。
        //
        // 好消息是**决策表本身不用改**：撤销操作的 source/target 本来就是
        // 把原操作的两个位置互换过来的，所以「源在、目标不在」等判定
        // 原样适用，只是语义变成「文件还在整理后的位置 → 撤销没做完」。
        // 规格特意写明「不能直接把 undone 写上」，指的正是下面这一步：
        // undone 是**核对出来**的结论，不是照抄日志里的字面状态。
        if run.direction == "undo" {
            reconcile_original_undo_status(db, operation, outcome.verdict, now_ms)?;
        }

        items.push(ReconciledItem {
            operation_id: operation.id.clone(),
            item_id: operation.item_id.clone(),
            source: operation.source.clone(),
            target: operation.target.clone(),
            verdict: outcome.verdict,
            resolution: operation.resolution,
            message: outcome.message,
        });
    }

    // 本次创建但尚未收尾的目录：**不猜归属，保留原状**。
    // 这里只补一条审计事件，让「为什么这个目录还在」有据可查。
    //
    // 事件挂在**第一条操作**上而不是造一个假 operationId：
    // `operation_events.operationId` 有外键指向 `operations(id)` 且 `foreign_keys` 是开着的
    // （规格 8.1），任何 `<runId>#dir` 这种合成 id 都会被约束直接拒掉。
    // 更糟的是它**静默**——写失败被忽略掉，界面上看起来「审计过了」，
    // 实际什么都没记下。宁可挂在一条真实操作上，也不要一条永远写不进去的假记录。
    if let Some(host) = operations.first().map(|operation| operation.id.clone()) {
        for dir in list_created_dirs(db, run_id)? {
            if dir.state != "created" {
                continue;
            }
            // 幂等闸门：同一个目录只留一条。
            //
            // `reconcile_run` 每次启动都会跑一遍，而这条事件描述的是
            // 「这个目录是本次留下的」，是一个**不变的事实**。
            // 不加判断地追加，会让审计表随着开应用次数线性膨胀——
            // 用户看到一堆完全相同的记录，真正有用的那条反而被埋掉。
            let already_recorded = list_events(db, &host)?.iter().any(|event| {
                event.phase == KEPT_DIRECTORY_EVENT
                    && event.payload_json.as_deref() == Some(&payload_for(run_id, &dir))
            });
            if already_recorded {
                continue;
            }

            append_event(
                db,
                &uuid::Uuid::new_v4().to_string(),
                &host,
                KEPT_DIRECTORY_EVENT,
                &rfc3339_from_unix_ms(now_ms),
                Some(&payload_for(run_id, &dir)),
            )
            // 这里必须传播错误。挂 `.ok()` 会让「审计写入失败」变成一个
            // 看不见的空操作——而恢复流程存在的意义就是留下可查的痕迹。
            ?;
        }
    }

    finish_from_operations(db, &run, items, now_ms)
}

/// 根据**当前落库的操作事实**推导 run 的终态并写回。
///
/// `reconcile_run` 与 `acknowledge_items` 共用它。两处各写一遍的话，
/// 「确认知晓之后到底该不该解除阻塞」迟早在两个入口上给出不同答案。
///
/// `examined` 是**本次调用真正看过的那些项**，由调用方给：
/// - `reconcile_run` 给的是它这一轮实际分类过的项；
/// - `acknowledge_items` 给的是它刚刚确认掉的那些项。
///
/// 不由这里从库里重新推导。`items` 的语义是「这一次需要人看的东西」，
/// 不是「库里所有没完成的项」——后者会让已经确定完成的项每次都重新冒出来，
/// 用户永远收敛不到一个干净的清单。
fn finish_from_operations(
    db: &Database,
    run: &runs::RunRow,
    examined: Vec<ReconciledItem>,
    now_ms: i64,
) -> Result<ReconcileOutcome, AppError> {
    let after = list_operations(db, &run.id)?;
    let blocks_new_runs = count_unresolved(&after) > 0;

    let counts = runs::count_operations(db, &run.id)?;
    let status = if blocks_new_runs {
        RunStatus::RecoveryRequired
    } else if counts.failed > 0 && counts.applied > 0 {
        RunStatus::Partial
    } else if counts.failed > 0 {
        RunStatus::Failed
    } else if counts.pending > 0 {
        // 还有没派发也没标记的项：说明这次核对没能覆盖全部，保守地当成未完成。
        RunStatus::Partial
    } else {
        RunStatus::Completed
    };

    // 写回终态。多次调用写入同样的值，因此是幂等的。
    runs::finish_run(db, &run.id, status, &rfc3339_from_unix_ms(now_ms))?;

    let items = examined;

    Ok(ReconcileOutcome {
        run_id: run.id.clone(),
        plan_id: run.plan_id.clone(),
        status,
        state_digest: crate::commands_execute::state_digest(&after),
        items,
        counts,
        blocks_new_runs,
    })
}

/// 「本次创建的目录已被清理」这条审计事件的名字。
const KEPT_DIRECTORY_EVENT: &str = "recoveryKeptDirectory";

/// 「用户确认保留现状」这条审计事件的名字。
const ACKNOWLEDGED_EVENT: &str = "recoveryAcknowledged";

/// 核对一条撤销操作之后，把**原操作**的撤销状态补齐（规格 8.4 第 6 条）。
///
/// 三种判定对应三种不同的补齐动作，区别在于「现在确不确定」：
///
/// | 判定 | 含义 | 原操作 undoStatus |
/// |---|---|---|
/// | `Applied` | 文件确实回到了原处 | `undone` |
/// | `NotApplied` | 文件还在整理后的位置，撤销没做 | 回到 `notRequested`，允许重试 |
/// | `Ambiguous` | 判不出来 | 保持 `prepared`，交给人 |
///
/// 最后一行是这条规则的要害：**判不出来时就该停在「不确定」**。
/// 顺手写个 `undone` 会让界面显示「已撤销」，而文件其实可能还在别处；
/// 写回 `notRequested` 则会让用户再撤一次，把可能已经回去的文件再搬一遍。
fn reconcile_original_undo_status(
    db: &Database,
    undo_operation: &OperationRow,
    verdict: Verdict,
    now_ms: i64,
) -> Result<(), AppError> {
    let Some(original_id) = undo_operation.original_operation_id.as_deref() else {
        // 撤销操作必然指回一个原操作（规格 5.1）。缺了它说明这条记录
        // 不是撤销流程写下的，不该由这里替它补状态。
        return Ok(());
    };
    let Some(original) = runs::find_operation(db, original_id)? else {
        return Ok(());
    };

    let target = match verdict {
        Verdict::Applied => Some(UndoStatus::Undone),
        Verdict::NotApplied => Some(UndoStatus::NotRequested),
        Verdict::Ambiguous => None,
    };
    let Some(target) = target else {
        return Ok(());
    };

    // 幂等闸门：`reconcile_run` 每次启动都跑，状态没变就不再追加事件，
    // 否则审计表会随着开应用次数线性膨胀。
    if original.undo_status == target {
        return Ok(());
    }

    runs::set_undo_status(db, original_id, target)?;
    append_event(
        db,
        &uuid::Uuid::new_v4().to_string(),
        original_id,
        match verdict {
            Verdict::Applied => "recoveryCompletedUndo",
            _ => "recoveryAbandonedUndo",
        },
        &rfc3339_from_unix_ms(now_ms),
        Some(&format!(
            "{{\"verdict\":\"{}\",\"undoOperationId\":{:?}}}",
            verdict.as_str(),
            undo_operation.id
        )),
    )?;
    Ok(())
}

/// 还有几项没被处置。
///
/// **`status == ambiguous` 只是必要条件，不是充分条件**：
/// 用户明确接受「保留现状」之后，磁盘事实仍然是 ambiguous（两份文件都还在，
/// 应用不会替用户删任何一份），但这件事已经有人管过了。
/// 只数 status 会让用户每次启动都被同一件事拦住——而他能做的只有再次点同一个按钮。
fn count_unresolved(operations: &[OperationRow]) -> usize {
    operations
        .iter()
        .filter(|op| {
            op.status == OpStatus::Ambiguous && op.resolution != OpResolution::Acknowledged
        })
        .count()
}

/// 把一批未决项标记为「保留现状并确认知晓」（规格 8.2）。
///
/// 交付给用户的是**显式审计事件**，不是「把日志删掉」或「假称已撤销」：
/// - `operations.status` 保持 `ambiguous` 不动——磁盘上确实还是两处都有文件；
/// - 只把 `resolution` 置为 `acknowledged`，另外追加一条
///   `recoveryAcknowledged` 事件，载荷里带上用户给的理由。
///
/// `expected_digest` 必须与当前事实一致。校验放在**写之前**，且作用于整批：
/// 用户核对的是「那一刻看到的这一屏」，中途任何一项变了，
/// 他确认过的东西就不再是他确认过的东西了。逐项校验会留下
/// 「前三条按旧事实确认、后两条按新事实确认」的混合结果，那比报错更糟。
pub fn acknowledge_items(
    db: &Database,
    run_id: &str,
    expected_digest: &str,
    reason: &str,
    now_ms: i64,
) -> Result<ReconcileOutcome, AppError> {
    let run = runs::load_run(db, run_id)?
        .ok_or_else(|| AppError::internal(format!("找不到执行记录 {run_id}")))?;

    let before = list_operations(db, run_id)?;
    let actual_digest = crate::commands_execute::state_digest(&before);
    if actual_digest != expected_digest {
        return Err(AppError::new(
            codes::STALE_PLAN,
            "未决项在核对之后发生了变化，请重新核对再确认",
        ));
    }

    let reason = reason.trim();
    if reason.is_empty() {
        // 理由不是装饰。半年后回看这条记录时，「为什么当时就这样算了」
        // 是唯一能让后来者（包括用户自己）做出正确判断的信息。
        return Err(AppError::new(
            codes::REQUEST_CONFLICT,
            "确认保留现状必须写明理由，便于日后追溯",
        ));
    }

    let timestamp = rfc3339_from_unix_ms(now_ms);
    let mut settled = Vec::new();
    for operation in &before {
        if operation.status != OpStatus::Ambiguous {
            continue;
        }
        // 已经确认过的跳过：重复提交同一份确认是幂等的，不该再追加一条事件。
        if operation.resolution == OpResolution::Acknowledged {
            continue;
        }

        acknowledge_operation(db, &operation.id)?;
        append_event(
            db,
            &uuid::Uuid::new_v4().to_string(),
            &operation.id,
            ACKNOWLEDGED_EVENT,
            &timestamp,
            Some(&acknowledged_payload(&operation.item_id, reason)),
        )?;

        settled.push(ReconciledItem {
            operation_id: operation.id.clone(),
            item_id: operation.item_id.clone(),
            source: operation.source.clone(),
            target: operation.target.clone(),
            verdict: Verdict::Ambiguous,
            resolution: OpResolution::Acknowledged,
            message: describe_settled(),
        });
    }

    finish_from_operations(db, &run, settled, now_ms)
}

/// 刚被确认接受的那一项，该怎么向用户复述。
///
/// 措辞要**明确说出「磁盘没变」**：用户点的是「保留现状」，
/// 不是「让应用处理掉」，含糊的说法会让人以为那两份文件已经被合并或删掉了。
fn describe_settled() -> String {
    "已确认保留现状：磁盘上的文件没有被改动，两处内容仍在原处。".to_owned()
}

/// 确认事件里记什么。
///
/// 用 `{:?}` 而不是手写引号拼接：理由来自用户输入，直接内插进 JSON
/// 会让一个引号或反斜杠把整条载荷变成非法 JSON——而这条载荷正是
/// 「为什么保留原状」的唯一书面记录。
fn acknowledged_payload(item_id: &str, reason: &str) -> String {
    serde_json::json!({ "itemId": item_id, "reason": reason }).to_string()
}

/// 保留目录那条事件的载荷。
///
/// 单独成函数是因为它被用来做**幂等比较**：只有当载荷逐字节相同时才认定
/// 「同一条事实已经记过了」。把 `format!` 写在两处，早晚会有一处被改而另一处没有，
/// 幂等判断就会静默失效。
fn payload_for(run_id: &str, dir: &CreatedDirRow) -> String {
    serde_json::json!({
        "runId": run_id,
        "path": dir.relative_path.join("\\"),
        "identity": dir.directory_identity,
    })
    .to_string()
}

/// 对单个操作做磁盘核对。
fn classify(root: &ApprovedRoot, operation: &OperationRow) -> Result<Classification, AppError> {
    let expected: Fingerprint = serde_json::from_str(&operation.expected_json)
        .map_err(|error| AppError::internal(format!("操作缺少指纹信息: {error}")))?;

    let source_now = fingerprint_at(root, &operation.source, &expected.volume_id)?;
    let target_now = fingerprint_at(root, &operation.target, &expected.volume_id)?;

    let source_ok = source_now
        .as_ref()
        .is_some_and(|fp| same_thing(fp, &expected));
    let target_ok = target_now
        .as_ref()
        .is_some_and(|fp| same_thing(fp, &expected));

    // 规格 8.3 的五种情形。**默认落到 ambiguous** —— 判不出来时不猜。
    let (verdict, message) = match (&source_now, &target_now) {
        (Some(_), None) if source_ok => (
            Verdict::NotApplied,
            "源文件还在原处、内容未变，这一项确实没有执行。".to_owned(),
        ),
        (None, Some(_)) if target_ok => (
            Verdict::Applied,
            "文件已经在目标位置，且身份与内容都对得上，这一项确实执行过了。".to_owned(),
        ),
        (Some(_), Some(_)) => (
            Verdict::Ambiguous,
            "源位置和目标位置**都**有文件。为了不丢数据，两处都保留，请人工确认哪个是要留下的。"
                .to_owned(),
        ),
        (None, None) => (
            Verdict::Ambiguous,
            "源位置和目标位置都没有文件。请根据上面两个路径自行查找，应用不会扫描全盘猜测。"
                .to_owned(),
        ),
        // 剩下的情形：某一处有东西，但身份或内容对不上。
        // 规格要求把「识别出文件」与「满足执行条件」分开说。
        (Some(_), None) => (
            Verdict::Ambiguous,
            "源位置有一个同名的文件，但**不是**当时记录的那个（身份或内容已改变），\
             无法确认它是不是被移动过。"
                .to_owned(),
        ),
        (None, Some(_)) => (
            Verdict::Ambiguous,
            "目标位置有一个文件，但**不是**当时记录的那个（身份或内容已改变），\
             既不能当成执行成功，也不能覆盖它。"
                .to_owned(),
        ),
    };

    Ok(Classification { verdict, message })
}

impl Verdict {
    fn into_status(self) -> OpStatus {
        match self {
            // 核对出来的「已应用」与执行时写下的 `applied` 是同一个事实，
            // 用同一个状态名，避免出现第二套词汇。
            Verdict::Applied => OpStatus::Applied,
            // 「未应用」回到 `failed`：它确实没完成，用户可以重新发起。
            Verdict::NotApplied => OpStatus::Failed,
            Verdict::Ambiguous => OpStatus::Ambiguous,
        }
    }
}

struct Classification {
    verdict: Verdict,
    message: String,
}

/// 两个指纹是不是「同一个东西，且内容没变」。
///
/// 规格 8.1 强调这两个身份**不能互相替代**：
/// `file_id` 是卷内身份（能检出「同名另一个文件」），
/// `sha256` 是内容身份（能检出「size 与 mtime 恰好相同的篡改」）。两个都要比。
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
        // 记录里没有内容哈希时无法确认内容——**不当作匹配**。
        // 计划阶段写下的指纹必然带 sha256（规格 6.1），走到这里说明日志被改过。
        _ => false,
    }
}

/// 取某个相对路径当前的指纹；路径不存在返回 `None`。
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

    fn fp(file_id: &str, sha: Option<&str>) -> Fingerprint {
        Fingerprint {
            volume_id: "V".to_owned(),
            file_id: file_id.to_owned(),
            size: "3".to_owned(),
            modified_ns: "0".to_owned(),
            sha256: sha.map(str::to_owned),
        }
    }

    #[test]
    fn the_same_file_with_the_same_content_matches() {
        assert!(same_thing(&fp("A", Some("x")), &fp("A", Some("x"))));
    }

    #[test]
    fn a_different_file_with_the_same_name_does_not_match() {
        // Windows 上「同一个路径现在是另一个文件」是真实存在的：
        // 删掉再建一个同名文件，路径没变、内容可能碰巧一样，但身份变了。
        assert!(!same_thing(&fp("B", Some("x")), &fp("A", Some("x"))));
    }

    #[test]
    fn the_same_inode_with_changed_content_does_not_match() {
        // 规格 8.3：身份相同但内容已修改，仍然不能当成满足条件。
        assert!(!same_thing(&fp("A", Some("y")), &fp("A", Some("x"))));
    }

    #[test]
    fn a_missing_content_hash_is_never_treated_as_a_match() {
        // 记录里没有哈希时不能「当作匹配」——那会把无法确认变成默认通过。
        assert!(!same_thing(&fp("A", None), &fp("A", Some("x"))));
        assert!(!same_thing(&fp("A", Some("x")), &fp("A", None)));
    }

    #[test]
    fn verdict_maps_to_a_stable_status_name() {
        assert_eq!(Verdict::Applied.as_str(), "applied");
        assert_eq!(Verdict::NotApplied.as_str(), "notApplied");
        assert_eq!(Verdict::Ambiguous.as_str(), "ambiguous");
    }
}
