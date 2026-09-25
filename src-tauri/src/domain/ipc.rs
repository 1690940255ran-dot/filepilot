//! IPC 返回信封。
//!
//! 规格 5.2：**全命令返回 `Result<T>`**，其 TypeScript 形状为
//! `{ ok: true; data: T } | { ok: false; error: AppError }`。
//!
//! 这里没有直接用 `serde` 的 internally-tagged enum，因为 serde 的 tag
//! 会把变体名写进 `ok` 字段（得到 `{"ok":"Ok",...}`），
//! 而契约要求的是布尔量 `true` / `false`。手写 `Serialize` 是唯一能精确
//! 匹配契约的做法，也比引入一层自定义 serde 适配器更容易审计。

use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::errors::AppError;

/// 命令返回值。TypeScript 侧对应 `Result<T>`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcResult<T> {
    Ok(T),
    Err(AppError),
}

impl<T> IpcResult<T> {
    pub fn ok(data: T) -> Self {
        IpcResult::Ok(data)
    }

    pub fn err(error: AppError) -> Self {
        IpcResult::Err(error)
    }

    /// 是否成功。测试与调用方用它做断言，避免直接匹配变体。
    pub fn is_ok(&self) -> bool {
        matches!(self, IpcResult::Ok(_))
    }

    pub fn data(self) -> Option<T> {
        match self {
            IpcResult::Ok(value) => Some(value),
            IpcResult::Err(_) => None,
        }
    }

    pub fn error(self) -> Option<AppError> {
        match self {
            IpcResult::Ok(_) => None,
            IpcResult::Err(err) => Some(err),
        }
    }
}

impl<T> From<Result<T, AppError>> for IpcResult<T> {
    fn from(value: Result<T, AppError>) -> Self {
        match value {
            Ok(data) => IpcResult::Ok(data),
            Err(error) => IpcResult::Err(error),
        }
    }
}

impl<T: Serialize> Serialize for IpcResult<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            IpcResult::Ok(data) => {
                let mut state = serializer.serialize_struct("IpcResult", 2)?;
                state.serialize_field("ok", &true)?;
                state.serialize_field("data", data)?;
                state.end()
            }
            IpcResult::Err(error) => {
                let mut state = serializer.serialize_struct("IpcResult", 2)?;
                state.serialize_field("ok", &false)?;
                state.serialize_field("error", error)?;
                state.end()
            }
        }
    }
}

impl<T: fmt::Display> fmt::Display for IpcResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcResult::Ok(_) => write!(f, "ok"),
            IpcResult::Err(error) => write!(f, "error[{}]: {}", error.code, error.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::errors::codes;

    #[test]
    fn success_envelope_uses_boolean_ok_field() {
        let value = IpcResult::ok(42_u32);
        let json = serde_json::to_string(&value).expect("序列化成功载荷不应失败");
        assert_eq!(json, r#"{"ok":true,"data":42}"#);
    }

    #[test]
    fn error_envelope_uses_boolean_ok_field() {
        let value: IpcResult<u32> =
            IpcResult::err(AppError::new(codes::FILE_BUSY, "文件被占用").retryable());
        let json = serde_json::to_string(&value).expect("序列化错误载荷不应失败");
        assert_eq!(
            json,
            r#"{"ok":false,"error":{"code":"FILE_BUSY","message":"文件被占用","retryable":true,"details":{}}}"#
        );
    }

    #[test]
    fn error_never_serializes_a_data_field() {
        let value: IpcResult<u32> = IpcResult::err(AppError::internal("boom"));
        let json = serde_json::to_string(&value).expect("序列化错误载荷不应失败");
        assert!(
            !json.contains("\"data\""),
            "错误信封不得携带 data 字段，否则前端可能把失败当成功"
        );
    }
}
