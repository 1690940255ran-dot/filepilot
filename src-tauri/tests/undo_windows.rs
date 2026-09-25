//! 规格 T09：撤销与冲突（规格 8.4）。
//!
//! 这些测试跑在真实 NTFS 上，验的不是「撤销函数算得对」，而是
//! **用户的文件确实回到了原样、或者在不能回的时候一个字节都没被动**。
//!
//! 因此每条断言都落在三件事上：
//!
//! 1. **文件树与字节内容**：正常撤销完成后，与整理之前**逐字节一致**；
//! 2. **冲突时两方都还在**：原位置的新文件和整理后的文件都必须保留
//!    （规格 INV-08：撤销不覆盖后来创建的文件，不擅自搬动被修改的文件）；
//! 3. **分项计数如实**：把部分撤销标成全部成功，会让用户以为文件都回去了。
//!
//! 规格 8.4 的八条对应关系见 `src/executor/undo.rs` 的模块文档。

#![cfg(windows)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use filepilot_lib::domain::types::{OpStatus, RunStatus, UndoOutcome, UndoStatus};
use filepilot_lib::executor::undo::{
    execute_undo, execute_undo_observed, preview_undo, report_for_undo_run, UndoObserver,
};
use filepilot_lib::platform::failpoint::{
    FAILPOINT_EXIT_CODE, FAILPOINT_UNDO_AFTER_RENAME, FAILPOINT_UNDO_BEFORE_RENAME,
};
use filepilot_lib::recovery::reconcile_run;
use filepilot_lib::rules::RuleKind;
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};
use filepilot_lib::storage::db::Database;
use filepilot_lib::storage::runs;

/// 目标目录名（按类型规则，`.txt` 归到这里）。
const TARGET_DIR: &str = "文档";

/// 撤销时用的固定时间，与子进程保持一致。
const NOW_MS: i64 = 1_700_000_000_000;

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn make_root() -> (tempfile::TempDir, ApprovedRoot) {
    let tmp = support::test_root();
    let approved = approve_root(tmp.path()).expect("临时根应被授权");
    (tmp, approved)
}

fn db() -> Database {
    Database::open_in_memory().expect("应能初始化内存库")
}

/// 扫描 → 建计划 → 落库 → 执行，把文件真的整理过去。
///
/// 撤销测试必须从一个**真实执行过**的现场出发：只有那样才有 applied 的操作、
/// 才有真实的文件身份可核对。
fn apply_everything(root: &ApprovedRoot, db: &Database, request_id: &str) -> String {
    let scan = filepilot_lib::scanner::scan(root, &filepilot_lib::scanner::ScanOptions::default())
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
            id: scan.scan_id.to_string(),
            root_id: root.id().to_string(),
            status: "completed".to_owned(),
            recursive: true,
            started_at: "2026-09-19T00:00:00.000Z".to_owned(),
        },
    )
    .expect("扫描应落库");
    filepilot_lib::storage::repositories::insert_files(db, &scan.records).expect("文件记录应落库");

    let mut plan = filepilot_lib::planner::build_rule_plan(
        root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划")
    .plan;
    for item in plan.items.iter_mut() {
        item.selected = true;
    }
    filepilot_lib::storage::repositories::save_plan(db, &plan, None).expect("落库");
    plan = filepilot_lib::storage::repositories::load_plan(db, &plan.id)
        .expect("读计划")
        .expect("计划应存在");
    plan.status = filepilot_lib::domain::types::PlanStatus::Validated;

    let outcome = filepilot_lib::executor::execute_plan(db, root, &plan, request_id, NOW_MS)
        .expect("执行应成功");

    // 只对 **Move** 项断言：`Noop` 项（目标与源相同）本来就不会被派发，
    // 拿 `plan.items.len()` 去比会在「用户先自建了分类目录」的夹具上误报。
    let move_items = plan
        .items
        .iter()
        .filter(|item| item.action == filepilot_lib::domain::types::PlanAction::Move)
        .count();
    assert_eq!(
        outcome.counts.applied, move_items as u32,
        "所有需要移动的项都应当被整理过去，否则撤销的现场不是这里想要的"
    );
    outcome.run_id
}

/// 把整棵树拍平成 `相对路径 -> 字节`，用于「撤销后与原始一致」的断言。
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    support::snapshot_tree(root).into_iter().collect()
}

fn read(root: &ApprovedRoot, relative: &[&str]) -> Option<Vec<u8>> {
    std::fs::read(support::join_under(root.canonical(), relative)).ok()
}

fn write(root: &ApprovedRoot, relative: &[&str], contents: &[u8]) {
    support::write_file(root.canonical(), relative, contents);
}

/// 预览里某一项的判定。
fn outcome_of(preview: &filepilot_lib::domain::types::UndoPreview, relative: &str) -> UndoOutcome {
    preview
        .items
        .iter()
        .find(|item| item.target.join("\\") == relative)
        .unwrap_or_else(|| panic!("预览里找不到 {relative}：{:#?}", preview.items))
        .outcome
}

/// 预览里 `Ready` 的项，按预览给出的顺序。
fn ready_ids(preview: &filepilot_lib::domain::types::UndoPreview) -> Vec<String> {
    preview
        .items
        .iter()
        .filter(|item| item.outcome == UndoOutcome::Ready)
        .map(|item| item.operation_id.clone())
        .collect()
}

/// 预览并执行一次完整撤销。
fn undo_all(
    db: &Database,
    root: &ApprovedRoot,
    original_run_id: &str,
    request_id: &str,
) -> filepilot_lib::domain::types::UndoReport {
    let preview = preview_undo(db, root, original_run_id, NOW_MS).expect("生成撤销预览");
    execute_undo(
        db,
        root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().expect("预览必须签发令牌"),
        &ready_ids(&preview),
        request_id,
        NOW_MS,
    )
    .expect("撤销应正常返回")
}

// ---------------------------------------------------------------------------
// 1. 正常撤销
// ---------------------------------------------------------------------------

#[test]
fn undoing_a_full_run_restores_the_original_tree_byte_for_byte() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"AAA");
    write(&root, &["b.txt"], b"BBB");
    let before = snapshot(root.canonical());

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");
    assert!(
        read(&root, &[TARGET_DIR, "a.txt"]).is_some(),
        "先说清楚现场：整理之后文件应当在目标目录里"
    );

    let report = undo_all(&db, &root, &run_id, "req-undo");

    assert_eq!(report.status, RunStatus::Completed, "全部撤回才算完成");
    assert_eq!(report.reverted, 2, "两项都应当被撤销");
    assert_eq!(report.conflicted, 0, "这一路不该有冲突");
    assert_eq!(report.untouched, 0, "全部都选中了，不该有未处理项");

    // 验收要求：正常路径最终文件树和字节内容与原始一致。
    assert_eq!(
        snapshot(root.canonical()),
        before,
        "撤销之后整棵树必须与整理之前逐字节一致"
    );
}

#[test]
fn undo_can_restore_a_recorded_original_directory_longer_than_the_new_name_limit() {
    let (_tmp, root) = make_root();
    let long_parent = "长".repeat(90);
    write(&root, &[&long_parent, "a.txt"], b"A");
    let before = snapshot(root.canonical());
    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply-long-original");
    let report = undo_all(&db, &root, &run_id, "req-undo-long-original");
    assert_eq!(report.status, RunStatus::Completed);
    assert_eq!(snapshot(root.canonical()), before);
}

#[test]
fn undo_walks_items_in_reverse_sequence() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");
    write(&root, &["b.txt"], b"B");
    write(&root, &["c.txt"], b"C");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let sequences: Vec<u32> = {
        let operations = runs::list_operations(&db, &run_id).expect("读原操作");
        preview
            .items
            .iter()
            .map(|item| {
                operations
                    .iter()
                    .find(|op| op.id == item.operation_id)
                    .expect("预览项必须对应一条真实操作")
                    .sequence
            })
            .collect()
    };

    let mut descending = sequences.clone();
    descending.sort_by(|left, right| right.cmp(left));
    assert_eq!(
        sequences, descending,
        "规格 8.4 第 1 条要求按原 sequence 逆序撤销"
    );
}

#[test]
fn the_original_operation_is_marked_undone_and_an_undo_run_exists() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");
    let report = undo_all(&db, &root, &run_id, "req-undo");

    // 规格 5.1：undoRunId 是 direction=undo 的普通 run。
    let undo_run = runs::load_run(&db, &report.run_id)
        .expect("读撤销 run")
        .expect("撤销 run 应当存在");
    assert_eq!(undo_run.direction, "undo");
    assert_eq!(undo_run.plan_id, {
        runs::load_run(&db, &run_id)
            .expect("读原 run")
            .expect("原 run 应当存在")
            .plan_id
    });

    // 每一项反向操作都指回被撤销的那个原操作。
    let undo_ops = runs::list_operations(&db, &report.run_id).expect("读撤销操作");
    assert_eq!(undo_ops.len(), 1);
    assert!(
        undo_ops[0].original_operation_id.is_some(),
        "撤销操作必须记录 originalOperationId，否则「撤过没有」只能靠内存记忆"
    );
    assert_eq!(undo_ops[0].status, OpStatus::Applied);

    // 原操作的撤销状态被置为 undone。
    let original = runs::find_operation(&db, &undo_ops[0].original_operation_id.clone().unwrap())
        .expect("读原操作")
        .expect("原操作应当存在");
    assert_eq!(original.undo_status, UndoStatus::Undone);
    assert_eq!(
        original.status,
        OpStatus::Applied,
        "撤销不得抹掉「当初确实整理成功了」这个事实"
    );
}

// ---------------------------------------------------------------------------
// 2. 部分撤销
// ---------------------------------------------------------------------------

#[test]
fn undoing_only_one_item_leaves_the_others_in_place_and_says_so() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"AAA");
    write(&root, &["b.txt"], b"BBB");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let ids = ready_ids(&preview);
    assert_eq!(ids.len(), 2);

    let report = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        // 只撤第一项。
        &ids[..1],
        "req-undo-partial",
        NOW_MS,
    )
    .expect("撤销应正常返回");

    assert_eq!(report.reverted, 1);
    assert_eq!(
        report.untouched, 1,
        "没被选中的那一项必须如实计入「未处理」，不能算成已完成"
    );
    assert_eq!(
        report.status,
        RunStatus::Partial,
        "只撤了一部分就不能报成 Completed —— 那会让用户以为文件都回去了"
    );

    // 两项各自的位置：一项回到原处，一项留在目标目录。
    let back_home = ["a.txt", "b.txt"]
        .into_iter()
        .filter(|name| read(&root, &[name]).is_some())
        .count();
    assert_eq!(back_home, 1, "只应有一项回到原位置");

    let still_in_target = ["a.txt", "b.txt"]
        .into_iter()
        .filter(|name| read(&root, &[TARGET_DIR, name]).is_some())
        .count();
    assert_eq!(still_in_target, 1, "另一项应当仍在目标目录里");

    let reloaded = report_for_undo_run(&db, &report.run_id).expect("应能从数据库重读报告");
    assert_eq!(
        reloaded, report,
        "重读报告不能丢掉未处理项、原 runId 或告警"
    );
}

struct CancelUndoAfterOne {
    completed: std::sync::atomic::AtomicU32,
}

impl UndoObserver for CancelUndoAfterOne {
    fn should_stop(&self) -> bool {
        self.completed.load(std::sync::atomic::Ordering::Acquire) >= 1
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        self.completed
            .store(processed, std::sync::atomic::Ordering::Release);
    }
}

#[test]
fn cancelling_undo_stops_before_dispatching_the_next_file() {
    let (_tmp, root) = make_root();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(&root, &[name], name.as_bytes());
    }
    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply-cancel-undo");
    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let ids = ready_ids(&preview);
    let observer = CancelUndoAfterOne {
        completed: std::sync::atomic::AtomicU32::new(0),
    };
    let report = execute_undo_observed(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        &ids,
        "req-undo-cancelled",
        NOW_MS,
        &observer,
    )
    .expect("取消应在安全点收尾");

    assert_eq!(report.status, RunStatus::Cancelled);
    assert_eq!(report.reverted, 1);
    assert_eq!(report.untouched, 2);
    let statuses: Vec<_> = runs::list_operations(&db, &report.run_id)
        .unwrap()
        .into_iter()
        .map(|operation| operation.status)
        .collect();
    assert_eq!(
        statuses,
        vec![OpStatus::Applied, OpStatus::Skipped, OpStatus::Skipped]
    );
    let at_home = ["a.txt", "b.txt", "c.txt"]
        .iter()
        .filter(|name| read(&root, &[*name]).is_some())
        .count();
    assert_eq!(at_home, 1);
}

/// 在第 N 项完成之后请求取消。
///
/// 与 `execute_windows.rs` 里的同名类型同源：本用例需要的是一个**真实的
/// 部分执行现场**（第一项 applied、后面几项 skipped），而不是手工改库造出来的。
struct CancelAfter {
    remaining: std::sync::atomic::AtomicU32,
}

impl filepilot_lib::executor::ExecutionObserver for CancelAfter {
    fn should_stop(&self) -> bool {
        self.remaining.load(std::sync::atomic::Ordering::SeqCst) == 0
    }

    fn on_progress(&self, processed: u32, _total: u32) {
        if processed >= 1 {
            self.remaining.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[test]
fn a_partially_executed_run_only_offers_its_applied_items() {
    // 「部分执行撤销」的另一半含义：原 run 只做成了一部分时，
    // 撤销清单里只该出现**真正 applied** 的那些项。
    //
    // 现场用真实的取消造出来，而不是手工把某一项改成 skipped：
    // `set_operation_status` 的 SQL 自带 `WHERE status NOT IN ('applied')`
    // （T07 的不变量：applied 一旦写下就是事实），手工改根本改不动，
    // 那种夹具只会安静地造出一个「什么都没发生」的假现场。
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");
    write(&root, &["b.txt"], b"B");

    let db = db();
    let scan = filepilot_lib::scanner::scan(&root, &filepilot_lib::scanner::ScanOptions::default())
        .expect("扫描应成功");
    filepilot_lib::storage::repositories::insert_root(
        &db,
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
        &db,
        &filepilot_lib::storage::repositories::ScanRow {
            id: scan.scan_id.to_string(),
            root_id: root.id().to_string(),
            status: "completed".to_owned(),
            recursive: true,
            started_at: "2026-09-19T00:00:00.000Z".to_owned(),
        },
    )
    .expect("扫描应落库");
    filepilot_lib::storage::repositories::insert_files(&db, &scan.records).expect("落库");

    let mut plan = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划")
    .plan;
    for item in plan.items.iter_mut() {
        item.selected = true;
    }
    filepilot_lib::storage::repositories::save_plan(&db, &plan, None).expect("落库");
    plan = filepilot_lib::storage::repositories::load_plan(&db, &plan.id)
        .expect("读计划")
        .expect("计划应存在");
    plan.status = filepilot_lib::domain::types::PlanStatus::Validated;

    let observer = CancelAfter {
        remaining: std::sync::atomic::AtomicU32::new(1),
    };
    let outcome = filepilot_lib::executor::execute_plan_observed(
        &db,
        &root,
        &plan,
        "req-cancelled",
        NOW_MS,
        &observer,
    )
    .expect("取消不是错误");
    assert_eq!(outcome.counts.applied, 1, "先说清楚现场：只做成了一项");
    assert_eq!(outcome.counts.skipped, 1, "另一项因取消而未派发");

    let preview = preview_undo(&db, &root, &outcome.run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        preview.items.len(),
        1,
        "从未派发的项不该出现在撤销清单里：它本来就没被移动过"
    );
    assert_eq!(preview.ready_count, 1);
}

// ---------------------------------------------------------------------------
// 3. 原位置出现新文件（R04 / INV-08）
// ---------------------------------------------------------------------------

#[test]
fn a_new_file_at_the_original_path_blocks_the_undo_and_keeps_both_files() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"ORIGINAL");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");
    assert!(read(&root, &[TARGET_DIR, "a.txt"]).is_some());

    // 用户在整理之后往原位置放了一个**同名但内容不同**的新文件。
    write(&root, &["a.txt"], b"USER WROTE THIS LATER");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        outcome_of(&preview, "a.txt"),
        UndoOutcome::Conflict,
        "原位置被占用必须是冲突"
    );
    let item = preview
        .items
        .iter()
        .find(|item| item.target.join("\\") == "a.txt")
        .unwrap();
    assert!(!item.selected, "规格 8.4 第 3 条：冲突项**默认不选中**");
    assert_eq!(preview.ready_count, 0, "没有可安全撤销的项");
    assert_eq!(preview.conflict_count, 1);

    let report = undo_all(&db, &root, &run_id, "req-undo");

    assert_eq!(report.reverted, 0, "冲突项不能被搬动");
    assert_eq!(report.conflicted, 1);
    assert_eq!(report.status, RunStatus::Partial);

    // R04：新文件与整理文件都保留。
    assert_eq!(
        read(&root, &["a.txt"]).as_deref(),
        Some(b"USER WROTE THIS LATER".as_slice()),
        "用户后来放的新文件必须原样保留"
    );
    assert_eq!(
        read(&root, &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(b"ORIGINAL".as_slice()),
        "整理过去的文件也必须原样保留，不能被覆盖"
    );
}

// ---------------------------------------------------------------------------
// 4. 整理后内容被修改（R05 / INV-08）
// ---------------------------------------------------------------------------

#[test]
fn a_modified_file_is_not_moved_back_by_default() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"ORIGINAL");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    // 用户整理之后又编辑了这个文件。
    write(&root, &[TARGET_DIR, "a.txt"], b"EDITED AFTERWARDS");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        outcome_of(&preview, "a.txt"),
        UndoOutcome::Conflict,
        "内容变了就不是当初那个文件了，不能当成「可以撤」"
    );

    let report = undo_all(&db, &root, &run_id, "req-undo");
    assert_eq!(report.reverted, 0);
    assert_eq!(
        read(&root, &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(b"EDITED AFTERWARDS".as_slice()),
        "被修改过的内容必须原样保留（INV-08：不擅自搬动被修改的文件）"
    );
    assert!(
        read(&root, &["a.txt"]).is_none(),
        "原位置不该凭空多出一个文件"
    );
}

#[test]
fn a_file_replaced_by_another_file_with_the_same_bytes_is_still_a_conflict() {
    // 规格 5.1：文件身份与内容哈希不能互相替代。
    // 删掉再建一个内容完全相同的文件，路径与字节都对得上，**身份**变了。
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"ORIGINAL");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let target = support::join_under(root.canonical(), &[TARGET_DIR, "a.txt"]);
    std::fs::remove_file(&target).expect("删掉已整理的文件");
    std::fs::write(&target, b"ORIGINAL").expect("再建一个同名同内容的");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        outcome_of(&preview, "a.txt"),
        UndoOutcome::Conflict,
        "同名同内容但换了身份的文件必须判成冲突——只比内容会把它放过去"
    );
}

// ---------------------------------------------------------------------------
// 5. 原父目录消失
// ---------------------------------------------------------------------------

#[test]
fn a_vanished_original_parent_directory_is_a_conflict_not_a_recreation() {
    let (_tmp, root) = make_root();
    write(&root, &["学习", "a.txt"], b"NOTES");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");
    assert!(read(&root, &[TARGET_DIR, "a.txt"]).is_some());

    // 用户把原来的目录删了。
    std::fs::remove_dir(support::join_under(root.canonical(), &["学习"])).expect("删掉原目录");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        outcome_of(&preview, "学习\\a.txt"),
        UndoOutcome::Conflict,
        "规格 8.4 第 4 条：原父目录消失要标冲突"
    );

    let report = undo_all(&db, &root, &run_id, "req-undo");
    assert_eq!(report.reverted, 0);
    assert!(
        !support::join_under(root.canonical(), &["学习"]).exists(),
        "v0.1 不擅自重建用户的目录——他删掉它可能正是有意的"
    );
    assert!(
        read(&root, &[TARGET_DIR, "a.txt"]).is_some(),
        "文件必须原样留在目标位置"
    );
}

// ---------------------------------------------------------------------------
// 6. 重复撤销（R06）
// ---------------------------------------------------------------------------

#[test]
fn undoing_twice_reports_already_done_and_never_moves_the_file_again() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let first = undo_all(&db, &root, &run_id, "req-undo-1");
    assert_eq!(first.reverted, 1);
    let after_first = snapshot(root.canonical());

    // 再撤一次：只回报「已完成」，绝不再搬动文件（规格 8.4 第 8 条）。
    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    assert_eq!(preview.already_undone_count, 1);
    assert_eq!(preview.ready_count, 0);
    assert_eq!(outcome_of(&preview, "a.txt"), UndoOutcome::AlreadyUndone);

    let second = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        &[],
        "req-undo-2",
        NOW_MS,
    )
    .expect("重复撤销不是错误");

    assert_eq!(second.reverted, 0, "已经撤过的项不能再被搬动");
    assert_eq!(second.already_undone, 1);
    assert_eq!(second.conflicted, 0);
    assert_eq!(
        snapshot(root.canonical()),
        after_first,
        "第二次撤销不得改动任何文件"
    );
}

#[test]
fn the_same_request_id_undoes_only_once() {
    // 规格 INV-10：重试、重复点击不得让同一操作执行两次。
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let token = preview.undo_token.clone().unwrap();
    let ids = ready_ids(&preview);

    let first = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        &token,
        &ids,
        "req-undo-same",
        NOW_MS,
    )
    .expect("第一次撤销应成功");
    let after_first = snapshot(root.canonical());

    // 完全相同的 requestId 与参数是网络重试：必须返回同一个 run，
    // 而不是因为令牌已消费就让调用方失去第一次执行的结果。
    let second = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        &token,
        &ids,
        "req-undo-same",
        NOW_MS,
    )
    .expect("相同请求重试应返回既有报告");
    assert_eq!(second.run_id, first.run_id);
    assert_eq!(second, first);
    assert_eq!(
        snapshot(root.canonical()),
        after_first,
        "被拒绝的那一次不得留下任何文件改动"
    );
    assert_eq!(first.reverted, 1);
}

#[test]
fn a_request_id_cannot_be_reused_with_different_undo_parameters() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");
    write(&root, &["b.txt"], b"B");
    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply-conflict");
    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let ids = ready_ids(&preview);
    let token = preview.undo_token.as_deref().unwrap();

    execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        token,
        &ids[..1],
        "req-undo-conflict",
        NOW_MS,
    )
    .expect("第一次撤销成功");
    let error = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        token,
        &ids,
        "req-undo-conflict",
        NOW_MS,
    )
    .expect_err("同一 requestId 不能换一组选中项");
    assert_eq!(
        error.code,
        filepilot_lib::domain::errors::codes::REQUEST_CONFLICT
    );
}

#[test]
fn a_new_request_id_cannot_reuse_a_consumed_undo_token() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");
    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply-consumed");
    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let ids = ready_ids(&preview);
    let token = preview.undo_token.as_deref().unwrap();
    execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        token,
        &ids,
        "req-undo-first",
        NOW_MS,
    )
    .unwrap();
    let error = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        token,
        &ids,
        "req-undo-second",
        NOW_MS,
    )
    .expect_err("新的 requestId 不能复用已消费令牌");
    assert_eq!(error.code, filepilot_lib::domain::errors::codes::TOKEN_USED);
}

#[test]
fn a_global_recovery_block_does_not_consume_the_undo_token() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");
    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply-block");
    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let ids = ready_ids(&preview);
    let original = runs::load_run(&db, &run_id).unwrap().unwrap();
    runs::insert_run(
        &db,
        "other-unresolved",
        &original.plan_id,
        "req-other-unresolved",
        "apply",
        "2026-09-19T00:00:00.000Z",
    )
    .unwrap();
    runs::finish_run(
        &db,
        "other-unresolved",
        RunStatus::RecoveryRequired,
        "2026-09-19T00:00:01.000Z",
    )
    .unwrap();

    let blocked = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        &ids,
        "req-undo-after-block",
        NOW_MS,
    )
    .expect_err("另一条未决记录必须阻止撤销");
    assert_eq!(
        blocked.code,
        filepilot_lib::domain::errors::codes::RECOVERY_REQUIRED
    );

    runs::finish_run(
        &db,
        "other-unresolved",
        RunStatus::Completed,
        "2026-09-19T00:00:02.000Z",
    )
    .unwrap();
    let report = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        &ids,
        "req-undo-after-block",
        NOW_MS,
    )
    .expect("恢复阻塞解除后原令牌仍可用");
    assert_eq!(report.reverted, 1);
}

#[test]
fn a_stale_digest_is_refused_before_anything_moves() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let preview = preview_undo(&db, &root, &run_id, NOW_MS).expect("生成撤销预览");
    let before = snapshot(root.canonical());

    // 预览之后、确认之前，用户在原位置放了一个文件 —— 事实变了。
    write(&root, &["a.txt"], b"APPEARED LATER");
    let after_change = snapshot(root.canonical());
    let _ = before;

    let result = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().unwrap(),
        &ready_ids(&preview),
        "req-undo-stale",
        NOW_MS,
    );

    let error = result.expect_err("摘要过期必须被拒绝");
    assert_eq!(error.code, filepilot_lib::domain::errors::codes::STALE_PLAN);
    assert_eq!(
        snapshot(root.canonical()),
        after_change,
        "拒绝要发生在**任何文件改动之前**"
    );
}

// ---------------------------------------------------------------------------
// 7. 目录清理（规格 8.4 第 7 条 / R07）
// ---------------------------------------------------------------------------

#[test]
fn an_empty_directory_created_by_the_run_is_cleaned_up() {
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");
    assert!(support::join_under(root.canonical(), &[TARGET_DIR]).is_dir());

    let report = undo_all(&db, &root, &run_id, "req-undo");
    assert_eq!(report.reverted, 1);
    assert!(
        report.warnings.is_empty(),
        "空目录应当顺利清理，不该有告警：{:#?}",
        report.warnings
    );
    assert!(
        !support::join_under(root.canonical(), &[TARGET_DIR]).exists(),
        "本次整理创建、且已经空的分类目录，撤销后应当被清理掉"
    );
}

#[test]
fn a_directory_that_still_holds_user_files_is_left_alone() {
    // R07：本次目录中后来出现用户文件 → 不递归删除目录。
    let (_tmp, root) = make_root();
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    // 用户往目标目录里放了自己的东西。注意它会被后续扫描看到，
    // 但撤销只关心**原 run 记下的**那些操作，所以它不会被当成待撤销项。
    write(&root, &[TARGET_DIR, "用户的资料.txt"], b"MINE");

    let report = undo_all(&db, &root, &run_id, "req-undo");
    assert_eq!(report.reverted, 1);

    assert!(
        support::join_under(root.canonical(), &[TARGET_DIR]).is_dir(),
        "目录里还有用户的文件，必须原样保留。实际文件树：{:#?}，warnings={:#?}",
        support::snapshot_tree(root.canonical())
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>(),
        report.warnings
    );
    assert_eq!(
        read(&root, &[TARGET_DIR, "用户的资料.txt"]).as_deref(),
        Some(b"MINE".as_slice()),
        "用户的文件一个字节都不能动"
    );
    assert!(
        !report.warnings.is_empty(),
        "目录没能清理应当如实给出告警，而不是悄悄失败"
    );
}

#[test]
fn a_directory_the_user_already_had_is_never_removed() {
    // 规格 8.4 第 7 条：只清理**本次创建**的目录。
    let (_tmp, root) = make_root();
    // 用户自己先建好了分类目录并放了自己的文件。
    write(&root, &[TARGET_DIR, "用户的资料.txt"], b"MINE");
    write(&root, &["a.txt"], b"A");

    let db = db();
    let run_id = apply_everything(&root, &db, "req-apply");

    let report = undo_all(&db, &root, &run_id, "req-undo");
    assert_eq!(report.reverted, 1);

    assert!(
        support::join_under(root.canonical(), &[TARGET_DIR]).is_dir(),
        "用户原有的目录不在清理范围内，哪怕它现在空了也不行"
    );
    assert_eq!(
        read(&root, &[TARGET_DIR, "用户的资料.txt"]).as_deref(),
        Some(b"MINE".as_slice())
    );
}

// ---------------------------------------------------------------------------
// 8. 撤销期间崩溃（R06）
// ---------------------------------------------------------------------------

/// 一次撤销用例的工作区：`root/` 与 `filepilot.sqlite` 都在里面。
struct Workspace {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let tmp = support::test_root();
        let dir = tmp.path().to_path_buf();
        Self { _tmp: tmp, dir }
    }

    fn root(&self) -> PathBuf {
        self.dir.join("root")
    }

    fn db_path(&self) -> PathBuf {
        self.dir.join("filepilot.sqlite")
    }
}

/// 找到 `recovery_crash_child` 可执行文件。
///
/// 它是 `[[bin]]` 目标，因此与测试可执行文件不在同一个目录：
/// 测试在 `target/debug/deps/`，bin 在 `target/debug/`。
fn child_binary() -> PathBuf {
    let mut debug_dir = std::env::current_exe().expect("取当前测试可执行文件路径");
    debug_dir.pop(); // -> target/debug/deps
    debug_dir.pop(); // -> target/debug

    let name = format!("recovery_crash_child{}", std::env::consts::EXE_SUFFIX);
    let path = debug_dir.join(&name);
    assert!(
        path.is_file(),
        "找不到子进程可执行文件：{}。\
         它是 `[[bin]] name = \"recovery_crash_child\"`（`required-features = [\"failpoints\"]`），\
         必须用 `cargo test --features failpoints` 构建才会产出。",
        path.display()
    );
    path
}

fn run_child(workspace: &Workspace, mode: &str, point: &str) -> std::process::Output {
    Command::new(child_binary())
        .arg("--mode")
        .arg(mode)
        .arg("--workspace")
        .arg(&workspace.dir)
        .arg("--point")
        .arg(point)
        .output()
        .unwrap_or_else(|error| panic!("拉起子进程失败（mode={mode}, point={point}）: {error}"))
}

/// 跑到完成为止的一个子进程模式。
fn run_to_completion(workspace: &Workspace, mode: &str) {
    let output = run_child(workspace, mode, "none");
    assert!(
        output.status.success(),
        "子进程 {mode} 未正常结束（退出码 {:?}）：\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 在指定注入点跑一次撤销，直到进程被杀。
fn crash_undo_at(workspace: &Workspace, point: &str) {
    let output = run_child(workspace, "undo", point);
    assert_eq!(
        output.status.code(),
        Some(FAILPOINT_EXIT_CODE),
        "注入点 {point} 没有把进程杀掉（退出码 {:?}）。\
         若子进程打印了 `survived`，说明这个注入点根本没被走到。\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_crash_before_the_rename_leaves_everything_in_place_and_is_retryable() {
    let workspace = Workspace::new();
    run_to_completion(&workspace, "seed");
    run_to_completion(&workspace, "apply-ok");

    let root = approve_root(&workspace.root()).expect("授权根");
    // 逆序撤销：最后派发的 `c.txt`（sequence 最大）最先被处理，
    // 因此注入点停下的是它。这不是巧合，而是规格 8.4 第 1 条的可见后果。
    let applied = read(&root, &[TARGET_DIR, "c.txt"]);
    assert!(applied.is_some(), "先说清楚现场：文件已经整理过去了");

    crash_undo_at(&workspace, FAILPOINT_UNDO_BEFORE_RENAME);

    // 新进程打开同一个库与目录。
    let db = Database::open(&workspace.db_path()).expect("打开崩溃留下的库");
    let root = approve_root(&workspace.root()).expect("重新授权根");
    assert_eq!(
        read(&root, &[TARGET_DIR, "c.txt"]),
        applied,
        "改名之前崩掉，文件必须原样留在目标位置"
    );

    let undo_run = runs::list_runs(&db, 20)
        .expect("列执行记录")
        .into_iter()
        .find(|row| row.direction == "undo")
        .expect("应当有一条被中断的撤销记录");

    let outcome = reconcile_run(&db, &root, &undo_run.id, NOW_MS).expect("核对应成功");
    assert_eq!(
        outcome.status,
        RunStatus::Failed,
        "文件没动过，这次撤销就是没做成，不该停在 recoveryRequired 上吓唬用户"
    );

    // 核对之后那一项应当回到「未撤销」，用户可以重试。
    let original = runs::list_operations(&db, &undo_run.id)
        .expect("读撤销操作")
        .into_iter()
        .find_map(|op| op.original_operation_id)
        .expect("撤销操作应指回原操作");
    let original = runs::find_operation(&db, &original)
        .expect("读原操作")
        .expect("原操作应当存在");
    assert_eq!(
        original.undo_status,
        UndoStatus::NotRequested,
        "核对确认「确实没撤」之后要允许重试，不能永远停在不确定上"
    );

    // 重试确实能成功：三项都应当重新变得可以撤销。
    let apply_run = runs::list_runs(&db, 20)
        .expect("列执行记录")
        .into_iter()
        .find(|row| row.direction == "apply")
        .expect("应有整理记录");
    let preview = preview_undo(&db, &root, &apply_run.id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        preview.ready_count, 3,
        "三项都没被撤过，核对之后应当全部重新可撤"
    );
    assert_eq!(preview.conflict_count, 0, "这一路不该凭空多出冲突");
}

#[test]
fn a_crash_after_the_rename_is_reconciled_from_disk_not_from_the_log() {
    let workspace = Workspace::new();
    run_to_completion(&workspace, "seed");
    run_to_completion(&workspace, "apply-ok");

    let root = approve_root(&workspace.root()).expect("授权根");
    // 同样是逆序最先处理的 `c.txt`。
    let payload = read(&root, &[TARGET_DIR, "c.txt"]).expect("文件已经整理过去");

    crash_undo_at(&workspace, FAILPOINT_UNDO_AFTER_RENAME);

    // 新进程打开同一个库与目录。
    let db = Database::open(&workspace.db_path()).expect("打开崩溃留下的库");
    let root = approve_root(&workspace.root()).expect("重新授权根");

    // 磁盘事实：文件**已经**回到原处了。
    assert_eq!(
        read(&root, &["c.txt"]),
        Some(payload.clone()),
        "改名已经发生，文件应当在原位置"
    );
    assert!(
        read(&root, &[TARGET_DIR, "c.txt"]).is_none(),
        "目标位置不该还留着一份"
    );

    // 日志事实：`undone` 还没来得及写 —— 这正是「不能照抄日志」的场景。
    let undo_run = runs::list_runs(&db, 20)
        .expect("列执行记录")
        .into_iter()
        .find(|row| row.direction == "undo")
        .expect("应当有一条被中断的撤销记录");
    let original_id = runs::list_operations(&db, &undo_run.id)
        .expect("读撤销操作")
        .into_iter()
        .find_map(|op| op.original_operation_id)
        .expect("撤销操作应指回原操作");
    assert_eq!(
        runs::find_operation(&db, &original_id)
            .expect("读原操作")
            .expect("原操作应当存在")
            .undo_status,
        UndoStatus::Prepared,
        "崩溃现场：日志里只有 prepared，磁盘却已经变了"
    );

    // 核对：按「源目标互换」的对应关系判定，得出「撤销其实已经完成」。
    reconcile_run(&db, &root, &undo_run.id, NOW_MS).expect("核对应成功");

    assert_eq!(
        runs::find_operation(&db, &original_id)
            .expect("读原操作")
            .expect("原操作应当存在")
            .undo_status,
        UndoStatus::Undone,
        "核对必须从磁盘事实得出 undone，而不是照抄日志里的字面状态"
    );

    // 幂等（规格 8.3：reconcile_run 多次调用结果一致）。
    let first = reconcile_run(&db, &root, &undo_run.id, NOW_MS).expect("第二次核对");
    let second = reconcile_run(&db, &root, &undo_run.id, NOW_MS).expect("第三次核对");
    assert_eq!(
        first.items.len(),
        second.items.len(),
        "重复核对必须得到同样的判定"
    );

    // 再预览一次：这一项应当已经是「已撤销」，且不会再次搬动文件。
    let apply_run = runs::list_runs(&db, 20)
        .expect("列执行记录")
        .into_iter()
        .find(|row| row.direction == "apply")
        .expect("应有整理记录");
    let preview = preview_undo(&db, &root, &apply_run.id, NOW_MS).expect("生成撤销预览");
    assert_eq!(
        preview.already_undone_count, 1,
        "核对补齐之后，这一项必须被认成「已撤销」，否则用户会再撤一次"
    );
    assert_eq!(preview.ready_count, 2, "另外两项还没撤过，仍然应当可撤");
    assert_eq!(
        read(&root, &["c.txt"]),
        Some(payload),
        "重复核对不得再搬动文件"
    );
}
