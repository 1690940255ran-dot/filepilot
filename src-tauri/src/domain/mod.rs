//! domain 层：纯类型、状态与规则。
//!
//! 规格 3.2：domain 不依赖 Tauri，不访问磁盘或网络。
//! 这一层可以被独立测试，也是 IPC 契约的唯一真源。

pub mod errors;
pub mod ipc;
pub mod states;
pub mod time;
pub mod types;

pub use errors::{codes, AppError, AppResult};
pub use ipc::IpcResult;
