//! AI 建议（规格 6.4）。
//!
//! 模块边界：这一层**只产生建议**——`Vec<Proposal>`。
//! 它不能移动文件、不能决定目标路径、不能绕过规划器与校验器。
//! 规格 6.4 的原文是「模型建议 → 严格校验 → 确定性规划」，
//! 所以这里的输出在下一站会被当成**不可信输入**重新校验一遍。
//!
//! 规格 3.3：密钥只进 Windows 凭据存储，配置只存引用。
//! 对应的实现是 [`crate::platform::credentials`]。
//!
//! 当前进度（T12）：传输层 [`http`]、提供商接口 [`provider`]、
//! 两个适配器 [`compatible`] / [`ollama`] 与预算 [`budget`] 已就位。

pub mod budget;
pub mod compatible;
pub mod disclosure;
pub mod http;
pub mod ollama;
pub mod prompt;
pub mod provider;
