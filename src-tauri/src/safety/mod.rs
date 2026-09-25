//! safety 层：路径、身份、快照、权限与计划一致性校验。
//!
//! 规格 3.2：这一层是执行器之前最后一道闸门。执行器只接受这里产出的
//! 授权对象，不接受来自 IPC 的裸路径。

pub mod confirmation;
pub mod fingerprint;
pub mod path;
pub mod root;
