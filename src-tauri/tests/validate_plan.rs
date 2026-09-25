//! T04 验收：确定性计划生成与计划校验（规格 7.1、7.2、10.1 的 N02–N05）。
//!
//! 这些测试全部跑在真实 NTFS 的临时根目录上（`support::test_root` 建的目录
//! 位于用户主目录下的普通子目录，能通过规格 1.2 的根授权拒绝清单）。
//!
//! 断言的重点不是「计划能生成」，而是三类容易做错的事：
//! 1. **确定性**：同一输入必须得到同一目标方案；
//! 2. **不越界、不覆盖**：冲突要分配新名，最终确认后出现的新冲突要报错而不是改名；
//! 3. **只读**：生成计划前后文件树必须逐字节不变（INV-01）。

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use filepilot_lib::domain::errors::codes;
use filepilot_lib::domain::types::{
    Issue, Mode, Plan, PlanAction, PlanItem, PlanItemOrigin, PlanStatus, Proposal, RelPath, Risk,
};
use filepilot_lib::planner::naming::TargetLedger;
use filepilot_lib::planner::{
    build_plan, build_rule_plan, compute_digest, validate_plan, PlanRequest,
};
use filepilot_lib::rules::RuleKind;
use filepilot_lib::safety::confirmation::{ConfirmationStore, TOKEN_TTL_MS};
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};
use filepilot_lib::scanner::{scan, ScanOptions};

/// 建一个可通过根授权的临时根目录。
fn make_root() -> (tempfile::TempDir, ApprovedRoot) {
    let tmp = support::test_root();
    let approved = approve_root(tmp.path()).expect("临时根应被授权");
    (tmp, approved)
}

/// 扫描 + 生成规则计划。
fn rule_plan(root: &ApprovedRoot, kind: RuleKind) -> (Plan, Vec<Issue>) {
    let outcome = scan(root, &ScanOptions::default()).expect("扫描应成功");
    let build = build_rule_plan(
        root,
        &outcome.scan_id.to_string(),
        kind,
        &outcome.records,
        SystemTime::now(),
    )
    .expect("生成计划应成功");
    (build.plan, build.issues)
}

/// 把计划里的「源 → 目标」拍成有序表，便于断言与比较。
fn mapping(plan: &Plan) -> BTreeMap<String, String> {
    plan.items
        .iter()
        .map(|item| (item.source.join("\\"), item.target.join("\\")))
        .collect()
}

fn issue_codes(issues: &[Issue]) -> BTreeSet<String> {
    issues.iter().map(|issue| issue.code.clone()).collect()
}

/// 独立计算文件内容的 SHA-256，用于核对计划绑定的指纹。
///
/// 刻意**不**调用项目自己的哈希实现：两边算错会互相印证。
fn sha256_of(path: &std::path::Path) -> String {
    let bytes = fs::read(path).expect("读文件应成功");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

/// 取一个计划项（按源路径），不存在则 panic。
fn item_of<'a>(plan: &'a Plan, source: &str) -> &'a PlanItem {
    plan.items
        .iter()
        .find(|item| item.source.join("\\") == source)
        .unwrap_or_else(|| panic!("计划里应有源为 {source} 的项"))
}

// ---------------------------------------------------------------------------
// byType / byMonth
// ---------------------------------------------------------------------------

#[test]
fn by_type_plan_groups_known_extensions_and_keeps_unknown_in_other() {
    let (_tmp, root) = make_root();
    for name in ["a.pdf", "b.png", "c.mp3", "d.zip", "e.xyz"] {
        support::write_file(root.canonical(), &[name], b"x");
    }

    let (plan, issues) = rule_plan(&root, RuleKind::ByType);
    let actual = mapping(&plan);

    assert_eq!(actual["a.pdf"], "文档\\a.pdf");
    assert_eq!(actual["b.png"], "图片\\b.png");
    assert_eq!(actual["c.mp3"], "音视频\\c.mp3");
    assert_eq!(actual["d.zip"], "压缩包\\d.zip");
    assert_eq!(
        actual["e.xyz"], "其他\\e.xyz",
        "未知扩展名必须进「其他」，不是失败也不是丢弃"
    );

    assert!(
        plan.items.iter().all(|item| item.selected),
        "规则模式下所有可规划项都应默认选中"
    );
    assert!(
        plan.items
            .iter()
            .all(|item| item.action == PlanAction::Move),
        "五类都是真实移动"
    );
    assert_eq!(plan.revision, 1);
    assert_eq!(plan.status, PlanStatus::Draft);
    assert_eq!(plan.mode, Mode::Rules);
    assert!(issues.is_empty(), "这一批不应产生任何问题：{issues:?}");
}

#[test]
fn by_month_plan_uses_the_local_month_of_the_last_modified_time() {
    let (_tmp, root) = make_root();
    let recent = support::write_file(root.canonical(), &["recent.txt"], b"a");
    let old = support::write_file(root.canonical(), &["old.txt"], b"b");

    let now = SystemTime::now();
    let forty_days = Duration::from_secs(40 * 24 * 60 * 60);
    fs::File::options()
        .write(true)
        .open(&recent)
        .expect("应能打开文件")
        .set_modified(now)
        .expect("应能设置修改时间");
    fs::File::options()
        .write(true)
        .open(&old)
        .expect("应能打开文件")
        .set_modified(now - forty_days)
        .expect("应能设置修改时间");

    let (plan, issues) = rule_plan(&root, RuleKind::ByMonth);
    assert!(issues.is_empty(), "不应有问题：{issues:?}");

    let actual = mapping(&plan);
    let recent_bucket = actual["recent.txt"].split('\\').next().unwrap().to_owned();
    let old_bucket = actual["old.txt"].split('\\').next().unwrap().to_owned();

    for bucket in [&recent_bucket, &old_bucket] {
        let bytes = bucket.as_bytes();
        assert_eq!(bytes.len(), 7, "{bucket:?} 必须是 YYYY-MM");
        assert_eq!(bytes[4], b'-');
        assert!(bytes[..4].iter().all(u8::is_ascii_digit));
        assert!(bytes[5..].iter().all(u8::is_ascii_digit));
    }
    assert_ne!(
        recent_bucket, old_bucket,
        "相隔 40 天必然跨月，两个文件不能落在同一个目录"
    );

    // 「最近」那个文件必须落在本机当前本地月份里——用系统时区 API 独立取值比对。
    let now_ns = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("本机时间应在纪元之后")
        .as_nanos() as i128;
    let local = filepilot_lib::platform::windows::local_datetime_from_unix_ns(now_ns)
        .expect("应能换算本地时间");
    assert_eq!(
        recent_bucket,
        format!("{:04}-{:02}", local.year, local.month),
        "byMonth 必须按本地时区分月，而不是 UTC"
    );
}

#[test]
fn the_original_extension_case_is_preserved() {
    let (_tmp, root) = make_root();
    // 规格 7.1 第 1 条：程序附加**原**扩展名，含大小写。
    support::write_file(root.canonical(), &["报告.PDF"], b"x");
    support::write_file(root.canonical(), &["无扩展名文件"], b"y");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    let actual = mapping(&plan);

    assert_eq!(
        actual["报告.PDF"], "文档\\报告.PDF",
        "扩展名大小写必须保持原样"
    );
    assert_eq!(
        actual["无扩展名文件"], "其他\\无扩展名文件",
        "无扩展名文件保持无扩展名"
    );
}

// ---------------------------------------------------------------------------
// 冲突分配与 noop（规格 7.1 第 7、9 条）
// ---------------------------------------------------------------------------

#[test]
fn files_with_the_same_name_get_stable_numbered_targets() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["学习", "a.pdf"], b"one");
    support::write_file(root.canonical(), &["资料", "a.pdf"], b"two");

    let (first, _) = rule_plan(&root, RuleKind::ByType);
    let (second, _) = rule_plan(&root, RuleKind::ByType);

    let targets: BTreeSet<String> = first
        .items
        .iter()
        .map(|item| item.target.join("\\"))
        .collect();
    assert_eq!(
        targets,
        BTreeSet::from(["文档\\a (2).pdf".to_owned(), "文档\\a.pdf".to_owned()]),
        "同名项必须分配 名称 (2).扩展名"
    );

    // 确定性：同一输入两次生成必须得到完全相同的目标方案。
    assert_eq!(
        mapping(&first),
        mapping(&second),
        "相同输入必须得到相同的目标方案"
    );
}

#[test]
fn an_existing_target_on_disk_shifts_the_new_name_instead_of_overwriting() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"source");
    support::make_dir(root.canonical(), &["文档"]);
    support::write_file(root.canonical(), &["文档", "a.pdf"], b"already there");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    assert_eq!(
        mapping(&plan)["a.pdf"],
        "文档\\a (2).pdf",
        "磁盘上已有的同名项必须被绕开，绝不覆盖"
    );
    assert_eq!(
        fs::read(support::join_under(root.canonical(), &["文档", "a.pdf"])).expect("读已有文件"),
        b"already there",
        "规划阶段绝不能改动磁盘上已有的文件"
    );
}

#[test]
fn a_file_already_in_its_target_folder_becomes_a_non_selectable_noop() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["文档", "a.pdf"], b"x");

    let (plan, issues) = rule_plan(&root, RuleKind::ByType);
    let item = item_of(&plan, "文档\\a.pdf");

    assert_eq!(item.action, PlanAction::Noop, "目标与当前位置相同 → noop");
    assert!(!item.selected, "noop 项不可选（规格 6.3）");
    assert!(
        issue_codes(&issues).contains("NOOP"),
        "应说明为什么这一项不需要移动：{issues:?}"
    );

    let report = validate_plan(&plan, &root).expect("校验应成功");
    assert_eq!(
        report.executable_count, 0,
        "没有任何可执行项时不能凭空产生一个"
    );
    assert!(
        issue_codes(&report.issues).contains("EMPTY_SELECTION"),
        "选中集合为空是全局阻断问题：{:?}",
        report.issues
    );
}

#[test]
fn cyclic_swaps_are_degraded_instead_of_planned_as_swaps() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"A");
    support::write_file(root.canonical(), &["b.txt"], b"B");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let id_of = |name: &str| {
        outcome
            .records
            .iter()
            .find(|record| record.relative_path == vec![name.to_owned()])
            .unwrap_or_else(|| panic!("扫描结果里应有 {name}"))
            .id
            .clone()
    };

    let proposal = |file_id: String, stem: &str| Proposal {
        file_id,
        category: Vec::new(),
        stem: stem.to_owned(),
        reason: "互换".to_owned(),
        confidence: 1.0,
        evidence_locator: None,
    };

    // 用户意图是互换 a.txt 与 b.txt —— 规格 7.1 第 9 条明确不支持。
    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![proposal(id_of("a.txt"), "b"), proposal(id_of("b.txt"), "a")],
        created_at: SystemTime::now(),
    })
    .expect("生成计划应成功");

    let actual = mapping(&build.plan);
    assert_ne!(
        actual["a.txt"], "b.txt",
        "不能把目标定在别人现在的源路径上——执行顺序不能当作前提"
    );
    assert_ne!(actual["b.txt"], "a.txt");
    assert_ne!(actual["a.txt"], actual["b.txt"], "两项不能指向同一个目标");
    // 降级结果仍然是一次合法的重命名：各自让开一步。
    assert_eq!(actual["a.txt"], "b (2).txt");
    assert_eq!(actual["b.txt"], "a (2).txt");
}

#[test]
fn a_file_blocking_the_target_directory_blocks_that_item() {
    let (_tmp, root) = make_root();
    // `文档` 是一个**文件**，而 byType 想把它当目录用
    support::write_file(root.canonical(), &["文档"], b"i am a file");
    support::write_file(root.canonical(), &["a.pdf"], b"x");

    let (plan, issues) = rule_plan(&root, RuleKind::ByType);
    let item = item_of(&plan, "a.pdf");

    assert!(!item.selected, "父路径被文件挡住时该项不可选");
    assert_eq!(item.target, item.source, "不可选项必须保持原位");
    assert!(
        issue_codes(&issues).contains(codes::TARGET_PARENT_IS_FILE),
        "必须报出「路径上已经有同名文件」：{issues:?}"
    );

    // 对照组：即使把这一项手工改成选中，校验也必须独立挡住它。
    //
    // 注意要先把另一项（那个叫 `文档` 的无扩展名文件会被规划到「其他」）去掉：
    // 它本身是合法的，留着它会让 executable_count 变成 1，掩盖我们要验证的问题。
    let mut forced = plan.clone();
    forced
        .items
        .retain(|candidate| candidate.source.join("\\") == "a.pdf");
    for candidate in forced.items.iter_mut() {
        candidate.selected = true;
        candidate.action = PlanAction::Move;
        candidate.target = RelPath::from(vec!["文档".to_owned(), "a.pdf".to_owned()]);
    }
    let report = validate_plan(&forced, &root).expect("校验应成功");
    assert_eq!(report.executable_count, 0, "被文件挡住的目录不能执行");
    assert!(
        issue_codes(&report.issues).contains(codes::TARGET_PARENT_IS_FILE),
        "校验必须独立发现父路径是文件：{:?}",
        report.issues
    );
}

#[test]
fn a_case_only_rename_is_refused() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["report.txt"], b"x");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let file_id = outcome.records[0].id.clone();

    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![Proposal {
            file_id,
            category: Vec::new(),
            // 规格 7.1 第 9 条：不支持仅改变大小写的重命名
            stem: "REPORT".to_owned(),
            reason: "统一大写".to_owned(),
            confidence: 1.0,
            evidence_locator: None,
        }],
        created_at: SystemTime::now(),
    })
    .expect("生成计划应成功");

    let item = item_of(&build.plan, "report.txt");
    assert!(!item.selected, "仅改大小写的项必须不可选");
    assert_eq!(item.target, item.source, "不可选项保持原位");
    assert!(
        issue_codes(&build.issues).contains(filepilot_lib::planner::CODE_CASE_ONLY_RENAME),
        "必须明确报出「仅改大小写不支持」：{:?}",
        build.issues
    );
}

#[test]
fn a_suggested_name_never_changes_the_extension() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["report.TXT"], b"x");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![Proposal {
            file_id: outcome.records[0].id.clone(),
            category: vec!["文档".to_owned()],
            // 建议里塞进扩展名与分隔符，程序必须只把它当"主体"，
            // 并按规格 7.1 第 1 条附加**源文件的**扩展名（含大小写）
            stem: "季度报告".to_owned(),
            reason: "改名".to_owned(),
            confidence: 1.0,
            evidence_locator: None,
        }],
        created_at: SystemTime::now(),
    })
    .expect("生成计划应成功");

    assert_eq!(
        mapping(&build.plan)["report.TXT"],
        "文档\\季度报告.TXT",
        "扩展名必须来自源文件且保持大小写"
    );
}

// ---------------------------------------------------------------------------
// 指纹绑定与只读性
// ---------------------------------------------------------------------------

#[test]
fn every_selected_item_binds_the_full_fingerprint_of_its_source() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["报告.pdf"], b"hello world");
    support::write_file(root.canonical(), &["照片.png"], vec![7u8; 300].as_slice());

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    assert!(plan.items.iter().all(|item| item.selected));

    for item in &plan.items {
        let source_path = support::join_under(root.canonical(), &[item.source[0].as_str()]);
        let sha = item
            .expected
            .sha256
            .as_deref()
            .expect("可执行项必须绑定内容哈希（规格 5.1）");
        assert_eq!(sha.len(), 64, "sha256 必须是十六进制摘要");
        assert_eq!(
            sha,
            sha256_of(&source_path),
            "绑定的哈希必须等于源文件的真实内容哈希"
        );
        assert_eq!(
            item.expected.size,
            fs::metadata(&source_path)
                .expect("读元数据")
                .len()
                .to_string()
        );
        assert_eq!(item.expected.volume_id, root.volume_id());
    }
}

#[test]
fn planning_does_not_change_the_file_tree() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"one");
    support::write_file(root.canonical(), &["子目录", "b.png"], b"two");
    support::write_file(root.canonical(), &["c.xyz"], b"three");

    let before = support::snapshot_tree(root.canonical());
    let _ = rule_plan(&root, RuleKind::ByType);
    let _ = rule_plan(&root, RuleKind::ByMonth);
    let after = support::snapshot_tree(root.canonical());

    assert_eq!(
        before, after,
        "生成计划前后文件树与内容必须逐字节相同（INV-01）"
    );
}

#[test]
fn an_unrepresentable_name_is_reported_instead_of_being_truncated() {
    let (_tmp, root) = make_root();
    // 81 个 UTF-16 单位：超过规格 7.1 第 2 条的 80 单位上限。
    // Windows 允许建出这样的文件，所以这是真实可达的情形。
    let long_name = format!("{}.txt", "中".repeat(78));
    support::write_file(root.canonical(), &[&long_name], b"x");

    let (plan, issues) = rule_plan(&root, RuleKind::ByType);
    let item = item_of(&plan, &long_name);

    assert!(!item.selected, "目标名超长时必须不可选，而不是静默截断");
    assert_eq!(
        item.target, item.source,
        "不可选项保持原位，不产生一个没被确认过的新名字"
    );
    assert!(
        issue_codes(&issues).contains(codes::PATH_TOO_LONG),
        "必须明确报出超长：{issues:?}"
    );
}

// ---------------------------------------------------------------------------
// 主动排查（2026-09-17）：以下用例来自针对实现的探测，每条都曾真实抓到过问题
// ---------------------------------------------------------------------------

/// 建议指向一个**被扫描跳过**的文件时，必须说清是"跳过了"，而不是"不存在"。
///
/// 这两种说法对用户意味着完全不同的动作：前者去看那个文件（例如未完成的下载），
/// 后者会让人怀疑扫描漏了文件。
#[test]
fn a_proposal_for_a_skipped_file_is_reported_as_skipped_not_missing() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["half.part"], b"partial");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let skipped = outcome
        .records
        .iter()
        .find(|record| record.skip_code.is_some())
        .expect("应有被跳过的记录");

    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![Proposal {
            file_id: skipped.id.clone(),
            category: vec!["其他".to_owned()],
            stem: "half".to_owned(),
            reason: "试探".to_owned(),
            confidence: 1.0,
            evidence_locator: None,
        }],
        created_at: SystemTime::now(),
    })
    .expect("生成计划不应失败");

    let codes = issue_codes(&build.issues);
    let messages: Vec<String> = build.issues.iter().map(|i| i.message.clone()).collect();
    assert!(
        codes.contains(filepilot_lib::planner::CODE_SKIPPED_AT_SCAN),
        "应报出「扫描阶段已跳过」：{messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("不存在")),
        "文件是存在的，只是被跳过了：{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("half.part")),
        "要说清是哪个文件：{messages:?}"
    );
}

/// 完整目标路径的上限是 **≤240**，边界必须精确。
///
/// 这条测试把根做深，让目标正好压在 240 与 241 上——一个 off-by-one 就会让
/// 「预览里合法」的路径在执行阶段超限。
#[test]
fn the_total_path_limit_is_enforced_at_exactly_240_units() {
    let tmp = support::test_root();
    let mut deep = tmp.path().to_path_buf();
    for index in 0..8 {
        deep.push(format!("{}{index:02}", "深".repeat(14)));
        use std::os::windows::ffi::OsStrExt as _;
        if deep.as_os_str().encode_wide().count() >= 165 {
            break;
        }
    }
    fs::create_dir_all(&deep).expect("建深目录应成功");
    let root = approve_root(&deep).expect("深目录应被授权");

    let root_units =
        filepilot_lib::planner::naming::total_path_units(&root, &[]).expect("应能算根长度");
    let overhead = root_units + 1 + "文档".encode_utf16().count() + 1;
    let budget = 240usize.checked_sub(overhead).expect("根不能长到没有预算");
    assert!(
        (4..=80).contains(&budget),
        "测试前提不成立：文件名预算 {budget} 需要落在 4..=80"
    );

    let at_limit = format!("{}.txt", "a".repeat(budget - 4));
    let over_limit = format!("{}.txt", "a".repeat(budget - 3));
    support::write_file(root.canonical(), &[at_limit.as_str()], b"x");
    support::write_file(root.canonical(), &[over_limit.as_str()], b"x");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let proposal = |name: &str| Proposal {
        file_id: outcome
            .records
            .iter()
            .find(|record| record.relative_path == vec![name.to_owned()])
            .expect("扫描结果里应有该文件")
            .id
            .clone(),
        category: vec!["文档".to_owned()],
        stem: name.trim_end_matches(".txt").to_owned(),
        reason: "边界".to_owned(),
        confidence: 1.0,
        // 带上依据，否则这条建议会被「AI 模式无依据默认不选中」拦下
        // （规格 4.2），而这条测试验的是**路径长度上限**，与依据无关。
        //
        // 两个维度都不满足时，测试失败会给出一条与它想问的问题无关的
        // 抱怨——`ok_item.selected` 为假，而真正的原因是「没给依据」。
        evidence_locator: Some("chars:0-1".to_owned()),
    };

    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![proposal(&at_limit), proposal(&over_limit)],
        created_at: SystemTime::now(),
    })
    .expect("生成计划不应失败");

    let item_of_name = |name: &str| {
        build
            .plan
            .items
            .iter()
            .find(|item| item.source == vec![name.to_owned()])
            .expect("计划里应有该项")
            .clone()
    };
    let ok_item = item_of_name(&at_limit);
    let bad_item = item_of_name(&over_limit);

    assert_eq!(
        ok_item.target,
        vec!["文档".to_owned(), at_limit.clone()],
        "正好 240 单位必须允许（上限是 ≤240）"
    );
    assert!(ok_item.selected);
    assert!(
        !bad_item.selected,
        "241 单位必须拒绝，否则执行阶段会撞上真实上限"
    );
    assert!(
        issue_codes(&build.issues).contains(codes::PATH_TOO_LONG),
        "必须明确报出超长：{:?}",
        build.issues
    );

    // 校验侧必须与规划侧给出一致的判断
    let report = validate_plan(&build.plan, &root).expect("校验应成功");
    assert_eq!(
        report.executable_count, 1,
        "只有正好 240 的那一项可执行：{:?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .filter(|issue| issue.severity == Risk::Block)
            .all(|issue| issue.item_id.as_deref() != Some(ok_item.id.as_str())),
        "校验不得对「正好 240」的项报阻断问题"
    );
}

/// 两条计划项共用同一个 itemId 时，问题归属与执行记录都会失去唯一性，
/// 因此必须直接判为不可执行。
#[test]
fn duplicate_item_ids_block_the_plan() {
    let (_tmp, root) = make_root();
    let a = support::write_file(root.canonical(), &["a.txt"], b"A");
    let b = support::write_file(root.canonical(), &["b.txt"], b"B");
    let fingerprint = |path: &std::path::Path| {
        filepilot_lib::scanner::snapshot::fingerprint(path, root.volume_id(), true).expect("指纹")
    };
    let item = |source: &str, target: &str, path: &std::path::Path| PlanItem {
        id: "same-id".to_owned(),
        file_id: format!("file-{source}"),
        source: RelPath::from(vec![source.to_owned()]),
        target: RelPath::from(target.split('\\').map(str::to_owned).collect::<Vec<_>>()),
        action: PlanAction::Move,
        selected: true,
        origin: PlanItemOrigin::Rule,
        reason: String::new(),
        expected: fingerprint(path),
    };
    let plan = Plan {
        id: "plan-dup-id".to_owned(),
        root_id: root.id().to_string(),
        scan_id: "scan".to_owned(),
        revision: 1,
        mode: Mode::Rules,
        status: PlanStatus::Draft,
        items: vec![
            item("a.txt", "文档\\a.txt", &a),
            item("b.txt", "图片\\b.txt", &b),
        ],
        created_at: "2026-09-17T00:00:00.000Z".to_owned(),
    };

    let report = validate_plan(&plan, &root).expect("校验应成功");
    assert_eq!(
        report.executable_count, 0,
        "重复 itemId 的计划不能有任何可执行项，实际 {}",
        report.executable_count
    );
    assert!(
        issue_codes(&report.issues).contains("DUPLICATE_ITEM_ID"),
        "必须报出 itemId 重复：{:?}",
        report.issues
    );
}

#[test]
fn validation_rejects_an_edited_target_that_changes_the_extension() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["报告.PDF"], b"content");
    let (mut plan, _) = rule_plan(&root, RuleKind::ByType);
    assert_eq!(plan.items.len(), 1);
    plan.items[0].target = RelPath::from(vec!["文档".to_owned(), "报告.txt".to_owned()]);

    let report = validate_plan(&plan, &root).expect("校验应返回报告");
    assert_eq!(report.executable_count, 0);
    assert!(
        issue_codes(&report.issues).contains("EXTENSION_CHANGED"),
        "编辑目标不能绕过扩展名不变约束：{:?}",
        report.issues
    );
}

#[test]
fn validation_rejects_two_selected_items_for_the_same_source() {
    let (_tmp, root) = make_root();
    let source = support::write_file(root.canonical(), &["a.txt"], b"A");
    let expected = filepilot_lib::scanner::snapshot::fingerprint(&source, root.volume_id(), true)
        .expect("指纹");
    let item = |id: &str, target_dir: &str| PlanItem {
        id: id.to_owned(),
        file_id: "same-file".to_owned(),
        source: RelPath::from(vec!["a.txt".to_owned()]),
        target: RelPath::from(vec![target_dir.to_owned(), "a.txt".to_owned()]),
        action: PlanAction::Move,
        selected: true,
        origin: PlanItemOrigin::Rule,
        reason: String::new(),
        expected: expected.clone(),
    };
    let plan = Plan {
        id: "plan-duplicate-source".to_owned(),
        root_id: root.id().to_string(),
        scan_id: "scan".to_owned(),
        revision: 1,
        mode: Mode::Rules,
        status: PlanStatus::Draft,
        items: vec![item("one", "文档"), item("two", "其他")],
        created_at: "2026-09-17T00:00:00.000Z".to_owned(),
    };

    let report = validate_plan(&plan, &root).expect("校验应返回报告");
    assert_eq!(report.executable_count, 0);
    assert!(
        issue_codes(&report.issues).contains("DUPLICATE_SOURCE"),
        "同一个源不能在一批里执行两次：{:?}",
        report.issues
    );
}

/// 磁盘上已有 `a.pdf`，而源文件叫 `A.PDF`：Windows 视为同一个名字，必须让开。
#[test]
fn a_case_insensitive_disk_collision_is_avoided() {
    let (_tmp, root) = make_root();
    support::make_dir(root.canonical(), &["文档"]);
    support::write_file(root.canonical(), &["文档", "a.pdf"], b"existing");
    support::write_file(root.canonical(), &["A.PDF"], b"source");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    assert_eq!(
        mapping(&plan)["A.PDF"],
        "文档\\A (2).PDF",
        "大小写不同在 Windows 上仍是同一个名字，必须让开并保留原扩展名大小写"
    );
}

/// 目标目录链上存在目录联接（junction）时，计划阶段就必须拒绝。
///
/// T03 已经为执行器补过这个洞；这条测试保证规划器不会把一个**永远执行不了**
/// 的项放进预览。
#[test]
fn a_target_directory_behind_a_junction_is_refused() {
    let tmp = support::test_root();
    let root_path = tmp.path().to_path_buf();
    // 外部目标也必须由 TempDir 管理；手工拼一个进程号目录会在测试成功后泄漏。
    let outside_tmp = support::test_root();
    let outside = outside_tmp.path().to_path_buf();

    let link = root_path.join("link-out");
    let created = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);
    if !created {
        // 与 tests/scan.rs 的 S02 同一处理：无法创建联接时不能算通过。
        println!("跳过：本环境无法创建 junction（不计入通过）");
        return;
    }

    let root = approve_root(&root_path).expect("根应被授权");
    support::write_file(root.canonical(), &["a.pdf"], b"x");

    let outcome = scan(&root, &ScanOptions::default()).expect("扫描应成功");
    let build = build_plan(&PlanRequest {
        root: &root,
        scan_id: &outcome.scan_id.to_string(),
        mode: Mode::AiCloud,
        records: &outcome.records,
        proposals: vec![Proposal {
            file_id: outcome
                .records
                .iter()
                .find(|record| record.relative_path == vec!["a.pdf".to_owned()])
                .expect("应有该文件")
                .id
                .clone(),
            category: vec!["link-out".to_owned(), "inner".to_owned()],
            stem: "a".to_owned(),
            reason: "穿链接".to_owned(),
            confidence: 1.0,
            evidence_locator: None,
        }],
        created_at: SystemTime::now(),
    })
    .expect("生成计划不应失败");

    assert!(
        !build.plan.items[0].selected,
        "不能计划一次穿过目录联接的移动"
    );
    assert!(
        issue_codes(&build.issues).contains(codes::REPARSE_POINT),
        "必须报 REPARSE_POINT：{:?}",
        build.issues
    );
}

/// 冲突候选被占满时必须**有界地**报错，而不是死循环或错误地成功。
///
/// 直接驱动 `TargetLedger` 并把尝试上限压到 3：这条路径要验证的是
/// 「循环有界 + 报错明确」，与上限的具体数值无关。用默认上限（1000）端到端复现
/// 需要真的造 1000 个文件——在本机（带文件过滤驱动）光建文件就要 20 秒以上，
/// 换来的信息量完全相同。分配失败如何传导到计划项（不可选 + 带错误码的问题）
/// 由 `a_file_blocking_the_target_directory_blocks_that_item` 覆盖同一条代码路径。
#[test]
fn allocation_exhaustion_is_an_explicit_error() {
    let (_tmp, root) = make_root();
    support::make_dir(root.canonical(), &["文档"]);
    for name in ["a.txt", "a (2).txt", "a (3).txt"] {
        support::write_file(root.canonical(), &["文档", name], b"x");
    }

    let mut ledger = TargetLedger::with_attempt_limit(&root, 3);
    let started = std::time::Instant::now();
    let error = ledger
        .allocate(&["文档".to_owned()], "a", ".txt", &["a.txt".to_owned()])
        .expect_err("三个候选都被占用时必须报错");

    assert_eq!(error.code, codes::TARGET_EXISTS);
    assert!(
        error.message.contains("3 个候选名称"),
        "错误信息要说清候选耗尽与上限：{}",
        error.message
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "冲突探测必须有界，实际用了 {:?}",
        started.elapsed()
    );
}

// ---------------------------------------------------------------------------
// 校验：过期、竞争与全局问题
// ---------------------------------------------------------------------------

#[test]
fn two_selected_items_with_the_same_target_block_the_plan() {
    let (_tmp, root) = make_root();
    let a = support::write_file(root.canonical(), &["a.txt"], b"A");
    let b = support::write_file(root.canonical(), &["b.txt"], b"B");

    let fingerprint = |path: &std::path::Path| {
        filepilot_lib::scanner::snapshot::fingerprint(path, root.volume_id(), true)
            .expect("应能取指纹")
    };

    let shared_target = RelPath::from(vec!["文档".to_owned(), "same.txt".to_owned()]);
    let crafted = Plan {
        id: "plan-dup".to_owned(),
        root_id: root.id().to_string(),
        scan_id: "scan-dup".to_owned(),
        revision: 1,
        mode: Mode::Rules,
        status: PlanStatus::Draft,
        items: vec![
            PlanItem {
                id: "item-a".to_owned(),
                file_id: "file-a".to_owned(),
                source: RelPath::from(vec!["a.txt".to_owned()]),
                target: shared_target.clone(),
                action: PlanAction::Move,
                selected: true,
                origin: PlanItemOrigin::Rule,
                reason: String::new(),
                expected: fingerprint(&a),
            },
            PlanItem {
                id: "item-b".to_owned(),
                file_id: "file-b".to_owned(),
                source: RelPath::from(vec!["b.txt".to_owned()]),
                target: shared_target,
                action: PlanAction::Move,
                selected: true,
                origin: PlanItemOrigin::Rule,
                reason: String::new(),
                expected: fingerprint(&b),
            },
        ],
        created_at: "2026-09-17T00:00:00.000Z".to_owned(),
    };

    let report = validate_plan(&crafted, &root).expect("校验应成功");
    assert_eq!(
        report.executable_count, 0,
        "两项指向同一目标时不能有任何一项被判为可执行"
    );
    assert!(
        issue_codes(&report.issues).contains("DUPLICATE_TARGET"),
        "必须报出目标重复：{:?}",
        report.issues
    );

    // 对照组：把第二项改成另一个目标后，两项都可执行。
    let mut fixed = crafted.clone();
    fixed.items[1].target = RelPath::from(vec!["文档".to_owned(), "other.txt".to_owned()]);
    let report = validate_plan(&fixed, &root).expect("校验应成功");
    assert_eq!(
        report.executable_count, 2,
        "没有真实冲突时不应把项判成不可执行：{:?}",
        report.issues
    );
}

#[test]
fn a_target_created_after_planning_blocks_validation_instead_of_being_renamed() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"source");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    assert_eq!(mapping(&plan)["a.pdf"], "文档\\a.pdf");

    // 计划生成之后，另一个进程抢先创建了目标（规格 7.1 第 8 条 / E02）
    support::make_dir(root.canonical(), &["文档"]);
    support::write_file(root.canonical(), &["文档", "a.pdf"], b"squatter");

    let report = validate_plan(&plan, &root).expect("校验应成功");
    assert_eq!(report.executable_count, 0);
    assert!(
        issue_codes(&report.issues).contains(codes::TARGET_EXISTS),
        "必须报出目标已被占用：{:?}",
        report.issues
    );
    assert_eq!(
        fs::read(support::join_under(root.canonical(), &["文档", "a.pdf"])).expect("读文件"),
        b"squatter",
        "校验阶段绝不能改动别人的文件"
    );
}

#[test]
fn a_source_modified_after_planning_blocks_validation() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["a.pdf"], b"original");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    // 预览之后文件被改写（规格 2.3：原行必须标记过期）
    fs::write(&path, b"modified after planning").expect("改写应成功");

    let report = validate_plan(&plan, &root).expect("校验应成功");
    assert_eq!(report.executable_count, 0);
    assert!(
        issue_codes(&report.issues).contains(codes::SOURCE_CHANGED),
        "源文件变化必须被检出：{:?}",
        report.issues
    );
}

#[test]
fn a_plan_cannot_be_validated_against_a_different_root() {
    let (_tmp_a, root_a) = make_root();
    let (_tmp_b, root_b) = make_root();
    support::write_file(root_a.canonical(), &["a.pdf"], b"x");

    let (plan, _) = rule_plan(&root_a, RuleKind::ByType);
    let report = validate_plan(&plan, &root_b).expect("校验应成功");

    assert_eq!(report.executable_count, 0, "根目录换了就不能执行");
    assert!(
        issue_codes(&report.issues).contains(codes::ROOT_CHANGED),
        "必须报出根目录不一致：{:?}",
        report.issues
    );
}

#[test]
fn a_sealed_plan_cannot_be_revalidated() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"x");

    let (mut plan, _) = rule_plan(&root, RuleKind::ByType);
    plan.status = PlanStatus::Sealed;

    let report = validate_plan(&plan, &root).expect("校验应成功");
    assert_eq!(report.executable_count, 0);
    assert!(
        issue_codes(&report.issues).contains(codes::STALE_PLAN),
        "已进入执行流程的计划不能再次校验：{:?}",
        report.issues
    );
}

#[test]
fn the_digest_follows_the_selection_and_the_bound_fingerprints() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"one");
    support::write_file(root.canonical(), &["b.png"], b"two");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    let digest = compute_digest(&plan, &root);

    // 选中集合变化 → 摘要必须变化（规格 7.2：编辑使旧确认失效）
    let mut reduced = plan.clone();
    reduced.items[0].selected = false;
    assert_ne!(digest, compute_digest(&reduced, &root));

    // 目标变化 → 摘要必须变化
    let mut renamed = plan.clone();
    renamed.items[0].target = RelPath::from(vec!["其他".to_owned(), "a.pdf".to_owned()]);
    assert_ne!(digest, compute_digest(&renamed, &root));

    // 版本变化 → 摘要必须变化
    let mut revised = plan.clone();
    revised.revision = 2;
    assert_ne!(digest, compute_digest(&revised, &root));

    // 同一个计划重复计算 → 摘要必须一致
    assert_eq!(digest, compute_digest(&plan, &root));
}

#[test]
fn the_validation_report_reports_no_token_in_this_phase() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.pdf"], b"x");

    let (plan, _) = rule_plan(&root, RuleKind::ByType);
    let report = validate_plan(&plan, &root).expect("校验应成功");

    assert!(
        report.validation_token.is_none(),
        "一次性令牌由 T05 签发；本阶段不得制造一个看起来可用的假令牌"
    );
    assert!(report.expires_at.is_none());
    assert_eq!(report.plan_id, plan.id);
    assert_eq!(report.revision, plan.revision);
    assert_eq!(report.executable_count, 1);
    assert_eq!(report.digest.len(), 64);
}

// ===========================================================================
// T05：一次性确认令牌（规格 7.4）
// ===========================================================================
//
// 规格写明：「后端 token 校验必须独立测试，禁止信任这个函数的布尔值。
// 安全规则必须在 Rust 端成立，即使前端完全绕过按钮。」
//
// 所以这一组测试**不经过任何界面**，直接对令牌仓库发起伪造、过期、改摘要、
// 重放四类攻击，确认后端全部拒绝。

/// 用一份真实计划的摘要签发令牌。
fn issue_for(plan: &Plan, root: &ApprovedRoot, now_ms: i64) -> (ConfirmationStore, String, String) {
    let report = validate_plan(plan, root).expect("校验应成功");
    let store = ConfirmationStore::new();
    let issued = store
        .issue(&plan.id, plan.revision, &report.digest, now_ms)
        .expect("签发应成功");
    (store, issued.token, report.digest)
}

#[test]
fn a_freshly_issued_token_is_accepted_exactly_once() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, token, digest) = issue_for(&plan, &root, 0);

    store
        .consume(&token, &plan.id, plan.revision, &digest, 10)
        .expect("首次消费应成功");
    assert_eq!(store.outstanding(10), 0);
}

#[test]
fn replaying_the_same_token_is_refused() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, token, digest) = issue_for(&plan, &root, 0);
    store
        .consume(&token, &plan.id, plan.revision, &digest, 10)
        .expect("首次应成功");

    let err = store
        .consume(&token, &plan.id, plan.revision, &digest, 20)
        .expect_err("重放必须被拒绝");
    assert_eq!(
        err.code,
        codes::TOKEN_USED,
        "重用要报 TOKEN_USED，前端才能给出「这次确认已经用过了」这种可操作的提示"
    );
}

#[test]
fn an_expired_token_is_refused_at_the_exact_boundary() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let issued_at = 1_000_000;
    let (store, token, digest) = issue_for(&plan, &root, issued_at);
    let expires_at = issued_at + TOKEN_TTL_MS;

    // 差 1 毫秒仍然有效
    store
        .consume(&token, &plan.id, plan.revision, &digest, expires_at - 1)
        .expect("尚未到期应通过");

    // 换一个新令牌验证恰好到期的边界
    let (store2, token2, digest2) = issue_for(&plan, &root, issued_at);
    let err = store2
        .consume(&token2, &plan.id, plan.revision, &digest2, expires_at)
        .expect_err("恰好到期就必须失效");
    assert_eq!(err.code, codes::TOKEN_EXPIRED);
}

#[test]
fn a_forged_token_is_refused_even_with_the_right_digest() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, _real, digest) = issue_for(&plan, &root, 0);

    // 伪造者手里可能有正确的 planId / revision / digest（这些都是明文），
    // 唯独拿不到令牌本身。这正是「只存哈希 + CSPRNG 生成」要防的。
    for forged in [
        "",
        "0".repeat(64).as_str(),
        "f".repeat(64).as_str(),
        "not-a-token-at-all",
    ] {
        let err = store
            .consume(forged, &plan.id, plan.revision, &digest, 10)
            .expect_err("伪造令牌必须被拒绝");
        assert_eq!(err.code, codes::STALE_PLAN);
    }
}

#[test]
fn changing_the_digest_after_issue_invalidates_the_token() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, token, digest) = issue_for(&plan, &root, 0);

    // 模拟「令牌签发之后，计划被改了」：摘要变了
    let tampered = format!("{digest}-tampered");
    let err = store
        .consume(&token, &plan.id, plan.revision, &tampered, 10)
        .expect_err("摘要不符必须被拒绝");
    assert_eq!(err.code, codes::STALE_PLAN);
    assert!(
        err.message.contains("内容"),
        "理由要指向「计划内容变了」，实际：{}",
        err.message
    );
}

#[test]
fn editing_the_plan_changes_the_digest_so_the_old_token_cannot_be_used() {
    // 这是端到端的形态：先签发，再改计划，然后用同一批参数消费 → 必须失败。
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    support::write_file(root.canonical(), &["b.txt"], b"y");
    let (mut plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, token, old_digest) = issue_for(&plan, &root, 0);

    // 改一处勾选 —— 摘要必须随之改变
    let target_id = plan
        .items
        .iter_mut()
        .find(|item| item.selected)
        .map(|item| {
            item.selected = false;
            item.id.clone()
        })
        .expect("应至少有一项被选中");
    plan.revision += 1;

    let new_report = validate_plan(&plan, &root).expect("重新校验应成功");
    assert_ne!(
        new_report.digest, old_digest,
        "改动选中集合后摘要必须变化，否则「编辑使旧确认失效」就没有依据"
    );

    let err = store
        .consume(&token, &plan.id, plan.revision, &new_report.digest, 10)
        .expect_err("版本已变，旧令牌必须失效");
    assert_eq!(err.code, codes::STALE_PLAN);

    // 连旧的摘要也救不了它
    let err = store
        .consume(&token, &plan.id, plan.revision, &old_digest, 10)
        .expect_err("版本不符同样必须拒绝");
    assert_eq!(err.code, codes::STALE_PLAN);

    let _ = target_id;
}

#[test]
fn a_token_issued_for_one_plan_cannot_be_used_on_another() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan_a, _) = rule_plan(&root, RuleKind::ByType);
    let (plan_b, _) = rule_plan(&root, RuleKind::ByMonth);

    let (store, token, digest_a) = issue_for(&plan_a, &root, 0);

    let err = store
        .consume(&token, &plan_b.id, plan_a.revision, &digest_a, 10)
        .expect_err("换一个计划就必须失效");
    assert_eq!(err.code, codes::STALE_PLAN);
}

#[test]
fn consumed_and_expired_entries_are_pruned() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (plan, _) = rule_plan(&root, RuleKind::ByType);

    let (store, token, digest) = issue_for(&plan, &root, 0);
    store
        .consume(&token, &plan.id, plan.revision, &digest, 10)
        .expect("消费应成功");

    assert_eq!(store.prune(10), 1, "已消费的那条应被清掉");
    assert_eq!(store.prune(TOKEN_TTL_MS + 1), 0, "已经没有可清的了");
}
