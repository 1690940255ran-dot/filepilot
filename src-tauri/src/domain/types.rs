//! IPC 契约类型 —— 规格 5.1 的 Rust 实现，**唯一真源**。
//!
//! 通过 `cargo run --bin export-contracts` 生成：
//! - `src/api/contracts.generated.ts`（TypeScript 类型）
//! - `src/api/contracts.schema.json`（JSON Schema，供前端运行时校验）
//!
//! 约定（规格 5.1）：
//! - 序列化统一 camelCase
//! - ID 为 UUID 字符串
//! - 时间使用 UTC RFC3339 字符串
//! - 文件大小与高精度时间戳跨 IPC 使用**十进制字符串**，避免 JavaScript 数值精度损失
//! - 相对路径内部为**组件数组**，不以字符串拼接充当安全验证

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 应用内标识。UUID 字符串。
///
/// 注意：它**不是** Windows 卷内文件身份，**也不是**内容哈希。
/// 这三者用途不同，不能互相替代（规格 5.1）。
pub type Id = String;

/// 相对路径，以组件数组表示，例如 `["学习", "数学", "第三章.pdf"]`。
pub type RelPath = Vec<String>;

/// 整理模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum Mode {
    /// 纯规则模式：不联网、不做内容提取，断网与无模型时仍然完整可用。
    Rules,
    /// 本地模型模式：只允许 loopback 地址。
    AiLocal,
    /// 云端模型模式：必须先展示并授权真实待发送载荷。
    AiCloud,
}

/// 校验问题的严重度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum Risk {
    Info,
    Warning,
    /// 阻断项：只要存在，就不能生成可执行计划。
    Block,
}

/// 内容提取状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum ExtractionStatus {
    Pending,
    Ok,
    /// 部分提取（例如超出字符上限被截断）。
    Partial,
    /// 格式不受支持（例如扫描型 PDF 无文本层）。
    Unsupported,
    Failed,
}

/// 后台任务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    /// 部分完成：首个失败即停止，已完成项保持完成，不自动回滚。
    Partial,
    Failed,
    Cancelled,
    /// 存在无法自动判定的执行状态，必须先完成恢复核对。
    RecoveryRequired,
}

/// 一次执行（含撤销执行）的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Partial,
    Failed,
    Cancelled,
    RecoveryRequired,
}

impl RunStatus {
    /// 与落库、与 IPC 一致的文本名。
    ///
    /// 真源是 `storage::runs::status_to_text`；这里只是给日志和诊断用的窥视孔，
    /// 两边一旦不一致，测试会先发现。
    pub fn as_text(self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Partial => "partial",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
            RunStatus::RecoveryRequired => "recoveryRequired",
        }
    }
}

/// 单个文件操作的状态（规格 8.2）。
///
/// 迁移规则见 [`crate::domain::states`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum OpStatus {
    Pending,
    /// 意图记录已持久化，但尚未确认文件是否被改动。
    Prepared,
    Applied,
    Failed,
    /// 因取消而未派发。
    Skipped,
    /// 无法确定是否已改动 —— 不能猜，必须人工核对。
    Ambiguous,
}

/// 撤销状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum UndoStatus {
    NotRequested,
    Prepared,
    Undone,
    /// 原路径被占用或文件内容已变化，默认不选中。
    Conflict,
}

/// 未决事实的人工处置（规格 8.2）。
///
/// **与 `OpStatus` 是两个维度，不要合并**：
/// `status` 记录「磁盘上现在是什么样」，`resolution` 记录「用户对这件事做了什么决定」。
/// 把「用户确认保留现状」写成 `status = skipped` 之类的值，等于把
/// 「这一项其实没做成」这个事实抹掉——审计记录要能同时回答这两个问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum OpResolution {
    /// 尚未处置。
    Open,
    /// 用户已核对并明确接受「保留现状」，附有理由。
    Acknowledged,
}

/// 计划状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum PlanStatus {
    Draft,
    /// 已通过校验并取得一次性 validationToken。
    Validated,
    /// 已随执行被密封，成为执行输入的唯一来源。
    Sealed,
    Archived,
}

/// 单个计划项的动作。v0.1 只有这两种 —— 没有删除、没有覆盖。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum PlanAction {
    Move,
    Noop,
}

/// 计划项的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum PlanItemOrigin {
    Rule,
    Ai,
    User,
}

/// 文件指纹。
///
/// 字段用途不同，不能互相替代：`file_id` 是 Windows 卷内身份（能检出「同名另一个文件」），
/// `sha256` 是内容身份（能检出「size 与 mtime 恰好相同的篡改」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Fingerprint {
    pub volume_id: String,
    /// Windows 文件身份（卷内唯一）。不等同于路径。
    pub file_id: String,
    /// 十进制字符串，避免 JavaScript 数值精度损失。
    pub size: String,
    /// UTC 纳秒时间戳，十进制字符串。
    pub modified_ns: String,
    /// 生成**可执行计划**时必须为 `Some`。仅用于展示的初扫结果可以为 `None`。
    pub sha256: Option<String>,
}

/// 扫描到的文件记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: Id,
    pub scan_id: Id,
    pub root_id: Id,
    pub relative_path: RelPath,
    pub extension: String,
    pub fingerprint: Fingerprint,
    pub extraction_status: ExtractionStatus,
    /// 跳过原因代码；`None` 表示未被跳过。
    pub skip_code: Option<String>,
}

/// 提取证据：定位信息 + 片段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Evidence {
    /// 例如 `page:1`、`chars:0-120`。必须来自本次真实提取结果，不接受模型自造引用。
    pub locator: String,
    pub excerpt: String,
}

/// 单个文件的内容提取结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Extraction {
    pub file_id: Id,
    /// 提取时同一只读句柄对应的快照，用于证明读取期间文件稳定。
    pub source_fingerprint: Fingerprint,
    pub status: ExtractionStatus,
    /// **仅内存或会话临时缓存**，不写持久日志。
    pub text: String,
    pub evidence: Vec<Evidence>,
    pub truncated: bool,
    /// 失败或降级原因代码。
    pub code: Option<String>,
}

/// 模型返回的单条建议。
///
/// 规格 5.1 与 6.4：它**不含绝对路径**，也不能携带执行指令。
/// 模型只能引用输入中提供的 `file_id`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Proposal {
    pub file_id: Id,
    /// 根目录下至多两层目录，每项只是一个目录名称（不含分隔符）。
    pub category: Vec<String>,
    /// 不含扩展名、绝对路径或路径分隔符。扩展名由程序从真实源名附加。
    pub stem: String,
    pub reason: String,
    pub confidence: f64,
    pub evidence_locator: Option<String>,
}

/// 计划中的单项。它已经过确定性规划器处理，带完整源指纹。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: Id,
    pub file_id: Id,
    pub source: RelPath,
    pub target: RelPath,
    pub action: PlanAction,
    pub selected: bool,
    pub origin: PlanItemOrigin,
    pub reason: String,
    /// 生成计划时绑定的源文件指纹。执行前必须重新核对，不一致即停止。
    pub expected: Fingerprint,
}

/// 整理计划。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Plan {
    pub id: Id,
    pub root_id: Id,
    pub scan_id: Id,
    /// 乐观锁版本。任何编辑都会 +1，并使旧校验与旧确认立即失效。
    pub revision: u32,
    pub mode: Mode,
    pub status: PlanStatus,
    pub items: Vec<PlanItem>,
    /// UTC RFC3339 字符串。
    pub created_at: String,
}

/// 校验或执行过程中的一个问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub severity: Risk,
    /// 与具体计划项相关时给出；全局问题为 `None`。
    pub item_id: Option<Id>,
    pub message: String,
}

/// 校验报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ValidationReport {
    pub plan_id: Id,
    /// 该报告对应的计划版本。与当前计划版本不一致即视为过期。
    pub revision: u32,
    /// 由后端对固定字段的规范序列化结果计算 SHA-256。
    pub digest: String,
    pub executable_count: u32,
    pub issues: Vec<Issue>,
    /// 一次性令牌。**只有**后端能生成，前端不得自行生成或伪造。
    pub validation_token: Option<String>,
    /// UTC RFC3339，最多 5 分钟有效。
    pub expires_at: Option<String>,
}

/// 执行结果计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RunCounts {
    pub applied: u32,
    pub failed: u32,
    pub skipped: u32,
    pub pending: u32,
    /// 判定不出来、需要人工核对的项数（规格 8.3）。
    ///
    /// **单独计数而不是并进 `pending`**：界面上「还没轮到他」和
    /// 「做了但说不清」需要不同的措辞和不同的动作，混在一起用户无从下手。
    pub ambiguous: u32,
}

/// 执行报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RunReport {
    pub run_id: Id,
    pub plan_id: Id,
    /// `apply`（整理）或 `undo`（撤销）。
    ///
    /// 历史列表**必须**能分清这两种记录：它们都会出现在同一条时间线上，
    /// 而「已完成 3 项」在整理里是「搬走了 3 个文件」、在撤销里是
    /// 「搬回了 3 个文件」——意思正好相反。少了这个字段，界面只能
    /// 给撤销记录也挂一个「撤销」按钮，而那是一次注定被拒绝的点击。
    pub direction: String,
    /// 当前操作事实与冲突集合的摘要，用于防止用户确认一份过期的报告。
    pub state_digest: String,
    pub status: RunStatus,
    pub counts: RunCounts,
    pub issues: Vec<Issue>,
}

/// 一个未决项的核对结果（规格 8.3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RecoveryItem {
    pub operation_id: Id,
    pub item_id: Id,
    pub source: RelPath,
    pub target: RelPath,
    pub status: OpStatus,
    pub resolution: OpResolution,
    /// 为什么是这个判定。这句话会直接显示在界面上，写的是「用户能据以行动」的内容。
    pub message: String,
}

/// 恢复核对的结果（规格 8.3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub run_id: Id,
    pub plan_id: Id,
    pub status: RunStatus,
    /// **确认必须带上它**：两次读取之间只要有任意一项的状态或处置变了，
    /// 摘要就会变，后端据此拒绝一份过期的确认。
    pub state_digest: String,
    pub items: Vec<RecoveryItem>,
    pub counts: RunCounts,
    /// 仍有未决项没被处置时为 true —— 此时后端**禁止**新的执行任务。
    pub blocks_new_runs: bool,
    /// 该 run 的根目录在当前会话里是否仍被授权。
    ///
    /// 未授权的根换不回路径，核对根本无从下手。这时要**如实说明**，
    /// 而不是回报一份「没有未决项」的空报告——那会让界面显示一切正常。
    pub root_authorized: bool,
}

// ============================================================================
// T09：撤销与冲突（规格 8.4）
// ============================================================================

/// 撤销预览中单项的判定。
///
/// 与 `OpStatus` 是**不同维度**：`OpStatus` 说的是「原操作当初做成了没有」，
/// 这里说的是「**现在**能不能把它移回去」。同一项完全可以是
/// `status = applied` 而 `outcome = conflict`——那正是「整理后被改动」的情形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum UndoOutcome {
    /// 条件都满足，可以安全移回。**默认选中**。
    Ready,
    /// 原路径被占用、内容被改动，或原父目录消失。**默认不选中**。
    Conflict,
    /// 之前已经撤销过。重复撤销只回报「已完成」，绝不再搬动文件。
    AlreadyUndone,
}

/// 撤销预览中的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct UndoItem {
    /// 被撤销的那个原操作。
    pub operation_id: Id,
    pub item_id: Id,
    /// 撤销时的**源** = 原操作的目标位置。
    pub source: RelPath,
    /// 撤销时的**目标** = 原操作的源位置。
    pub target: RelPath,
    pub outcome: UndoOutcome,
    /// 是否被选中执行。`Conflict` 与 `AlreadyUndone` 初始为 false。
    pub selected: bool,
    /// 判定原因，直接显示在界面上。
    pub message: String,
}

/// 撤销预览（规格 8.4）。
///
/// 与执行前的计划预览同构：**也有摘要和一次性确认**，
/// 因为撤销同样是一次会改动用户文件的操作，不是异常后的无条件补偿。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct UndoPreview {
    /// 本次预览落库的撤销计划 id。执行时原样带回，用来消费一次性令牌。
    pub undo_plan_id: Id,
    pub original_run_id: Id,
    /// 当前撤销事实的摘要。执行时必须原样带回，变了就说明这一屏已经过期。
    pub digest: String,
    pub items: Vec<UndoItem>,
    /// 可安全撤销的项数。
    pub ready_count: u32,
    /// 有冲突的项数。
    pub conflict_count: u32,
    /// 已经撤销过的项数（重复撤销时它们不会再次被搬动）。
    pub already_undone_count: u32,
    /// 一次性令牌。**只有**后端能签发；前端不得自行生成。
    pub undo_token: Option<String>,
    /// UTC RFC3339，最多 5 分钟有效。
    pub expires_at: Option<String>,
}

/// 撤销执行的分项结果（规格 T09）。
///
/// 「已撤销 / 有冲突 / 未处理」分开计数是硬要求：把部分撤销标成全部成功，
/// 会让用户以为文件都回去了，而实际上还有几项留在原地。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct UndoReport {
    /// 本次撤销产生的 run（`direction = 'undo'`，可用 `get_run` 查询）。
    pub run_id: Id,
    pub original_run_id: Id,
    pub status: RunStatus,
    /// 已成功移回的项数。
    pub reverted: u32,
    /// 因冲突而保留现状的项数。
    pub conflicted: u32,
    /// 本次之前就已经撤销过、这次只是再次确认的项数。
    pub already_undone: u32,
    /// 用户没选中、因而未处理的项数。
    pub untouched: u32,
    pub items: Vec<UndoItem>,
    /// 目录清理失败等**不影响已移回文件事实**的告警。
    ///
    /// 单独一个列表而不是塞进 `items`：文件已经安全回去了，
    /// 把「空目录没删掉」混进分项结果会让用户以为撤销失败了。
    pub warnings: Vec<Issue>,
}

/// 非敏感应用设置。
///
/// 规格 3.3 与 8.1：**禁止**在这里放密钥。API Key 只进 Windows 凭据存储，
/// 设置中只保存 `selected_provider_id` 这类引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct AppSettings {
    pub mode: Mode,
    /// 规格 6.1 默认值：最多 10,000 个普通文件。
    pub scan_max_files: u32,
    /// 规格 6.1 默认值：递归深度 20。
    pub scan_max_depth: u32,
    pub selected_provider_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // 默认纯规则模式：未配置模型、断网或模型失败时产品都必须可用。
            mode: Mode::Rules,
            scan_max_files: 10_000,
            scan_max_depth: 20,
            selected_provider_id: None,
        }
    }
}

/// 一次计划编辑。
///
/// 规格 5.2：`update_plan` 接受一组这样的编辑，配合 `expectedRevision` 做乐观锁。
///
/// 只暴露**可编辑的两件事**（勾选与目标路径），其余字段（rootId / scanId / mode /
/// createdAt / fileId / expected 指纹）一律不接受前端提交——
/// 让前端能改指纹等于让它可以绕过执行前的核对。
///
/// `Option<T>` 在这里表示「必填可空」：字段必须在，值为 `null` 表示不修改。
/// **不要给这两个字段加 `#[schemars(required)]`** —— 那会把 `Option<T>` 当成 `T`，
/// 把 `null` 从契约里吃掉，前端传 `null` 就会被自己的运行时校验拒绝。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct PlanItemEdit {
    pub item_id: Id,
    /// 不修改勾选状态时传 `null`。
    pub selected: Option<bool>,
    /// 新的目标相对路径（组件数组）。不修改时传 `null`。
    pub target: Option<RelPath>,
}

/// 供 `details` 字段使用的脱敏键值集合。
///
/// 规格 5.1：`details` 必须脱敏，不包含文件正文与密钥。
pub type ErrorDetails = HashMap<String, String>;

// ============================================================================
// T02：根授权、扫描任务与分页
// ============================================================================

/// 已授权根目录的摘要。
///
/// 规格 3.3：前端只拿到 `rootId`，**后续提交 rootId 而不是路径**。
/// `displayPath` 仅用于展示，前端不能拿它去拼路径再传回来，
/// 后端也不会接受前端传来的任何路径字符串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RootSummary {
    pub root_id: Id,
    /// 面向用户的原始路径（可能是长路径或含中文）。
    pub display_path: String,
    /// 卷标识。前端可利用它提示「根目录换了卷」。
    pub volume_id: String,
}

/// 一次扫描任务的结果摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ScanSummary {
    /// 规格 5.2：`start_scan` 返回 taskId。
    pub task_id: Id,
    pub scan_id: Id,
    pub root_id: Id,
    /// 枚举到的条目总数（含被跳过的）。
    pub total: u32,
    /// 可参与整理的普通文件数。
    pub usable: u32,
    /// 被跳过的条目数。
    pub skipped: u32,
    /// 规格 6.1：达到数量/深度上限时为 true，界面必须要求缩小范围。
    pub truncated: bool,
}

/// 文件列表的一页。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct FilePage {
    pub items: Vec<FileRecord>,
    /// 下一页游标；为 `None` 表示已经是最后一页。
    ///
    /// 游标是**上一页最后一条记录的 id**，不是偏移量：
    /// 偏移量在底层列表变化时会漏项或重复，id 不会。
    pub next_cursor: Option<String>,
    /// 该 scan 下的条目总数，便于界面显示「x / total」。
    pub total: u32,
}

/// 任务状态查询结果。
///
/// 规格 5.2：页面切换不得终止任务，重新打开界面要能用 `taskId` 查回来。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskSummary {
    pub task_id: Id,
    pub status: TaskStatus,
    pub processed: u32,
    /// 总量尚未确定时为 `null`。
    pub total: Option<u32>,
    /// 扫描类任务完成后的 scanId；其他任务为 `null`。
    pub scan_id: Option<String>,
    /// 失败时的结构化错误；成功或进行中为 `null`。
    ///
    /// 注意：`AppError` 在 `domain::errors` 里，这里用 `crate::domain::errors::AppError`
    /// 会导致 types.rs 反向依赖 errors.rs。为避免循环，改用脱敏后的最小结构。
    pub error: Option<TaskError>,
}

/// 任务失败原因的最小结构。
///
/// 与 `AppError` 字段一致，但定义在 `types` 里以避免 `types` ↔ `errors` 互相依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl From<&crate::domain::errors::AppError> for TaskError {
    fn from(e: &crate::domain::errors::AppError) -> Self {
        Self {
            code: e.code.clone(),
            message: e.message.clone(),
            retryable: e.retryable,
        }
    }
}
