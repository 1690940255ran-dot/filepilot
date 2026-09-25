//! AI 分析的载荷预览与授权（规格 5.2 / 6.4）。
//!
//! 三条命令里的两条在这里，第三条（`start_analysis`）紧随其后：
//!
//! ```text
//! 本地提取 → 展示实际拟发送字段 → 授权 → 模型建议 → 严格校验 → 确定性规划
//! ```
//!
//! ## 为什么命令体很薄
//!
//! 真正的工作在 [`build_preview`] 与 [`grant`] 里，而它们是普通函数。
//! 这不是为了「好看」：Tauri 的 `State` 在集成测试里**构造不出来**，
//! 而「授权绑定的是用户看到的那一份」这件事必须在**真实存取路径**上验证
//! ——用一段复现逻辑去测，测的是那段复现逻辑。

use tauri::{Emitter as _, Manager as _, State};

use crate::ai::budget::{TaskBudget, MAX_INSTRUCTION_CHARS};
use crate::ai::disclosure::{
    build_payload, DisclosureGrant, DisclosurePreview, FrozenPayload, PayloadSource,
    StartRequestFacts,
};
use crate::ai::provider::ProviderKind;
use crate::app_state::AppState;
use crate::commands::TaskProgress;
use crate::domain::errors::{codes, AppError};
use crate::domain::ipc::IpcResult;
use crate::domain::types::{
    ExtractionStatus, FileRecord, Id, Mode, Proposal, TaskError, TaskStatus,
};
use crate::extractors::service::{extract_batch, ExtractRequest, NoopExtractObserver};
use crate::storage::repositories::{self, AnalysisRow};

/// 预览里每个文件展示多少个字符。
///
/// 规格要求预览给的是「**实际文本预览**」而不是一个概括——用户要据此判断
/// 「发出去的是不是我以为的那段」。200 个字符够看出「是不是这份文件」，
/// 又不至于把整个界面撑爆；后面的 [`DisclosurePreview`] 同时给出完整字符数，
/// 想看全的可以去那里。
const PREVIEW_EXCERPT_CHARS: usize = 200;

/// 预览待发送内容（规格 5.2：「**只在本地准备**」）。
///
/// 它不发任何网络请求，只把「如果授权，会发什么」算出来、**冻结**下来，
/// 并把摘要交给前端。
#[tauri::command]
pub fn preview_disclosure(
    state: State<'_, AppState>,
    scan_id: String,
    selected_file_ids: Vec<Id>,
    mode: Mode,
    provider_id: String,
    instruction: String,
    // 用户关掉了正文的那些文件（规格 6.4：「预览中允许排除某个文件/
    // **关闭其正文**」）。它们仍在 `selected_file_ids` 里——关掉的是正文，
    // 不是文件本身：文件名与扩展名照常发送，只是不带摘要。
    //
    // 省略即为「全都要发正文」。
    excluded_file_ids: Option<Vec<Id>>,
) -> IpcResult<DisclosurePreview> {
    let excluded = excluded_file_ids.unwrap_or_default();
    build_preview(
        &state,
        &scan_id,
        &selected_file_ids,
        mode,
        &provider_id,
        &instruction,
        &excluded,
    )
    .into()
}

/// 授权一份**已经预览过**的载荷（规格 5.2：`grant_disclosure`）。
///
/// 输入只有摘要与提供商 id——**没有内容**。这不是「为了少传参数」：
/// 让这个接口拿不到内容，「授权一份用户没看过的载荷」在实现上就无从表达。
#[tauri::command]
pub fn grant_disclosure(
    state: State<'_, AppState>,
    payload_digest: String,
    provider_id: String,
) -> IpcResult<DisclosureGrant> {
    grant(&state, &payload_digest, &provider_id).into()
}

/// [`preview_disclosure`] 的主体。
pub fn build_preview(
    state: &AppState,
    scan_id: &str,
    selected_file_ids: &[Id],
    mode: Mode,
    provider_id: &str,
    instruction: &str,
    // 用户关掉了正文的那些文件（规格 6.4：「预览中允许排除某个文件/
    // **关闭其正文**」）。它们仍在 `selected_file_ids` 里。
    excluded_file_ids: &[Id],
) -> Result<DisclosurePreview, AppError> {
    let config =
        crate::storage::repositories::load_provider(state.db(), provider_id)?.ok_or_else(|| {
            AppError::new(
                codes::INVALID_ENDPOINT,
                format!("找不到 id 为 {provider_id} 的提供商配置。"),
            )
        })?;

    ensure_mode_matches_provider(mode, config.kind)?;

    // 与批处理用**同一条**长度规则：超长是拒绝而不是截断。
    // 用户在输入框里写了 1,500 个字却被悄悄砍掉一半，他会看到一份明显
    // 不符合要求的建议，却不知道为什么。
    let length = instruction.chars().count();
    if length > MAX_INSTRUCTION_CHARS {
        return Err(AppError::new(
            codes::BUDGET_EXCEEDED,
            format!("整理要求最多 {MAX_INSTRUCTION_CHARS} 个字符，当前 {length} 个。"),
        ));
    }

    let sources = payload_sources(state, scan_id, selected_file_ids, excluded_file_ids)?;
    let payload = build_payload(
        scan_id,
        mode,
        provider_id,
        &config.model,
        instruction,
        &sources,
    )?;

    // 先取预览再冻结：`prepare` 会消费载荷，而预览要读它。
    // 两者算出的摘要必须一致——它们来自同一个对象。
    let preview = payload.preview(PREVIEW_EXCERPT_CHARS);
    state.disclosures().prepare(payload);

    Ok(preview)
}

/// [`grant_disclosure`] 的主体。
pub fn grant(
    state: &AppState,
    payload_digest: &str,
    provider_id: &str,
) -> Result<DisclosureGrant, AppError> {
    state.disclosures().grant(payload_digest, provider_id)
}

/// [`start_analysis`] 的返回。
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    ts_rs::TS,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct AnalysisStart {
    /// 后台任务的 id（与扫描共用同一套任务通道）。
    pub task_id: String,
    /// 这次分析的 id。`create_plan` 用它取回建议。
    pub analysis_id: Id,
}

/// 开始一次分析（规格 5.2）。
///
/// ## 这个命令是「确认前不发云请求」的落点
///
/// 规格 6.4 的流程是「本地提取 → 展示实际拟发送字段 → **授权** → 模型建议」。
/// 所以这里做的第一件事是把**冻结的**载荷取回来并逐字段核对，而不是
/// 「按当前状态重新拼一份」。
///
/// ## 关于 `too_many_arguments`
///
/// 规格 5.2 把这个命令的输入列成六个字段（`scanId`、`selectedFileIds`、
/// `mode`、`providerId`、`instruction`、`consentId`），加上注进来的
/// `app` 与 `state` 一共八个。**它们就是 IPC 的形状**——把它们收成一个
/// 结构体会改变前端要传的东西，而那是契约，不该为了过 linter 去动。
///
/// 命令体本身很薄：真正的六个字段在 [`AnalysisRequest`] 里，
/// 业务在 [`begin_analysis`] 里。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn start_analysis(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    scan_id: String,
    selected_file_ids: Vec<Id>,
    mode: Mode,
    provider_id: Option<String>,
    instruction: String,
    consent_id: Option<String>,
) -> IpcResult<AnalysisStart> {
    let request = AnalysisRequest {
        scan_id,
        selected_file_ids,
        mode,
        provider_id: provider_id.clone(),
        instruction,
        consent_id,
    };

    let (started, payload) = match begin_analysis(&state, &request) {
        Ok(value) => value,
        Err(error) => return IpcResult::err(error),
    };

    let task_id = started.task_id.clone();
    let analysis_id = started.analysis_id.clone();
    let file_count = payload.items.len() as u32;

    // 后台跑：分析要发网络请求（本地模型也可能要几秒），
    // 不该把 IPC 线程占住。前端用 `get_task` 看进度。
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let cancellation = state.cancellation_token(&task_id);

        let outcome = run_analysis(
            &state,
            &payload,
            provider_id.as_deref(),
            cancellation.as_deref(),
        );

        match outcome {
            Ok(proposals) => {
                // 建议落库。`proposalJson` 里只有建议本身——规格 8.1 的
                // 「analyses 表不保存正文」由**载荷本身**保证：
                // `Proposal` 里没有能装正文的字段。
                let json = serde_json::to_string(&proposals).unwrap_or_else(|_| "[]".to_owned());
                if let Err(error) = repositories::finish_analysis(
                    state.db(),
                    &analysis_id,
                    "completed",
                    Some(&json),
                ) {
                    state.fail_task(&task_id, TaskError::from(&error));
                    return;
                }
                state.finish_task_with_status(&task_id, TaskStatus::Completed);
                let _ = app.emit(
                    crate::commands::TASK_PROGRESS_CHANNEL,
                    TaskProgress {
                        task_id,
                        seq: 2,
                        status: TaskStatus::Completed,
                        processed: file_count,
                        total: Some(file_count),
                    },
                );
            }
            Err(error) => {
                let _ = repositories::finish_analysis(state.db(), &analysis_id, "failed", None);
                state.fail_task(&task_id, TaskError::from(&error));
                let _ = app.emit(
                    crate::commands::TASK_PROGRESS_CHANNEL,
                    TaskProgress {
                        task_id,
                        seq: 2,
                        status: TaskStatus::Failed,
                        processed: 0,
                        total: Some(file_count),
                    },
                );
            }
        }
    });

    IpcResult::ok(started)
}

/// 一次分析的输入事实（规格 5.2 的 `start_analysis` 参数）。
///
/// 与 [`StartRequestFacts`] 是同一组字段，但那个是**借用**形式（只用于与
/// 冻结载荷比对），这个是**拥有**形式（用于建记录）。合成一个会逼出
/// 生命周期上的麻烦，而它们服务的场合确实不同。
///
/// 收成结构体还有一个直接的好处：`begin_analysis` 原先有 8 个位置参数，
/// 而它们里有三个是 `Option` 加三个 `&str` —— 调用点写错顺序不会有任何提示。
pub struct AnalysisRequest {
    pub scan_id: String,
    pub selected_file_ids: Vec<Id>,
    pub mode: Mode,
    pub provider_id: Option<String>,
    pub instruction: String,
    /// 云端必须给（规格：「云模式必须逐字段匹配已授权载荷」）；
    /// 本地模式为 `None`（规格：「本地模式 `consentId=null`」）。
    pub consent_id: Option<String>,
}

/// [`start_analysis`] 的前半段：取载荷、核对、登记。
///
/// 返回给前端的那个，以及**后台线程要用的那一份冻结载荷**。
pub fn begin_analysis(
    state: &AppState,
    request: &AnalysisRequest,
) -> Result<(AnalysisStart, FrozenPayload), AppError> {
    let payload = resolve_payload(state, request.mode, request.consent_id.as_deref())?;

    // **逐字段核对**。规格 5.2：「云模式必须逐字段匹配已授权载荷」。
    let facts = StartRequestFacts {
        scan_id: &request.scan_id,
        selected_file_ids: &request.selected_file_ids,
        mode: request.mode,
        provider_id: request.provider_id.as_deref(),
        instruction: &request.instruction,
    };
    state.disclosures().verify_start(&payload, &facts)?;

    let analysis_id = uuid::Uuid::new_v4().to_string();

    // `inputFingerprintsJson`：事后要能回答「这次分析基于哪一版文件」。
    let fingerprints = payload
        .items
        .iter()
        .map(|item| item.fingerprint.clone())
        .collect::<Vec<_>>();
    let fingerprints_json = serde_json::to_string(&fingerprints).ok();

    repositories::insert_analysis(
        state.db(),
        &AnalysisRow {
            id: analysis_id.clone(),
            scan_id: request.scan_id.clone(),
            mode: mode_name(request.mode),
            provider_id: request.provider_id.clone(),
            status: "running".to_owned(),
            proposal_json: None,
            input_fingerprints_json: fingerprints_json,
            prompt_version: Some(crate::ai::prompt::PROMPT_VERSION.to_owned()),
        },
    )?;

    let task_id = state.start_task(Some(payload.items.len() as u32));

    Ok((
        AnalysisStart {
            task_id,
            analysis_id,
        },
        payload,
    ))
}

/// 取回要发送的载荷。
///
/// ## 传 `null` 不等于跳过授权
///
/// 规格说「本地模式 `consentId=null`」，于是 `None` 这条路上**必须**
/// 确认拿到的是本地模式的载荷：云端分析没有授权 id 就一律拒绝。
/// 这是「传 null 就能免授权」那条绕过路径唯一会被堵住的地方。
fn resolve_payload(
    state: &AppState,
    mode: Mode,
    consent_id: Option<&str>,
) -> Result<FrozenPayload, AppError> {
    match consent_id {
        Some(id) => state.disclosures().resolve(id),
        None => {
            let payload = state.disclosures().resolve_latest()?;
            if payload.mode != Mode::AiLocal {
                return Err(AppError::new(
                    codes::STALE_PLAN,
                    "云端分析必须带上授权 id。请先预览要发送的内容并确认。",
                ));
            }
            let _ = mode;
            Ok(payload)
        }
    }
}

/// 真正发请求的那一段：取提供商、取密钥、调适配器。
///
/// 它**只**从冻结载荷构造请求（`to_suggestion_batch`），不碰任何别的东西
/// ——规格那句「不重新拼接未展示内容」在这里兑现。
///
/// 对 crate 公开是为了让集成测试能直接驱动它：`start_analysis` 是 Tauri
/// 命令（要 `AppHandle`），而「假服务收到的请求与预览一致」这件事必须在
/// **真实的发送路径**上验，不能用一段复现逻辑代替。
pub fn run_analysis(
    state: &AppState,
    payload: &FrozenPayload,
    provider_id: Option<&str>,
    cancellation: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<Proposal>, AppError> {
    let config = load_provider(state, provider_id)?;

    let secret = match &config.credential_ref {
        Some(reference) => crate::platform::credentials::read(reference)?,
        None => None,
    };

    let context = crate::ai::provider::ProviderContext::from_config(&config, secret)?;
    let mut proposals = suggest_with_retries(&context, payload, cancellation)?;

    // 核对模型给的证据引用。放在这里而不是 `parse_proposals` 里，是因为
    // 规格规定了 `suggest(batch, cancellation)` 的签名，而 `batch` 里
    // **没有**证据定位——那种「本地有、但不发给模型」的东西不该混进
    // 发给模型的形状里。
    drop_fabricated_evidence(&mut proposals, payload);

    Ok(proposals)
}

/// 丢掉模型**编造**的证据引用（规格 6.4 末段）。
///
/// > `evidenceLocator` 必须匹配本次提取的真实定位信息，前端证据片段从
/// > 本地提取结果读取，**不接受模型自造引用**。
///
/// ## 为什么是「清掉引用」而不是「丢掉整条建议」
///
/// 分类与命名才是模型的主要产出，引用只是它的**出处**。一个编造的出处
/// 说明模型在这一点上不可信，但不该让它给出的分类一起作废——那对用户
/// 是净损失，而他看到的会是「模型没有给出建议」：真实原因（模型引用了
/// 不存在的位置）永远不会浮出来。
///
/// 前端按 locator 去**本地**提取结果里取片段，取不到就不显示，所以被
/// 清掉的引用不会在界面上留下任何假证据。
fn drop_fabricated_evidence(proposals: &mut [Proposal], payload: &FrozenPayload) {
    for proposal in proposals.iter_mut() {
        let Some(locator) = proposal.evidence_locator.as_deref() else {
            continue;
        };

        let Some(item) = payload
            .items
            .iter()
            .find(|item| item.file_id == proposal.file_id)
        else {
            // 未知 fileId 本该在前一步就被丢掉。走到这里说明顺序出了问题，
            // 但那也不该在这里 panic：清掉引用、让建议留下，比让整批
            // 分析崩掉有用得多（与 `parse_proposals` 的处理一致）。
            proposal.evidence_locator = None;
            continue;
        };

        if !item
            .evidence
            .iter()
            .any(|evidence| evidence.locator == locator)
        {
            proposal.evidence_locator = None;
        }
    }
}

/// 重试与格式修复的**上限**（规格 6.4：「指数退避并**服从上限**」）。
///
/// 一次退避就把时间预算吃光时，等待本身已经没有意义——用户宁可立刻
/// 看到「服务端在忙」，也不想对着转圈等 30 秒。
const MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);

/// 取提供商配置，缺 `providerId` 时回落到设置里选中的那一个。
fn load_provider(
    state: &AppState,
    provider_id: Option<&str>,
) -> Result<crate::ai::provider::ProviderConfig, AppError> {
    // 本地模式的 `providerId` 可以为 `null`（规格 5.2 的「providerId 或 null」），
    // 那时用设置里选中的那个提供商。
    let provider_id = match provider_id {
        Some(id) => id.to_owned(),
        None => state.settings().selected_provider_id.ok_or_else(|| {
            AppError::new(
                codes::INVALID_ENDPOINT,
                "还没有选择模型提供商。请先在设置里配置一个。",
            )
        })?,
    };

    repositories::load_provider(state.db(), &provider_id)?.ok_or_else(|| {
        AppError::new(
            codes::INVALID_ENDPOINT,
            format!("找不到 id 为 {provider_id} 的提供商配置。"),
        )
    })
}

/// 带重试与格式修复的调用（规格 6.4）。
///
/// 规格原文：
///
/// > 429/可恢复 5xx 最多重试 2 次，指数退避并服从上限；401/403 不重试。
/// > 最多 1 次输出格式修复，仍失败则返回模型格式错误。
///
/// 三种情况**分开处理**，因为它们要的动作不同：
///
/// | 情况 | 动作 |
/// |---|---|
/// | 可重试（429 / 可恢复 5xx） | 退避后再问一次，最多 2 次 |
/// | 输出不是合法 JSON | **修复**：再问一次并明确要求 JSON，最多 1 次 |
/// | 其他（401/403 等） | 直接返回——重试多少次都是一样的结果 |
///
/// 「可重试」的判定读的是 `AppError::retryable`，而它由
/// `ai::budget::Retry` 在构造错误时置位——**规则只有一条**。
fn suggest_with_retries(
    context: &crate::ai::provider::ProviderContext,
    payload: &FrozenPayload,
    cancellation: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<Proposal>, AppError> {
    let mut budget = TaskBudget::new();
    let mut repair_used = false;

    loop {
        // **先记再发**：一个失败的分支不该让计数漏掉一次真实调用，
        // 而「最多 30 次请求」这条上限是按真实调用算的。
        budget.begin_request()?;

        let batch = request_batch(payload, repair_used);
        let outcome =
            crate::ai::provider::suggest(context, &batch, &CancellationFlag(cancellation));

        let error = match outcome {
            Ok(proposals) => return Ok(proposals),
            Err(error) => error,
        };

        // 取消不是失败：它不该被重试，也不该被「修复」。
        if is_cancelled(cancellation) {
            return Err(error);
        }

        if error.retryable && budget.allow_retry() {
            std::thread::sleep(crate::ai::budget::backoff_delay(
                budget.retries(),
                MAX_BACKOFF,
            ));
            continue;
        }

        // 格式问题走**修复**而不是重试：再问一次，并明确要求 JSON 本身。
        // 重试同一个请求只会拿到同一个不合法的输出。
        if error.code == codes::MODEL_INVALID_OUTPUT && !repair_used && budget.allow_repair() {
            repair_used = true;
            continue;
        }

        return Err(error);
    }
}

/// 构造这一次要发的批次。
///
/// 正常情况就是**冻结载荷本身**。修复时在用户要求后面追加一段**固定文本**
/// （[`crate::ai::prompt::REPAIR_SUFFIX`]）——它只谈格式，不含任何来自本次
/// 请求的内容，所以「用户看到并授权的那份」与实际发出去的，在**内容**上
/// 仍然一致，只有格式要求被说得更死。
fn request_batch(payload: &FrozenPayload, repair: bool) -> crate::ai::provider::SuggestionBatch {
    let mut batch = payload.to_suggestion_batch();
    if repair {
        batch.instruction = format!(
            "{}\n\n{}",
            batch.instruction,
            crate::ai::prompt::REPAIR_SUFFIX
        );
    }
    batch
}

fn is_cancelled(cancellation: Option<&std::sync::atomic::AtomicBool>) -> bool {
    cancellation.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
}

/// 把任务里的取消标志适配成传输层要的 [`Cancellation`]。
///
/// [`Cancellation`]: crate::ai::http::Cancellation
struct CancellationFlag<'a>(Option<&'a std::sync::atomic::AtomicBool>);

impl crate::ai::http::Cancellation for CancellationFlag<'_> {
    fn should_stop(&self) -> bool {
        self.0
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
    }
}

/// `Mode` 在库里存成 camelCase 文本。
fn mode_name(mode: Mode) -> String {
    match serde_json::to_value(mode) {
        Ok(serde_json::Value::String(text)) => text,
        _ => "rules".to_owned(),
    }
}

/// 模式与提供商类型必须一致。
///
/// 规格把模式分成「本地模型」与「云端模型」，而提供商也分本地与兼容云——
/// 两者说的是同一件事，不一致时载荷会被送到与用户预期**不同**的地方：
/// 界面显示「本机运行」，而实际发去了云端（或反过来）。
fn ensure_mode_matches_provider(mode: Mode, kind: ProviderKind) -> Result<(), AppError> {
    let expected = match mode {
        Mode::AiLocal => ProviderKind::Local,
        Mode::AiCloud => ProviderKind::Compatible,
        Mode::Rules => {
            return Err(AppError::new(
                codes::INVALID_ENDPOINT,
                "规则模式不请求模型，也就没有需要授权的内容。",
            ))
        }
    };

    if expected != kind {
        let mode_text = match mode {
            Mode::AiLocal => "本地模型",
            Mode::AiCloud => "云端模型",
            Mode::Rules => "规则模式",
        };
        let kind_text = match kind {
            ProviderKind::Local => "本机提供商",
            ProviderKind::Compatible => "兼容云提供商",
        };
        return Err(AppError::new(
            codes::INVALID_ENDPOINT,
            format!(
                "{mode_text}不能配 {kind_text}：那会把内容送到与预期不同的地方。\
                 请改选提供商，或换一个模式。"
            ),
        ));
    }

    Ok(())
}

/// 把选中的文件变成载荷来源（名字、指纹、**实际正文**）。
///
/// 正文优先从会话缓存取（T11）；未命中才提取。规格 8.1 要求正文
/// 「仅内存或会话临时缓存」，所以这整条路径都不落库。
fn payload_sources(
    state: &AppState,
    scan_id: &str,
    selected_file_ids: &[Id],
    // 用户关掉了正文的那些文件 id。它们**仍在** `selected_file_ids` 里——
    // 关掉的是正文，不是文件本身：文件名与扩展名照常发送。
    excluded_file_ids: &[Id],
) -> Result<Vec<PayloadSource>, AppError> {
    let records = state.scan_records(scan_id).ok_or_else(|| {
        AppError::new(
            codes::STALE_PLAN,
            "这次扫描的结果已经不在了。请重新扫描后再整理。",
        )
    })?;

    let root_id = state.scan_root_id(scan_id).ok_or_else(|| {
        AppError::new(codes::STALE_PLAN, "这次扫描的根目录信息丢了。请重新扫描。")
    })?;
    let root = state.root(&root_id).ok_or_else(|| {
        AppError::new(
            codes::INVALID_PATH,
            "这个目录的授权已经失效（程序重启过，或它被移到了别处）。请重新选择目录。",
        )
    })?;

    // 按**用户给定的顺序**取：顺序是载荷语义的一部分——模型按次序逐项回答，
    // 而 `verify_start` 也会逐项比顺序。
    let mut selected: Vec<&FileRecord> = Vec::with_capacity(selected_file_ids.len());
    for file_id in selected_file_ids {
        let record = records
            .iter()
            .find(|record| &record.id == file_id)
            .ok_or_else(|| {
                AppError::new(
                    codes::INVALID_PATH,
                    format!("这次扫描的结果里没有 {file_id} 这个文件。"),
                )
            })?;
        selected.push(record);
    }

    if selected.is_empty() {
        return Err(AppError::new(
            codes::BUDGET_EXCEEDED,
            "没有选中任何文件，不需要请求模型。",
        ));
    }

    let mut cache = state.extraction_cache();

    // 先问一遍命中情况，再统一取值：`get` 返回的 `&str` 会借住缓存，
    // 而紧接着的 `put` 需要可变借用——两件事不能同时发生。
    let mut missing: Vec<ExtractRequest> = Vec::new();
    for record in &selected {
        if cache.get(&record.id, &record.fingerprint).is_none() {
            missing.push(ExtractRequest {
                file_id: record.id.clone(),
                relative_path: record.relative_path.clone(),
            });
        }
    }

    if !missing.is_empty() {
        let extracted = extract_batch(&root, &missing, &NoopExtractObserver);
        for extraction in &extracted {
            // **失败的结果也要进缓存**。
            //
            // 第一版把 `Failed` 跳过了，理由是「别把一次失败缓存成『这个文件
            // 没有正文』」。那个理由本身没错，但代价是**失败这件事本身就丢了**
            // ——预览里只剩一个「0 个字符」，而用户从那个数字里读不出
            // 「本该读出来却没读成」。
            //
            // 现在缓存整份结果（含状态），预览因此能如实说出是哪一种，
            // 见 `DisclosureTextStatus`。
            //
            // 缓存键用**扫描记录里的指纹**，而**不是**提取返回的那个。
            //
            // 这两者不是同一个东西：`Extraction.source_fingerprint` 是提取时
            // 算出的快照（它带着 `sha256`），而扫描记录里的那份可能还没有哈希
            // ——规格 8.1 说「仅用于展示的初扫结果可以为 `None`」。
            //
            // 用 A 写、用 B 读的话 `get` 永远不命中（它靠指纹相等判断），
            // 于是每一次预览都要重新提取一遍，而正文会是**空的**——
            // 这个 bug 真实发生过，是被 `tests/analysis.rs` 里那条
            // 「预览里要能看到真实正文」抓住的。
            let Some(record) = selected
                .iter()
                .find(|record| record.id == extraction.file_id)
            else {
                continue;
            };
            cache.put(&extraction.file_id, &record.fingerprint, extraction.clone());
        }
    }

    let sources = selected
        .into_iter()
        .map(|record| {
            // 命中时缓存的指纹与 `record.fingerprint` 必然一致（`get` 就是这么
            // 判断的），所以载荷里的指纹用哪一个都一样——用记录的那个。
            let cached = cache.get(&record.id, &record.fingerprint);

            PayloadSource {
                file_id: record.id.clone(),
                relative_path: record.relative_path.clone(),
                extension: record.extension.clone(),
                fingerprint: record.fingerprint.clone(),
                text: cached.map(|hit| hit.text.clone()).unwrap_or_default(),
                // 证据定位随正文一起冻结。规格 6.4 要用它核对模型给的
                // `evidenceLocator` 是不是编的，所以它必须来自**这一次**
                // 提取，而不是事后重新提取的结果。
                evidence: cached.map(|hit| hit.evidence.clone()).unwrap_or_default(),
                // 拿不到缓存时按**失败**算：那说明提取没有产出可用的结果，
                // 而「提取失败」正是界面要说的话。
                status: cached
                    .map(|hit| hit.status)
                    .unwrap_or(ExtractionStatus::Failed),
                excluded: excluded_file_ids.iter().any(|id| id == &record.id),
            }
        })
        .collect();

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_mode_cannot_use_a_cloud_provider() {
        let error = ensure_mode_matches_provider(Mode::AiLocal, ProviderKind::Compatible)
            .expect_err("本地模式配云端提供商必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
        assert!(error.message.contains("本地模型"), "{}", error.message);
    }

    #[test]
    fn a_cloud_mode_cannot_use_a_local_provider() {
        let error = ensure_mode_matches_provider(Mode::AiCloud, ProviderKind::Local)
            .expect_err("云端模式配本机提供商必须被拒绝");
        assert!(error.message.contains("云端模型"), "{}", error.message);
    }

    #[test]
    fn the_matching_combinations_pass() {
        assert!(ensure_mode_matches_provider(Mode::AiLocal, ProviderKind::Local).is_ok());
        assert!(ensure_mode_matches_provider(Mode::AiCloud, ProviderKind::Compatible).is_ok());
    }

    #[test]
    fn the_rules_mode_has_nothing_to_disclose() {
        let error = ensure_mode_matches_provider(Mode::Rules, ProviderKind::Local)
            .expect_err("规则模式不该有要授权的内容");
        assert!(error.message.contains("规则模式"), "{}", error.message);
    }
}
