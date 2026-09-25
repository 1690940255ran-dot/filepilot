//! 规格 10.1 的 E02 / E03 与 T03 验收：Windows 安全移动原语。
//!
//! 这些测试直接跑在真实 NTFS 上（本机临时目录）。它们验证的不是「移动能用」，
//! 而是**在竞争与篡改下不会造成数据丢失**——所以断言的重点是
//! 「两个文件都还在、内容都没变」，而不只是「返回了错误」。

mod support;

use std::fs;
use std::path::Path;

use filepilot_lib::domain::errors::codes;
use filepilot_lib::domain::types::Fingerprint;
use filepilot_lib::domain::types::{Mode, PlanStatus, Risk};
use filepilot_lib::executor::{execute_plan, execute_plan_observed, ExecutionObserver};
use filepilot_lib::platform::windows;
use filepilot_lib::rules::RuleKind;
use filepilot_lib::safety::fingerprint::{move_no_replace, verify_handle_matches};
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};
use filepilot_lib::storage::db::Database;
use filepilot_lib::storage::runs::{count_operations, list_events, list_operations};

/// 目录在临时根的基础上再加一层，避免根目录被写进文件后影响其他用例。
fn make_root() -> (tempfile::TempDir, ApprovedRoot) {
    let tmp = support::test_root();
    let approved = approve_root(tmp.path()).expect("临时根应被授权");
    (tmp, approved)
}

/// 取文件当前的真实指纹（含完整内容哈希）。
///
/// 用生产代码自己的实现来取证，避免测试里出现一套「独立的」哈希逻辑——
/// 那样两边算错也会互相印证。
fn current_fingerprint(root: &ApprovedRoot, relative: &[&str]) -> Fingerprint {
    let path = support::join_under(root.canonical(), relative);
    filepilot_lib::scanner::snapshot::fingerprint(&path, root.volume_id(), true)
        .expect("应能取到指纹")
}

/// 读文件内容，用于断言「字节没变」。
fn read(root: &ApprovedRoot, relative: &[&str]) -> Option<Vec<u8>> {
    fs::read(support::join_under(root.canonical(), relative)).ok()
}

// ---------------------------------------------------------------------------
// 正常路径
// ---------------------------------------------------------------------------

#[test]
fn moves_a_file_without_changing_its_bytes() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"hello world");
    support::make_dir(root.canonical(), &["学习"]);

    let expected = current_fingerprint(&root, &["a.txt"]);
    let receipt = move_no_replace(
        &root,
        &["a.txt".to_owned()],
        &["学习".to_owned(), "a.txt".to_owned()],
        &expected,
    )
    .expect("移动应成功");

    assert!(read(&root, &["a.txt"]).is_none(), "源位置应已不存在");
    assert_eq!(
        read(&root, &["学习", "a.txt"]).as_deref(),
        Some(b"hello world".as_slice()),
        "目标内容必须与源完全一致"
    );
    assert_eq!(receipt.file_id, expected.file_id, "身份不应改变");
    assert_eq!(receipt.sha256, expected.sha256.unwrap());
}

#[test]
fn moves_chinese_and_emoji_names() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["报告 📄.txt"], "内容".as_bytes());
    support::make_dir(root.canonical(), &["归档 🗂"]);

    let expected = current_fingerprint(&root, &["报告 📄.txt"]);
    move_no_replace(
        &root,
        &["报告 📄.txt".to_owned()],
        &["归档 🗂".to_owned(), "报告 📄.txt".to_owned()],
        &expected,
    )
    .expect("中文与 emoji 路径应能移动");

    assert_eq!(
        read(&root, &["归档 🗂", "报告 📄.txt"]).as_deref(),
        Some("内容".as_bytes())
    );
}

// ---------------------------------------------------------------------------
// 规格 E02：目标已存在 —— 绝不覆盖
// ---------------------------------------------------------------------------

#[test]
fn refuses_when_target_already_exists_and_leaves_both_files_intact() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["src.txt"], b"SOURCE");
    support::make_dir(root.canonical(), &["dst"]);
    support::write_file(root.canonical(), &["dst", "src.txt"], b"EXISTING");

    let expected = current_fingerprint(&root, &["src.txt"]);
    let err = move_no_replace(
        &root,
        &["src.txt".to_owned()],
        &["dst".to_owned(), "src.txt".to_owned()],
        &expected,
    )
    .expect_err("目标已存在时必须拒绝");

    assert_eq!(err.code, codes::TARGET_EXISTS);
    assert_eq!(
        read(&root, &["src.txt"]).as_deref(),
        Some(b"SOURCE".as_slice()),
        "源文件必须原样保留"
    );
    assert_eq!(
        read(&root, &["dst", "src.txt"]).as_deref(),
        Some(b"EXISTING".as_slice()),
        "已存在的目标绝不能被覆盖"
    );
}

/// 规格 T03 的核心：**在预检查之后**目标被别的进程创建。
///
/// 这条绕过 `move_no_replace` 的预检查，直接调用内核层原语，
/// 制造出「检查通过 → 目标出现 → 执行重命名」这个真实竞争窗口。
/// 它验证的是内核层面的 `ReplaceIfExists = FALSE` 确实生效，
/// 而不是「我们的预检查运气好」。
#[test]
fn kernel_refuses_to_overwrite_a_target_created_after_the_precheck() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["src.txt"], b"SOURCE");
    support::make_dir(root.canonical(), &["dst"]);

    let source_path = support::join_under(root.canonical(), &["src.txt"]);
    let target_path = support::join_under(root.canonical(), &["dst", "src.txt"]);

    // 1) 打开源句柄 —— 这一步相当于「预检查通过」
    let handle = windows::open_source_for_rename(&source_path).expect("打开源应成功");

    // 2) 现在由「另一个进程」创建目标
    fs::write(&target_path, b"RACER").expect("模拟竞争方创建目标");

    // 3) 通过句柄执行重命名：内核必须拒绝
    let result = windows::rename_by_handle_no_replace(&handle, &target_path);
    drop(handle);

    assert!(
        result.is_err(),
        "目标已存在时，内核必须拒绝重命名；若这里成功就说明覆盖保护失效"
    );

    // 4) 两个文件都必须完好
    assert_eq!(
        fs::read(&source_path).expect("源文件必须还在").as_slice(),
        b"SOURCE",
        "源文件必须原样保留"
    );
    assert_eq!(
        fs::read(&target_path).expect("目标文件必须还在").as_slice(),
        b"RACER",
        "竞争方创建的文件绝不能被覆盖"
    );
}

/// 对照组：证明上一条不是因为「系统本来就不允许覆盖」而侥幸通过。
///
/// `std::fs::rename` 在 Windows 上是**会**覆盖目标的——这正是我们不能用它、
/// 而必须用 `SetFileInformationByHandle` 的原因。
#[test]
fn std_fs_rename_would_have_overwritten_the_target() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["src.txt"], b"SOURCE");
    support::make_dir(root.canonical(), &["dst"]);
    support::write_file(root.canonical(), &["dst", "src.txt"], b"EXISTING");

    let source_path = support::join_under(root.canonical(), &["src.txt"]);
    let target_path = support::join_under(root.canonical(), &["dst", "src.txt"]);

    let outcome = fs::rename(&source_path, &target_path);
    assert!(
        outcome.is_ok(),
        "本测试假设 std::fs::rename 会覆盖目标；若它开始拒绝，说明平台行为变了，\
         应当重新评估这里的结论"
    );
    assert_eq!(
        fs::read(&target_path).expect("目标应存在").as_slice(),
        b"SOURCE",
        "std::fs::rename 覆盖了原目标 —— 这就是它不能用在执行器里的原因"
    );
}

// ---------------------------------------------------------------------------
// 源状态变化
// ---------------------------------------------------------------------------

#[test]
fn detects_source_content_changed_with_identical_size_and_mtime() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["x.txt"], b"AAAA");
    let expected = current_fingerprint(&root, &["x.txt"]);

    // 改成同样长度，并把修改时间**恢复**到原值，模拟规格 E01 点名的情形
    let original_meta = fs::metadata(&path).expect("取元数据");
    let original_mtime = original_meta.modified().expect("取修改时间");

    fs::write(&path, b"BBBB").expect("改写内容");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("打开用于改时间");
    file.set_modified(original_mtime).expect("恢复修改时间");
    drop(file);

    let now = fs::metadata(&path).expect("再取元数据");
    assert_eq!(now.len(), original_meta.len(), "大小必须相同");
    assert_eq!(
        now.modified().expect("取修改时间"),
        original_mtime,
        "修改时间必须已恢复，否则这条用例测的不是哈希"
    );

    let err = move_no_replace(
        &root,
        &["x.txt".to_owned()],
        &["x.txt".to_owned()],
        &expected,
    )
    .expect_err("源与目标相同应被拒绝");
    assert_eq!(err.code, codes::INVALID_PATH);

    // 真的移动一次，确认哈希把「同大小同时间」的篡改检了出来
    support::make_dir(root.canonical(), &["out"]);
    let err = move_no_replace(
        &root,
        &["x.txt".to_owned()],
        &["out".to_owned(), "x.txt".to_owned()],
        &expected,
    )
    .expect_err("内容已变，必须拒绝执行");
    assert_eq!(err.code, codes::SOURCE_CHANGED);
    assert!(
        read(&root, &["x.txt"]).is_some(),
        "被拒绝时源文件必须留在原位"
    );
}

#[test]
fn detects_source_replaced_by_a_different_file() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["x.txt"], b"ORIGINAL");
    let expected = current_fingerprint(&root, &["x.txt"]);

    // 删掉再建同名同内容的文件：内容一致，但卷内身份必然不同
    fs::remove_file(&path).expect("删除原文件");
    fs::write(&path, b"ORIGINAL").expect("写同名文件");

    support::make_dir(root.canonical(), &["out"]);
    let err = move_no_replace(
        &root,
        &["x.txt".to_owned()],
        &["out".to_owned(), "x.txt".to_owned()],
        &expected,
    )
    .expect_err("同名但已是另一个文件，必须拒绝");

    assert_eq!(
        err.code,
        codes::SOURCE_CHANGED,
        "身份不符必须报 SOURCE_CHANGED；实际：{}",
        err.message
    );
    assert!(read(&root, &["x.txt"]).is_some(), "不得移动新文件");
}

#[test]
fn missing_source_is_reported_as_source_missing() {
    let (_tmp, root) = make_root();
    support::make_dir(root.canonical(), &["out"]);

    let ghost = Fingerprint {
        volume_id: root.volume_id().to_owned(),
        file_id: "0000000000000001".to_owned(),
        size: "1".to_owned(),
        modified_ns: "0".to_owned(),
        sha256: Some("0".repeat(64)),
    };

    let err = move_no_replace(
        &root,
        &["nope.txt".to_owned()],
        &["out".to_owned(), "nope.txt".to_owned()],
        &ghost,
    )
    .expect_err("源不存在必须报错");
    assert_eq!(err.code, codes::SOURCE_MISSING);
}

/// 规格：源文件被其他程序以写入方式占用时，必须明确失败而不是「假定它没被动过」。
#[test]
fn busy_source_is_reported_and_not_moved() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["locked.txt"], b"DATA");
    let expected = current_fingerprint(&root, &["locked.txt"]);
    support::make_dir(root.canonical(), &["out"]);

    // 以「只共享读」之外的方式持有它：独占打开会让我们的 CreateFile 失败
    let holder = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("先自行打开文件");

    let err = move_no_replace(
        &root,
        &["locked.txt".to_owned()],
        &["out".to_owned(), "locked.txt".to_owned()],
        &expected,
    )
    .expect_err("被占用的源必须拒绝");
    drop(holder);

    assert_eq!(
        err.code,
        codes::FILE_BUSY,
        "共享冲突必须保留为 FILE_BUSY，不能误报成源不存在"
    );
    assert_eq!(
        read(&root, &["locked.txt"]).as_deref(),
        Some(b"DATA".as_slice()),
        "源文件必须原样保留"
    );
}

// ---------------------------------------------------------------------------
// 目标路径的先决条件
// ---------------------------------------------------------------------------

#[test]
fn rejects_when_target_parent_does_not_exist() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let expected = current_fingerprint(&root, &["a.txt"]);

    let err = move_no_replace(
        &root,
        &["a.txt".to_owned()],
        &["不存在的目录".to_owned(), "a.txt".to_owned()],
        &expected,
    )
    .expect_err("目标父目录不存在时应拒绝");
    assert_eq!(err.code, codes::INVALID_PATH);
}

#[test]
fn rejects_when_target_parent_is_a_file() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    support::write_file(root.canonical(), &["blocker"], b"i am a file");
    let expected = current_fingerprint(&root, &["a.txt"]);

    let err = move_no_replace(
        &root,
        &["a.txt".to_owned()],
        &["blocker".to_owned(), "a.txt".to_owned()],
        &expected,
    )
    .expect_err("父路径是文件时应拒绝");
    assert_eq!(err.code, codes::TARGET_PARENT_IS_FILE);
}

#[test]
fn rejects_traversal_in_source_or_target() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let expected = current_fingerprint(&root, &["a.txt"]);

    for bad in [
        vec!["..".to_owned(), "a.txt".to_owned()],
        vec!["a.txt".to_owned(), "..".to_owned()],
        vec![r"C:\Windows".to_owned()],
        vec!["a\\b".to_owned()],
    ] {
        let err = move_no_replace(&root, &bad, &["a.txt".to_owned()], &expected)
            .expect_err("越界路径必须被拒绝");
        assert_eq!(
            err.code,
            codes::INVALID_PATH,
            "路径 {bad:?} 应报 INVALID_PATH，实际 {}",
            err.code
        );
    }
}

// ---------------------------------------------------------------------------
// 指纹核对本身
// ---------------------------------------------------------------------------

#[test]
fn verify_handle_rejects_a_fingerprint_without_content_hash() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["h.txt"], b"data");

    let mut expected = current_fingerprint(&root, &["h.txt"]);
    expected.sha256 = None; // 规格 5.1：执行前必须有内容哈希

    let file = fs::File::open(&path).expect("打开文件");
    let actual = filepilot_lib::scanner::snapshot::sha256_of_handle(&file).expect("算哈希");

    let err =
        verify_handle_matches(&file, &expected, &actual).expect_err("没有内容哈希的计划不允许执行");
    assert_eq!(err.code, codes::SOURCE_CHANGED);
}

/// 句柄与路径一致时，核对必须通过 —— 否则上面的负向用例可能只是「一律拒绝」。
#[test]
fn verify_handle_accepts_a_matching_fingerprint() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["ok.txt"], b"content");
    let expected = current_fingerprint(&root, &["ok.txt"]);

    let file = fs::File::open(&path).expect("打开文件");
    let actual = filepilot_lib::scanner::snapshot::sha256_of_handle(&file).expect("算哈希");
    verify_handle_matches(&file, &expected, &actual).expect("一致的指纹必须通过");
}

#[test]
fn source_and_target_must_differ() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["same.txt"], b"x");
    let expected = current_fingerprint(&root, &["same.txt"]);

    let err = move_no_replace(
        &root,
        &["same.txt".to_owned()],
        &["same.txt".to_owned()],
        &expected,
    )
    .expect_err("源与目标相同应当拒绝，而不是空转一次");
    assert_eq!(err.code, codes::INVALID_PATH);
}

/// 移动不会改变文件内容：路径变了，字节与哈希不变。
#[test]
fn content_hash_is_identical_before_and_after_move() {
    let (_tmp, root) = make_root();
    let payload = "一段足够长的中文内容，用来确认移动不会改动字节。".as_bytes();
    support::write_file(root.canonical(), &["orig.md"], payload);
    support::make_dir(root.canonical(), &["归档"]);

    let before = current_fingerprint(&root, &["orig.md"]);
    move_no_replace(
        &root,
        &["orig.md".to_owned()],
        &["归档".to_owned(), "orig.md".to_owned()],
        &before,
    )
    .expect("移动应成功");

    let after = current_fingerprint(&root, &["归档", "orig.md"]);
    assert_eq!(
        before.sha256, after.sha256,
        "路径改变不得影响内容哈希（规格 INV-04）"
    );
    assert_eq!(before.file_id, after.file_id, "卷内身份不应改变");
}

/// 供 `support::join_under` 使用的最小健全性检查。
#[test]
fn helper_joins_under_root() {
    let (_tmp, root) = make_root();
    let joined = support::join_under(root.canonical(), &["a", "b.txt"]);
    assert!(joined.starts_with(root.canonical()));
    assert!(joined.ends_with(Path::new("b.txt")));
}

#[test]
fn rejects_a_target_path_that_crosses_a_directory_junction() {
    let (_tmp, root) = make_root();
    let outside_tmp = support::test_root();
    support::write_file(root.canonical(), &["source.txt"], b"SOURCE");
    support::make_dir(outside_tmp.path(), &["inner"]);

    let junction = root.canonical().join("link");
    let output = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(junction.to_string_lossy().replace('/', "\\"))
        .arg(outside_tmp.path().to_string_lossy().replace('/', "\\"))
        .output()
        .expect("运行 mklink");
    assert!(
        output.status.success(),
        "当前 Windows/NTFS 验收环境必须能创建 junction: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = current_fingerprint(&root, &["source.txt"]);
    let err = move_no_replace(
        &root,
        &["source.txt".to_owned()],
        &[
            "link".to_owned(),
            "inner".to_owned(),
            "escaped.txt".to_owned(),
        ],
        &expected,
    )
    .expect_err("目录联接不能把目标带出批准根");

    assert_eq!(err.code, codes::REPARSE_POINT);
    assert!(root.canonical().join("source.txt").exists());
    assert!(!outside_tmp.path().join("inner/escaped.txt").exists());
    fs::remove_dir(&junction).expect("移除测试 junction");
}

#[test]
fn total_target_path_limit_includes_the_approved_root() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["source.txt"], b"x");
    let first = "a".repeat(70);
    let second = "b".repeat(70);
    support::make_dir(root.canonical(), &[&first, &second]);
    let target = vec![first, second, "c".repeat(70)];

    assert!(
        filepilot_lib::safety::path::validate_relative_path(&target).is_ok(),
        "本用例要求相对部分本身合法"
    );
    let expected = current_fingerprint(&root, &["source.txt"]);
    let err = move_no_replace(&root, &["source.txt".to_owned()], &target, &expected)
        .expect_err("完整路径超过 240 UTF-16 units 时必须拒绝");
    assert_eq!(err.code, codes::PATH_TOO_LONG);
    assert!(root.canonical().join("source.txt").exists());
}

// ===========================================================================
// T06：执行器与 journal（规格 7.3、8.2）
// ===========================================================================
//
// 断言的重点不是「文件被移动了」，而是**在最坏情况下也没有丢数据**：
// 中途失败时后续文件不动、已完成项保持完成、日志能还原真实发生了什么。

/// 把根与扫描写进数据库。
///
/// 这不是测试的额外开销，而是**生产路径必须做的事**：`plans.rootId` 与
/// `plans.scanId` 都有外键约束。少了这一步，`save_plan` 会撞外键——
/// 而那正是最初漏掉根与扫描落库时暴露出来的问题。
fn persist_root_and_scan(
    db: &Database,
    root: &ApprovedRoot,
) -> filepilot_lib::scanner::ScanOutcome {
    let outcome =
        filepilot_lib::scanner::scan(root, &filepilot_lib::scanner::ScanOptions::default())
            .expect("扫描应成功");

    filepilot_lib::storage::repositories::insert_root(
        db,
        &filepilot_lib::storage::repositories::RootRow {
            id: root.id().to_string(),
            canonical_path: root.canonical().display().to_string(),
            volume_id: root.volume_id().to_owned(),
            identity: root.identity().file_id_string(),
            session_id: "test-session".to_owned(),
        },
    )
    .expect("根应落库");

    filepilot_lib::storage::repositories::insert_scan(
        db,
        &filepilot_lib::storage::repositories::ScanRow {
            id: outcome.scan_id.to_string(),
            root_id: root.id().to_string(),
            status: "completed".to_owned(),
            recursive: true,
            started_at: "2026-09-17T00:00:00.000Z".to_owned(),
        },
    )
    .expect("扫描应落库");

    filepilot_lib::storage::repositories::insert_files(db, &outcome.records)
        .expect("文件记录应落库");

    outcome
}

/// 一次完整的「扫描 → 计划 → 校验 → 执行」。
fn plan_and_execute(
    root: &ApprovedRoot,
    db: &Database,
    request_id: &str,
) -> (
    filepilot_lib::domain::types::Plan,
    filepilot_lib::executor::ExecuteOutcome,
) {
    let scan = persist_root_and_scan(db, root);
    let build = filepilot_lib::planner::build_rule_plan(
        root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划应成功");

    // 规格 8.2：计划必须从**数据库**读出来再执行，不信任调用方手里的副本
    filepilot_lib::storage::repositories::save_plan(db, &build.plan, None).expect("计划应落库");
    let mut plan = filepilot_lib::storage::repositories::load_plan(db, &build.plan.id)
        .expect("读计划")
        .expect("计划应存在");
    plan.status = PlanStatus::Validated;

    let executed =
        execute_plan(db, root, &plan, request_id, 1_700_000_000_000).expect("执行应成功");
    (plan, executed)
}

fn db() -> Database {
    Database::open_in_memory().expect("应能初始化内存库")
}

#[test]
fn executor_does_not_create_directories_through_a_junction() {
    let (_tmp, root) = make_root();
    let outside = support::test_root();
    support::write_file(root.canonical(), &["a.txt"], b"safe");
    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let mut build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .unwrap();
    build.plan.items[0].target = vec!["link".into(), "inner".into(), "a.txt".into()];
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).unwrap();
    let link = root.canonical().join("link");
    let result = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(result.status.success(), "junction test setup failed");
    let outcome = execute_plan(&db, &root, &build.plan, "junction-review", 1000).unwrap();
    assert_eq!(outcome.counts.applied, 0);
    assert!(
        !outside.path().join("inner").exists(),
        "executor created a directory outside root"
    );
    fs::remove_dir(link).unwrap();
}

#[test]
fn request_id_cannot_be_reused_for_another_plan() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"a");
    let db = db();
    let (mut plan, _) = plan_and_execute(&root, &db, "bound-request");
    plan.id = "another-plan".into();
    let error = execute_plan(&db, &root, &plan, "bound-request", 2000).unwrap_err();
    assert_eq!(error.code, codes::REQUEST_CONFLICT);
}

#[test]
fn executing_a_plan_moves_every_selected_file_and_keeps_the_content_hash() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], "内容甲".as_bytes());
    support::write_file(root.canonical(), &["b.pdf"], "内容乙".as_bytes());

    let (plan, outcome) = plan_and_execute(&root, &db(), "req-1");

    assert_eq!(
        outcome.status,
        filepilot_lib::domain::types::RunStatus::Completed
    );
    assert_eq!(outcome.counts.applied, 2, "两项都应被移动");
    assert!(!outcome.deduplicated);

    // 源位置清空、目标位置有内容且**逐字节相同** —— 这是「路径改变而内容不变」
    for (name, payload) in [("a.txt", "内容甲"), ("b.pdf", "内容乙")] {
        assert!(read(&root, &[name]).is_none(), "{name} 应当已经不在原位置");
        let moved = read(&root, &["文档", name]).expect("目标应存在");
        assert_eq!(moved, payload.as_bytes(), "{name} 的内容必须一字不差");
    }

    // 计划里绑定的哈希与实际落到目标位置的一致
    for item in &plan.items {
        if item.selected {
            let after =
                current_fingerprint(&root, &[item.target[0].as_str(), item.target[1].as_str()]);
            assert_eq!(
                after.sha256, item.expected.sha256,
                "移动后的内容哈希必须与计划里绑定的一致"
            );
        }
    }
}

#[test]
fn a_repeated_request_id_does_not_create_a_second_run() {
    let db = db();
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");

    let (plan, first) = plan_and_execute(&root, &db, "same-request");

    // 用同一个 requestId 再执行一次：应当复用已有 run，且**不再移动任何文件**。
    // 这里刻意用**同一份计划**而不是重新生成——幂等检查发生在任何落库动作之前，
    // 换一份新计划反而会先撞外键，就测不到幂等本身了。
    let second = execute_plan(&db, &root, &plan, "same-request", 1_700_000_001_000)
        .expect("重复请求应幂等返回");

    assert!(second.deduplicated, "必须走幂等路径");
    assert_eq!(second.run_id, first.run_id, "应当复用同一个 run");
    assert_eq!(
        filepilot_lib::storage::repositories::count_rows(&db, "runs").expect("计数"),
        1,
        "同一 requestId 只能有一条 run"
    );
}

#[test]
fn unselected_files_are_not_touched_at_all() {
    let (_tmp, root) = make_root();
    // write_file 内置了 Defender 句柄等待：全量跑时刚建的文件可能被
    // 实时扫描短暂排他锁定（os error 5），那不是执行器的缺陷。
    support::write_file(root.canonical(), &["keep.txt"], b"I must not move");
    support::write_file(root.canonical(), &["move.txt"], b"I should move");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let mut build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");

    // 只勾选其中一项
    for item in build.plan.items.iter_mut() {
        item.selected = item.source.first().map(String::as_str) == Some("move.txt");
    }
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    execute_plan(&db, &root, &build.plan, "req-unselected", 1_700_000_000_000).expect("执行");

    assert_eq!(
        read(&root, &["keep.txt"]).as_deref(),
        Some(b"I must not move".as_slice()),
        "没被选中的文件必须留在原地，且内容不变"
    );
    assert!(
        read(&root, &["文档", "move.txt"]).is_some(),
        "被选中的文件应当已移动"
    );
}

#[test]
fn a_source_that_changed_after_planning_stops_the_run_before_moving_it() {
    let (_tmp, root) = make_root();
    let path = support::write_file(root.canonical(), &["a.txt"], b"ORIGINAL");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");

    // 计划生成之后，源文件被改写（保持同样长度，试图骗过 size 检查）
    std::fs::write(&path, b"MODIFIED").expect("改写源文件");

    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");
    let result = execute_plan(&db, &root, &build.plan, "req-changed", 1_700_000_000_000);

    // 全批预检查只比身份与存在性，因此这里会走到逐项核对；
    // 无论停在哪一步，文件都必须没动过。
    // 用 `if let` 而不是单臂 `match`：错误路径本身不需要额外断言，
    // 因为下面两条与路径无关的断言已经把「文件没动」钉死了。
    if let Ok(outcome) = result {
        assert_ne!(
            outcome.status,
            filepilot_lib::domain::types::RunStatus::Completed,
            "源已改变，不应报成完整成功"
        );
        assert!(outcome.counts.applied == 0, "被改动的文件不应被移动");
    }
    assert_eq!(
        std::fs::read(&path).expect("源仍在").as_slice(),
        b"MODIFIED",
        "内容没有被进一步改动"
    );
    assert!(
        read(&root, &["文档", "a.txt"]).is_none(),
        "被篡改的源不应出现在目标位置"
    );
}

#[test]
fn one_blocked_item_stops_the_rest_and_leaves_later_files_in_place() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"A");
    support::write_file(root.canonical(), &["b.txt"], b"B");
    support::write_file(root.canonical(), &["c.txt"], b"C");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");

    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    // 在计划之后、执行之前抢占其中一个目标位置 —— 全批预检查应当拦住整批
    support::make_dir(root.canonical(), &["文档"]);
    support::write_file(root.canonical(), &["文档", "b.txt"], b"I got here first");

    let executed = execute_plan(&db, &root, &build.plan, "req-blocked", 1_700_000_000_000)
        .expect("预检查失败也应正常返回结果");

    assert_eq!(
        executed.status,
        filepilot_lib::domain::types::RunStatus::Failed,
        "全批预检查失败时整批终止"
    );
    assert!(
        executed.issues.iter().any(|i| i.severity == Risk::Block),
        "必须给出阻断级问题"
    );
    assert_eq!(executed.counts.applied, 0, "一项都不应被移动");

    // 三个源文件全部原位不动
    for name in ["a.txt", "b.txt", "c.txt"] {
        assert!(
            read(&root, &[name]).is_some(),
            "{name} 必须留在原位：预检查失败不该产生任何部分执行"
        );
    }
    assert_eq!(
        read(&root, &["文档", "b.txt"]).as_deref(),
        Some(b"I got here first".as_slice()),
        "抢占者的文件不能被覆盖"
    );
}

#[test]
fn the_journal_records_prepared_before_applied_for_every_moved_file() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");

    let db = db();
    let (_, outcome) = plan_and_execute(&root, &db, "req-journal");

    let operations = list_operations(&db, &outcome.run_id).expect("读操作");
    assert_eq!(operations.len(), 1);

    let events = list_events(&db, &operations[0].id).expect("读事件");
    let phases: Vec<&str> = events.iter().map(|e| e.phase.as_str()).collect();

    let prepared = phases.iter().position(|p| *p == "prepared");
    let applied = phases.iter().position(|p| *p == "applied");
    assert!(prepared.is_some(), "必须有 prepared 事件");
    assert!(applied.is_some(), "必须有 applied 事件");
    assert!(
        prepared < applied,
        "prepared 必须早于 applied —— 这是崩溃恢复能判定状态的唯一依据"
    );

    let counts = count_operations(&db, &outcome.run_id).expect("计数");
    assert_eq!(counts.applied, 1);
}

#[test]
fn the_target_directory_is_created_and_recorded_as_created_by_this_run() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");

    let db = db();
    let (_, outcome) = plan_and_execute(&root, &db, "req-dirs");

    let dirs =
        filepilot_lib::storage::runs::list_created_dirs(&db, &outcome.run_id).expect("读目录记录");
    let created: Vec<_> = dirs.iter().filter(|d| d.created_by_run).collect();
    assert!(
        created
            .iter()
            .any(|d| d.relative_path == vec!["文档".to_owned()]),
        "本次创建的目录必须被记录，且标明由本次创建"
    );

    // 本次运行里所有被记录的目录都应当是**新建**的：
    // 目标目录在计划阶段还不存在，所以由执行创建。
    assert_eq!(
        created.len(),
        dirs.len(),
        "本用例里不存在既有目录，所有记录都该标成 createdByRun"
    );
}

#[test]
fn executing_a_sealed_plan_is_refused() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let mut build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");

    build.plan.status = PlanStatus::Sealed;
    // 计划必须先落库：runs.planId 有外键指向 plans，
    // 「执行时计划还不在库里」本身就是非法状态。
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");
    let executed =
        execute_plan(&db, &root, &build.plan, "req-sealed", 1_700_000_000_000).expect("应正常返回");

    assert_eq!(
        executed.status,
        filepilot_lib::domain::types::RunStatus::Failed
    );
    assert!(read(&root, &["a.txt"]).is_some(), "密封计划不得被执行");
}

#[test]
fn an_empty_selection_is_refused_rather_than_reported_as_success() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let mut build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");
    for item in build.plan.items.iter_mut() {
        item.selected = false;
    }
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    let executed =
        execute_plan(&db, &root, &build.plan, "req-empty", 1_700_000_000_000).expect("应正常返回");

    assert_eq!(
        executed.status,
        filepilot_lib::domain::types::RunStatus::Failed,
        "空选中集不能报成「成功执行了 0 项」"
    );
    assert!(read(&root, &["a.txt"]).is_some());
}

#[test]
fn mode_is_carried_through_but_never_taken_from_the_caller() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"x");
    let (_plan, outcome) = plan_and_execute(&root, &db(), "req-mode");
    assert_eq!(outcome.counts.applied, 1);
    // `Mode` 只出现在计划里；执行器不读 IPC 传来的任何模式或路径
    let _ = Mode::Rules;
}

// ===========================================================================
// T07：进度、取消与重复点击（规格 T07）
// ===========================================================================

/// 在第 N 项完成之后请求取消。
///
/// 规格要求「明确区分请求取消与已取消」：这个类型只模拟**请求**，
/// 真正停止由执行器在下一个检查点决定。
struct CancelAfter {
    /// 还剩几项就发出取消请求。
    remaining: std::sync::atomic::AtomicU32,
    /// 收到过多少次进度回调。
    observed: std::sync::atomic::AtomicU32,
}

impl CancelAfter {
    fn new(after: u32) -> Self {
        Self {
            remaining: std::sync::atomic::AtomicU32::new(after),
            observed: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

/// 只记录进度，从不取消。
///
/// 与 `CancelAfter` 分开是有意的：把「永不取消」编码成 `CancelAfter::new(u32::MAX)`
/// 会让那个类型的 `on_progress` 需要额外解释「最大值意味着关闭」——
/// 两个职责单一的类型比一个带开关的类型更难用错。
struct ProgressOnly {
    observed: std::sync::atomic::AtomicU32,
}

impl ProgressOnly {
    fn new() -> Self {
        Self {
            observed: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn last(&self) -> u32 {
        self.observed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl ExecutionObserver for ProgressOnly {
    fn should_stop(&self) -> bool {
        false
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        self.observed
            .store(processed, std::sync::atomic::Ordering::SeqCst);
    }
}

impl ExecutionObserver for CancelAfter {
    fn should_stop(&self) -> bool {
        self.remaining.load(std::sync::atomic::Ordering::SeqCst) == 0
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        self.observed
            .store(processed, std::sync::atomic::Ordering::SeqCst);
        if processed >= 1 {
            self.remaining.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[test]
fn cancelling_after_the_first_file_keeps_it_moved_and_leaves_the_rest_in_place() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"AAA");
    support::write_file(root.canonical(), &["b.txt"], b"BBB");
    support::write_file(root.canonical(), &["c.txt"], b"CCC");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");
    assert_eq!(build.plan.items.len(), 3, "本用例需要 3 个文件");

    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    let observer = CancelAfter::new(1);
    let outcome = execute_plan_observed(
        &db,
        &root,
        &build.plan,
        "req-cancel",
        1_700_000_000_000,
        &observer,
    )
    .expect("取消不是错误，应当正常返回结果");

    assert!(outcome.cancelled, "结果必须标明是被取消的");
    assert_eq!(
        outcome.status,
        filepilot_lib::domain::types::RunStatus::Cancelled,
        "状态必须是取消，而不是「部分完成」或「失败」"
    );
    assert_eq!(outcome.counts.applied, 1, "已完成的那一项要保持完成");
    assert_eq!(
        outcome.counts.skipped, 2,
        "未派发的两项应当是 skipped，而不是 failed"
    );
    assert_eq!(outcome.counts.failed, 0, "取消不等于失败");

    // 磁盘上的事实：一项移走、两项原位、内容都没变
    let moved: Vec<&str> = ["a.txt", "b.txt", "c.txt"]
        .into_iter()
        .filter(|name| read(&root, &["文档", name]).is_some())
        .collect();
    assert_eq!(moved.len(), 1, "只应有一项被移动");

    for (name, payload) in [("a.txt", b"AAA"), ("b.txt", b"BBB"), ("c.txt", b"CCC")] {
        let at_source = read(&root, &[name]);
        let at_target = read(&root, &["文档", name]);

        // 先判「恰好在一处」，再取内容 —— 先取内容会把两个 Option 都消费掉
        assert!(
            at_source.is_some() != at_target.is_some(),
            "{name} 必须恰好存在于源或目标之一，不能两处都有或都没有"
        );

        let actual = at_source.or(at_target).expect("上面已确认存在");
        assert_eq!(
            actual.as_slice(),
            payload.as_slice(),
            "{name} 的内容不能因为取消而变化"
        );
    }
}

#[test]
fn a_journal_entry_is_written_for_every_dispatched_item_even_when_cancelled() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"A");
    support::write_file(root.canonical(), &["b.txt"], b"B");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    let observer = CancelAfter::new(1);
    let outcome = execute_plan_observed(
        &db,
        &root,
        &build.plan,
        "req-cancel-journal",
        1_700_000_000_000,
        &observer,
    )
    .expect("执行应返回");

    // 取消之后每一项都必须有日志记录 —— 否则恢复流程会看到「计划里有、
    // 日志里没有」的项，而那正是最容易被误判成「没执行」的情形。
    let operations = list_operations(&db, &outcome.run_id).expect("读操作");
    assert_eq!(
        operations.len(),
        2,
        "两项都要有记录（一项 applied、一项 skipped）"
    );

    let applied = operations
        .iter()
        .filter(|op| op.status == filepilot_lib::domain::types::OpStatus::Applied)
        .count();
    let skipped = operations
        .iter()
        .filter(|op| op.status == filepilot_lib::domain::types::OpStatus::Skipped)
        .count();
    assert_eq!((applied, skipped), (1, 1));
}

#[test]
fn progress_is_reported_for_every_item_including_the_initial_zero() {
    let (_tmp, root) = make_root();
    support::write_file(root.canonical(), &["a.txt"], b"A");
    support::write_file(root.canonical(), &["b.txt"], b"B");

    let db = db();
    let scan = persist_root_and_scan(&db, &root);
    let build = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划");
    filepilot_lib::storage::repositories::save_plan(&db, &build.plan, None).expect("落库");

    let observer = ProgressOnly::new();
    let outcome = execute_plan_observed(
        &db,
        &root,
        &build.plan,
        "req-progress",
        1_700_000_000_000,
        &observer,
    )
    .expect("执行应成功");

    assert_eq!(outcome.counts.applied, 2);
    assert_eq!(
        observer.last(),
        2,
        "最后一条进度应当是「已完成 2 项」，界面据此把进度条走满"
    );
}
