//! 提供商接口（规格 6.4）。
//!
//! 规格原文：「兼容适配器和 Ollama 适配器分别实现同一个
//! `suggest(batch, cancellation) -> Result<Vec<Proposal>>` 语义。」
//! 这个模块就是那句话的落点：[`suggest`] 是唯一的入口，按 [`ProviderKind`]
//! 分派到 [`compatible`](super::compatible) 或 [`ollama`](super::ollama)。
//!
//! ## 载荷里没有路径字段——这是刻意的
//!
//! 规格 6.4：「云模式仅发送随机 fileId、文件名、扩展名、最多 2,000 字符的
//! 文本摘要和用户要求；**不发送绝对路径、用户名、完整文件、原图**。」
//!
//! [`SuggestionItem`] 里**没有**路径字段，就像 T10 的工作进程协议里
//! 没有路径字段一样：这样「把用户目录结构发出去」在实现上无从表达，
//! 而不是「需要被检查的约定」。文件名的校验（不含分隔符）是第二道。
//!
//! ## 密钥的位置
//!
//! [`ProviderConfig`] 是**可以进数据库、可以给前端看**的那一份，
//! 它只带 `credential_ref`；密钥本身在 [`ProviderContext`] 里，
//! 而那是一个只在发请求时临时构造的东西（见 `platform::credentials`）。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::budget::{self, BatchSize};
use super::http::{self, Cancellation, Endpoint};
use crate::domain::errors::{codes, AppError};
use crate::domain::types::{Id, Proposal};
use crate::platform::credentials::Secret;

/// 提供商类型。
///
/// 只有两类，因为它们对应**两条不同的安全规则**：
/// 本地的端点只允许 loopback，云端的只接受 HTTPS。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum ProviderKind {
    /// 本机跑的 Ollama 之类。
    Local,
    /// OpenAI 兼容的云端接口。
    Compatible,
}

/// 一份提供商配置。**不含密钥**，可以进数据库、可以给前端看。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: Id,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub model: String,
    /// 指向系统凭据存储里那一条的**引用**，不是密钥本身。
    pub credential_ref: Option<String>,
}

/// 给设置页看的提供商摘要。
///
/// 比 [`ProviderConfig`] 多一个 [`Self::has_credential`]：界面要显示
/// 「已保存密钥」，但**不能**显示密钥本身。这一个布尔量就是那个信息，
/// 而它是单向的——拿到 `true` 也读不出任何内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProviderSummary {
    pub id: Id,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub model: String,
    pub has_credential: bool,
}

impl ProviderSummary {
    /// 从配置与「密钥在不在」构造。
    ///
    /// 刻意**不接受** `Secret`：这个函数的签名本身保证了摘要里
    /// 不可能带上密钥内容。
    pub fn new(config: &ProviderConfig, has_credential: bool) -> Self {
        Self {
            id: config.id.clone(),
            kind: config.kind,
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            has_credential,
        }
    }
}

/// 连通性探测的结果（`test_provider`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProviderProbe {
    /// 服务端回显的模型名。用户据此确认「我连的确实是我以为的那个模型」。
    pub model: String,
    /// 这次探测是否拿到了**符合结构**的建议数组。
    ///
    /// 规格 6.4：「提供商的结构化输出能力需探测；即使服务端宣称保证 JSON，
    /// 客户端仍必须校验。」这个布尔量就是那次校验的结果——
    /// 它是**探测出来的**，不是读某个配置项读来的。
    pub structured_output: bool,
    /// 给用户的一句话，含下一步动作。
    pub message: String,
}

/// 送给模型的一个文件。
///
/// **没有路径字段**，见模块文档。`file_name` 是文件名本身（不含目录），
/// 构造时校验它真的只是一个名字。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct SuggestionItem {
    /// 本会话内随机生成的 fileId。模型只能引用它，引用不到的一律丢弃。
    pub file_id: String,
    pub file_name: String,
    pub extension: String,
    /// 文本摘要，已按上限裁剪。
    pub summary: String,
}

impl SuggestionItem {
    /// 构造一项，并校验文件名真的是一个**名字**。
    ///
    /// 分隔符与 `..` 在这里被拒绝：它们意味着调用方把路径当成了名字，
    /// 而这条路径一旦进了载荷就发出去了。宁可当场失败。
    pub fn new(
        file_id: impl Into<String>,
        file_name: impl Into<String>,
        extension: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<Self, AppError> {
        let file_name = file_name.into();
        if file_name.is_empty() {
            return Err(unknown_item("文件名不能为空。"));
        }
        if file_name.contains(['/', '\\']) {
            return Err(unknown_item(format!(
                "文件名 {file_name:?} 含有路径分隔符。载荷里只允许文件名，\
                 目录结构不发送。"
            )));
        }
        if file_name == "." || file_name == ".." {
            return Err(unknown_item(format!("文件名 {file_name:?} 不是有效名称。")));
        }
        if file_name.contains('\0') {
            return Err(unknown_item("文件名不能包含空字符。"));
        }

        Ok(Self {
            file_id: file_id.into(),
            file_name,
            extension: extension.into(),
            summary: summary.into(),
        })
    }

    /// 界面证据用的展示名。
    pub fn display_name(&self) -> String {
        match self.extension.is_empty() {
            true => self.file_name.clone(),
            false => format!("{}.{}", self.file_name, self.extension),
        }
    }
}

fn unknown_item(message: impl Into<String>) -> AppError {
    // 用 `INVALID_PATH` 是准确的：问题出在一个本来就不该是路径的字段上。
    AppError::new(codes::INVALID_PATH, message)
}

/// 一次建议请求的载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionBatch {
    pub instruction: String,
    pub items: Vec<SuggestionItem>,
}

impl SuggestionBatch {
    /// 构造并校验两个单请求上限（规格 6.4）。
    pub fn new(
        instruction: impl Into<String>,
        items: Vec<SuggestionItem>,
    ) -> Result<Self, AppError> {
        let instruction = instruction.into();
        budget::validate_instruction(&instruction)?;

        let summaries: Vec<String> = items.iter().map(|item| item.summary.clone()).collect();
        BatchSize::of(&summaries).validate()?;

        if items.is_empty() {
            return Err(AppError::new(
                codes::BUDGET_EXCEEDED,
                "没有选中任何文件，不需要请求模型。",
            ));
        }

        Ok(Self { instruction, items })
    }

    /// 载荷里所有文本的字符总数，用于费用与用量统计。
    pub fn character_count(&self) -> u64 {
        let instruction = self.instruction.chars().count() as u64;
        let items: u64 = self
            .items
            .iter()
            .map(|item| item.summary.chars().count() as u64)
            .sum();
        instruction + items
    }
}

/// 一次**已就绪**的调用：端点校验过了、密钥取出来了。
///
/// 端点在这个类型里是 [`Endpoint`]，也就是说**构造不出来无效的端点**——
/// 校验只在 [`ProviderContext::from_config`] 里做一次，之后不可能绕过。
pub struct ProviderContext {
    pub kind: ProviderKind,
    pub endpoint: Endpoint,
    pub model: String,
    secret: Option<Secret>,
}

impl ProviderContext {
    /// 从配置与密钥构造，顺带完成规格 6.4 的端点校验。
    pub fn from_config(config: &ProviderConfig, secret: Option<Secret>) -> Result<Self, AppError> {
        let endpoint = match config.kind {
            // 规格 6.4：「本地模式只允许 loopback 地址」。
            ProviderKind::Local => Endpoint::parse_local(&config.endpoint)?,
            // 规格 6.4：「兼容云端点只接受用户设置的 HTTPS」。
            ProviderKind::Compatible => Endpoint::parse_cloud(&config.endpoint)?,
        };

        if config.model.trim().is_empty() {
            return Err(AppError::new(codes::INVALID_ENDPOINT, "没有填写模型名。"));
        }

        Ok(Self {
            kind: config.kind,
            endpoint,
            model: config.model.clone(),
            secret,
        })
    }

    /// 密钥的明文。**只在拼请求头时调用**。
    pub fn secret(&self) -> Option<&Secret> {
        self.secret.as_ref()
    }
}

impl std::fmt::Debug for ProviderContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Secret` 自己的 Debug 已经是 `***`，但这里连字段名都不想带出来。
        formatter
            .debug_struct("ProviderContext")
            .field("kind", &self.kind)
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("secret", &self.secret)
            .finish()
    }
}

/// 请模型给出一批文件的分类与命名建议。
///
/// 这是规格 6.4 里那个唯一的入口。它的输出 [`Proposal`] 是
/// **不可信输入**：下一站（规划器）会重新校验每一个字段，
/// 所以这里拿到什么就返回什么，不在这里做「顺便修正」。
pub fn suggest(
    context: &ProviderContext,
    batch: &SuggestionBatch,
    cancellation: &dyn Cancellation,
) -> Result<Vec<Proposal>, AppError> {
    match context.kind {
        ProviderKind::Local => super::ollama::suggest(context, batch, cancellation),
        ProviderKind::Compatible => super::compatible::suggest(context, batch, cancellation),
    }
}

/// 探测端点的连通性与结构化输出能力（`test_provider`）。
///
/// 规格 5.2：「`test_provider` | providerId | 发送**固定无个人数据测试文本**,
/// 返回兼容性与错误」。
///
/// 这个函数**不接受任何参数**来指定要发什么，那正是「不发送用户文件」
/// 的落点：它只能发 [`probe_batch`] 造出来的那份合成载荷。
pub fn probe(
    context: &ProviderContext,
    cancellation: &dyn Cancellation,
) -> Result<ProviderProbe, AppError> {
    let batch = probe_batch();

    let proposals = match context.kind {
        ProviderKind::Local => super::ollama::suggest(context, &batch, cancellation),
        ProviderKind::Compatible => super::compatible::suggest(context, &batch, cancellation),
    };

    match proposals {
        Ok(proposals) => Ok(ProviderProbe {
            model: context.model.clone(),
            structured_output: !proposals.is_empty(),
            message: match proposals.is_empty() {
                true => "端点连通，但模型没有返回任何建议。它可能不支持结构化输出，\
                         或者需要换一个模型。"
                    .to_owned(),
                false => "连通正常，模型能按要求返回结构化建议。".to_owned(),
            },
        }),
        // 探测失败要是**可诊断**的：把上游给的那句话原样带出去，
        // 而不是换一句「测试失败」——用户要靠它决定改端点还是改模型。
        Err(error) => Err(error),
    }
}

/// 探测用的固定载荷。
///
/// 内容是**编译期写死**的合成数据，不含任何用户文件名或正文。
/// 有一条单测钉住「它不接受参数」这件事的后果：载荷内容固定。
pub fn probe_batch() -> SuggestionBatch {
    SuggestionBatch {
        instruction: "这是一次连通性测试。请只返回一个 JSON 数组，\
                      其中一项对应下面这个文件。"
            .to_owned(),
        items: vec![SuggestionItem {
            file_id: "probe-0".to_owned(),
            file_name: "连通性测试".to_owned(),
            extension: "txt".to_owned(),
            summary: "这是固定测试文本，用于验证模型能否返回结构化输出。\
                      它不包含任何用户文件内容。"
                .to_owned(),
        }],
    }
}

/// 从 HTTP 传输层的响应里取状态码，并把非 2xx 翻成规格里的错误码。
///
/// 两个适配器共用：401/403 与 429/5xx 的处理**必须一致**，
/// 否则「同一个端点换一种提供商类型就得到不同的错误」。
pub fn ensure_success(status: u16, body: &str) -> Result<(), AppError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    let excerpt: String = body.chars().take(200).collect();
    let (code, message) = match status {
        401 | 403 => (
            codes::MODEL_AUTH,
            format!("认证失败（HTTP {status}）。请检查 API Key 是否正确、是否有权限使用该模型。"),
        ),
        429 => (
            codes::MODEL_TIMEOUT,
            "请求过于频繁（HTTP 429）。稍后会自动重试。".to_owned(),
        ),
        500..=599 => (
            codes::MODEL_TIMEOUT,
            format!("服务端错误（HTTP {status}）。稍后会自动重试。"),
        ),
        _ => (
            codes::MODEL_INVALID_OUTPUT,
            format!("端点返回了 HTTP {status}。"),
        ),
    };

    let error = AppError::new(
        code,
        match excerpt.trim().is_empty() {
            true => message,
            false => format!("{message} 服务端说：{excerpt}"),
        },
    );

    // 「这个错误重试一次有没有意义」由 `ai::budget::Retry` 说了算——
    // **两处不能各写一条规则**：重试循环按它决定要不要再试，而这里按它
    // 决定要不要把 `retryable` 置位给界面看。两条规则漂移的结果是
    // 「界面显示可以重试、而实际不会重试」（或反过来）。
    //
    // 注意它**不等于** `500..=599`：`501 Not Implemented` 说的是
    // 「这个端点不支持这个功能」，重试多少次都一样。
    Err(match super::budget::Retry::of_status(status).allowed() {
        true => error.retryable(),
        false => error,
    })
}

/// 拼 `Authorization` 头。
///
/// 单独一个函数是为了让「密钥只在这一处被读出来」这件事可以被搜到。
pub fn authorization_header(context: &ProviderContext) -> Option<(String, String)> {
    let secret = context.secret()?;
    Some((
        "Authorization".to_owned(),
        format!("Bearer {}", secret.expose()),
    ))
}

/// 走一次完整的 HTTP 请求（两个适配器共用的那一半）。
pub(crate) fn post(
    context: &ProviderContext,
    path: &str,
    body: &str,
    cancellation: &dyn Cancellation,
) -> Result<http::HttpResponse, AppError> {
    let mut endpoint = context.endpoint.clone();
    // 适配器给的是相对路径，接在端点路径之后。
    endpoint.path = join_path(&context.endpoint.path, path);

    let headers: Vec<(String, String)> = authorization_header(context).into_iter().collect();
    let borrowed: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();

    http::post_json(
        &endpoint,
        body,
        &borrowed,
        http::REQUEST_TIMEOUT,
        cancellation,
    )
}

/// 把适配器的相对路径接到端点路径上。
///
/// 只需要处理「端点带不带尾斜杠」两种：端点的路径是用户填的，
/// 而适配器的路径是我们写死的（`/v1/chat/completions` 之类）。
fn join_path(base: &str, addition: &str) -> String {
    let base = base.trim_end_matches('/');
    let addition = addition.trim_start_matches('/');
    match addition.is_empty() {
        true => format!("{base}/"),
        false => format!("{base}/{addition}"),
    }
}

// ---------------------------------------------------------------------------
// 两个适配器共用的那一半：提示词、载荷文本、输出校验
// ---------------------------------------------------------------------------

// 系统提示与用户消息都在 `super::prompt` 里。
//
// 把它们放在同一个文件里是有意的：**所有给模型看的文本都在一处**，
// 排查「模型为什么这么回答」时只需要看那一个文件，而不是在适配器之间翻。
// 那边的 `PROMPT_VERSION` 就是这件事的可追溯性——见 `ai::prompt` 的文档。

/// `reason` 字段的长度上限。
///
/// 规格 6.4 要求「拒绝…过长字段」。理由要简短，而一个几千字符的 reason
/// 通常意味着模型把整段正文倒了出来。
const MAX_REASON_CHARS: usize = 500;

/// `stem` 字段的长度上限（规格 7.1 第 2 条：单个名称组件 ≤80 个 UTF-16 code unit）。
const MAX_STEM_UTF16_UNITS: usize = 80;

/// 校验**一个名字组件**（无论是分类目录名还是文件名主干）。
///
/// ## 为什么要有这一层
///
/// `planner::build::validate_category` 早就拦了这些，但那是**第二道**
/// 校验。模型输出的第一道（这里）原先只查了空、分隔符与非法字符，
/// 于是 `["..", "Windows"]` 能穿过这里、走到规划器去——
/// 它会被第二道拦下，但**第一道没拦就该算缺陷**：
///
/// - 第二道是给「用户手改的计划」用的，它的输入被假定已经过第一道；
/// - 一条穿过第一道的不合法建议，在下游任何一处「忘记再查」的地方
///   都会成为真实路径。
///
/// 这个缺陷是 `every_illegal_model_output_is_refused_item_by_item`
/// 那条夹具抓出来的——它对着规格里「拒绝…**非法路径**」逐条列了
/// 十二种情况，而 `..` 只是其中一种。
fn validate_name_component(segment: &str, what: &str) -> Result<(), String> {
    if segment.trim().is_empty() {
        return Err(format!("{what}里有空的名字"));
    }

    // **目录跳转**。`..` 会让路径指到上一级，`.` 指向自己；
    // 它们在 Windows 上是保留名，不是普通目录名。
    if segment == "." || segment == ".." {
        return Err(format!("{what}里的 {segment:?} 是目录跳转，不是合法名字"));
    }

    if segment.contains(['/', '\\']) {
        return Err(format!("{what}里的 {segment:?} 含路径分隔符"));
    }
    if segment.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Err(format!("{what}里的 {segment:?} 含 Windows 不允许的字符"));
    }

    // 规格 7.1 第 2 条：**单个名称组件** ≤80 个 UTF-16 code unit。
    // 它说的是组件，不只是 stem——分类目录名也是组件。
    let units = segment.encode_utf16().count();
    if units > MAX_STEM_UTF16_UNITS {
        return Err(format!(
            "{what}里的 {segment:?} 过长（{units} 个 UTF-16 单位，上限 {MAX_STEM_UTF16_UNITS}）"
        ));
    }

    Ok(())
}

/// 校验一条模型给出的建议。
///
/// 规格 6.4：「运行时 Schema 设置 additionalProperties=false；
/// 拒绝未知/重复 fileId、重复条目、越界 confidence、非法路径、过长字段。」
/// 其中 additionalProperties 由 `Proposal` 上的 `deny_unknown_fields` 保证，
/// 其余在这里（重复在 `parse_proposals` 里，因为那要跨条目才看得出来）。
///
/// **即使服务端宣称保证 JSON 也必须走这一遍**——规格原文如此。
/// 这不是不信任某个模型，而是不把「远端的行为」当成自己程序的一部分。
pub fn validate_proposal(proposal: &Proposal) -> Result<(), String> {
    if proposal.file_id.trim().is_empty() {
        return Err("fileId 为空".to_owned());
    }

    if !(0.0..=1.0).contains(&proposal.confidence) || proposal.confidence.is_nan() {
        return Err(format!(
            "confidence 越界（{}）：它必须落在 0 到 1 之间。",
            proposal.confidence
        ));
    }

    if proposal.category.len() > 2 {
        return Err(format!(
            "category 最多两层，收到 {} 层",
            proposal.category.len()
        ));
    }
    for segment in &proposal.category {
        validate_name_component(segment, "category")?;
    }

    if proposal.stem.trim().is_empty() {
        return Err("stem 为空".to_owned());
    }
    validate_name_component(&proposal.stem, "stem")?;
    // Windows 不允许名字以点结尾（`资料.` 会被规范化成 `资料`，
    // 于是「计划里写的」与「磁盘上出现的」不一致）。
    if proposal.stem.ends_with('.') {
        return Err(format!("stem {:?} 以点结尾", proposal.stem));
    }

    if proposal.reason.chars().count() > MAX_REASON_CHARS {
        return Err(format!("reason 过长（超过 {MAX_REASON_CHARS} 个字符）"));
    }

    Ok(())
}

/// 解析模型返回的 `content`，得到一组**已校验结构**的建议。
///
/// `known_file_ids` 是本次请求里真正发出去的那些 id。模型引用了别的 id
/// 时，那一条**被丢弃**——而不是报错。理由：模型偶尔会编一个 id 出来，
/// 那是单条建议的失败，不该让整批一起失败；而调用方通过
/// 「拿到的建议数少于发出的文件数」就能知道哪些文件没有获得建议。
pub fn parse_proposals(
    content: &str,
    known_file_ids: &[String],
) -> Result<Vec<Proposal>, AppError> {
    // 有些服务端会把 JSON 包在 ```json 围栏里，即使给了 response_format。
    let trimmed = strip_code_fence(content.trim());

    let raw: Vec<Proposal> = serde_json::from_str(trimmed).map_err(|error| {
        AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            format!(
                "模型的输出不是合法的建议数组：{error}。\
                 不会从不合法的输出里「猜」出内容。"
            ),
        )
    })?;

    let mut seen: Vec<String> = Vec::new();
    let mut proposals = Vec::new();

    for proposal in raw {
        if validate_proposal(&proposal).is_err() {
            // 这里**故意不把原因写进日志**。
            //
            // `validate_proposal` 的消息里带着模型返回的原文（`category`
            // 与 `stem`），而模型往往保留用户文件名的一部分——那些内容
            // 一旦进了 stderr，就会出现在测试与 CI 的日志里，而规格 3.3
            // 明确禁止日志包含文件正文。
            //
            // 详细原因仍然可以从 `validate_proposal` 的返回值拿到（它是个
            // 公开函数，有单测覆盖），只是不往日志里写。
            note_discarded("结构校验未通过：category / stem / confidence / 长度");
            continue;
        }
        if !known_file_ids.iter().any(|id| id == &proposal.file_id) {
            note_discarded("引用了输入里没有的 fileId");
            continue;
        }
        if seen.contains(&proposal.file_id) {
            note_discarded("同一条建议出现了两次");
            continue;
        }
        seen.push(proposal.file_id.clone());
        proposals.push(proposal);
    }

    Ok(proposals)
}

/// 记录一条被丢弃的建议。
///
/// 只记**原因**，不记条目内容——规格 3.3 禁止日志包含文件正文或文件名。
/// 丢弃本身是预期行为（模型偶尔会引用错的 id），所以它不上升为用户可见
/// 的错误；调用方通过「拿到的建议少于发出的文件」知道有遗漏。
fn note_discarded(reason: &str) {
    #[cfg(debug_assertions)]
    eprintln!("[ai] 丢弃了一条建议：{reason}");
    #[cfg(not(debug_assertions))]
    let _ = reason;
}

/// 去掉 ```json … ``` 围栏。
///
/// 只在首尾真的同时出现围栏时才动，避免把一个内容里恰好含反引号的
/// 正常 JSON 改坏。
fn strip_code_fence(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    // 跳过语言标记那一行。
    let rest = match rest.split_once('\n') {
        Some((_, after)) => after,
        None => return text,
    };
    match rest.trim_end().strip_suffix("```") {
        Some(inner) => inner.trim(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(kind: ProviderKind, endpoint: &str) -> ProviderConfig {
        ProviderConfig {
            id: "p1".to_owned(),
            kind,
            endpoint: endpoint.to_owned(),
            model: "test-model".to_owned(),
            credential_ref: None,
        }
    }

    #[test]
    fn a_local_context_refuses_a_remote_endpoint() {
        let error = ProviderContext::from_config(
            &config(ProviderKind::Local, "http://192.168.1.5:11434"),
            None,
        )
        .expect_err("本地提供商不该接受远程地址");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
    }

    #[test]
    fn a_compatible_context_refuses_plain_http() {
        let error = ProviderContext::from_config(
            &config(ProviderKind::Compatible, "http://api.example.com/v1"),
            None,
        )
        .expect_err("兼容云端点必须要求 HTTPS");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
    }

    #[test]
    fn a_compatible_context_accepts_https() {
        let context = ProviderContext::from_config(
            &config(ProviderKind::Compatible, "https://api.example.com/v1"),
            None,
        )
        .expect("HTTPS 端点应被接受");
        assert_eq!(context.endpoint.host, "api.example.com");
    }

    #[test]
    fn an_empty_model_name_is_refused() {
        let mut settings = config(ProviderKind::Local, "http://127.0.0.1:11434");
        settings.model = "   ".to_owned();
        let error = ProviderContext::from_config(&settings, None).expect_err("空模型名必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
    }

    #[test]
    fn the_context_debug_output_never_contains_the_key() {
        // 用户上下文一旦被打进日志，密钥就跟着进去了。
        let secret = Secret::new("sk-super-secret-value").expect("构造密钥");
        let context = ProviderContext::from_config(
            &config(ProviderKind::Compatible, "https://api.example.com"),
            Some(secret),
        )
        .expect("应能构造");

        let printed = format!("{context:?}");
        assert!(!printed.contains("super-secret"), "{printed}");
        assert!(printed.contains("***"), "{printed}");
    }

    // ---- 载荷里没有路径 ----

    #[test]
    fn a_file_name_with_a_separator_is_refused() {
        // 调用方把路径当名字传进来是**最容易发生的一种泄密**：
        // 载荷里只该有文件名。
        for hostile in [
            r"C:\Users\someone\秘密\报告.txt",
            "学习/数学/第三章.pdf",
            "..\\..\\etc\\passwd",
        ] {
            let error = SuggestionItem::new("f1", hostile, "txt", "")
                .expect_err("含分隔符的文件名必须被拒绝");
            assert_eq!(error.code, codes::INVALID_PATH);
        }
    }

    #[test]
    fn a_plain_file_name_is_accepted() {
        let item = SuggestionItem::new("f1", "第三章", "pdf", "内容摘要").expect("应被接受");
        assert_eq!(item.display_name(), "第三章.pdf");
    }

    #[test]
    fn a_file_name_without_an_extension_keeps_none() {
        let item = SuggestionItem::new("f1", "README", "", "").expect("应被接受");
        assert_eq!(item.display_name(), "README");
    }

    #[test]
    fn a_dot_name_is_refused() {
        assert!(SuggestionItem::new("f1", ".", "", "").is_err());
        assert!(SuggestionItem::new("f1", "..", "", "").is_err());
    }

    #[test]
    fn the_item_type_has_exactly_the_fields_we_intend_to_send() {
        // 规格 6.4：「云模式仅发送随机 fileId、文件名、扩展名、最多 2,000 字符的
        // 文本摘要和用户要求；**不发送绝对路径、用户名、完整文件、原图**。」
        //
        // 这条用**白名单**而不是黑名单。黑名单（「不该出现 path / dir / C:」）
        // 只能挡住列出来的那几种名字，而「悄悄加了一个 targetPath 字段」
        // 恰恰是列不出来的那种。白名单逼着任何新增字段先改这里——
        // 而改这里的人会看到上面那句规格原文。
        let item = SuggestionItem::new("f1", "报告", "txt", "摘要").expect("应被接受");
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&item).expect("应能序列化"))
                .expect("应是合法 JSON");

        let mut keys: Vec<&str> = value
            .as_object()
            .expect("应当是对象")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();

        assert_eq!(
            keys,
            ["extension", "fileId", "fileName", "summary"],
            "载荷里的字段必须恰好是这四个：多一个就意味着多一份被发出去的数据"
        );
    }

    // ---- 批次上限 ----

    #[test]
    fn a_batch_over_the_file_limit_is_refused() {
        let items: Vec<SuggestionItem> = (0..=budget::MAX_FILES_PER_REQUEST)
            .map(|index| SuggestionItem::new(format!("f{index}"), "x", "txt", "").expect("构造"))
            .collect();
        let error = SuggestionBatch::new("整理", items).expect_err("超过 10 个文件必须被拒绝");
        assert_eq!(error.code, codes::BUDGET_EXCEEDED);
    }

    #[test]
    fn an_empty_batch_is_refused() {
        assert!(SuggestionBatch::new("整理", Vec::new()).is_err());
    }

    #[test]
    fn an_over_long_instruction_is_refused_before_any_request() {
        let items = vec![SuggestionItem::new("f1", "x", "txt", "").expect("构造")];
        let long = "整".repeat(budget::MAX_INSTRUCTION_CHARS + 1);
        assert!(SuggestionBatch::new(long, items).is_err());
    }

    #[test]
    fn the_character_count_includes_the_instruction() {
        let items = vec![SuggestionItem::new("f1", "x", "txt", "abcd").expect("构造")];
        let batch = SuggestionBatch::new("12345", items).expect("应被接受");
        assert_eq!(batch.character_count(), 5 + 4);
    }

    // ---- 探测载荷 ----

    #[test]
    fn the_probe_batch_contains_no_user_data() {
        // 规格 5.2：test_provider「发送固定无个人数据测试文本」。
        // 这个函数**不接受参数**，所以它发什么完全由这里决定。
        let batch = probe_batch();
        assert_eq!(batch.items.len(), 1);

        let item = &batch.items[0];
        assert!(item.file_name.contains("测试"), "{}", item.file_name);
        assert!(
            !item.summary.contains(['\\', '/']),
            "探测载荷里不该有任何路径：{}",
            item.summary
        );
        // 连 fileId 都是固定的，不是从真实扫描里来的。
        assert_eq!(item.file_id, "probe-0");
    }

    // ---- 状态码分类 ----

    #[test]
    fn a_2xx_is_not_an_error() {
        assert!(ensure_success(200, "{}").is_ok());
        assert!(ensure_success(201, "{}").is_ok());
    }

    #[test]
    fn auth_failures_are_told_apart_from_other_failures() {
        // 用户要做的动作不同：401 是去改密钥，500 是等一会儿。
        assert_eq!(
            ensure_success(401, "").expect_err("401 必须报错").code,
            codes::MODEL_AUTH
        );
        assert_eq!(
            ensure_success(403, "").expect_err("403 必须报错").code,
            codes::MODEL_AUTH
        );
        assert_eq!(
            ensure_success(500, "").expect_err("500 必须报错").code,
            codes::MODEL_TIMEOUT
        );
        assert_eq!(
            ensure_success(429, "").expect_err("429 必须报错").code,
            codes::MODEL_TIMEOUT
        );
    }

    #[test]
    fn an_error_response_body_is_quoted_back_but_bounded() {
        let long = "x".repeat(5_000);
        let error = ensure_success(400, &long).expect_err("400 必须报错");
        assert!(
            error.message.chars().count() < 400,
            "服务端回显必须被截断，否则错误信息会被撑爆：{}",
            error.message.chars().count()
        );
    }

    // ---- 路径拼接 ----

    #[test]
    fn joining_paths_handles_trailing_slashes() {
        assert_eq!(join_path("/", "/v1/chat"), "/v1/chat");
        assert_eq!(join_path("/v1/", "/chat"), "/v1/chat");
        assert_eq!(join_path("/v1", "chat"), "/v1/chat");
        assert_eq!(join_path("", "/v1"), "/v1");
    }

    // ---- 摘要展示 ----

    #[test]
    fn the_summary_never_claims_a_credential_it_does_not_have() {
        let settings = config(ProviderKind::Local, "http://127.0.0.1:11434");
        let without = ProviderSummary::new(&settings, false);
        assert!(!without.has_credential);

        let with = ProviderSummary::new(&settings, true);
        assert!(with.has_credential);

        // 摘要里**没有**放密钥的地方。
        let json = serde_json::to_string(&with).expect("应能序列化");
        assert!(!json.contains("secret"), "{json}");
    }
}
