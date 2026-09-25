//! 故障注入点。规格 T08：用来测试**进程在任意一步被强杀**之后能否正确恢复。
//!
//! ## 为什么不是「抛个错让上层处理」
//!
//! 崩溃恢复要防的是**进程直接消失**：不走 unwinding、不跑 `Drop`、
//! SQLite 连接来不及正常关闭。用 `Err` 模拟是测不出真实行为的——
//! 那样 `Drop` 会跑、事务会回滚，而真实崩溃时这些都不会发生。
//!
//! 所以这里用 [`std::process::exit`]：它同样不跑栈上对象的 `Drop`，
//! 与你用任务管理器结束进程最接近。
//!
//! ## 开关
//!
//! 需要**同时**满足两个条件才生效：
//!
//! 1. 启用 `failpoints` feature（`cargo test --features failpoints`）；
//! 2. 是调试构建（`debug_assertions`）。
//!
//! 第二个条件是硬要求（规格 T08「release 构建不接受外部 failpoint 设置」）：
//! 发布版即使误开了 feature，整段代码也会被编译掉，
//! 不可能被环境变量从外部打开。两个条件都满足时函数体才会存在。

/// 进程因故障注入而退出的退出码。
///
/// 用一个可识别的非零值：测试据此确认「子进程是被注入点杀掉的」，
/// 而不是因为别的原因（比如断言失败）挂掉。
pub const FAILPOINT_EXIT_CODE: i32 = 86;

/// 环境变量名。值是上面某个 [`FAILPOINT_*`] 常量。
pub const FAILPOINT_ENV: &str = "FILEPILOT_FAILPOINT";

/// 意图已落盘，**重命名尚未执行**。
pub const FAILPOINT_BEFORE_RENAME: &str = "before-rename";
/// 重命名已成功，**`applied` 尚未落盘**。
///
/// 这是最危险的一个点：磁盘已经变了，日志里却只有 `prepared`。
pub const FAILPOINT_AFTER_RENAME: &str = "after-rename";
/// `applied` 已落盘。
pub const FAILPOINT_AFTER_APPLIED: &str = "after-applied";
/// 目录记录已落盘。
pub const FAILPOINT_AFTER_DIR_CREATED: &str = "after-dir-created";

/// 撤销：`undo prepared` 已落盘、反向重命名尚未执行（规格 8.4 第 5 条）。
pub const FAILPOINT_UNDO_BEFORE_RENAME: &str = "undo-before-rename";
/// 撤销：反向重命名已成功，**但 `undone` 尚未落盘**（规格 8.4 第 6 条）。
///
/// 与 [`FAILPOINT_AFTER_RENAME`] 同样是「磁盘变了、日志没记」，区别在于
/// 恢复时判定的方向相反：这一项要看**文件有没有回到原处**。
/// 不单独设一个点，就没法证明撤销的中断恢复走的是对的映射关系。
pub const FAILPOINT_UNDO_AFTER_RENAME: &str = "undo-after-rename";

/// 名字类型别名，只为让调用点读起来更像文档。
pub type FailPointName = &'static str;

#[cfg(all(feature = "failpoints", debug_assertions))]
mod enabled {
    use super::{FAILPOINT_ENV, FAILPOINT_EXIT_CODE};

    /// 当前进程被要求停在哪个点上。每个进程只读一次环境变量。
    ///
    /// 用 `OnceLock` 而不是每次读环境变量：测试里会有多个子进程，
    /// 而同一个进程内的行为必须稳定——中途环境变量被改（例如并发测试）
    /// 不该让「已经过了这个点」的代码突然停住。
    fn target() -> Option<&'static str> {
        use std::sync::OnceLock;
        static TARGET: OnceLock<Option<String>> = OnceLock::new();
        TARGET
            .get_or_init(|| std::env::var(FAILPOINT_ENV).ok())
            .as_deref()
    }

    pub fn fail_point(name: &str) {
        if target() == Some(name) {
            // 不是 panic：要模拟的是进程被强杀。`exit` 不跑栈上对象的 `Drop`，
            // SQLite 连接不会 checkpoint，WAL 留在磁盘上 —— 正是要测的状态。
            std::process::exit(FAILPOINT_EXIT_CODE);
        }
    }
}

#[cfg(not(all(feature = "failpoints", debug_assertions)))]
mod enabled {
    /// 发布构建（或未启用 feature）下是**完全的空操作**。
    ///
    /// `#[inline(always)]` 让它被优化掉，不留下任何可被触发的入口。
    #[inline(always)]
    pub fn fail_point(_name: &str) {}
}

pub use enabled::fail_point;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_name_does_not_stop_anything() {
        // 名字不匹配时必须是纯粹的 no-op。这条测试本身不会退出进程，
        // 因为测试进程没有设置 FILEPILOT_FAILPOINT。
        fail_point("这个点不存在");
        fail_point(FAILPOINT_BEFORE_RENAME);
    }

    #[test]
    fn exit_code_is_nonzero_and_distinct_from_common_failures() {
        // 1 是「一般错误」，101 是 Rust panic 的默认退出码，都不该被复用：
        // 测试要能区分「被注入点杀掉」与「自己挂了」。
        assert_ne!(FAILPOINT_EXIT_CODE, 0);
        assert_ne!(FAILPOINT_EXIT_CODE, 1);
        assert_ne!(FAILPOINT_EXIT_CODE, 101);
    }
}
