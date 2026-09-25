//! 平台层：把 Windows 原生文件系统语义收敛在一处。
//!
//! 规格 3.2 要求 `platform/windows` 提供文件身份、禁止覆盖移动、句柄锁定、
//! 重解析点检测等能力。这一层之外不允许直接调用 Win32。

pub mod cache;
pub mod child_process;
pub mod credentials;
pub mod failpoint;
pub mod ocr;
pub mod windows;
