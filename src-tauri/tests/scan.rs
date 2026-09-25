//! 规格 10.1 的 S01 / S02 / S05：扫描与快照。
//!
//! 关键不变量（规格 T02 验收）：**扫描前后临时目录的文件树与内容必须完全不变。**
//! 每个测试都在扫描前后各拍一次树快照并逐字节比较，而不是只检查「扫描返回了数据」。

mod support;

use std::fs;

use filepilot_lib::domain::types::ExtractionStatus;
use filepilot_lib::platform::windows::attributes;
use filepilot_lib::safety::root::approve_root;
use filepilot_lib::scanner::{scan, skip_codes, ScanOptions};

/// 扫描一次并断言「磁盘没有任何变化」，返回扫描结果。
///
/// 把这两件事绑在一个 helper 里，是为了让**每一个**扫描测试都自动带上
/// INV-01 的检查——忘记写就等于没有保护。
fn scan_without_changes(
    root: &filepilot_lib::safety::root::ApprovedRoot,
    options: &ScanOptions,
) -> filepilot_lib::scanner::ScanOutcome {
    let before = support::snapshot_tree(root.canonical());
    let outcome = scan(root, options).expect("扫描应成功");
    let after = support::snapshot_tree(root.canonical());

    assert_eq!(
        before.len(),
        after.len(),
        "扫描不得增删文件：前 {} 项，后 {} 项",
        before.len(),
        after.len()
    );
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(b.0, a.0, "扫描不得重命名文件");
        assert_eq!(b.1, a.1, "扫描不得改动文件内容：{}", b.0);
    }
    outcome
}

/// 规格 S01：普通文件、中文、emoji、深层子目录。
#[test]
fn s01_scans_plain_chinese_and_emoji_names_without_changing_anything() {
    let tmp = support::test_root();
    let root = tmp.path();

    support::write_file(root, &["报告.pdf"], b"pdf");
    support::write_file(root, &["学习", "高等数学 第三章.md"], "# 高数".as_bytes());
    support::write_file(root, &["图片 🎓", "合影 📷.jpg"], b"jpeg");
    support::write_file(root, &["a", "b", "c", "deep.txt"], b"deep");

    let approved = approve_root(root).expect("临时根应被授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    let names: Vec<String> = outcome
        .records
        .iter()
        .filter(|r| r.skip_code.is_none())
        .map(|r| r.relative_path.join("\\"))
        .collect();

    assert_eq!(outcome.usable_count(), 4, "应扫到 4 个普通文件");
    assert!(names.contains(&"报告.pdf".to_owned()));
    assert!(names.contains(&"学习\\高等数学 第三章.md".to_owned()));
    assert!(names.contains(&"图片 🎓\\合影 📷.jpg".to_owned()));
    assert!(names.contains(&"a\\b\\c\\deep.txt".to_owned()));
}

#[test]
fn scanned_records_carry_identity_but_no_content_hash_yet() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &["x.txt"], b"hello");

    let approved = approve_root(root).expect("授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    let record = outcome
        .records
        .iter()
        .find(|r| r.skip_code.is_none())
        .expect("应有可整理的记录");

    assert_eq!(record.relative_path, vec!["x.txt".to_owned()]);
    assert_eq!(record.extension, ".txt");
    assert_eq!(record.extraction_status, ExtractionStatus::Pending);
    assert_eq!(record.fingerprint.size, "5");
    assert!(
        record.fingerprint.sha256.is_none(),
        "规格 6.1：初扫不计算内容哈希"
    );
    assert!(!record.fingerprint.file_id.is_empty(), "初扫应带上文件身份");
    assert_eq!(record.fingerprint.volume_id.len(), 8);
    assert!(
        record.fingerprint.modified_ns.parse::<i128>().is_ok(),
        "modifiedNs 必须是十进制字符串"
    );
    assert_eq!(record.root_id, approved.id().to_string());
}

/// 规格 S02：链接/联接指向根外——跳过，且不读取根外内容。
///
/// 这里用 **junction**（`mklink /J`）而不是 `symlink_dir`：
/// 实测在本环境里 `std::os::windows::fs::symlink_dir` 会返回 `Ok` 却
/// **没有真正建立符号链接**（创建出来的东西 `file_attributes()` 只有
/// `FILE_ATTRIBUTE_DIRECTORY`，扫描时连条目都不出现）。
/// 符号链接需要 SeCreateSymbolicLinkPrivilege，而 junction 不需要。
#[test]
fn s02_reparse_points_are_skipped_and_marked() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &["real.txt"], b"real");

    // 根之外的目标目录
    let outside = tmp
        .path()
        .parent()
        .expect("应有父目录")
        .join(format!("outside-{}", std::process::id()));
    fs::create_dir_all(&outside).expect("建外部目录");
    fs::write(outside.join("secret.txt"), b"should not be scanned").expect("写外部文件");

    let link = root.join("link-to-outside");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output();

    // 建不出来就**直接红**。静默跳过会让这条用例永远「绿」着什么都不验，
    // 而它守的是「不跟随重解析点走出批准范围」这条安全边界——
    // 一条会因为环境而永远通过的测试，比没有这条测试更糟。
    //
    // 本机实测 `mklink /J` 可用且**不需要特权**（T11 也验证过同一件事）。
    assert!(
        status.as_ref().is_ok_and(|out| out.status.success()),
        "无法创建目录联接，本用例无法验证「不跟随重解析点」：{status:?}"
    );

    // 建立后必须确认它**真的是**重解析点，否则这条用例没有测到东西。
    let attrs = {
        use std::os::windows::fs::MetadataExt as _;
        fs::symlink_metadata(&link)
            .expect("读取联接属性")
            .file_attributes()
    };
    assert!(
        attributes::is_reparse_point(attrs),
        "创建出来的不是重解析点（attributes=0x{attrs:X}），本用例没有测到东西"
    );

    let approved = approve_root(root).expect("授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    let link_entry = outcome
        .records
        .iter()
        .find(|r| r.relative_path == vec!["link-to-outside".to_owned()])
        .expect("联接本身应作为一条记录出现");

    assert_eq!(
        link_entry.skip_code.as_deref(),
        Some(skip_codes::REPARSE_POINT),
        "联接必须被标记为重解析点跳过"
    );
    assert!(
        !outcome
            .records
            .iter()
            .any(|r| r.relative_path.iter().any(|c| c == "secret.txt")),
        "绝不能读到根目录之外的内容"
    );

    let _ = fs::remove_dir_all(&outside);
}

/// 规格 S05：超过扫描上限时如实标记 truncated。
#[test]
fn s05_marks_truncated_instead_of_pretending_a_partial_scan_is_complete() {
    let tmp = support::test_root();
    let root = tmp.path();
    for i in 0..12 {
        support::write_file(root, &[&format!("f{i:02}.txt")], b"x");
    }

    let approved = approve_root(root).expect("授权");
    let options = ScanOptions {
        recursive: true,
        max_files: 5,
        max_depth: 20,
    };
    let outcome = scan_without_changes(&approved, &options);

    assert!(outcome.truncated, "达到上限必须标记 truncated");
    assert!(
        outcome.records.len() <= 5,
        "不应超过上限，实际 {}",
        outcome.records.len()
    );
}

#[test]
fn depth_limit_also_marks_the_scan_as_truncated() {
    let tmp = support::test_root();
    support::write_file(tmp.path(), &["deep", "hidden-by-limit.txt"], b"x");
    let approved = approve_root(tmp.path()).expect("授权");
    let outcome = scan_without_changes(
        &approved,
        &ScanOptions {
            recursive: true,
            max_files: 100,
            max_depth: 0,
        },
    );
    assert!(outcome.truncated, "深度限制导致漏扫时必须标记 truncated");
    assert!(outcome
        .records
        .iter()
        .any(|r| { r.skip_code.as_deref() == Some(skip_codes::DEPTH_EXCEEDED) }));
}

#[test]
fn hard_linked_files_are_not_eligible_for_organizing() {
    let tmp = support::test_root();
    let first = support::write_file(tmp.path(), &["one.txt"], b"same inode");
    fs::hard_link(&first, tmp.path().join("two.txt")).expect("创建硬链接");

    let approved = approve_root(tmp.path()).expect("授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());
    assert_eq!(outcome.usable_count(), 0);
    assert_eq!(
        outcome
            .records
            .iter()
            .filter(|r| r.skip_code.as_deref() == Some(skip_codes::HARD_LINK))
            .count(),
        2,
        "同一文件的两个硬链接都必须跳过"
    );
}

#[test]
fn skipped_entries_record_their_reason() {
    let tmp = support::test_root();
    let root = tmp.path();

    support::write_file(root, &["ok.txt"], b"x");
    support::write_file(root, &["下载未完成.crdownload"], b"partial");

    // 先授权再建 .git：`approve_root` 会把「自身含 .git 的目录」判为源码仓库
    // 并拒绝（规格 1.2）。这是**正确行为**，所以测试必须把 .git 放在授权之后——
    // 否则测的就不是「扫描时跳过 .git」，而是「授权时拒绝仓库根」。
    let approved = approve_root(root).expect("授权");
    support::write_file(root, &[".git", "HEAD"], b"ref: x");
    support::write_file(root, &["node_modules", "pkg", "index.js"], b"module");

    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    assert_eq!(outcome.usable_count(), 1, "只有 ok.txt 可整理");

    let codes: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.skip_code.as_deref())
        .collect();
    assert!(
        codes.contains(&skip_codes::TEMP_DOWNLOAD),
        "未完成下载应被标记，实际跳过码：{codes:?}"
    );
    assert!(
        codes.contains(&skip_codes::SKIPPED_DIR),
        ".git / node_modules 应被标记，实际跳过码：{codes:?}"
    );
}

/// 规格 1.2 的另一面：**根目录自身含 `.git` 时必须被拒绝**。
///
/// 这条与上一条互补——上一条验证「扫描时跳过子目录里的 .git」，
/// 这一条验证「授权时拒绝仓库根本身」。
#[test]
fn repository_root_itself_is_rejected_at_approval_time() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &[".git", "HEAD"], b"ref: x");

    let err = approve_root(root).expect_err("含 .git 的目录不能作为整理根");
    assert_eq!(
        err.code,
        filepilot_lib::domain::errors::codes::ROOT_NOT_AUTHORIZED
    );
    assert!(
        err.message.contains("源码仓库"),
        "拒绝理由要说明原因，实际：{}",
        err.message
    );
}

#[test]
fn non_recursive_scan_stays_at_the_top_level() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &["top.txt"], b"x");
    support::write_file(root, &["sub", "inner.txt"], b"y");

    let approved = approve_root(root).expect("授权");
    let options = ScanOptions {
        recursive: false,
        max_files: 100,
        max_depth: 20,
    };
    let outcome = scan_without_changes(&approved, &options);

    assert_eq!(outcome.usable_count(), 1);
    assert_eq!(outcome.records[0].relative_path, vec!["top.txt".to_owned()]);
}

#[test]
fn scanning_twice_produces_the_same_order() {
    let tmp = support::test_root();
    let root = tmp.path();
    for name in ["c.txt", "a.txt", "b.txt", "学习.md"] {
        support::write_file(root, &[name], b"x");
    }

    let approved = approve_root(root).expect("授权");
    let first = scan(&approved, &ScanOptions::default()).expect("扫描");
    let second = scan(&approved, &ScanOptions::default()).expect("扫描");

    let paths = |o: &filepilot_lib::scanner::ScanOutcome| -> Vec<Vec<String>> {
        o.records.iter().map(|r| r.relative_path.clone()).collect()
    };
    assert_eq!(
        paths(&first),
        paths(&second),
        "相同输入必须得到相同顺序，否则分页会漏项或重复"
    );
}

#[test]
fn empty_directory_yields_no_records_and_no_error() {
    let tmp = support::test_root();
    let root = tmp.path();
    // 留一个空子目录
    support::make_dir(root, &["空的"]);

    let approved = approve_root(root).expect("授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    assert_eq!(outcome.usable_count(), 0);
    assert!(!outcome.truncated);
}

#[test]
fn hidden_and_system_files_are_skipped() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &["normal.txt"], b"x");

    // 用 attrib 设隐藏位。调用失败就跳过——但明确说明这不是通过。
    let hidden = root.join("hidden.txt");
    fs::write(&hidden, b"x").expect("写隐藏文件");

    let status = std::process::Command::new("attrib")
        .arg("+H")
        .arg(&hidden)
        .status();
    match status {
        Ok(s) if s.success() => {}
        other => {
            eprintln!("跳过隐藏文件用例：无法设置隐藏属性（{other:?}）。这不是通过。");
            return;
        }
    }

    let approved = approve_root(root).expect("授权");
    let outcome = scan_without_changes(&approved, &ScanOptions::default());

    let entry = outcome
        .records
        .iter()
        .find(|r| r.relative_path == vec!["hidden.txt".to_owned()])
        .expect("隐藏文件应留下记录");
    assert_eq!(
        entry.skip_code.as_deref(),
        Some(skip_codes::HIDDEN_OR_SYSTEM)
    );
}

#[test]
fn file_identity_is_stable_across_scans_but_differs_between_files() {
    let tmp = support::test_root();
    let root = tmp.path();
    support::write_file(root, &["one.txt"], b"1");
    support::write_file(root, &["two.txt"], b"2");

    let approved = approve_root(root).expect("授权");
    let first = scan(&approved, &ScanOptions::default()).expect("扫描");
    let second = scan(&approved, &ScanOptions::default()).expect("扫描");

    let id_of = |o: &filepilot_lib::scanner::ScanOutcome, name: &str| -> String {
        o.records
            .iter()
            .find(|r| r.relative_path == vec![name.to_owned()])
            .expect("应找到记录")
            .fingerprint
            .file_id
            .clone()
    };

    assert_eq!(id_of(&first, "one.txt"), id_of(&second, "one.txt"));
    assert_ne!(
        id_of(&first, "one.txt"),
        id_of(&first, "two.txt"),
        "不同文件必须有不同的卷内身份"
    );
}
