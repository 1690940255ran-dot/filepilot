//! 待发送载荷的冻结与授权（规格 6.4）。
//!
//! 规格原文：
//!
//! > `preview_disclosure` 生成且**冻结**本次待发送载荷，包括 instruction、
//! > providerId、model、选中文件、源指纹和实际文本。`grant_disclosure`
//! > **仅授权此载荷**；`start_analysis` 按 consentId 读取同一份会话缓存，
//! > **不重新拼接未展示内容**。缓存失效或程序重启后重新预览授权；
//! > 本地模式 `consentId=null`。选择其他 provider/model 或编辑 instruction
//! > 同样令旧授权失效。
//!
//! ## 这个模块守的是什么
//!
//! 「用户看到的」与「实际发出去的」必须是同一份东西。做到这一点需要三件事，
//! 每一件都在下面有对应的落点：
//!
//! 1. **展示的载荷就是冻结的载荷**——预览时算好的 [`FrozenPayload`]
//!    原样存下来，分析时**取出来用**，而不是按当前状态重新拼一遍。
//!    重新拼接会引入一个窗口：用户看着 A 点了授权，程序发出去的却是 B。
//! 2. **授权绑定摘要**——[`ConsentStore::grant`] 只认 digest，
//!    而 digest 覆盖了所有参与发送的字段（见 [`canonical_form`]）。
//! 3. **开始时逐字段比对**——[`ConsentStore::verify_start`] 把请求里那几个
//!    字段与冻结载荷逐项比，任何一项变了都拒绝，并**指出是哪一项**。
//!
//! ## 为什么它只活在内存里
//!
//! 规格要求「程序重启后重新预览授权」。落库会让「上次授权过」跨越重启，
//! 而用户对一个月前那次授权的记忆早就没了——那时他看到的是一份自己
//! 完全不记得的载荷被发了出去。

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use ts_rs::TS;

use crate::domain::errors::{codes, AppError};
use crate::domain::types::{Evidence, ExtractionStatus, Fingerprint, Id, Mode, RelPath};

/// 摘要的规范串版本。
///
/// 与 `planner::validate` 用的是同一套做法：**改口径就换版本**，
/// 这样旧摘要不会被误当成新口径下的结果。
const DISCLOSURE_DIGEST_VERSION: &str = "filepilot.disclosure.payload.v1";

/// 存量上限。
///
/// 规格没有规定，但**无界增长是内存泄漏**：一个长驻进程里，用户每点一次
/// 「预览」就多一份载荷（每份可能几十 KB 的文本）。超出后淘汰最旧的——
/// 被淘汰的那份再想用就会「找不到授权」，而重新预览一步就能拿回来，
/// 代价远小于让内存一直涨。
const MAX_ENTRIES: usize = 32;

/// 一次发送里，单个文件的**实际内容**。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct DisclosureItem {
    pub file_id: Id,
    /// **不含目录、也不含扩展名**的文件名。目录不发送（规格 6.4），
    /// 而扩展名是独立的 [`Self::extension`] 字段。
    pub file_name: String,
    pub extension: String,
    /// 提取时的源指纹。
    ///
    /// 冻结它是为了让「授权时看到的那份文件」与「实际发送的内容」
    /// 可以被核对：文件在这之后被改过，摘要就会对不上。
    pub fingerprint: Fingerprint,
    /// 实际会被发送的文本（已按上限裁剪）。
    ///
    /// `None` 与 `Some("")` 是**两件事**：前者是用户关掉了这个文件的正文
    /// （规格：「预览中允许排除某个文件/关闭其正文」），后者是这个文件
    /// 本来就没有可提取的文本。
    ///
    /// 两者对模型都表现为「没有摘要」，但对用户是**选择**与**事实**的区别
    /// ——而界面上要说的话完全不同。
    pub text: Option<String>,
    /// `text` 是不是**因为上限**被裁过。
    ///
    /// 它是个独立字段，而不是「字符数等于上限就认为被裁过」这种推断：
    /// 原文正好 2,000 字符时那种推断会误报，而界面会据此告诉用户
    /// 「这段被截断了」——一句不真的话。
    ///
    /// 它不进摘要：`truncated` 由 `text` 决定，两者不可能分别变化。
    pub truncated: bool,
    /// 这份提取结果里**真实存在**的证据定位（例如 `page:1`、`chars:0-120`）。
    ///
    /// 规格 6.4 末段：「`evidenceLocator` 必须匹配本次提取的真实定位信息，
    /// 前端证据片段从本地提取结果读取，**不接受模型自造引用**。」
    ///
    /// 它**不发给模型**（见 [`DisclosureItem::to_suggestion`] 与
    /// [`FrozenPayload::to_suggestion_batch`]），而是留在冻结载荷里，
    /// 供分析回来后核对模型给的那个引用是不是编的。
    ///
    /// 它进摘要：提取结果变了，模型能引用的定位也就变了。
    pub evidence: Vec<Evidence>,
    /// 这次提取的结果状态。
    ///
    /// 它是 [`DisclosureItemPreview::text_status`] 的依据——空白本身
    /// 说明不了什么，而「为什么没有正文」是用户要做决定的地方。
    ///
    /// 它**不进摘要**：`text` 已经在摘要里，而 `status` 说的是「文本是怎么
    /// 来的」，不是「发出去的是什么」。
    pub status: ExtractionStatus,
}

/// 一份**已冻结**的待发送载荷。
///
/// 它从不直接出现在 IPC 契约里：给界面看的是 [`DisclosurePreview`]
/// （含逐文件的预览与数量），而这里保留的是**完整文本**——那是真正会发出去的东西。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenPayload {
    pub scan_id: Id,
    pub mode: Mode,
    pub provider_id: Id,
    pub model: String,
    pub instruction: String,
    pub items: Vec<DisclosureItem>,
    pub digest: String,
}

impl FrozenPayload {
    /// 用当前的各个字段算出摘要并组装。
    pub fn seal(
        scan_id: Id,
        mode: Mode,
        provider_id: Id,
        model: String,
        instruction: String,
        items: Vec<DisclosureItem>,
    ) -> Self {
        let mut payload = Self {
            scan_id,
            mode,
            provider_id,
            model,
            instruction,
            items,
            digest: String::new(),
        };
        payload.digest = compute_digest(&payload);
        payload
    }

    /// 参与发送的字符总数（用于用量统计与预算）。
    pub fn character_count(&self) -> u64 {
        let instruction = self.instruction.chars().count() as u64;
        let body: u64 = self
            .items
            .iter()
            .map(|item| item.text.as_deref().unwrap_or("").chars().count() as u64)
            .sum();
        instruction + body
    }

    /// 转成发给模型的批次。
    ///
    /// ## 这是冻结载荷进入请求的**唯一出口**
    ///
    /// 它只读自己，不接受任何「临时的补充」。规格那句「`start_analysis`
    /// 按 consentId 读取同一份会话缓存，**不重新拼接未展示内容**」就落在这里
    /// ——如果发送路径能自己拼一份，那么「用户看到的」与「实际发出去的」
    /// 迟早会分家，而分家的那一刻正是授权失去意义的那一刻。
    pub fn to_suggestion_batch(&self) -> super::provider::SuggestionBatch {
        super::provider::SuggestionBatch {
            instruction: self.instruction.clone(),
            items: self
                .items
                .iter()
                .map(|item| super::provider::SuggestionItem {
                    file_id: item.file_id.clone(),
                    file_name: item.file_name.clone(),
                    extension: item.extension.clone(),
                    // 关掉正文的文件发空摘要——但**仍然发文件名与扩展名**，
                    // 那是「只按名字猜」所需的全部（也正是用户关正文时想要的）。
                    summary: item.text.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }

    /// 给界面看的预览。
    pub fn preview(&self, excerpt_chars: usize) -> DisclosurePreview {
        DisclosurePreview {
            payload_digest: self.digest.clone(),
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            file_count: self.items.len() as u32,
            character_count: self.character_count(),
            instruction: self.instruction.clone(),
            items: self
                .items
                .iter()
                .map(|item| DisclosureItemPreview {
                    file_id: item.file_id.clone(),
                    file_name: display_name(&item.file_name, &item.extension),
                    character_count: item.text.as_deref().unwrap_or("").chars().count() as u32,
                    truncated: item.truncated,
                    excerpt: item
                        .text
                        .as_deref()
                        .unwrap_or("")
                        .chars()
                        .take(excerpt_chars)
                        .collect(),
                    text_status: DisclosureTextStatus::of(item.status, item.text.as_deref()),
                })
                .collect(),
        }
    }
}

/// 把文件名与扩展名拼成展示名。
///
/// 与 `provider::SuggestionItem::display_name` 是同一条规则，但**不合并**：
/// 那个拼的是「送给模型看的名字」，这个拼的是「给用户看的名字」。
/// 将来前者要加防注入处理时，合并会让界面上也跟着变。
fn display_name(file_name: &str, extension: &str) -> String {
    match extension.is_empty() {
        true => file_name.to_owned(),
        false => format!("{file_name}.{extension}"),
    }
}

/// 预览里逐文件的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct DisclosureItemPreview {
    pub file_id: Id,
    pub file_name: String,
    pub character_count: u32,
    /// 这段文本被裁剪过（原文更长）。
    ///
    /// 规格原话：「**文本裁剪不是隐私脱敏保证**」——它只是控制载荷大小。
    /// 界面上要如实说明「被截过」，免得用户以为剩下的内容不会发出去。
    pub truncated: bool,
    /// 实际文本的开头。
    pub excerpt: String,
    /// 这一项**有没有可发送的正文**，以及为什么没有。
    ///
    /// 少了它，界面只能显示「0 个字符」，而用户从那个数字里读不出
    /// 该做什么：
    ///
    /// | 情况 | 用户该做的事 |
    /// |---|---|
    /// | 文件本来就没有文字 | 换一个文件 |
    /// | 格式读不出文字（扫描型 PDF） | 找它的文字版 |
    /// | **提取失败** | 重试，或者查这台机器上的解析环境 |
    /// | 用户自己关掉了正文 | 没别的，就是他选的 |
    ///
    /// 把它们混成一句「没有正文」，用户会去检查一个完全正常的文件，
    /// 或者白白放弃一个其实能用的文件。
    pub text_status: DisclosureTextStatus,
}

/// 预览里这一项的正文状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum DisclosureTextStatus {
    /// 有正文可以发送。
    Present,
    /// 提取成功，但内容里确实没有可读文本。
    Empty,
    /// 格式不受支持（例如扫描型 PDF 没有文本层）。
    Unsupported,
    /// **提取失败**：本该读出来却没读成。这是最需要用户知道的一种。
    Failed,
    /// 用户关掉了这个文件的正文（规格：「预览中允许排除某个文件/关闭其正文」）。
    Excluded,
    /// 还没提取过。正常情况下不该出现在预览里。
    Pending,
}

impl DisclosureTextStatus {
    /// 从「提取状态 + 正文」判断。
    fn of(status: ExtractionStatus, text: Option<&str>) -> Self {
        match text {
            // 用户关掉了正文：这个文件**曾经**提取出什么已经不重要了，
            // 界面要说的是他自己的那个选择。
            None => Self::Excluded,
            Some(body) if !body.is_empty() => Self::Present,
            Some(_) => match status {
                ExtractionStatus::Ok | ExtractionStatus::Partial => Self::Empty,
                ExtractionStatus::Unsupported => Self::Unsupported,
                ExtractionStatus::Failed => Self::Failed,
                ExtractionStatus::Pending => Self::Pending,
            },
        }
    }
}

/// `preview_disclosure` 的返回：**只在本机准备的**一份待发送载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct DisclosurePreview {
    /// 这份载荷的摘要。`grant_disclosure` 用它换授权。
    pub payload_digest: String,
    pub provider_id: Id,
    pub model: String,
    pub file_count: u32,
    pub character_count: u64,
    pub instruction: String,
    pub items: Vec<DisclosureItemPreview>,
}

/// `grant_disclosure` 的返回。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct DisclosureGrant {
    pub consent_id: String,
    pub payload_digest: String,
}

/// `start_analysis` 里可能影响「该不该放行」的那几个字段。
///
/// 把它们收成一个结构体，是为了让 [`ConsentStore::verify_start`] 的签名
/// 说出它到底在比什么——参数散成五个 `&str` 时，漏比一个不会有任何提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequestFacts<'a> {
    pub scan_id: &'a str,
    pub selected_file_ids: &'a [Id],
    pub mode: Mode,
    pub provider_id: Option<&'a str>,
    pub instruction: &'a str,
}

/// 会话内的载荷与授权。
///
/// 用 `Mutex` 而不是无锁结构：这里的操作都是「读一份、写一份」，
/// 竞争极少，而锁能让「检查 + 修改」成为一次原子动作——
/// 授权这件事最不该出现的错就是两个请求之间被插进一次修改。
#[derive(Default)]
pub struct ConsentStore {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// digest → 已预览、尚未授权。
    pending: HashMap<String, FrozenPayload>,
    /// consentId → 已授权。
    granted: HashMap<String, FrozenPayload>,
    /// 插入顺序（先进先出），用于超量淘汰。
    order: VecDeque<String>,
    /// 最近一次 `prepare` 的摘要。
    ///
    /// 本地模式没有 consentId（规格：「本地模式 `consentId=null`」），
    /// 于是 `start_analysis` 传的是 `null`——它靠什么找到载荷？靠这个。
    latest_digest: Option<String>,
}

impl ConsentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 冻结一份载荷，返回它的摘要。
    ///
    /// 规格：「`preview_disclosure` …**只在本地准备**」——它不发出任何请求，
    /// 只是把「如果授权，会发什么」算出来并留一份。
    pub fn prepare(&self, payload: FrozenPayload) -> String {
        let digest = payload.digest.clone();
        let mut inner = self.lock();
        inner.pending.insert(digest.clone(), payload);
        inner.order.push_back(digest.clone());
        inner.latest_digest = Some(digest.clone());
        inner.evict();
        digest
    }

    /// 取回**最近一次预览**的载荷，不要求授权。
    ///
    /// 规格：「本地模式 `consentId=null`」。本地模式的数据不出本机，所以它
    /// 没有授权环节，`start_analysis` 传的是 `null`——那它靠什么找到载荷？
    /// 靠这个：最近一次预览的那一份。
    ///
    /// **它不检查模式**，因为这里不知道调用方打算做什么。那条规则由
    /// `start_analysis` 执行：那里会确认拿到的载荷确实是本地模式的，
    /// 否则「传 null 就能跳过授权」会成为绕过云端授权的一条路。
    pub fn resolve_latest(&self) -> Result<FrozenPayload, AppError> {
        let inner = self.lock();
        let digest = inner.latest_digest.clone().ok_or_else(|| {
            AppError::new(
                codes::TOKEN_EXPIRED,
                "还没有预览过要发送的内容。请先预览，再开始分析。",
            )
        })?;

        // 最近一次预览可能已经授权（在 `granted` 里），也可能还没有
        // （本地模式永远停在 `pending`）。
        if let Some(payload) = inner.granted.get(&digest) {
            return Ok(payload.clone());
        }
        inner.pending.get(&digest).cloned().ok_or_else(|| {
            AppError::new(
                codes::TOKEN_EXPIRED,
                "这次预览已经失效（被新的预览替换，或程序重启过）。请重新预览。",
            )
        })
    }

    /// 用摘要换一份授权。
    ///
    /// 规格：「`grant_disclosure` 仅授权**此**载荷」。所以这里只认 digest，
    /// 并且**要求 provider 也匹配**——一个针对 A 提供商的授权不该能用在 B 上。
    pub fn grant(
        &self,
        payload_digest: &str,
        provider_id: &str,
    ) -> Result<DisclosureGrant, AppError> {
        let mut inner = self.lock();

        let payload = inner.pending.get(payload_digest).ok_or_else(|| {
            AppError::new(
                codes::TOKEN_EXPIRED,
                "这份待发送内容已经不在了（可能被新的预览替换，或程序重启过）。\
                 请重新预览后再授权。",
            )
        })?;

        if payload.provider_id != provider_id {
            return Err(AppError::new(
                codes::REQUEST_CONFLICT,
                format!(
                    "这份内容是为提供商 {} 准备的，不能授权给 {provider_id}。\
                     换了提供商请重新预览。",
                    payload.provider_id
                ),
            ));
        }

        // 从 pending 移到 granted：同一份载荷不给两个 consentId，
        // 否则「撤销授权」会多出一个「撤哪一个」的问题。
        let payload = inner
            .pending
            .remove(payload_digest)
            .expect("上面刚查过存在");
        let consent_id = new_consent_id();
        inner.granted.insert(consent_id.clone(), payload);

        Ok(DisclosureGrant {
            consent_id,
            payload_digest: payload_digest.to_owned(),
        })
    }

    /// 按 consentId 取回冻结的载荷。
    ///
    /// 规格：「`start_analysis` 按 consentId 读取同一份会话缓存，
    /// **不重新拼接未展示内容**」。所以这里是**取**，不是**算**。
    pub fn resolve(&self, consent_id: &str) -> Result<FrozenPayload, AppError> {
        self.lock().granted.get(consent_id).cloned().ok_or_else(|| {
            AppError::new(
                codes::TOKEN_EXPIRED,
                "这次授权已经失效（程序重启过，或内容被新的预览替换）。\
                     请重新预览并确认要发送的内容。",
            )
        })
    }

    /// 把请求里的事实与冻结的载荷逐字段比对。
    ///
    /// 规格：「选择其他 provider/model 或编辑 instruction 同样令旧授权失效」，
    /// 以及命令表里那句「云模式必须**逐字段匹配**已授权载荷」。
    ///
    /// 不一致时**指出是哪一项**：一句笼统的「授权失效」会让用户以为
    /// 是软件出了问题，而实际上他刚刚改了输入框里的一个字。
    pub fn verify_start(
        &self,
        payload: &FrozenPayload,
        request: &StartRequestFacts<'_>,
    ) -> Result<(), AppError> {
        let mismatch = |what: &str, expected: &str, got: &str| {
            AppError::new(
                codes::STALE_PLAN,
                format!(
                    "{what} 与已授权的内容不一致（授权时是 {expected}，现在是 {got}）。\
                     改动之后必须重新预览并确认。"
                ),
            )
        };

        if payload.scan_id != request.scan_id {
            return Err(mismatch("扫描结果", &payload.scan_id, request.scan_id));
        }
        if payload.instruction != request.instruction {
            return Err(mismatch(
                "整理要求",
                &payload.instruction,
                request.instruction,
            ));
        }
        if payload.mode != request.mode {
            return Err(mismatch(
                "整理模式",
                mode_text(payload.mode),
                mode_text(request.mode),
            ));
        }

        match request.provider_id {
            Some(provider_id) if provider_id == payload.provider_id => {}
            Some(provider_id) => {
                return Err(mismatch("模型提供商", &payload.provider_id, provider_id))
            }
            // 云模式必须给提供商；本地模式本来就没有（规格：本地 consentId=null）。
            None => {
                return Err(AppError::new(
                    codes::INVALID_ENDPOINT,
                    "这次分析缺少模型提供商。",
                ))
            }
        }

        // 文件集合要**逐项**比，而且顺序也要一致：顺序变了意味着给模型的
        // 上下文变了，而模型的行为对顺序并非无感。
        let expected: Vec<&str> = payload
            .items
            .iter()
            .map(|item| item.file_id.as_str())
            .collect();
        let got: Vec<&str> = request
            .selected_file_ids
            .iter()
            .map(String::as_str)
            .collect();
        if expected != got {
            return Err(AppError::new(
                codes::STALE_PLAN,
                format!(
                    "选中的文件与已授权的内容不一致（授权时 {} 个，现在 {} 个）。\
                     改动之后必须重新预览并确认。",
                    expected.len(),
                    got.len()
                ),
            ));
        }

        Ok(())
    }

    /// 当前存了多少份（测试与诊断用）。
    pub fn len(&self) -> usize {
        let inner = self.lock();
        inner.pending.len() + inner.granted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 会话结束：整份丢掉。
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.pending.clear();
        inner.granted.clear();
        inner.order.clear();
        inner.latest_digest = None;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // 某个操作 panic 会让锁中毒。这里的数据没有「被破坏到不能用」的
        // 形态（最坏是少一份载荷，而那时 `resolve` 会如实报「授权失效」），
        // 所以取回被毒化的锁，而不是把一次 panic 扩散成所有后续操作失败。
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Inner {
    /// 超出上限时淘汰最旧的。
    ///
    /// 只淘汰 `pending`（还没授权的那批）：**已授权的载荷不主动丢**——
    /// 用户点了授权，分析还没开始，这时把它清掉只会让他莫名其妙地
    /// 收到「授权失效」。
    fn evict(&mut self) {
        while self.pending.len() > MAX_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if self.pending.remove(&oldest).is_none() {
                // 它已经被 grant 走了，不算在 pending 里。
                continue;
            }
        }
    }
}

fn mode_text(mode: Mode) -> &'static str {
    match mode {
        Mode::Rules => "规则模式",
        Mode::AiLocal => "本地模型",
        Mode::AiCloud => "云端模型",
    }
}

/// 构建载荷时的一个来源：本地已经拿到的文件与它的正文。
///
/// 做成结构体而不是一串位置参数，是为了让调用方**必须**逐项给出来——
/// 七个 `&str` 的位置参数里少传一个不会有任何提示，而少传的那一个可能
/// 正是「正文」。
#[derive(Debug, Clone)]
pub struct PayloadSource {
    pub file_id: Id,
    pub relative_path: RelPath,
    pub extension: String,
    pub fingerprint: Fingerprint,
    /// 提取到的正文。空串表示这个文件没有可提取的文本。
    pub text: String,
    /// 这份提取结果里真实存在的证据定位。见 [`DisclosureItem::evidence`]。
    pub evidence: Vec<Evidence>,
    /// 这次提取的结果状态。见 [`DisclosureItem::status`]。
    pub status: ExtractionStatus,
    /// 用户是否关掉了这个文件的正文（规格 6.4：「预览中允许排除某个文件/
    /// **关闭其正文**」）。
    ///
    /// 关掉之后文件名与扩展名**照常发送**——那是一份合法的「只按名字猜」
    /// 的请求，也正是用户关正文时想要的。
    pub excluded: bool,
}

/// 从本地已经拿到的文件与正文构建一份待发送载荷。
///
/// 这是**唯一的载荷构造入口**：`preview_disclosure` 用它产出用户看到的那一份，
/// 而之后实际发送时只会去 [`ConsentStore::resolve`] 取回同一份，
/// 不在这里重新拼装（规格：「不重新拼接未展示内容」）。
pub fn build_payload(
    scan_id: &str,
    mode: Mode,
    provider_id: &str,
    model: &str,
    instruction: &str,
    sources: &[PayloadSource],
) -> Result<FrozenPayload, AppError> {
    if sources.is_empty() {
        return Err(AppError::new(
            codes::BUDGET_EXCEEDED,
            "没有选中任何文件，不需要请求模型。",
        ));
    }

    let mut items = Vec::with_capacity(sources.len());
    for source in sources {
        let (file_name, extension) = file_stem(&source.relative_path, &source.extension)?;

        // 规格 6.4：「最多 2,000 字符的文本摘要」。裁到什么程度必须**如实记录**——
        // 规格另一句提醒：「文本裁剪不是隐私脱敏保证」，所以界面要能说
        // 「这段是截断的」，而不是让用户以为发出去的就是全文。
        let (body, over_limit) = super::budget::truncate_summary(&source.text);

        // 关掉正文时 `truncated` 没有意义：没有任何内容会被发出去。
        let (text, truncated) = match source.excluded {
            true => (None, false),
            false => (Some(body), over_limit),
        };

        items.push(DisclosureItem {
            file_id: source.file_id.clone(),
            file_name,
            extension,
            fingerprint: source.fingerprint.clone(),
            text,
            truncated,
            evidence: source.evidence.clone(),
            status: source.status,
        });
    }

    Ok(FrozenPayload::seal(
        scan_id.to_owned(),
        mode,
        provider_id.to_owned(),
        model.to_owned(),
        instruction.to_owned(),
        items,
    ))
}

/// 从相对路径取「不含扩展名的文件名」，并返回**实际可用的扩展名**。
///
/// ## 为什么不按「最后一个点」切
///
/// `报告.tar.gz` 的扩展名是 `gz`，按第一个点切会得到 `报告`（错），
/// 按最后一个点切会得到 `报告.tar`（对）——而 `没有扩展名` 这种名字
/// 按点切会把它自己切没。用已知的 `extension` 字段去 `strip_suffix`
/// 最可靠：它不猜，只做一次字符串比对。
///
/// ## 第二个返回值是干什么的
///
/// 数据不一致时（`extension` 与文件名对不上）保留**完整文件名**并把扩展名
/// 置空。否则展示名会被拼成 `报告.PDF.txt`——那是一个不存在的文件名。
/// 宁可显示 `报告.PDF`，也不显示一个从没存在过的名字。
fn file_stem(relative_path: &RelPath, extension: &str) -> Result<(String, String), AppError> {
    let last = relative_path
        .last()
        .ok_or_else(|| AppError::internal("文件记录没有相对路径。".to_owned()))?;

    if extension.is_empty() {
        return Ok((last.clone(), String::new()));
    }

    let suffix = format!(".{extension}");
    match last.strip_suffix(&suffix) {
        Some(stem) if !stem.is_empty() => Ok((stem.to_owned(), extension.to_owned())),
        _ => Ok((last.clone(), String::new())),
    }
}

/// 摘要的规范输入串。
///
/// 与 [`compute_digest`] 分开是为了能在单测里直接检查
/// 「**哪些字段参与、顺序如何**」，而不必构造一份完整的载荷。
///
/// ## 哪些字段必须参与
///
/// 凡是**会影响发出去的内容**的，一个都不能少：
/// `scanId`、`mode`、`providerId`、`model`、`instruction`，
/// 以及每个文件的 `fileId` / 展示名 / 扩展名 / 源指纹 / 实际文本。
///
/// 少一个就是一个真实的漏洞：例如漏掉 `text`，用户看到的是第 1 版正文，
/// 而在授权之前文件被改过，发出去的就会是第 2 版——**摘要相同，内容不同**。
pub fn canonical_form(payload: &FrozenPayload) -> String {
    let mut form = String::new();
    form.push_str(DISCLOSURE_DIGEST_VERSION);
    form.push('\u{1e}');

    push_field(&mut form, &payload.scan_id);
    push_field(&mut form, mode_text(payload.mode));
    push_field(&mut form, &payload.provider_id);
    push_field(&mut form, &payload.model);
    push_field(&mut form, &payload.instruction);

    for item in &payload.items {
        form.push('\u{1d}');
        push_field(&mut form, &item.file_id);
        push_field(&mut form, &item.file_name);
        push_field(&mut form, &item.extension);
        push_field(&mut form, &item.fingerprint.volume_id);
        push_field(&mut form, &item.fingerprint.file_id);
        push_field(&mut form, &item.fingerprint.size);
        push_field(&mut form, &item.fingerprint.modified_ns);
        push_field(&mut form, item.fingerprint.sha256.as_deref().unwrap_or(""));
        // 「用户关掉了正文」与「这个文件没有正文」必须产出**不同**的规范串。
        // 否则关掉某个文件的正文之后，载荷其实变了、摘要却没变——
        // 旧授权继续有效，而用户以为自己的改动生效了。
        match &item.text {
            Some(body) => push_field(&mut form, body),
            None => push_field(&mut form, "\u{1}excluded"),
        }
        // 证据定位也参与摘要：提取结果变了，模型**能引用**的定位就变了，
        // 而那是「这次分析基于哪一版内容」的一部分。
        for evidence in &item.evidence {
            push_field(&mut form, &evidence.locator);
        }
    }

    form
}

/// 追加一个字段，用 `\x1f` 分隔。
///
/// 用**不可打印分隔符**而不是 `,` 或 `|`：文件名与正文里可以有任意字符，
/// 用可打印分隔符会让 `["a,b", "c"]` 与 `["a", "b,c"]` 产生相同的规范串
/// ——两份不同的载荷共用一个摘要，正是这个模块最不能出的错。
fn push_field(form: &mut String, value: &str) {
    form.push('\u{1f}');
    form.push_str(value);
}

fn compute_digest(payload: &FrozenPayload) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_form(payload).as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 一个新的授权 id。
///
/// 用操作系统的 CSPRNG（与 T05 的一次性令牌同一套理由）：可猜测的
/// consentId 意味着别人能构造一次「已授权的」分析。
fn new_consent_id() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // 拿不到随机数就**不能**退回时间戳或计数器——那正是可猜测的来源。
        // 返回一个每次都不可能撞上的值，让 `resolve` 必然失败：
        // 「授权拿不到」是安全的失败方向，「授权被猜到」不是。
        return "unavailable".to_owned();
    }
    let mut out = String::with_capacity(32);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(marker: &str) -> Fingerprint {
        Fingerprint {
            volume_id: "V".to_owned(),
            file_id: marker.to_owned(),
            size: "10".to_owned(),
            modified_ns: "0".to_owned(),
            sha256: Some(marker.to_owned()),
        }
    }

    fn item(file_id: &str, text: &str) -> DisclosureItem {
        DisclosureItem {
            file_id: file_id.to_owned(),
            file_name: "报告".to_owned(),
            extension: "txt".to_owned(),
            fingerprint: fingerprint(file_id),
            text: Some(text.to_owned()),
            truncated: false,
            evidence: Vec::new(),
            status: ExtractionStatus::Ok,
        }
    }

    fn source(name: &str, extension: &str, text: &str) -> PayloadSource {
        PayloadSource {
            file_id: format!("id-{name}"),
            // 名字就是**完整文件名**（含扩展名）：这样才能构造出
            // 「扩展名与名字对不上」那种数据不一致的情形。
            relative_path: vec!["子目录".to_owned(), name.to_owned()],
            extension: extension.to_owned(),
            fingerprint: fingerprint(name),
            text: text.to_owned(),
            evidence: Vec::new(),
            status: ExtractionStatus::Ok,
            excluded: false,
        }
    }

    fn payload(instruction: &str, items: Vec<DisclosureItem>) -> FrozenPayload {
        FrozenPayload::seal(
            "scan-1".to_owned(),
            Mode::AiCloud,
            "p1".to_owned(),
            "m1".to_owned(),
            instruction.to_owned(),
            items,
        )
    }

    // ---- 摘要覆盖哪些字段 ----

    #[test]
    fn changing_the_instruction_changes_the_digest() {
        let a = payload("按主题分类", vec![item("f1", "内容")]);
        let b = payload("按时间分类", vec![item("f1", "内容")]);
        assert_ne!(a.digest, b.digest, "改了要求必须换摘要");
    }

    #[test]
    fn changing_the_text_changes_the_digest() {
        // 这条是**最关键**的一条：如果正文不参与摘要，那么
        // 「用户看到的正文」与「实际发出的正文」可以在摘要相同的情况下不同。
        let a = payload("整理", vec![item("f1", "第一版正文")]);
        let b = payload("整理", vec![item("f1", "第二版正文")]);
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn changing_the_fingerprint_changes_the_digest() {
        let mut items = vec![item("f1", "内容")];
        let a = payload("整理", items.clone());
        items[0].fingerprint = fingerprint("改过了");
        let b = payload("整理", items);
        assert_ne!(a.digest, b.digest, "文件的指纹也是要发出去的信息之一");
    }

    #[test]
    fn changing_the_model_or_provider_changes_the_digest() {
        let a = payload("整理", vec![item("f1", "x")]);
        let mut b = payload("整理", vec![item("f1", "x")]);
        b.model = "m2".to_owned();
        b.digest = compute_digest(&b);
        assert_ne!(a.digest, b.digest);

        let mut c = payload("整理", vec![item("f1", "x")]);
        c.provider_id = "p2".to_owned();
        c.digest = compute_digest(&c);
        assert_ne!(a.digest, c.digest);
    }

    #[test]
    fn reordering_the_files_changes_the_digest() {
        let a = payload("整理", vec![item("f1", "甲"), item("f2", "乙")]);
        let b = payload("整理", vec![item("f2", "乙"), item("f1", "甲")]);
        assert_ne!(a.digest, b.digest, "顺序变了，送给模型的上下文就变了");
    }

    #[test]
    fn the_same_content_always_produces_the_same_digest() {
        let a = payload("整理", vec![item("f1", "内容")]);
        let b = payload("整理", vec![item("f1", "内容")]);
        assert_eq!(a.digest, b.digest, "摘要必须可复现");
    }

    #[test]
    fn field_boundaries_cannot_be_forged() {
        // 用可打印分隔符（逗号、竖线）会出这个错：
        // `["a,b", "c"]` 与 `["a", "b,c"]` 拼出同一个串，于是两份**不同的**
        // 载荷共用一个摘要——而摘要正是授权的依据。
        let a = payload("整理", vec![item("a,b", "c")]);
        let b = payload("整理", vec![item("a", "b,c")]);
        assert_ne!(
            canonical_form(&a),
            canonical_form(&b),
            "字段边界不能被内容伪造"
        );
    }

    // ---- 授权 ----

    fn store_with(payload: FrozenPayload) -> (ConsentStore, String) {
        let store = ConsentStore::new();
        let digest = store.prepare(payload);
        (store, digest)
    }

    #[test]
    fn granting_returns_a_consent_that_resolves_to_the_same_payload() {
        let original = payload("整理", vec![item("f1", "内容")]);
        let (store, digest) = store_with(original.clone());

        let grant = store.grant(&digest, "p1").expect("授权应成功");
        assert_eq!(grant.payload_digest, digest);

        let resolved = store.resolve(&grant.consent_id).expect("应能取回");
        assert_eq!(
            resolved, original,
            "取回的必须是**冻结的那一份**，不是按当前状态重算的"
        );
    }

    #[test]
    fn granting_an_unknown_digest_is_refused() {
        let store = ConsentStore::new();
        let error = store.grant("不存在的摘要", "p1").expect_err("必须拒绝");
        assert_eq!(error.code, codes::TOKEN_EXPIRED);
    }

    #[test]
    fn a_grant_cannot_be_moved_to_another_provider() {
        // 针对 A 的授权不该能用在 B 上——那等于绕过了「换了提供商要重新授权」。
        let (store, digest) = store_with(payload("整理", vec![item("f1", "内容")]));
        let error = store.grant(&digest, "另一个提供商").expect_err("必须拒绝");
        assert_eq!(error.code, codes::REQUEST_CONFLICT);
    }

    #[test]
    fn the_same_digest_cannot_be_granted_twice() {
        // 一份载荷只给一个 consentId，否则「撤销授权」会多出
        // 「撤哪一个」的问题。
        let (store, digest) = store_with(payload("整理", vec![item("f1", "内容")]));
        store.grant(&digest, "p1").expect("第一次应成功");
        assert!(store.grant(&digest, "p1").is_err(), "第二次必须被拒");
    }

    #[test]
    fn an_unknown_consent_resolves_to_an_explicit_error() {
        let store = ConsentStore::new();
        let error = store.resolve("没见过").expect_err("必须报错");
        assert_eq!(error.code, codes::TOKEN_EXPIRED);
        assert!(error.message.contains("重新预览"), "{}", error.message);
    }

    #[test]
    fn consents_are_unique() {
        // 可猜测的 consentId 等于别人能构造一次「已授权的」分析。
        let (store, digest) = store_with(payload("整理", vec![item("f1", "内容")]));
        let first = store.grant(&digest, "p1").expect("授权");

        let (store2, digest2) = store_with(payload("整理", vec![item("f1", "内容")]));
        let second = store2.grant(&digest2, "p1").expect("授权");
        assert_ne!(first.consent_id, second.consent_id);
        assert_eq!(first.consent_id.len(), 32, "16 字节的十六进制");
    }

    #[test]
    fn clearing_drops_everything() {
        let (store, digest) = store_with(payload("整理", vec![item("f1", "内容")]));
        store.grant(&digest, "p1").expect("授权");
        assert!(!store.is_empty());

        store.clear();
        assert!(store.is_empty(), "会话结束要整份丢掉");
    }

    // ---- 逐字段比对 ----

    #[test]
    fn a_matching_start_request_passes() {
        let frozen = payload("整理", vec![item("f1", "内容")]);
        let (store, digest) = store_with(frozen.clone());
        let grant = store.grant(&digest, "p1").expect("授权");
        let resolved = store.resolve(&grant.consent_id).expect("取回");

        let request = StartRequestFacts {
            scan_id: "scan-1",
            selected_file_ids: &["f1".to_owned()],
            mode: Mode::AiCloud,
            provider_id: Some("p1"),
            instruction: "整理",
        };
        assert!(store.verify_start(&resolved, &request).is_ok());
    }

    #[test]
    fn an_edited_instruction_is_refused_and_names_the_field() {
        let frozen = payload("整理", vec![item("f1", "内容")]);
        let request = StartRequestFacts {
            scan_id: "scan-1",
            selected_file_ids: &["f1".to_owned()],
            mode: Mode::AiCloud,
            provider_id: Some("p1"),
            instruction: "改成按时间整理",
        };
        let error = store_of(&frozen)
            .verify_start(&frozen, &request)
            .expect_err("必须拒绝");

        assert_eq!(error.code, codes::STALE_PLAN);
        assert!(
            error.message.contains("整理要求"),
            "要指出是哪一项不一致，否则用户以为软件坏了：{}",
            error.message
        );
    }

    fn store_of(_payload: &FrozenPayload) -> ConsentStore {
        ConsentStore::new()
    }

    #[test]
    fn a_changed_file_set_is_refused() {
        let frozen = payload("整理", vec![item("f1", "内容"), item("f2", "内容2")]);
        let store = ConsentStore::new();
        let request = StartRequestFacts {
            scan_id: "scan-1",
            selected_file_ids: &["f1".to_owned()],
            mode: Mode::AiCloud,
            provider_id: Some("p1"),
            instruction: "整理",
        };
        let error = store.verify_start(&frozen, &request).expect_err("必须拒绝");
        assert!(error.message.contains("文件"), "{}", error.message);
    }

    #[test]
    fn swapping_the_provider_is_refused() {
        let frozen = payload("整理", vec![item("f1", "内容")]);
        let store = ConsentStore::new();
        let request = StartRequestFacts {
            scan_id: "scan-1",
            selected_file_ids: &["f1".to_owned()],
            mode: Mode::AiCloud,
            provider_id: Some("p2"),
            instruction: "整理",
        };
        let error = store.verify_start(&frozen, &request).expect_err("必须拒绝");
        assert!(error.message.contains("提供商"), "{}", error.message);
    }

    #[test]
    fn changing_the_mode_is_refused() {
        let frozen = payload("整理", vec![item("f1", "内容")]);
        let store = ConsentStore::new();
        let request = StartRequestFacts {
            scan_id: "scan-1",
            selected_file_ids: &["f1".to_owned()],
            mode: Mode::AiLocal,
            provider_id: Some("p1"),
            instruction: "整理",
        };
        assert!(store.verify_start(&frozen, &request).is_err());
    }

    // ---- 预览 ----

    #[test]
    fn the_preview_shows_what_would_really_be_sent() {
        let frozen = payload("整理", vec![item("f1", "这是一段正文")]);
        let preview = frozen.preview(4);

        assert_eq!(preview.payload_digest, frozen.digest);
        assert_eq!(preview.file_count, 1);
        assert_eq!(preview.items[0].file_name, "报告.txt");
        assert_eq!(
            preview.items[0].excerpt, "这是一段",
            "预览里要能看到**实际文本**，而不是一个概括"
        );
        assert!(!preview.items[0].truncated);
    }

    #[test]
    fn the_preview_marks_a_truncated_summary() {
        // 走**真的会裁剪的**构建路径：只有那里知道裁剪到底发生了没有，
        // 而它把那个事实记进 `truncated` 字段。
        //
        // 这条测试原先用「字符数 >= 上限」来断言，而那个推断有个假阳性：
        // 原文**正好**等于上限时并没有被裁，界面却会说「被截断」。
        let long = "字".repeat(super::super::budget::MAX_SUMMARY_CHARS + 10);
        let frozen = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("超长.txt", "txt", &long)],
        )
        .expect("构建");

        let preview = frozen.preview(10);
        assert!(preview.items[0].truncated, "被裁剪过就要如实标出来");
        assert_eq!(
            preview.items[0].character_count as usize,
            super::super::budget::MAX_SUMMARY_CHARS
        );
    }

    #[test]
    fn the_character_count_covers_instruction_and_text() {
        let frozen = payload("12345", vec![item("f1", "abcd")]);
        assert_eq!(frozen.character_count(), 9);
    }

    // ---- 上限 ----

    #[test]
    fn the_store_does_not_grow_without_bound() {
        let store = ConsentStore::new();
        for index in 0..(MAX_ENTRIES * 3) {
            store.prepare(payload(&format!("整理 {index}"), vec![item("f1", "内容")]));
        }
        assert!(
            store.len() <= MAX_ENTRIES,
            "无界增长就是内存泄漏：{}",
            store.len()
        );
    }

    #[test]
    fn evicting_never_drops_a_granted_payload() {
        // 用户点了授权、分析还没开始——这时把它清掉只会让他
        // 莫名其妙地收到「授权失效」。
        let store = ConsentStore::new();
        let digest = store.prepare(payload("整理", vec![item("f1", "内容")]));
        let grant = store.grant(&digest, "p1").expect("授权");

        for index in 0..(MAX_ENTRIES * 3) {
            store.prepare(payload(
                &format!("后来的 {index}"),
                vec![item("f1", "内容")],
            ));
        }

        assert!(
            store.resolve(&grant.consent_id).is_ok(),
            "已授权的载荷不该被淘汰掉"
        );
    }

    // ---- 载荷构建 ----

    #[test]
    fn the_file_name_loses_its_directory() {
        // 规格 6.4：「不发送绝对路径」。相对路径里最后一段之前的那些目录
        // **不进载荷**——这里断言的是构造出来的名字里没有它们。
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.txt", "txt", "正文")],
        )
        .expect("应能构建");

        assert_eq!(built.items[0].file_name, "报告");
        assert_eq!(built.items[0].extension, "txt");
        assert!(
            !canonical_form(&built).contains("子目录"),
            "目录不该出现在载荷里"
        );
    }

    #[test]
    fn a_name_with_two_dots_keeps_the_inner_part() {
        // `报告.tar.gz` 的扩展名是 `gz`，剥掉之后是 `报告.tar`。
        // 按「第一个点」切会得到 `报告`，那会丢掉 `.tar`——
        // 而这个信息对「按类型分类」是有用的。
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.tar.gz", "gz", "正文")],
        )
        .expect("应能构建");

        assert_eq!(built.items[0].file_name, "报告.tar");
        assert_eq!(built.items[0].extension, "gz");
    }

    #[test]
    fn a_mismatched_extension_is_not_concatenated_twice() {
        // 数据不一致时（记录里的扩展名与文件名对不上）保留完整文件名、
        // 扩展名置空——否则展示名会拼成 `报告.PDF.txt`，
        // 那是一个从来没存在过的文件名。
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.PDF", "txt", "正文")],
        )
        .expect("应能构建");

        assert_eq!(built.items[0].file_name, "报告.PDF");
        assert_eq!(built.items[0].extension, "");
        assert_eq!(built.preview(10).items[0].file_name, "报告.PDF");
    }

    #[test]
    fn a_body_over_the_limit_is_truncated_and_flagged() {
        let long = "字".repeat(super::super::budget::MAX_SUMMARY_CHARS + 5);
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.txt", "txt", &long)],
        )
        .expect("应能构建");

        assert_eq!(
            built.items[0].text.as_deref().unwrap_or("").chars().count(),
            super::super::budget::MAX_SUMMARY_CHARS
        );
        assert!(built.items[0].truncated, "裁过就要如实标出来");
    }

    #[test]
    fn a_body_exactly_at_the_limit_is_not_flagged_as_truncated() {
        // 这条钉住的是「用字符数推断 truncated」那个假阳性：
        // 原文正好等于上限时它**没有**被裁，界面就不该说「被截断」。
        let exact = "字".repeat(super::super::budget::MAX_SUMMARY_CHARS);
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.txt", "txt", &exact)],
        )
        .expect("应能构建");

        assert!(!built.items[0].truncated, "正好等于上限不等于被截断");
        assert_eq!(
            built.preview(5).items[0].character_count as usize,
            super::super::budget::MAX_SUMMARY_CHARS
        );
    }

    #[test]
    fn building_with_no_files_is_refused() {
        let error = build_payload("scan-1", Mode::AiCloud, "p1", "m1", "整理", &[])
            .expect_err("没有文件必须被拒绝");
        assert_eq!(error.code, codes::BUDGET_EXCEEDED);
    }

    #[test]
    fn the_digest_covers_the_body_that_will_actually_be_sent() {
        // 裁剪发生在构建时，摘要盖的应该是**裁过之后**的正文——
        // 那才是真正会发出去的东西。所以超出上限的那部分内容
        // 不该影响摘要（它不会离开本机）。
        let long = "字".repeat(super::super::budget::MAX_SUMMARY_CHARS + 100);
        let first = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.txt", "txt", &long)],
        )
        .expect("构建");
        let second = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "整理",
            &[source("报告.txt", "txt", &long)],
        )
        .expect("构建");
        assert_eq!(first.digest, second.digest, "同样的输入必须得到同样的摘要");

        let mut beyond_limit = source("报告.txt", "txt", &long);
        beyond_limit.text.push('尾');
        let third = build_payload("scan-1", Mode::AiCloud, "p1", "m1", "整理", &[beyond_limit])
            .expect("构建");
        assert_eq!(
            first.digest, third.digest,
            "上限之外的字符不会发出去，所以它不该改变摘要"
        );

        // 改**上限之内**的内容。这里整个重新构造而不是 `replace_range`：
        // 对含中文的字符串做字节切片会撞上「不是一个字符边界」而 panic，
        // 而那条 panic 与要验的行为毫无关系。
        let mut within_limit = source("报告.txt", "txt", &long);
        within_limit.text = format!(
            "改{}",
            "字".repeat(super::super::budget::MAX_SUMMARY_CHARS - 1)
        );
        let fourth = build_payload("scan-1", Mode::AiCloud, "p1", "m1", "整理", &[within_limit])
            .expect("构建");
        assert_ne!(
            first.digest, fourth.digest,
            "上限**之内**改了内容，摘要必须变"
        );
    }

    #[test]
    fn the_built_payload_reaches_the_model_as_the_preview_showed_it() {
        // 端到端的一小步：构建 → 预览 → 冻结 →（转成批次）。
        // 批次里的内容必须来自冻结的那一份，且**没有目录**。
        let built = build_payload(
            "scan-1",
            Mode::AiCloud,
            "p1",
            "m1",
            "按主题分类",
            &[source("报告.txt", "txt", "季度总结")],
        )
        .expect("构建");

        let store = ConsentStore::new();
        let digest = store.prepare(built.clone());
        let grant = store.grant(&digest, "p1").expect("授权");
        let resolved = store.resolve(&grant.consent_id).expect("取回");

        assert_eq!(resolved.items[0].text.as_deref(), Some("季度总结"));
        assert_eq!(resolved.items[0].file_name, "报告");
        // 展示名（含扩展名）是给界面的，而发给模型的是分开的两项。
        assert_eq!(built.preview(10).items[0].file_name, "报告.txt");
    }

    #[test]
    fn the_text_status_tells_the_kinds_of_blank_apart() {
        // 「提取失败」「格式读不出文字」「文件本来就没有文字」「用户关掉了正文」
        // 是**四件不同的事**，而用户要做的动作各不相同。
        //
        // 把它们混成一句「没有正文」，用户会去检查一个完全正常的文件，
        // 或者白白放弃一个其实能用的文件。
        use DisclosureTextStatus::*;

        let cases = [
            (ExtractionStatus::Ok, Some("正文"), Present),
            (ExtractionStatus::Partial, Some("半截"), Present),
            // 提取成功，但内容里确实没有可读文本。
            (ExtractionStatus::Ok, Some(""), Empty),
            // 扫描型 PDF 这类：格式在那儿，只是没有文本层。
            (ExtractionStatus::Unsupported, Some(""), Unsupported),
            // 本该读出来却没读成——最需要用户知道的一种。
            (ExtractionStatus::Failed, Some(""), Failed),
            (ExtractionStatus::Pending, Some(""), Pending),
            // 关掉正文**优先于**提取状态：界面要说的是用户自己的那个选择，
            // 而不是一个他已经不关心的提取结果。
            (ExtractionStatus::Ok, None, Excluded),
            (ExtractionStatus::Failed, None, Excluded),
        ];

        for (status, text, expected) in cases {
            assert_eq!(
                DisclosureTextStatus::of(status, text),
                expected,
                "status={status:?} text={text:?}"
            );
        }
    }

    #[test]
    fn excluding_a_body_still_sends_the_file_name() {
        // 规格 6.4：「预览中允许排除某个文件/**关闭其正文**」。
        //
        // 关掉正文不等于把文件从请求里删掉：用户要的是「只按名字猜」，
        // 而文件名与扩展名正是猜的依据。
        let mut sources = vec![source("报告.txt", "txt", "季度总结")];
        sources.push(source("会议.txt", "txt", "讨论排期"));
        sources[1].excluded = true;

        let built =
            build_payload("scan-1", Mode::AiLocal, "p1", "m1", "整理", &sources).expect("应能构建");

        assert_eq!(built.items.len(), 2, "关掉正文不该把文件从载荷里删掉");
        assert_eq!(built.items[0].text.as_deref(), Some("季度总结"));
        assert!(built.items[1].text.is_none(), "被关掉的那一份没有正文");

        let batch = built.to_suggestion_batch();
        assert_eq!(batch.items[1].file_name, "会议", "文件名照常发送");
        assert_eq!(batch.items[1].extension, "txt", "扩展名照常发送");
        assert_eq!(batch.items[1].summary, "", "只有摘要为空");

        // 预览要说的是用户自己的那个选择。
        let preview = built.preview(10);
        assert_eq!(preview.items[1].text_status, DisclosureTextStatus::Excluded);
        assert_eq!(preview.items[0].text_status, DisclosureTextStatus::Present);
    }

    #[test]
    fn excluding_a_body_changes_the_digest() {
        // 关掉正文让载荷**真的变了**，所以摘要必须变——否则旧授权继续有效，
        // 而用户以为自己的改动生效了。
        let mut open = vec![source("报告.txt", "txt", "季度总结")];
        open.push(source("会议.txt", "txt", "讨论排期"));
        let mut closed = open.clone();
        closed[1].excluded = true;

        let a = build_payload("scan-1", Mode::AiLocal, "p1", "m1", "整理", &open).expect("构建");
        let b = build_payload("scan-1", Mode::AiLocal, "p1", "m1", "整理", &closed).expect("构建");

        assert_ne!(
            a.digest, b.digest,
            "关掉正文必须让摘要变化，否则旧授权还能用"
        );
    }
}
