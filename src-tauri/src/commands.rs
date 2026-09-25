//! Tauri 命令。
//!
//! 规格 5.2 的白名单是唯一允许的入口。每条命令都必须：
//! 1. 只接受 **id**，不接受路径——前端伪造不出授权（规格 3.3）；
//! 2. 返回 `IpcResult<T>`，失败带结构化错误（规格 0.8）；
//! 3. 不把绝对路径以外的能力暴露给前端。

use std::sync::atomic::Ordering;

use tauri::{Emitter, Manager, State};

use crate::ai::http::NeverCancel;
use crate::ai::provider::{
    ProviderConfig, ProviderContext, ProviderKind, ProviderProbe, ProviderSummary,
};
use crate::app_state::{AppState, DEFAULT_PAGE_LIMIT};
use crate::domain::types::{
    AppSettings, FilePage, RootSummary, ScanSummary, TaskError, TaskStatus, TaskSummary,
};
use crate::domain::{errors::codes, errors::AppError, IpcResult};
use crate::extractors::protocol::OcrAvailabilityReport;
use crate::scanner::{scan_cancellable, ScanOptions};

/// 规格 5.2：事件通道**仅**使用 `task-progress`。
pub const TASK_PROGRESS_CHANNEL: &str = "task-progress";

/// `task-progress` 的载荷。
///
/// 规格 5.2：`{taskId, seq, status, processed, total}`，不含正文。
/// `seq` 对每个任务单调递增，前端据此丢弃乱序事件。
///
/// 它对 crate 内公开：规格说「事件通道**仅**使用 `task-progress`」，
/// 所以分析（`commands_analysis`）与执行发的必须是**同一个形状**——
/// 各模块自己定义一个「差不多的」结构，前端就要按来源分别解析。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub task_id: String,
    pub seq: u32,
    pub status: TaskStatus,
    pub processed: u32,
    pub total: Option<u32>,
}

/// 读取非敏感应用设置。
///
/// 返回 `AppSettings`，其中**不含密钥**（规格 3.3）。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> IpcResult<AppSettings> {
    IpcResult::ok(state.settings())
}

/// 查询本机 OCR 可用状态（规格 T11）。
///
/// 设置页据此显示「可用」或者「为什么不可用、下一步做什么」，而不是等
/// 用户对着一张扫描件提取失败之后才去猜。
///
/// 它**不碰任何用户文件**：探测在一个独立的工作进程里跑，只向系统问一句
/// 「有哪些识别语言」。规格要求「Windows OCR 适配只在本机运行，不调用
/// 云端视觉接口」——这条在本机是**构造性**的，代码里就没有那条网络路径。
///
/// 探测要起一个子进程（几十毫秒量级），所以它**独立成一条命令**，
/// 而不是塞进 `get_settings`：后者在启动时会被调用，把它拖慢没必要。
#[tauri::command]
pub fn get_ocr_status() -> IpcResult<OcrAvailabilityReport> {
    crate::extractors::service::ocr_availability().into()
}

// ---------------------------------------------------------------------------
// 提供商配置（T12，规格 5.2）
// ---------------------------------------------------------------------------

/// 凭据引用名。由 providerId 派生，**稳定且可复现**。
///
/// 用派生而不是随机值有两个后果，都是我们要的：
/// 同一个 provider 再保存一次是**覆盖**那一条凭据（不留下孤儿条目），
/// 而删除 provider 时也能算出该删哪一条。
fn credential_ref_for(provider_id: &str) -> String {
    format!("provider-{provider_id}")
}

/// 保存提供商配置。
///
/// 规格 5.2：「`save_provider` | providerId、kind、endpoint、model、可选 secret |
/// 保存配置和凭据引用，**不返回 secret**」。
///
/// 返回的是 [`ProviderSummary`]——它只有一个布尔量表示「密钥在不在」，
/// 没有任何字段能装下密钥本身。前端保存之后再也不能读回明文（规格 3.3）。
///
/// ## `secret` 为空时**保留原有密钥**
///
/// 用户可能只想把端点从一个地址改到另一个地址，此时他不该被要求重填密钥。
/// 所以 `None` 的含义是「不动密钥」，而不是「清掉密钥」。
#[tauri::command]
pub fn save_provider(
    state: State<'_, AppState>,
    provider_id: String,
    kind: ProviderKind,
    endpoint: String,
    model: String,
    secret: Option<String>,
) -> IpcResult<ProviderSummary> {
    save_provider_impl(state.db(), &provider_id, kind, &endpoint, &model, secret).into()
}

/// [`save_provider`] 的业务主体。
///
/// 与命令分开是为了让它**能被集成测试直接调用**：Tauri 的 `State` 在
/// 测试里构造不出来，而「密钥不会进入数据库」这条断言必须在**真实的
/// 存取路径**上验证——用一段复现逻辑去测，测的是那段复现逻辑。
pub fn save_provider_impl(
    db: &crate::storage::db::Database,
    provider_id: &str,
    kind: ProviderKind,
    endpoint: &str,
    model: &str,
    secret: Option<String>,
) -> Result<ProviderSummary, AppError> {
    let provider_id = provider_id.trim().to_owned();
    if provider_id.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "提供商 id 不能为空。"));
    }

    let credential_ref = credential_ref_for(&provider_id);

    // 先落密钥再落配置：反过来的话，一个写凭据失败会让数据库里多出一条
    // 指向不存在凭据的配置，用户看到「已保存密钥」而实际上没有。
    let stored_secret = match secret {
        Some(plain) => {
            let secret = crate::platform::credentials::Secret::new(plain)?;
            crate::platform::credentials::store(&credential_ref, &secret)?;
            // `secret` 在这里 drop，明文被擦掉；它从来没有被写进别的变量。
            true
        }
        // 没给密钥：沿用已存在的那一条（可能本来就有一条，也可能没有）。
        None => crate::platform::credentials::read(&credential_ref)?.is_some(),
    };

    let config = ProviderConfig {
        id: provider_id,
        kind,
        endpoint: endpoint.trim().to_owned(),
        model: model.trim().to_owned(),
        credential_ref: match stored_secret {
            true => Some(credential_ref),
            false => None,
        },
    };

    // **在保存时就校验端点**，而不是等第一次发请求才失败。
    // 端点不合规（本地写了外网地址、云端写了 http）在这里就被挡住，
    // 用户当场看到原因，不必等到整理文件时才发现。
    ProviderContext::from_config(&config, None)?;

    crate::storage::repositories::save_provider(db, &config)?;
    Ok(ProviderSummary::new(&config, stored_secret))
}

/// 列出全部提供商配置。
///
/// 返回的每一条都**不含密钥**，只带一个「有没有配置密钥」的布尔量。
/// 那个布尔量要真的去读一次凭据存储才知道——不能靠
/// 「`credentialRef` 字段非空」推断，因为用户可能已经手工在
/// 「凭据管理器」里把那条删掉了。
#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> IpcResult<Vec<ProviderSummary>> {
    let configs = match crate::storage::repositories::list_providers(state.db()) {
        Ok(configs) => configs,
        Err(error) => return IpcResult::err(error),
    };

    let summaries = configs
        .iter()
        .map(|config| {
            let has_credential = match &config.credential_ref {
                // 取到明文只用来看「存不存在」，随后立刻丢弃（`Secret` 的
                // Drop 会擦掉那段内存）。这个值从不离开这个闭包。
                //
                // 读失败时**当作「没有」而不是报错**：一条凭据读不出来
                // （比如用户在「凭据管理器」里删掉了它）不该让整个列表
                // 打不开——那会连「去改配置」这条路都堵死。
                // 注意这与 `save_provider_impl` 的处理相反：那里读失败要
                // 如实报错，因为用户正在配密钥，他需要知道存不进去。
                Some(reference) => crate::platform::credentials::read(reference)
                    .map(|secret| secret.is_some())
                    .unwrap_or(false),
                None => false,
            };
            ProviderSummary::new(config, has_credential)
        })
        .collect();

    IpcResult::ok(summaries)
}

/// 测试一个提供商的连通性与结构化输出能力。
///
/// 规格 5.2：「`test_provider` | providerId | 发送**固定无个人数据测试文本**，
/// 返回兼容性与错误」。
///
/// 载荷由 `ai::provider::probe_batch` 在编译期写死，那个函数**不接受参数**
/// ——所以「测试时误发用户文件」在这条路径上无从表达。
#[tauri::command]
pub fn test_provider(state: State<'_, AppState>, provider_id: String) -> IpcResult<ProviderProbe> {
    let config = match crate::storage::repositories::load_provider(state.db(), &provider_id) {
        Ok(Some(config)) => config,
        Ok(None) => {
            return IpcResult::err(AppError::new(
                codes::INVALID_ENDPOINT,
                format!("找不到 id 为 {provider_id} 的提供商配置。"),
            ))
        }
        Err(error) => return IpcResult::err(error),
    };

    // 有引用才去取密钥；没有引用时传 `None`，本地 Ollama 就是这样。
    let secret = match &config.credential_ref {
        Some(reference) => match crate::platform::credentials::read(reference) {
            Ok(secret) => secret,
            Err(error) => return IpcResult::err(error),
        },
        None => None,
    };

    let context = match ProviderContext::from_config(&config, secret) {
        Ok(context) => context,
        Err(error) => return IpcResult::err(error),
    };

    crate::ai::provider::probe(&context, &NeverCancel).into()
}

/// 弹出原生目录选择框，授权一个整理根。
///
/// 规格 3.3：**由后端取得根目录并生成 rootId**，前端只能拿到 `rootId`。
///
/// 返回 `Option`：用户取消是正常流程，不是错误。
/// 把它做成错误会逼迫前端用「错误码白名单」来区分取消与真失败，
/// 那正是容易出错的地方。
#[tauri::command]
pub fn choose_root(state: State<'_, AppState>) -> IpcResult<Option<RootSummary>> {
    let picked = rfd::FileDialog::new()
        .set_title("选择要整理的文件夹")
        .pick_folder();

    let Some(path) = picked else {
        return IpcResult::ok(None);
    };

    match crate::safety::root::approve_root(&path) {
        Ok(root) => {
            let matching = match crate::storage::repositories::reauthorize_matching_roots(
                state.db(),
                root.volume_id(),
                &root.identity().file_id_string(),
                state.session_id(),
            ) {
                Ok(rows) => rows,
                Err(error) => return IpcResult::err(error),
            };
            if let Some(primary) = matching.first() {
                let primary_id = primary.id.clone();
                let mut summary = None;
                for saved in matching {
                    let Ok(id) = uuid::Uuid::parse_str(&saved.id) else {
                        return IpcResult::err(AppError::internal(
                            "历史根目录记录包含无效 id，已拒绝重新授权",
                        ));
                    };
                    let registered = state.register_root(root.with_id(id));
                    if saved.id == primary_id {
                        summary = Some(registered);
                    }
                }
                if let Err(error) = crate::recovery::startup::recover_unfinished_runs(
                    state.inner(),
                    crate::safety::confirmation::now_unix_ms(),
                ) {
                    return IpcResult::err(error);
                }
                return IpcResult::ok(summary);
            }

            // 规格 8.1：计划必须持久化，而 plans.rootId 有外键指向 roots。
            // 根不落库，等到 create_plan 时就会撞外键约束——那时离用户
            // 已经走了很远，错误也难解释。所以在这里就写。
            let row = crate::storage::repositories::RootRow {
                id: root.id().to_string(),
                canonical_path: root.canonical().display().to_string(),
                volume_id: root.volume_id().to_owned(),
                identity: root.identity().file_id_string(),
                session_id: state.session_id().to_owned(),
            };
            if let Err(error) = crate::storage::repositories::insert_root(state.db(), &row) {
                return IpcResult::err(error);
            }
            let summary = state.register_root(root);
            if let Err(error) = crate::recovery::startup::recover_unfinished_runs(
                state.inner(),
                crate::safety::confirmation::now_unix_ms(),
            ) {
                return IpcResult::err(error);
            }
            IpcResult::ok(Some(summary))
        }
        Err(error) => IpcResult::err(error),
    }
}

/// 扫描一个已授权的根目录。
///
/// 规格 5.2：返回 `taskId`；后端记录 `scanId`，完成时任务结果返回 `scanId`。
///
/// 命令登记任务后立即返回 taskId，枚举在线程中进行；页面通过 `get_task`
/// 查询终态，并可用 `cancel_task` 请求停止后续枚举。
#[tauri::command]
pub fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    root_id: String,
    recursive: bool,
) -> IpcResult<String> {
    // 1) rootId -> 授权对象。换不回来就直接拒绝，绝不退化成「用传来的字符串当路径」。
    let Some(root) = state.root(&root_id) else {
        return IpcResult::err(AppError::new(
            codes::ROOT_NOT_AUTHORIZED,
            "该根目录未被授权或授权已失效，请重新选择文件夹",
        ));
    };

    let scan_guard = match state.begin_scan() {
        Ok(guard) => guard,
        Err(error) => return IpcResult::err(error),
    };

    // 2) 扫描参数来自设置，前端不能任意放大上限
    let settings = state.settings();
    let options = ScanOptions {
        recursive,
        max_files: settings.scan_max_files,
        max_depth: settings.scan_max_depth,
    };

    // 3) 建任务并立即发一次进度，让界面知道任务已开始
    let task_id = state.start_task(None);
    let _ = app.emit(
        TASK_PROGRESS_CHANNEL,
        TaskProgress {
            task_id: task_id.clone(),
            seq: 1,
            status: TaskStatus::Running,
            processed: 0,
            total: None,
        },
    );

    let cancellation = state
        .cancellation_token(&task_id)
        .expect("刚登记的任务必须带取消令牌");
    let returned_task_id = task_id.clone();

    // 4) 后台枚举；命令立即把 taskId 返回前端，页面用 get_task 恢复状态。
    std::thread::spawn(move || {
        let _scan_guard = scan_guard;
        let state = app.state::<AppState>();
        let outcome = scan_cancellable(&root, &options, &|| cancellation.load(Ordering::Acquire));
        match outcome {
            Ok(None) => {
                state.finish_cancelled(&task_id);
                let _ = app.emit(
                    TASK_PROGRESS_CHANNEL,
                    TaskProgress {
                        task_id,
                        seq: 2,
                        status: TaskStatus::Cancelled,
                        processed: 0,
                        total: None,
                    },
                );
            }
            Err(error) => {
                state.fail_task(&task_id, TaskError::from(&error));
                let _ = app.emit(
                    TASK_PROGRESS_CHANNEL,
                    TaskProgress {
                        task_id,
                        seq: 2,
                        status: TaskStatus::Failed,
                        processed: 0,
                        total: None,
                    },
                );
            }
            Ok(Some(outcome)) => {
                // 先持久化，再改内存：界面看到「扫描完成」时库里一定已经有记录，
                // create_plan 才不会撞 scans 的外键。
                //
                // scanId 必须用 outcome 里那一个 —— 内存与数据库必须是同一个 id，
                // 否则计划挂在库里的 scan 上，而界面拿的是另一个。
                let persisted_scan_id = outcome.scan_id.to_string();
                let truncated_flag = outcome.truncated;
                let records_for_db = outcome.records.clone();
                let now_ms = crate::safety::confirmation::now_unix_ms();
                let db = state.db();

                let persisted = crate::storage::repositories::insert_scan(
                    db,
                    &crate::storage::repositories::ScanRow {
                        id: persisted_scan_id.clone(),
                        root_id: root_id.clone(),
                        status: if truncated_flag {
                            "partial"
                        } else {
                            "completed"
                        }
                        .to_owned(),
                        recursive: options.recursive,
                        started_at: crate::domain::time::rfc3339_from_unix_ms(now_ms),
                    },
                )
                .and_then(|()| {
                    crate::storage::repositories::insert_files(db, &records_for_db).map(|_| ())
                })
                .and_then(|()| {
                    crate::storage::repositories::finish_scan(
                        db,
                        &persisted_scan_id,
                        if truncated_flag {
                            "partial"
                        } else {
                            "completed"
                        },
                        &crate::domain::time::rfc3339_from_unix_ms(now_ms),
                        truncated_flag,
                    )
                });

                if let Err(error) = persisted {
                    state.fail_task(&task_id, TaskError::from(&error));
                    state.finish_cancelled(&task_id);
                    let _ = app.emit(
                        TASK_PROGRESS_CHANNEL,
                        TaskProgress {
                            task_id,
                            seq: 2,
                            status: TaskStatus::Failed,
                            processed: 0,
                            total: None,
                        },
                    );
                    return;
                }

                let (scan_id, total, usable, skipped, truncated) = state.store_scan(outcome);
                let summary = ScanSummary {
                    task_id: task_id.clone(),
                    scan_id: scan_id.clone(),
                    root_id,
                    total,
                    usable,
                    skipped,
                    truncated,
                };
                state.finish_task(&task_id, &scan_id, &summary);
                let status = if truncated {
                    TaskStatus::Partial
                } else {
                    TaskStatus::Completed
                };
                let _ = app.emit(
                    TASK_PROGRESS_CHANNEL,
                    TaskProgress {
                        task_id,
                        seq: 2,
                        status,
                        processed: total,
                        total: Some(total),
                    },
                );
            }
        }
    });

    IpcResult::ok(returned_task_id)
}

/// 请求取消扫描。返回当前状态；真正停止由后台任务确认。
#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> IpcResult<Option<TaskSummary>> {
    let _ = state.request_cancel(&task_id);
    IpcResult::ok(state.task(&task_id))
}

/// 查询任务状态。
///
/// 规格 5.2：页面切换不得终止任务，重新打开界面要能用 `taskId` 查回来。
///
/// 返回 `Option`：查不到不是「错误」，而是「这个 id 在本会话里不存在」
/// （最常见的原因是应用重启后旧界面还在引用上一个会话的 taskId）。
/// 硬塞一个错误码会让前端拿到语义不符的 `FILE_BUSY`
/// 并据此显示「文件被占用」这类误导性提示。
#[tauri::command]
pub fn get_task(state: State<'_, AppState>, task_id: String) -> IpcResult<Option<TaskSummary>> {
    IpcResult::ok(state.task(&task_id))
}

/// 分页读取扫描到的文件。
///
/// 规格 5.2：`limit` 1–200；返回下一页 cursor。
/// 与 `get_task` 同理，scanId 不存在返回 `None` 而不是编造错误码。
#[tauri::command]
pub fn list_files(
    state: State<'_, AppState>,
    scan_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
) -> IpcResult<Option<FilePage>> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    IpcResult::ok(state.page(&scan_id, cursor.as_deref(), limit))
}
