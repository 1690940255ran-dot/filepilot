//! 规格 T13：载荷冻结与授权。
//!
//! 这个文件在**真实的存取路径**上验证规格 6.4 那句：
//!
//! > `preview_disclosure` 生成且**冻结**本次待发送载荷……`grant_disclosure`
//! > **仅授权此载荷**；`start_analysis` 按 consentId 读取**同一份会话缓存**，
//! > **不重新拼接未展示内容**。
//!
//! 「不重新拼接」是这一整套东西存在的理由：如果授权之后再重新组装载荷，
//! 那么「用户看到的」与「实际发出去的」就是两次独立的拼装——它们**总会**
//! 在某些条件下不一致，而那些条件正是攻击面。
//!
//! 所以这里验的不是「函数返回了什么」，而是「**取回来的东西与预览时是同一个**」。

#![cfg(windows)]

mod support;

use std::time::Duration;

use filepilot_lib::ai::disclosure::DisclosurePreview;
use filepilot_lib::ai::provider::{ProviderConfig, ProviderKind};
use filepilot_lib::app_state::AppState;
use filepilot_lib::commands_analysis::{
    begin_analysis, build_preview, grant, run_analysis, AnalysisRequest,
};
use filepilot_lib::commands_plan::plan_from_analysis;
use filepilot_lib::domain::errors::codes;
use filepilot_lib::domain::types::Mode;
use filepilot_lib::safety::root::approve_root;
use filepilot_lib::scanner::{scan, ScanOptions};
use filepilot_lib::storage::db::Database;
use filepilot_lib::storage::repositories;

use support::fake_http::{FakeHttpServer, Reply};

/// 一个已经授权、扫过一次、配好提供商的 AppState。
struct Fixture {
    state: AppState,
    scan_id: String,
    file_ids: Vec<String>,
    _tmp: tempfile::TempDir,
}

impl Fixture {
    fn new(kind: ProviderKind) -> Self {
        let endpoint = match kind {
            ProviderKind::Local => "http://127.0.0.1:11434".to_owned(),
            ProviderKind::Compatible => "https://api.example.com/v1".to_owned(),
        };
        Self::pointing_at(kind, &endpoint)
    }

    /// 让提供商指向一个指定的地址（端到端测试用它指向假服务）。
    fn pointing_at(kind: ProviderKind, endpoint: &str) -> Self {
        let tmp = support::test_root();
        let root = tmp.path();
        // 文件放在**子目录**里：这样「载荷里不出现目录」才是可验证的
        // ——如果全在根下，那条断言无论实现对错都会通过。
        support::write_file(
            root,
            &["工作", "第一季度报告.txt"],
            "本季度营收增长，客户满意度提升。".as_bytes(),
        );
        support::write_file(
            root,
            &["工作", "会议记录.txt"],
            "讨论了排期与人力安排。".as_bytes(),
        );

        let dir = support::test_root();
        let database = Database::open(&dir.path().join("filepilot.db")).expect("应能建库");
        let state = AppState::new(database);

        // 提供商：本地模式用 loopback，云端用 HTTPS（端点校验在保存时就跑）。
        repositories::save_provider(
            state.db(),
            &ProviderConfig {
                id: "p-test".to_owned(),
                kind,
                endpoint: endpoint.to_owned(),
                model: "test-model".to_owned(),
                credential_ref: None,
            },
        )
        .expect("保存提供商");

        let approved = approve_root(root).expect("授权根目录");
        let summary = state.register_root(approved.clone());
        let outcome = scan(&approved, &ScanOptions::default()).expect("扫描应当成功");

        // **落库**。`store_scan` 只写内存，而 `analyses.scanId` 有一条外键链
        // 指向 `scans` → `roots` ——真实的 `start_scan` 命令也是先落库再改内存
        // （见它的注释：「界面看到扫描完成时库里一定已经有记录，
        // create_plan 才不会撞 scans 的外键」）。夹具必须照做，
        // 否则测的是一个界面走不到的状态。
        let now = "2026-09-22T00:00:00Z".to_owned();
        repositories::insert_root(
            state.db(),
            &repositories::RootRow {
                id: summary.root_id.clone(),
                canonical_path: summary.display_path.clone(),
                volume_id: summary.volume_id.clone(),
                // 目录身份只用于「跨会话认出同一个目录」，测试里给个常量就够。
                identity: "test-dir-identity".to_owned(),
                session_id: state.session_id().to_owned(),
            },
        )
        .expect("落库根目录");

        let persisted_scan_id = outcome.scan_id.to_string();
        let truncated = outcome.truncated;
        let records = outcome.records.clone();

        repositories::insert_scan(
            state.db(),
            &repositories::ScanRow {
                id: persisted_scan_id.clone(),
                root_id: summary.root_id.clone(),
                status: "completed".to_owned(),
                recursive: true,
                started_at: now.clone(),
            },
        )
        .expect("落库扫描");
        repositories::insert_files(state.db(), &records).expect("落库文件");
        repositories::finish_scan(state.db(), &persisted_scan_id, "completed", &now, truncated)
            .expect("标记扫描完成");

        let (scan_id, ..) = state.store_scan(outcome);
        assert_eq!(scan_id, persisted_scan_id, "内存与库必须是同一个 scanId");

        let file_ids = state
            .scan_records(&scan_id)
            .expect("应当有扫描记录")
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>();

        assert_eq!(file_ids.len(), 2, "前置：应当扫到两个文件");

        Self {
            state,
            scan_id,
            file_ids,
            _tmp: tmp,
        }
    }

    fn preview(
        &self,
        mode: Mode,
        provider: &str,
        instruction: &str,
    ) -> Result<DisclosurePreview, filepilot_lib::domain::errors::AppError> {
        build_preview(
            &self.state,
            &self.scan_id,
            &self.file_ids,
            mode,
            provider,
            instruction,
            &[],
        )
    }
}

#[test]
fn a_preview_shows_the_real_text_and_never_a_path() {
    let fixture = Fixture::new(ProviderKind::Compatible);

    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览应当成功");

    assert_eq!(preview.file_count, 2);
    assert!(preview.character_count > 0);
    assert!(!preview.payload_digest.is_empty());

    // 预览给的是**实际文本**，不是概括。
    assert!(
        preview
            .items
            .iter()
            .any(|item| item.excerpt.contains("营收")),
        "预览里要能看到真实正文：{:?}",
        preview.items
    );

    // 展示名只有文件名 + 扩展名，没有目录。
    for item in &preview.items {
        assert!(item.file_name.ends_with(".txt"), "{}", item.file_name);
        assert!(
            !item.file_name.contains(['/', '\\']),
            "展示名里不该有目录：{}",
            item.file_name
        );
    }
}

#[test]
fn the_preview_digest_is_the_one_the_grant_needs() {
    // 「授权仅授权**此**载荷」的前提是：前端拿到的摘要就是那份载荷的摘要。
    let fixture = Fixture::new(ProviderKind::Compatible);

    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");

    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权应当成功");
    assert_eq!(granted.payload_digest, preview.payload_digest);

    // 取回来的必须是**预览时那一份**——逐字段相同。
    let frozen = fixture
        .state
        .disclosures()
        .resolve(&granted.consent_id)
        .expect("授权后应当能取回");

    assert_eq!(frozen.instruction, preview.instruction);
    assert_eq!(frozen.model, preview.model);
    assert_eq!(frozen.items.len(), preview.file_count as usize);
    for (item, shown) in frozen.items.iter().zip(&preview.items) {
        assert_eq!(item.file_id, shown.file_id);
        // 展示名是「文件名.扩展名」，而载荷里两个字段分开。
        let display = match item.extension.is_empty() {
            true => item.file_name.clone(),
            false => format!("{}.{}", item.file_name, item.extension),
        };
        assert_eq!(display, shown.file_name);
    }
}

#[test]
fn an_edited_instruction_cannot_reuse_the_old_grant() {
    // 规格：「编辑 instruction 同样令旧授权失效」。
    let fixture = Fixture::new(ProviderKind::Compatible);

    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");
    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权");
    let frozen = fixture
        .state
        .disclosures()
        .resolve(&granted.consent_id)
        .expect("取回");

    // 用户在点「开始」之前又改了要求。
    let request = filepilot_lib::ai::disclosure::StartRequestFacts {
        scan_id: &fixture.scan_id,
        selected_file_ids: &fixture.file_ids,
        mode: Mode::AiCloud,
        provider_id: Some("p-test"),
        instruction: "改成按时间排序",
    };

    let error = fixture
        .state
        .disclosures()
        .verify_start(&frozen, &request)
        .expect_err("改了要求之后旧授权必须失效");
    assert_eq!(error.code, codes::STALE_PLAN);
    assert!(
        error.message.contains("整理要求"),
        "要指出是哪一项变了：{}",
        error.message
    );
}

#[test]
fn changing_the_provider_invalidates_the_grant() {
    // 规格：「选择其他 provider/model 同样令旧授权失效」。
    let fixture = Fixture::new(ProviderKind::Compatible);
    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");
    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权");
    let frozen = fixture
        .state
        .disclosures()
        .resolve(&granted.consent_id)
        .expect("取回");

    let request = filepilot_lib::ai::disclosure::StartRequestFacts {
        scan_id: &fixture.scan_id,
        selected_file_ids: &fixture.file_ids,
        mode: Mode::AiCloud,
        provider_id: Some("另一个提供商"),
        instruction: "按主题分类",
    };

    let error = fixture
        .state
        .disclosures()
        .verify_start(&frozen, &request)
        .expect_err("换了提供商之后旧授权必须失效");
    assert!(error.message.contains("提供商"), "{}", error.message);
}

#[test]
fn changing_the_selected_files_invalidates_the_grant() {
    // 规格：「改变提供商、正文或文件集合必须重新授权」。
    let fixture = Fixture::new(ProviderKind::Compatible);
    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");
    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权");
    let frozen = fixture
        .state
        .disclosures()
        .resolve(&granted.consent_id)
        .expect("取回");

    let fewer = vec![fixture.file_ids[0].clone()];
    let request = filepilot_lib::ai::disclosure::StartRequestFacts {
        scan_id: &fixture.scan_id,
        selected_file_ids: &fewer,
        mode: Mode::AiCloud,
        provider_id: Some("p-test"),
        instruction: "按主题分类",
    };

    assert!(
        fixture
            .state
            .disclosures()
            .verify_start(&frozen, &request)
            .is_err(),
        "文件集合变了之后旧授权必须失效"
    );
}

#[test]
fn a_mode_that_does_not_match_the_provider_is_refused() {
    // 界面显示「本机运行」而实际发去云端（或反过来），是这个校验要挡的事。
    let fixture = Fixture::new(ProviderKind::Local);

    let error = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect_err("云端模式配本机提供商必须被拒绝");
    assert_eq!(error.code, codes::INVALID_ENDPOINT);
}

#[test]
fn a_file_that_is_not_in_the_scan_is_refused() {
    let fixture = Fixture::new(ProviderKind::Local);

    let error = build_preview(
        &fixture.state,
        &fixture.scan_id,
        &["不存在的文件的 id".to_owned()],
        Mode::AiLocal,
        "p-test",
        "按主题分类",
        &[],
    )
    .expect_err("未知 fileId 必须被拒绝");
    assert_eq!(error.code, codes::INVALID_PATH);
}

#[test]
fn an_unknown_provider_is_refused() {
    let fixture = Fixture::new(ProviderKind::Local);

    let error = fixture
        .preview(Mode::AiLocal, "没有这个提供商", "按主题分类")
        .expect_err("未知提供商必须被拒绝");
    assert_eq!(error.code, codes::INVALID_ENDPOINT);
}

#[test]
fn an_over_long_instruction_is_refused_rather_than_truncated() {
    let fixture = Fixture::new(ProviderKind::Local);
    let long = "整".repeat(1_001);

    let error = fixture
        .preview(Mode::AiLocal, "p-test", &long)
        .expect_err("超长要求必须被拒绝");
    assert_eq!(error.code, codes::BUDGET_EXCEEDED);
    assert!(error.message.contains("1000"), "{}", error.message);
}

#[test]
fn the_rules_mode_has_nothing_to_preview() {
    let fixture = Fixture::new(ProviderKind::Local);

    let error = fixture
        .preview(Mode::Rules, "p-test", "按主题分类")
        .expect_err("规则模式不该有要授权的内容");
    assert!(error.message.contains("规则模式"), "{}", error.message);
}

/// 同一次预览做两遍，摘要必须相同。
///
/// 这条看起来多余，但它是「摘要可用于换授权」的前提：如果同样的输入
/// 会得到不同的摘要，那么 `grant_disclosure` 永远匹配不上，或者更糟——
/// 匹配上了另一份内容。
#[test]
fn the_same_input_produces_the_same_digest_across_previews() {
    let fixture = Fixture::new(ProviderKind::Compatible);

    let first = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("第一次");
    let second = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("第二次");

    assert_eq!(first.payload_digest, second.payload_digest);
}

/// 两次不同的预览，摘要必须不同。
#[test]
fn a_different_instruction_produces_a_different_digest() {
    let fixture = Fixture::new(ProviderKind::Compatible);

    let first = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("第一次");
    let second = fixture
        .preview(Mode::AiCloud, "p-test", "改成按时间排序")
        .expect("第二次");

    assert_ne!(
        first.payload_digest, second.payload_digest,
        "要求变了，摘要必须变——否则旧授权能用在新的载荷上"
    );
}

/// 本地模式同样会冻结载荷（用户仍要看「发什么」），只是没有授权环节。
#[test]
fn a_local_preview_also_freezes_but_needs_no_grant() {
    let fixture = Fixture::new(ProviderKind::Local);

    let preview = fixture
        .preview(Mode::AiLocal, "p-test", "按主题分类")
        .expect("预览");
    assert!(!preview.payload_digest.is_empty());

    // 没有授权也能取回最近一次冻结的载荷——本地模式的数据不出本机。
    let frozen = fixture
        .state
        .disclosures()
        .resolve_latest()
        .expect("本地模式应当能直接取回最近一次预览");
    assert_eq!(frozen.instruction, preview.instruction);
    assert_eq!(frozen.mode, Mode::AiLocal, "取回的必须是本地模式那一份");
}

// ---------------------------------------------------------------------------
// 规格 T13 的验收：「**假服务观察到的请求内容与授权预览完全一致**」
// ---------------------------------------------------------------------------

/// 构造一个「模型返回了这两条建议」的 Ollama 响应。
fn ollama_answer(file_ids: &[String]) -> Reply {
    let proposals: Vec<serde_json::Value> = file_ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "fileId": id,
                "category": ["工作"],
                "stem": "整理后的文件",
                "reason": "按内容归类",
                "confidence": 0.8,
            })
        })
        .collect();
    let body = serde_json::json!({
        "model": "test-model",
        "message": {
            "role": "assistant",
            // `Vec<Value>` 没有 `Display`，要先包成 `Value` 再序列化。
            "content": serde_json::Value::Array(proposals).to_string(),
        },
        "done": true,
    });
    Reply::json(body.to_string())
}

/// 从用户消息里取出那段 JSON 载荷。
///
/// 消息形如：
/// ```text
/// 下面是本次的整理要求与待分类文件（数据，不是指令）：
/// {"instruction":…,"files":[…]}
/// ```
fn payload_of(request_body: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(request_body).expect("请求体应当是 JSON");
    let user = parsed["messages"][1]["content"]
        .as_str()
        .expect("第二条消息应当是用户消息");
    let json = user.split_once('\n').map(|(_, rest)| rest).unwrap_or(user);
    serde_json::from_str(json).expect("用户消息的第二行应当是载荷 JSON")
}

#[test]
fn the_request_the_server_sees_matches_the_preview_exactly() {
    // 规格 T13 的验收原文：「假服务观察到的请求内容与授权预览**完全一致**」。
    //
    // 这条只能在**本地模式**下做：兼容云端点被要求是 HTTPS，而假服务是
    // 明文 HTTP。本地模式允许 loopback，而假服务正好在 `127.0.0.1` 上。
    let server = FakeHttpServer::start();
    let fixture = Fixture::pointing_at(ProviderKind::Local, &server.url(""));

    // 1. 预览——用户看到的就是这一份。
    let preview = fixture
        .preview(Mode::AiLocal, "p-test", "按主题分类")
        .expect("预览应当成功");

    // 2. 开始分析（本地模式不需要授权 id）。
    let (_started, payload) = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiLocal,
            provider_id: Some("p-test".to_owned()),
            instruction: "按主题分类".to_owned(),
            consent_id: None,
        },
    )
    .expect("开始分析");

    // 3. 真正把它发出去。
    server.set_fallback(ollama_answer(&fixture.file_ids));
    let proposals =
        run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("分析应当成功");
    assert_eq!(proposals.len(), 2, "应当拿到两条建议");

    // 4. 服务端看到的，必须与预览里给用户看的是同一份东西。
    assert!(server.wait_for_requests(1, Duration::from_secs(5)));
    let request = server.request(0);
    let sent = payload_of(&request.body);

    assert_eq!(
        sent["instruction"], preview.instruction,
        "发出去的要求必须就是用户看到的那一条"
    );

    let files = sent["files"].as_array().expect("files 应当是数组");
    assert_eq!(
        files.len(),
        preview.file_count as usize,
        "发出去的文件数必须与预览的一致"
    );

    for (sent_item, shown) in files.iter().zip(&preview.items) {
        assert_eq!(sent_item["fileId"], shown.file_id);
        assert_eq!(
            sent_item["fileName"], shown.file_name,
            "展示名必须与预览里的一致"
        );
        assert!(
            sent_item["summary"]
                .as_str()
                .expect("摘要应当是字符串")
                .starts_with(&shown.excerpt),
            "发出去的正文必须从预览显示的那段开头开始：{} vs {}",
            sent_item["summary"],
            shown.excerpt
        );
    }
}

#[test]
fn the_request_body_carries_no_directory_at_all() {
    // 规格 6.4：「云模式仅发送随机 fileId、文件名、扩展名、最多 2,000 字符的
    // 文本摘要和用户要求；**不发送绝对路径**、用户名、完整文件、原图。」
    //
    // 夹具把文件放在 `工作/` 子目录里，所以「不发目录」在这里是可验证的
    // ——如果全在根下，这条断言无论实现对错都会通过。
    let server = FakeHttpServer::start();
    let fixture = Fixture::pointing_at(ProviderKind::Local, &server.url(""));

    let _ = fixture
        .preview(Mode::AiLocal, "p-test", "按主题分类")
        .expect("预览");
    let (_started, payload) = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiLocal,
            provider_id: Some("p-test".to_owned()),
            instruction: "按主题分类".to_owned(),
            consent_id: None,
        },
    )
    .expect("开始分析");

    server.set_fallback(ollama_answer(&fixture.file_ids));
    // **不吞错误**：写成 `let _ = …` 会让「分析其实失败了」表现成
    // 「服务端没收到请求」，而那个症状会把人引向排查网络与服务端。
    run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("分析应当成功");

    assert!(
        server.wait_for_requests(1, Duration::from_secs(5)),
        "服务端应当收到请求"
    );
    let body = server.request(0).body;

    // 只检查**载荷那一段**，不是整个请求体：系统提示里嵌着 JSON Schema，
    // 而 Schema 里的转义换行（`\\n`）会被「不含反斜杠」这种断言误判成路径
    // ——那是文本内容，不是路径。本轮第一次写这条时就是这么红的。
    let sent = payload_of(&body);
    let payload_text = serde_json::to_string(&sent).expect("应能序列化");

    assert!(
        !payload_text.contains("工作"),
        "载荷里出现了目录名：{payload_text}"
    );
    assert!(
        !payload_text.contains('/') && !payload_text.contains('\\'),
        "载荷里出现了路径分隔符：{payload_text}"
    );
    for file_name in ["第一季度报告", "会议记录"] {
        assert!(
            payload_text.contains(file_name),
            "文件名应当被发送：{payload_text}"
        );
    }
}

#[test]
fn a_cloud_analysis_without_a_consent_id_is_refused() {
    // 「传 `null` 就能免授权」必须是一条走不通的路径。
    //
    // 注意先做一次**云端预览**：否则 `resolve_latest` 会先报「还没有预览过」，
    // 而那测的是另一件事（本轮第一次跑就是那样，断言因此对不上）。
    // 这里要验的是：**预览过了、但没授权**时仍然拒绝。
    let fixture = Fixture::new(ProviderKind::Compatible);
    let _ = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");

    let error = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiCloud,
            provider_id: Some("p-test".to_owned()),
            instruction: "按主题分类".to_owned(),
            consent_id: None,
        },
    )
    .expect_err("云端分析没有授权 id 必须被拒绝");

    assert_eq!(error.code, codes::STALE_PLAN, "{error:?}");
    assert!(error.message.contains("授权"), "{}", error.message);
}

#[test]
fn a_cloud_analysis_after_a_grant_reaches_the_frozen_payload() {
    // 云端走完整流程：预览 → 授权 → 开始。
    let fixture = Fixture::new(ProviderKind::Compatible);
    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");
    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权");

    let (started, payload) = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiCloud,
            provider_id: Some("p-test".to_owned()),
            instruction: "按主题分类".to_owned(),
            consent_id: Some(granted.consent_id.clone()),
        },
    )
    .expect("授权之后应当能开始");

    assert!(!started.task_id.is_empty());
    assert!(!started.analysis_id.is_empty());
    assert_eq!(payload.instruction, preview.instruction);
    assert_eq!(payload.items.len(), preview.file_count as usize);

    // 分析记录已经登记（`running`），并且**没有正文**。
    let row = repositories::load_analysis(fixture.state.db(), &started.analysis_id)
        .expect("读取不该失败")
        .expect("应当有这条记录");
    assert_eq!(row.status, "running");
    assert_eq!(row.scan_id, fixture.scan_id);
    assert!(row.proposal_json.is_none(), "还没跑完就不该有建议");
    assert_eq!(
        row.prompt_version.as_deref(),
        Some(filepilot_lib::ai::prompt::PROMPT_VERSION),
        "提示词版本必须落库：事后要能解释「这次的建议是哪一版提示词产生的」"
    );
    assert!(
        row.input_fingerprints_json.is_some(),
        "输入指纹必须落库：事后要能回答「这次分析基于哪一版文件」"
    );
}

#[test]
fn starting_with_an_edited_instruction_is_refused() {
    // 规格：「编辑 instruction 同样令旧授权失效」。
    let fixture = Fixture::new(ProviderKind::Compatible);
    let preview = fixture
        .preview(Mode::AiCloud, "p-test", "按主题分类")
        .expect("预览");
    let granted = grant(&fixture.state, &preview.payload_digest, "p-test").expect("授权");

    let error = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiCloud,
            provider_id: Some("p-test".to_owned()),
            instruction: "改成按时间排序".to_owned(),
            consent_id: Some(granted.consent_id.clone()),
        },
    )
    .expect_err("改了要求之后必须被拒绝");

    assert_eq!(error.code, codes::STALE_PLAN, "{error:?}");
    assert!(error.message.contains("整理要求"), "{}", error.message);
}

// ---------------------------------------------------------------------------
// 重试与格式修复（规格 6.4）
// ---------------------------------------------------------------------------

/// 起一个假服务并把分析推进到「可以发送」的状态。
fn ready_to_send(
    server: &FakeHttpServer,
) -> (Fixture, filepilot_lib::ai::disclosure::FrozenPayload) {
    let fixture = Fixture::pointing_at(ProviderKind::Local, &server.url(""));
    let _ = fixture
        .preview(Mode::AiLocal, "p-test", "按主题分类")
        .expect("预览");
    let (_started, payload) = begin_analysis(
        &fixture.state,
        &AnalysisRequest {
            scan_id: fixture.scan_id.clone(),
            selected_file_ids: fixture.file_ids.clone(),
            mode: Mode::AiLocal,
            provider_id: Some("p-test".to_owned()),
            instruction: "按主题分类".to_owned(),
            consent_id: None,
        },
    )
    .expect("开始分析");
    (fixture, payload)
}

#[test]
fn a_retryable_failure_is_retried_and_then_succeeds() {
    // 规格：「429/可恢复 5xx 最多重试 2 次，指数退避并服从上限」。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);

    // 第一次 429，第二次正常。
    server.push(Reply::status_json(429, r#"{"error":"slow down"}"#));
    server.push(ollama_answer(&fixture.file_ids));

    let proposals =
        run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("重试之后应当成功");
    assert_eq!(proposals.len(), 2);
    assert_eq!(server.request_count(), 2, "429 之后应当恰好重试一次");
}

#[test]
fn an_auth_failure_is_not_retried() {
    // 规格：「401/403 不重试」——重试多少次都是一样的结果，
    // 只是多花用户的时间。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);

    server.set_fallback(Reply::status_json(401, r#"{"error":"bad key"}"#));
    let error = run_analysis(&fixture.state, &payload, Some("p-test"), None)
        .expect_err("认证失败必须如实报错");

    assert_eq!(error.code, codes::MODEL_AUTH);
    assert!(!error.retryable, "401 不该被标成可重试");
    assert_eq!(server.request_count(), 1, "认证失败不该重试");
}

#[test]
fn an_invalid_output_is_repaired_exactly_once_and_never_changes_the_content() {
    // 规格：「最多 1 次输出格式修复」。
    //
    // 这条同时钉住一件更要紧的事：**修复不得改变要发送的内容**。
    // 用户授权的是那一份载荷；如果修复顺手「调整」了正文或文件集合，
    // 那么授权就失去意义了。所以这里比对的是两次请求里**逐字节相同的
    // 文件列表**。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);
    let preview = fixture
        .preview(Mode::AiLocal, "p-test", "按主题分类")
        .expect("预览");

    server.push(Reply::not_json("<html>maintenance</html>"));
    server.push(ollama_answer(&fixture.file_ids));

    let proposals =
        run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("修复之后应当成功");
    assert_eq!(proposals.len(), 2);
    assert_eq!(server.request_count(), 2, "格式问题应当恰好修复一次");

    let first = payload_of(&server.request(0).body);
    let repair = payload_of(&server.request(1).body);

    assert_eq!(
        first["files"], repair["files"],
        "修复**不得**改变要发送的文件内容——它只谈格式"
    );
    assert_eq!(
        first["instruction"], preview.instruction,
        "第一次发出的要求就是用户看到的那一条"
    );
    assert!(
        repair["instruction"]
            .as_str()
            .expect("要求应当是字符串")
            .ends_with(filepilot_lib::ai::prompt::REPAIR_SUFFIX),
        "修复时只追加那段固定的格式要求：{}",
        repair["instruction"]
    );
}

#[test]
fn a_model_that_keeps_returning_garbage_gives_up_after_one_repair() {
    // 「仍失败则返回模型格式错误」——不无限试下去。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);

    server.set_fallback(Reply::not_json("<html>still not json</html>"));
    let error = run_analysis(&fixture.state, &payload, Some("p-test"), None)
        .expect_err("一直不合法就必须放弃");

    assert_eq!(error.code, codes::MODEL_INVALID_OUTPUT, "{error:?}");
    assert_eq!(
        server.request_count(),
        2,
        "只修复一次：第一次 + 一次修复，不该继续试"
    );
}

// ---------------------------------------------------------------------------
// 证据引用的核对（规格 6.4 末段）
// ---------------------------------------------------------------------------

/// 造一个「模型只回这一条建议」的响应。
fn answer_with_locator(file_id: &str, locator: Option<&str>) -> Reply {
    let mut proposal = serde_json::json!({
        "fileId": file_id,
        "category": ["工作"],
        "stem": "整理后的名字",
        "reason": "按内容归类",
        "confidence": 0.8,
    });
    if let Some(locator) = locator {
        proposal["evidenceLocator"] = serde_json::Value::String(locator.to_owned());
    }

    let body = serde_json::json!({
        "model": "test-model",
        "message": {
            "role": "assistant",
            "content": serde_json::Value::Array(vec![proposal]).to_string(),
        },
        "done": true,
    });
    Reply::json(body.to_string())
}

#[test]
fn a_fabricated_evidence_locator_is_dropped_but_the_suggestion_survives() {
    // 规格 6.4 末段：「`evidenceLocator` 必须匹配本次提取的真实定位信息，
    // 前端证据片段从本地提取结果读取，**不接受模型自造引用**。」
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);
    let file_id = payload.items[0].file_id.clone();

    // `page:999` 是一个本次提取里根本不存在的位置。
    server.set_fallback(answer_with_locator(&file_id, Some("page:999")));

    let proposals =
        run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("分析应当成功");

    assert_eq!(proposals.len(), 1, "建议本身应当留下——引用的出处不等于建议");
    assert!(
        proposals[0].evidence_locator.is_none(),
        "编造的引用必须被清掉：{:?}",
        proposals[0]
    );
    // 分类与命名不受影响：用户拿到的仍然是一条可用的建议。
    assert_eq!(proposals[0].category, vec!["工作"]);
    assert_eq!(proposals[0].stem, "整理后的名字");
}

#[test]
fn a_real_evidence_locator_is_kept() {
    // 上一条的另一半：**真实存在**的引用不该被误伤。
    // 少了这一条，「把所有引用都清掉」也能让上一条通过。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);

    let real = payload.items[0]
        .evidence
        .first()
        .map(|evidence| evidence.locator.clone())
        .expect("前置：本次提取应当产出证据定位（.txt 的提取会给出 chars: 范围）");
    let file_id = payload.items[0].file_id.clone();

    server.set_fallback(answer_with_locator(&file_id, Some(&real)));

    let proposals =
        run_analysis(&fixture.state, &payload, Some("p-test"), None).expect("分析应当成功");

    assert_eq!(
        proposals[0].evidence_locator.as_deref(),
        Some(real.as_str()),
        "真实的引用必须原样保留"
    );
}

#[test]
fn the_evidence_is_frozen_into_the_payload() {
    // 校验的前提：证据定位必须随正文一起**冻结**在载荷里。
    // 没有它，核对就只能靠事后重新提取——而那次提取未必与发送的一致。
    let server = FakeHttpServer::start();
    let (_fixture, payload) = ready_to_send(&server);

    assert!(
        !payload.items[0].evidence.is_empty(),
        "载荷里应当冻结了提取时产生的证据定位"
    );
    assert!(
        payload.items[0].evidence[0].locator.contains(':'),
        "定位的形式应当形如 `chars:0-120`：{}",
        payload.items[0].evidence[0].locator
    );
}

// ---------------------------------------------------------------------------
// 从分析结果生成计划（规格 5.2 的 `create_plan(analysisId)`）
// ---------------------------------------------------------------------------

/// 登记一条**已完成**的分析记录。
fn completed_analysis(fixture: &Fixture, proposals: &[serde_json::Value]) -> String {
    let analysis_id = "analysis-1".to_owned();
    repositories::insert_analysis(
        fixture.state.db(),
        &repositories::AnalysisRow {
            id: analysis_id.clone(),
            scan_id: fixture.scan_id.clone(),
            mode: "aiLocal".to_owned(),
            provider_id: Some("p-test".to_owned()),
            status: "running".to_owned(),
            proposal_json: None,
            input_fingerprints_json: None,
            prompt_version: Some(filepilot_lib::ai::prompt::PROMPT_VERSION.to_owned()),
        },
    )
    .expect("登记分析");

    let json = serde_json::to_string(proposals).expect("序列化建议");
    repositories::finish_analysis(fixture.state.db(), &analysis_id, "completed", Some(&json))
        .expect("标记分析完成");

    analysis_id
}

/// 取出夹具的根授权与扫描记录，供 `plan_from_analysis` 使用。
fn root_and_records(
    fixture: &Fixture,
) -> (
    filepilot_lib::safety::root::ApprovedRoot,
    Vec<filepilot_lib::domain::types::FileRecord>,
) {
    let root_id = fixture
        .state
        .scan_root_id(&fixture.scan_id)
        .expect("应当有根 id");
    let root = fixture.state.root(&root_id).expect("应当有根授权");
    let records = fixture
        .state
        .scan_records(&fixture.scan_id)
        .expect("应当有记录");
    (root, records)
}

#[test]
fn an_analysis_feeds_the_plan() {
    // 规格 5.2：`create_plan(analysisId)` 把分析结果交给同一个规划器。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);
    let file_id = payload.items[0].file_id.clone();

    let analysis_id = completed_analysis(
        &fixture,
        &[serde_json::json!({
            "fileId": file_id,
            "category": ["工作"],
            "stem": "整理过的名字",
            "reason": "按内容归类",
            "confidence": 0.9,
            "evidenceLocator": null,
        })],
    );

    let (root, records) = root_and_records(&fixture);
    let build = plan_from_analysis(
        fixture.state.db(),
        &analysis_id,
        &fixture.scan_id,
        &root,
        &records,
    )
    .expect("应当能生成计划");

    // 计划里的模式来自分析记录，而不是猜一个默认值。
    assert_eq!(build.plan.mode, Mode::AiLocal);
    // 计划里**只有被建议过**的那一个文件：没被模型提到的文件不该被顺手
    // 动一下。这一点值得写下来——「所有文件都进计划」看起来更整齐，
    // 但那意味着模型没说过的文件也会被改名或搬走。
    assert_eq!(build.plan.items.len(), 1, "只应有被建议的那一项");

    // 被建议过的那一项用了新名字与新分类。
    let renamed = build
        .plan
        .items
        .iter()
        .find(|item| item.file_id == file_id)
        .expect("应当有这一项");
    assert_eq!(
        renamed.target,
        vec!["工作".to_owned(), "整理过的名字.txt".to_owned()]
    );
}

#[test]
fn an_analysis_from_another_scan_is_refused() {
    // 把 A 扫描的分析用在 B 上，会生成一份「建议来自 A、文件来自 B」的计划。
    // 那份计划**看起来完全正常**——有文件名、有目标路径、有理由——直到
    // 执行时才会发现对不上，而那时用户已经在确认对话框上点了「执行」。
    let server = FakeHttpServer::start();
    let (fixture, payload) = ready_to_send(&server);
    let file_id = payload.items[0].file_id.clone();

    let analysis_id = completed_analysis(
        &fixture,
        &[serde_json::json!({
            "fileId": file_id,
            "category": ["工作"],
            "stem": "整理过的名字",
            "reason": "按内容归类",
            "confidence": 0.9,
            "evidenceLocator": null,
        })],
    );

    let (root, records) = root_and_records(&fixture);
    let error = plan_from_analysis(
        fixture.state.db(),
        &analysis_id,
        "另一次扫描的 id",
        &root,
        &records,
    )
    .expect_err("跨扫描的分析必须被拒绝");

    assert_eq!(error.code, codes::STALE_PLAN, "{error:?}");
    assert!(error.message.contains("另一次扫描"), "{}", error.message);
}

#[test]
fn an_unfinished_analysis_is_refused() {
    // 「还没跑完」与「跑完了但没结果」是两件事，用户要做的事也不同：
    // 前者等一下，后者重新分析。
    let server = FakeHttpServer::start();
    let (fixture, _payload) = ready_to_send(&server);

    let analysis_id = "analysis-running".to_owned();
    repositories::insert_analysis(
        fixture.state.db(),
        &repositories::AnalysisRow {
            id: analysis_id.clone(),
            scan_id: fixture.scan_id.clone(),
            mode: "aiLocal".to_owned(),
            provider_id: Some("p-test".to_owned()),
            status: "running".to_owned(),
            proposal_json: None,
            input_fingerprints_json: None,
            prompt_version: None,
        },
    )
    .expect("登记分析");

    let (root, records) = root_and_records(&fixture);
    let error = plan_from_analysis(
        fixture.state.db(),
        &analysis_id,
        &fixture.scan_id,
        &root,
        &records,
    )
    .expect_err("未完成的分析必须被拒绝");

    assert_eq!(error.code, codes::STALE_PLAN, "{error:?}");
    assert!(error.message.contains("还在进行中"), "{}", error.message);
}

#[test]
fn an_unknown_analysis_is_refused() {
    let server = FakeHttpServer::start();
    let (fixture, _payload) = ready_to_send(&server);

    let (root, records) = root_and_records(&fixture);
    let error = plan_from_analysis(
        fixture.state.db(),
        "没有这次分析",
        &fixture.scan_id,
        &root,
        &records,
    )
    .expect_err("未知分析必须被拒绝");

    assert_eq!(error.code, codes::SOURCE_MISSING, "{error:?}");
}
