//! 结构化错误与错误码。
//!
//! 规格 0.8：对输入错误返回结构化错误；**不吞异常、不用空列表假装成功**。
//! 规格 8.5 定义了完整的错误码与对应处理约定。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::types::ErrorDetails;

/// 规格 8.5 的错误码表。
///
/// 集中定义的原因：错误码是前后端共享契约，散落的字符串字面量会让
/// 「前端认识、后端不认识」的组合悄悄出现。
pub mod codes {
    // ---- 根目录与存储 ----
    pub const ROOT_NOT_AUTHORIZED: &str = "ROOT_NOT_AUTHORIZED";
    pub const ROOT_CHANGED: &str = "ROOT_CHANGED";
    pub const UNSUPPORTED_STORAGE: &str = "UNSUPPORTED_STORAGE";
    pub const REPARSE_POINT: &str = "REPARSE_POINT";

    // ---- 路径与命名 ----
    pub const INVALID_PATH: &str = "INVALID_PATH";
    pub const RESERVED_NAME: &str = "RESERVED_NAME";
    pub const PATH_TOO_LONG: &str = "PATH_TOO_LONG";
    pub const TARGET_EXISTS: &str = "TARGET_EXISTS";
    pub const TARGET_PARENT_IS_FILE: &str = "TARGET_PARENT_IS_FILE";

    // ---- 源文件状态 ----
    pub const SOURCE_CHANGED: &str = "SOURCE_CHANGED";
    pub const SOURCE_MISSING: &str = "SOURCE_MISSING";
    pub const FILE_BUSY: &str = "FILE_BUSY";
    pub const PERMISSION_DENIED: &str = "PERMISSION_DENIED";

    /// 文件的时间戳无法换算成可表示的本地时间（T04 新增）。
    ///
    /// 规格 8.5 的错误表是**处理约定**而不是封闭集合：这里需要一个能区分
    /// 「文件有问题」与「我们代码有问题」的码。用 `INTERNAL` 会把用户的
    /// 坏时间戳报成程序缺陷，用 `INVALID_PATH` 又会误导用户去改文件名。
    pub const INVALID_TIMESTAMP: &str = "INVALID_TIMESTAMP";

    // ---- 计划与确认 ----
    pub const STALE_PLAN: &str = "STALE_PLAN";
    pub const TOKEN_EXPIRED: &str = "TOKEN_EXPIRED";
    pub const TOKEN_USED: &str = "TOKEN_USED";
    pub const REQUEST_CONFLICT: &str = "REQUEST_CONFLICT";
    pub const TASK_BUSY: &str = "TASK_BUSY";

    // ---- 提取 ----
    pub const EXTRACTION_TIMEOUT: &str = "EXTRACTION_TIMEOUT";
    pub const UNSUPPORTED_FORMAT: &str = "UNSUPPORTED_FORMAT";

    /// 文件超过该格式的提取上限（规格 6.2 的表格）。
    ///
    /// 与 `UNSUPPORTED_FORMAT` 分开：后者说的是「这类文件本来就不提取正文」
    /// （例如 .exe），用户能做的只是换一份文件；这里说的是
    /// 「这份文件太大了」，用户可以把内容拆小再来。两者的下一步动作不同。
    pub const EXTRACTION_TOO_LARGE: &str = "EXTRACTION_TOO_LARGE";

    /// 编码无法可靠解码（规格 6.2：「乱码明确失败」）。
    ///
    /// **刻意不猜编码**：猜一个「可能是对的」编码会把一段乱码
    /// 变成一份看起来正常、实际错误的正文，而下游会拿它去给文件分类——
    /// 那比直接告诉用户「读不出来」糟糕得多。
    pub const EXTRACTION_GARBLED: &str = "EXTRACTION_GARBLED";

    /// 文件结构损坏，解析器读不下去。
    pub const EXTRACTION_CORRUPT: &str = "EXTRACTION_CORRUPT";

    /// 文件被加密，无法读取正文。
    ///
    /// 单列一个码而不是并进 `CORRUPT`：加密是一份**完好**的文件，
    /// 只是我们没钥匙。用户看到「文件损坏」会去检查文件，
    /// 而正确动作是提供密码或换一份未加密的。
    pub const EXTRACTION_ENCRYPTED: &str = "EXTRACTION_ENCRYPTED";

    /// 解析进程被资源限制终止（内存超限或超时后强杀）。
    ///
    /// 与 `EXTRACTION_TIMEOUT` 分开：超时是「等够了，主动放弃」，
    /// 这个是「它已经被杀掉了」。用户看到的应当都是「这份文件没读完」，
    /// 但排查方向完全不同（前者调页数上限，后者查内存上限）。
    pub const EXTRACTION_KILLED: &str = "EXTRACTION_KILLED";

    /// 解析进程起不来或没给出可用的结果。
    pub const EXTRACTION_WORKER_FAILED: &str = "EXTRACTION_WORKER_FAILED";

    /// 这台电脑上系统 OCR 用不了（没装识别语言、没装中文，或不支持）。
    ///
    /// 单列一个码而不是并进 `UNSUPPORTED_FORMAT`：后者说的是「这类文件
    /// 本来就不提正文」，而这里说的是「**这台电脑**还没准备好」。
    /// 用户的下一步完全不同——前者只能换文件，后者去装个语言包就能用。
    pub const OCR_UNAVAILABLE: &str = "OCR_UNAVAILABLE";

    // ---- 模型 ----
    pub const MODEL_AUTH: &str = "MODEL_AUTH";
    pub const MODEL_TIMEOUT: &str = "MODEL_TIMEOUT";
    pub const MODEL_INVALID_OUTPUT: &str = "MODEL_INVALID_OUTPUT";
    pub const BUDGET_EXCEEDED: &str = "BUDGET_EXCEEDED";

    /// Windows 凭据存储读写不了（T12）。
    ///
    /// 规格 8.5 的错误表是**处理约定**而不是封闭集合（T04 新增
    /// `INVALID_TIMESTAMP` 时已按同一理由扩过一次）。这里必须单独一个码：
    ///
    /// * 报 `MODEL_AUTH` 会误导用户去改密钥——而密钥本身没问题，
    ///   是**存不进去**；
    /// * 报 `INTERNAL` 会把一次环境问题（凭据服务被禁用）说成程序缺陷；
    /// * 用户真正要做的动作是唯一的：去看 Windows「凭据管理器」是否可用。
    pub const CREDENTIAL_UNAVAILABLE: &str = "CREDENTIAL_UNAVAILABLE";

    /// 提供商端点不符合规格 6.4 的地址要求（T12）。
    ///
    /// 本地模式只允许 loopback、兼容云只接受 HTTPS。这两条都是**拒绝**，
    /// 不是「警告后照发」。用 `INVALID_PATH` 会让人以为这是文件路径问题，
    /// 而它要用户改的是设置页上的那一栏。
    pub const INVALID_ENDPOINT: &str = "INVALID_ENDPOINT";

    // ---- 持久化 ----
    pub const JOURNAL_WRITE_FAILED: &str = "JOURNAL_WRITE_FAILED";
    pub const DB_UNAVAILABLE: &str = "DB_UNAVAILABLE";

    // ---- 恢复 ----
    pub const RECOVERY_REQUIRED: &str = "RECOVERY_REQUIRED";
    pub const UNDO_CONFLICT: &str = "UNDO_CONFLICT";

    /// 内部不变量被破坏。它意味着代码缺陷，不是用户操作问题。
    pub const INTERNAL: &str = "INTERNAL";
}

/// 返回给前端的脱敏错误。
///
/// `details` **不得**包含文件正文、绝对路径或密钥（规格 3.3、5.1）。
///
/// `deny_unknown_fields` 同时做两件事：让 Rust 反序列化拒绝多余字段，
/// 以及让 schemars 生成 `additionalProperties: false`（规格 6.4 的硬要求）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    /// 指明「重试一次可能成功」。为 `true` 时前端才显示重试入口。
    pub retryable: bool,
    pub details: ErrorDetails,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            details: ErrorDetails::new(),
        }
    }

    /// 标记为可重试。只用于「重试一次可能成功」的瞬时故障，
    /// 例如 `FILE_BUSY`；不能用于 `UNSUPPORTED_STORAGE` 这类确定性失败。
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    /// 追加一条脱敏明细。调用方负责确保值本身已经脱敏。
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// 内部缺陷。界面应显示为「未知错误」，不能当作业务错误引导用户操作。
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL, message)
    }
}

/// 后端内部使用的 Result 别名。所有跨 IPC 的返回都必须转成
/// [`crate::domain::ipc::IpcResult`]，不能直接把 `Result` 抛给前端。
pub type AppResult<T> = Result<T, AppError>;
