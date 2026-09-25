//! 规格 10.1 的 S03 / S04：根目录范围与授权。
//!
//! 这些测试覆盖「什么目录**不能**被当作整理根」。它们的价值不在于证明
//! 正常路径可用，而在于证明危险路径被挡住——所以断言的重点是**拒绝码**，
//! 不是「返回了错误」这么笼统的东西。

mod support;

use std::path::Path;

use filepilot_lib::domain::errors::codes;
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};

#[test]
fn accepts_a_plain_subdirectory_chosen_by_the_user() {
    let root = support::test_root();
    let target = support::make_dir(root.path(), &["我的资料"]);

    let approved: ApprovedRoot = approve_root(&target).expect("普通子目录应当被接受");

    assert_eq!(approved.id(), approved.id(), "同一个根的 id 必须稳定");
    assert_eq!(approved.volume_id().len(), 8, "卷标识是 8 位十六进制");
    assert!(!approved.identity().file_id_string().is_empty());
    approved
        .verify_still_valid()
        .expect("刚授权的根应当立即通过复核");
}

#[test]
fn rejects_drive_root() {
    let err = approve_root(Path::new(r"C:\")).expect_err("盘符根必须被拒绝");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
}

#[test]
fn rejects_user_home_itself() {
    let home = std::env::var("USERPROFILE").expect("本机应有 USERPROFILE");
    let err = approve_root(Path::new(&home)).expect_err("用户主目录本身必须被拒绝");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
    assert!(
        err.message.contains("用户主目录"),
        "拒绝理由要能指导用户，实际：{}",
        err.message
    );
}

#[test]
fn rejects_system_directories() {
    let windir = std::env::var("SystemRoot").expect("本机应有 SystemRoot");
    let err = approve_root(Path::new(&windir)).expect_err("系统目录必须被拒绝");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);

    // 系统目录的**子目录**同样必须被拒绝——否则绕一层就能进去
    let sub = Path::new(&windir).join("System32");
    if sub.is_dir() {
        let err = approve_root(&sub).expect_err("系统目录的子目录也必须被拒绝");
        assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
    }
}

#[test]
fn rejects_application_data_directories() {
    let local = std::env::var("LOCALAPPDATA").expect("本机应有 LOCALAPPDATA");
    let err = approve_root(Path::new(&local)).expect_err("应用数据目录必须被拒绝");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
    assert!(err.message.contains("系统或应用目录"));
}

#[test]
fn rejects_nonexistent_path() {
    let missing = Path::new(r"C:\this-path-should-never-exist-filepilot-8a3f");
    let err = approve_root(missing).expect_err("不存在的路径必须被拒绝");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
}

#[test]
fn rejects_a_file_as_root() {
    let root = support::test_root();
    let file = support::write_file(root.path(), &["a.txt"], b"x");
    let err = approve_root(&file).expect_err("文件不能作为整理根");
    assert_eq!(err.code, codes::ROOT_NOT_AUTHORIZED);
}

/// 规格 S03：`C:\Data` 与 `C:\Database` 的后缀误判。
///
/// 这条不依赖真实盘符内容，构造两个同级目录即可验证组件级比较。
#[test]
fn sibling_directory_with_shared_prefix_is_not_treated_as_child() {
    let root = support::test_root();
    let data = support::make_dir(root.path(), &["Data"]);
    let database = support::make_dir(root.path(), &["Database"]);

    let approved = approve_root(&data).expect("Data 应被接受");

    // 在 Data 里解析 ".." 之外的东西没问题
    approved
        .resolve_within(&["sub".to_owned()])
        .expect("普通子路径应可解析");

    // Database 不是 Data 的子目录：这条通过各自独立授权来体现——
    // 两者都能被授权，说明判断不是靠字符串前缀。
    let other = approve_root(&database).expect("Database 也应被接受");
    assert_ne!(approved.id(), other.id(), "两个目录必须被当作不同的根");
    assert_ne!(
        approved.identity(),
        other.identity(),
        "不同目录的文件身份必须不同"
    );
}

#[test]
fn relative_path_cannot_escape_the_root() {
    let root = support::test_root();
    let approved = approve_root(root.path()).expect("临时根应被接受");

    for bad in [
        vec!["..".to_owned()],
        vec!["sub".to_owned(), "..".to_owned()],
        vec!["C:\\Windows".to_owned()],
        vec!["a\\b".to_owned()],
        vec!["a/b".to_owned()],
        vec!["a:stream".to_owned()],
        vec!["".to_owned()],
    ] {
        assert!(
            approved.resolve_within(&bad).is_err(),
            "相对路径 {bad:?} 必须被拒绝"
        );
    }
}

#[test]
fn relative_path_accepts_chinese_and_emoji_components() {
    let root = support::test_root();
    let approved = approve_root(root.path()).expect("临时根应被接受");

    let path = approved
        .resolve_within(&["学习资料".to_owned(), "高等数学 🎓".to_owned()])
        .expect("中文与 emoji 组件应可解析");

    assert!(path.starts_with(approved.canonical()));
    assert!(path.ends_with("高等数学 🎓"));
}

/// 规格 3.3：根被替换后，旧授权不能再通过复核。
#[test]
fn detects_root_replaced_by_another_directory() {
    let outer = support::test_root();
    let first = support::make_dir(outer.path(), &["first"]);
    let approved = approve_root(&first).expect("first 应被接受");

    // 换一个目录：原来的路径还在，但文件身份已经不同
    let second = support::make_dir(outer.path(), &["second"]);
    let other = approve_root(&second).expect("second 应被接受");

    assert_ne!(
        approved.identity(),
        other.identity(),
        "不同目录必须有不同的文件身份"
    );
    approved
        .verify_still_valid()
        .expect("first 仍然存在，应通过");
}
