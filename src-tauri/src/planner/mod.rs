//! 确定性规划器（规格 7.1、T04）。
//!
//! 规格 3.2 的分工：`rules` / `ai` 只负责「建议」，`planner` 负责把建议
//! 变成**确定性、可审核、可复算**的计划。这一层里不允许出现随机数、
//! 系统时间或目录枚举顺序影响结果——否则「相同输入得到相同目标方案」
//! 这条要求就无法成立。
//!
//! ```text
//! FileRecord + Proposal[]
//!      │
//!      ├─ naming::TargetLedger  目标名占用台账（磁盘现有项 + 本批已保留 + 本批源路径）
//!      ├─ build::build_plan     指纹绑定、冲突分配、选中集合、版本
//!      └─ validate::validate_plan  可执行性检查 + 摘要 digest
//! ```
//!
//! 规划器**不修改任何文件**（INV-01）。唯一会读内容的地方是「进入可选状态前
//! 绑定完整指纹」——那是只读打开。

pub mod build;
pub mod naming;
pub mod validate;

pub use build::{
    build_plan, build_rule_plan, PlanBuild, PlanRequest, CODE_CASE_ONLY_RENAME,
    CODE_DUPLICATE_PROPOSAL, CODE_NOOP, CODE_SKIPPED_AT_SCAN, MAX_CATEGORY_DEPTH,
};
pub use naming::{
    check_target_parent_chain, comparison_key, normalize_name, Observed, TargetLedger,
};
pub use validate::{compute_digest, validate_plan};
