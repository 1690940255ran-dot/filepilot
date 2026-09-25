//! 状态迁移规则。
//!
//! 规格 3.2：domain 层是纯逻辑，不依赖 Tauri、不访问磁盘或网络，因此可以被独立测试。
//!
//! 把迁移集中在这里的原因：规格 8.2 明确「不能把『API 报错』一律等同于『文件未动』」。
//! 如果允许各调用点自行写 `status = failed`，这条约束会在某处被悄悄破坏。

use std::fmt;

use super::types::{OpStatus, PlanStatus};

/// 推动单文件操作状态迁移的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpEvent {
    /// 意图记录（prepared）已成功提交到 journal。
    JournalCommitted,
    /// 移动成功，且结果记录（applied）也已提交。
    Applied,
    /// 明确失败，**且可以确定文件未被改动**。
    FailedWithoutChange,
    /// 操作可能已产生副作用，但无法确认。必须进 ambiguous，禁止猜测。
    Undetermined,
    /// 因取消或前序失败而未派发到这一项。
    Skipped,
}

impl fmt::Display for OpEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            OpEvent::JournalCommitted => "JournalCommitted",
            OpEvent::Applied => "Applied",
            OpEvent::FailedWithoutChange => "FailedWithoutChange",
            OpEvent::Undetermined => "Undetermined",
            OpEvent::Skipped => "Skipped",
        };
        f.write_str(name)
    }
}

/// 非法的状态迁移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError {
    pub from: OpStatus,
    pub event: OpEvent,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "非法的操作状态迁移：{:?} --{}--> (拒绝)",
            self.from, self.event
        )
    }
}

impl std::error::Error for TransitionError {}

/// 单文件操作是否已经进入终态。终态不可再迁移。
pub fn is_terminal_op_status(status: OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Applied | OpStatus::Failed | OpStatus::Skipped | OpStatus::Ambiguous
    )
}

/// 计算单文件操作的下一个状态（规格 8.2）。
///
/// 唯一合法的路径：
/// - `pending -> prepared -> applied`
/// - `pending | prepared -> failed`（且只在能确定文件未动时）
/// - `prepared -> ambiguous`（无法确定是否已改动）
/// - `pending -> skipped`（未派发）
///
/// 注意 `pending --Applied--> *` 被拒绝：跳过 `prepared` 意味着没有持久化意图记录，
/// 而规格 INV-07 要求每次变更前必须有持久化意图记录。
pub fn next_op_status(current: OpStatus, event: OpEvent) -> Result<OpStatus, TransitionError> {
    use OpEvent as E;
    use OpStatus as S;

    let reject = || {
        Err(TransitionError {
            from: current,
            event,
        })
    };

    match (current, event) {
        (S::Pending, E::JournalCommitted) => Ok(S::Prepared),
        (S::Pending, E::FailedWithoutChange) => Ok(S::Failed),
        (S::Pending, E::Skipped) => Ok(S::Skipped),

        (S::Prepared, E::Applied) => Ok(S::Applied),
        (S::Prepared, E::FailedWithoutChange) => Ok(S::Failed),
        (S::Prepared, E::Undetermined) => Ok(S::Ambiguous),

        _ => reject(),
    }
}

/// 推动计划状态迁移的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanEvent {
    /// validate_plan 成功，取得一次性 validationToken。
    Validated,
    /// 用户编辑了计划。规格 2.3：revision +1，旧验证与旧确认立即失效。
    Edited,
    /// execute_plan 取得执行锁后密封计划。
    Sealed,
    Archived,
}

impl fmt::Display for PlanEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PlanEvent::Validated => "Validated",
            PlanEvent::Edited => "Edited",
            PlanEvent::Sealed => "Sealed",
            PlanEvent::Archived => "Archived",
        };
        f.write_str(name)
    }
}

/// 非法的计划状态迁移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanTransitionError {
    pub from: PlanStatus,
    pub event: PlanEvent,
}

impl fmt::Display for PlanTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "非法的计划状态迁移：{:?} --{}--> (拒绝)",
            self.from, self.event
        )
    }
}

impl std::error::Error for PlanTransitionError {}

/// 计划是否已不可变。密封后任何编辑都必须是「复制出新计划」而不是原地修改。
pub fn is_plan_immutable(status: PlanStatus) -> bool {
    matches!(status, PlanStatus::Sealed | PlanStatus::Archived)
}

/// 计算计划的下一个状态。
///
/// 关键约束：`Validated --Edited--> Draft` 会让旧的 validationToken 失效。
/// 回到 Draft 而不是停留在 Validated，是为了让「有 token 就能执行」这个
/// 危险假设不可能成立 —— 状态本身已经说明确认已作废。
pub fn next_plan_status(
    current: PlanStatus,
    event: PlanEvent,
) -> Result<PlanStatus, PlanTransitionError> {
    use PlanEvent as E;
    use PlanStatus as S;

    let reject = || {
        Err(PlanTransitionError {
            from: current,
            event,
        })
    };

    match (current, event) {
        (S::Draft, E::Validated) => Ok(S::Validated),
        (S::Draft, E::Edited) => Ok(S::Draft),
        (S::Validated, E::Edited) => Ok(S::Draft),
        (S::Validated, E::Sealed) => Ok(S::Sealed),
        (S::Sealed, E::Archived) => Ok(S::Archived),

        _ => reject(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use OpEvent as E;
    use OpStatus as S;

    #[test]
    fn happy_path_reaches_applied() {
        let prepared =
            next_op_status(S::Pending, E::JournalCommitted).expect("pending 应先变 prepared");
        assert_eq!(prepared, S::Prepared);
        let applied = next_op_status(prepared, E::Applied).expect("prepared 应能变 applied");
        assert_eq!(applied, S::Applied);
        assert!(is_terminal_op_status(applied));
    }

    #[test]
    fn applied_requires_persisted_intent_first() {
        // 规格 INV-07：变更前必须有持久化意图记录，因此不能直接 pending -> applied
        assert!(next_op_status(S::Pending, E::Applied).is_err());
    }

    #[test]
    fn prepared_can_become_ambiguous_but_pending_cannot() {
        assert_eq!(
            next_op_status(S::Prepared, E::Undetermined).expect("prepared 可进 ambiguous"),
            S::Ambiguous
        );
        // 还没动过就没有「不确定」可言，出现这种情况说明调用方搞错了阶段
        assert!(next_op_status(S::Pending, E::Undetermined).is_err());
    }

    #[test]
    fn failure_is_only_allowed_before_apply() {
        assert_eq!(
            next_op_status(S::Pending, E::FailedWithoutChange).expect("pending 可失败"),
            S::Failed
        );
        assert_eq!(
            next_op_status(S::Prepared, E::FailedWithoutChange).expect("prepared 可失败"),
            S::Failed
        );
        // applied 之后不能再变 failed：文件已经在目标位置了
        assert!(next_op_status(S::Applied, E::FailedWithoutChange).is_err());
    }

    #[test]
    fn terminal_states_accept_nothing() {
        for terminal in [S::Applied, S::Failed, S::Skipped, S::Ambiguous] {
            for event in [
                E::JournalCommitted,
                E::Applied,
                E::FailedWithoutChange,
                E::Undetermined,
                E::Skipped,
            ] {
                assert!(
                    next_op_status(terminal, event).is_err(),
                    "终态 {terminal:?} 不应接受事件 {event}"
                );
            }
        }
    }

    #[test]
    fn only_pending_can_be_skipped() {
        assert_eq!(
            next_op_status(S::Pending, E::Skipped).expect("pending 可被跳过"),
            S::Skipped
        );
        // prepared 已经写了意图记录，不能再当成「未派发」
        assert!(next_op_status(S::Prepared, E::Skipped).is_err());
    }

    #[test]
    fn editing_a_validated_plan_invalidates_it() {
        let validated =
            next_plan_status(PlanStatus::Draft, PlanEvent::Validated).expect("draft 可被校验");
        assert_eq!(validated, PlanStatus::Validated);

        let after_edit =
            next_plan_status(validated, PlanEvent::Edited).expect("已校验计划可被编辑");
        assert_eq!(
            after_edit,
            PlanStatus::Draft,
            "编辑后必须回到 draft，使旧确认在状态层面就失效"
        );
    }

    #[test]
    fn sealed_plan_is_immutable() {
        let sealed =
            next_plan_status(PlanStatus::Validated, PlanEvent::Sealed).expect("validated 可被密封");
        assert!(is_plan_immutable(sealed));
        assert!(next_plan_status(sealed, PlanEvent::Edited).is_err());
        assert_eq!(
            next_plan_status(sealed, PlanEvent::Archived).expect("密封计划可归档"),
            PlanStatus::Archived
        );
    }

    #[test]
    fn draft_cannot_be_sealed_directly() {
        // 必须先 validate，拿到一次性 token 才能密封执行
        assert!(next_plan_status(PlanStatus::Draft, PlanEvent::Sealed).is_err());
    }
}
