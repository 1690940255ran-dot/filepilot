//! 启动恢复（T08）集成测试的**子进程**入口。
//!
//! 规格 T08 要求「测试启动独立子进程，在每个点强制退出（不走 Drop），
//! 再用新进程打开同一测试 DB 与目录」。
//!
//! ## 为什么是一个独立可执行文件，而不是 `tests/` 下的 `#[test]`
//!
//! `cargo test` 默认在**同一个进程**里多线程跑完全部用例。任何一次
//! `process::exit` 都会连带杀掉同进程的其他用例，于是「通过」与
//! 「被邻居顺手干掉」就分不清了。
//!
//! 也不能放在 `[[test]]` 目标里：那种目标会被链接上 libtest，
//! 而 libtest 的参数解析器会**先于我们的 `main`** 处理命令行。
//! 实测它的行为是：父进程传 `--mode crash` 直接被拒
//! （`Unrecognized option: 'mode'`）；改成 `-- --mode crash` 之后
//! libtest 认为「过滤到的测试数为 0」并**直接返回 0**，我们的代码根本没跑。
//!
//! 所以这里是一个真正的 `[[bin]]`（`required-features = ["failpoints"]`），
//! `main` 由我们自己写。副作用正好是我们想要的：正常 `cargo build`
//! 不会产出它，release 里也就不存在任何可被外部触发的注入入口。
//!
//! ## 参数
//!
//! * `--workspace <dir>`：本次用例的工作区（`root/` 与 `filepilot.sqlite` 都在里面）。
//!   父进程与子进程共用它，这就是「新进程打开同一测试 DB 与目录」的实现。
//! * `--point <name>`：要停在哪个 failpoint。
//! * `--mode <mode>`：
//!   - `seed`：只建文件树，不碰 DB；
//!   - `crash`：建 DB + 执行整理，直到被注入点杀掉；
//!   - `apply-ok`：建 DB + 执行整理并**跑完**（供撤销用例准备现场）；
//!   - `undo`：打开既有 DB，预览并执行撤销，直到被注入点杀掉。

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use filepilot_lib::domain::types::PlanStatus;
use filepilot_lib::executor::execute_plan;
use filepilot_lib::executor::undo::{execute_undo, preview_undo};
use filepilot_lib::platform::failpoint::FAILPOINT_ENV;
use filepilot_lib::safety::root::approve_root;
use filepilot_lib::storage::db::Database;

/// 子进程使用的固定口径的「当前时间」。
///
/// 固定值让落库的 RFC3339 字符串可复现 —— 断言里不需要忽略时间字段。
const NOW_MS: i64 = 1_700_000_000_000;

/// 计划要移动的文件（相对根的路径片段）。
///
/// 三份文件而不是一份：恢复流程必须证明**已完成的项不会被重复移动，
/// 未派发的项还留在原位**。只有一项的文件树看不出「后面的项有没有被动过」。
const ITEMS: [&[&str]; 3] = [&["a.txt"], &["b.txt"], &["c.txt"]];

fn main() -> ExitCode {
    let args = parse_args();
    match args.mode.as_str() {
        "seed" => {
            seed(&args);
            ExitCode::SUCCESS
        }
        "crash" => {
            crash(&args);
            // 走到这里说明注入点**没有**触发 —— 那是个 bug，不是成功。
            eprintln!("failpoint {:?} was never reached", args.point);
            ExitCode::from(70)
        }
        "apply-ok" => {
            // `none` 不是任何注入点的名字，因此这一趟不会被杀掉；
            // 三项全派发，好让撤销用例有完整的现场可撤。
            apply(&args, "none", false);
            ExitCode::SUCCESS
        }
        "undo" => {
            undo(&args);
            eprintln!("undo failpoint {:?} was never reached", args.point);
            ExitCode::from(70)
        }
        other => {
            eprintln!("unknown mode {other}");
            ExitCode::from(64)
        }
    }
}

struct Args {
    workspace: PathBuf,
    point: String,
    mode: String,
}

fn parse_args() -> Args {
    let mut workspace = None;
    let mut point = None;
    let mut mode = None;

    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let value = argv.next().unwrap_or_default();
        match flag.as_str() {
            "--workspace" => workspace = Some(PathBuf::from(value)),
            "--point" => point = Some(value),
            "--mode" => mode = Some(value),
            _ => {}
        }
    }

    Args {
        workspace: workspace.expect("--workspace 必填"),
        point: point.expect("--point 必填"),
        mode: mode.expect("--mode 必填"),
    }
}

fn root_dir(args: &Args) -> PathBuf {
    args.workspace.join("root")
}

fn db_path(args: &Args) -> PathBuf {
    args.workspace.join("filepilot.sqlite")
}

/// 建出文件树但**不碰数据库**。
///
/// 单独一个模式，是为了让父测试能在「数据库还不存在」的时刻先给文件树的
/// 指纹做一份基线 —— 崩溃之后要与这份基线逐字节比对。
fn seed(args: &Args) {
    std::fs::create_dir_all(root_dir(args)).expect("建根目录");
    for relative in ITEMS {
        let path = join_under(&root_dir(args), relative);
        std::fs::create_dir_all(path.parent().expect("有父目录")).expect("建父目录");
        std::fs::write(&path, payload(relative)).expect("写文件");
    }
    // 目标目录**不预先创建**：`after-dir-created` 这个点要测的正是
    // 「目录刚建好、身份刚记录」时崩溃，所以必须让执行器自己去建。
}

/// 打开数据库、落库、执行计划 —— 直到被注入点杀掉。
fn crash(args: &Args) {
    apply(args, &args.point, true);
}

/// 把整批文件都整理过去并跑完，供撤销用例准备现场。
fn apply(args: &Args, point: &str, only_first: bool) {
    // 注入点必须在**任何库操作之前**设好：`fail_point` 每个进程只读一次环境变量，
    // 而这里要保证它读到的是本次要测的那个点。
    std::env::set_var(FAILPOINT_ENV, point);

    let db = Database::open(&db_path(args)).expect("打开测试库");

    let root = approve_root(&root_dir(args)).expect("授权根");
    let scan = filepilot_lib::scanner::scan(&root, &filepilot_lib::scanner::ScanOptions::default())
        .expect("扫描");

    persist_root_and_scan(&db, &root, &scan);

    let mut plan = filepilot_lib::planner::build_rule_plan(
        &root,
        &scan.scan_id.to_string(),
        filepilot_lib::rules::RuleKind::ByType,
        &scan.records,
        std::time::SystemTime::now(),
    )
    .expect("生成计划")
    .plan;

    if only_first {
        // 只派发第一项。
        //
        // 这不是为了「让测试好写」，而是为了让它**像真实崩溃**：进程在第一项上
        // 消失时，后面的项本来就还没被派发。若三项都派发，`after-rename` 这种
        // 「每项都会走到」的点会在第一项之后继续走到第二项，
        // 于是磁盘上会出现两个已移动的文件 —— 那测的就不是「崩在第一项」了。
        //
        // 计划项的 `source` 是相对路径，取首段文件名比对。找不到就 panic：
        // 悄悄跳过会让下面的断言在一个「什么都没移动」的现场上瞎猜。
        let first = plan.items.first().expect("计划至少要有一项").source.clone();
        for item in plan.items.iter_mut() {
            item.selected = item.source == first;
        }
    }

    // 计划必须落库再执行：规格 8.2 要求执行的是**库里的**那份，不是手里的副本。
    filepilot_lib::storage::repositories::save_plan(&db, &plan, None).expect("落库");
    plan = filepilot_lib::storage::repositories::load_plan(&db, &plan.id)
        .expect("读计划")
        .expect("计划应存在");
    plan.status = PlanStatus::Validated;

    let request_id = if only_first {
        "crash-request"
    } else {
        "apply-all-request"
    };
    let outcome = execute_plan(&db, &root, &plan, request_id, NOW_MS)
        .expect("执行本身不应报错（被杀是另一回事）");

    // 没被杀掉时把结果打出来，父测试据此给出可读的失败原因。
    println!(
        "survived run={} applied={}",
        outcome.run_id, outcome.counts.applied
    );
}

/// 预览并执行一次撤销 —— 直到被注入点杀掉。
///
/// 与 `crash` 一样，注入点必须在任何库操作之前设好，否则 `fail_point`
/// 可能已经缓存了「没有注入点」的判定，于是这一趟根本停不下来。
fn undo(args: &Args) {
    std::env::set_var(FAILPOINT_ENV, &args.point);

    let db = Database::open(&db_path(args)).expect("打开测试库");
    let root = approve_root(&root_dir(args)).expect("授权根");

    let original = filepilot_lib::storage::runs::list_runs(&db, 20)
        .expect("列执行记录")
        .into_iter()
        .find(|row| row.direction == "apply" && row.status == "completed")
        .expect("应有一条已完成的整理记录可供撤销");

    let preview = preview_undo(&db, &root, &original.id, NOW_MS).expect("生成撤销预览");
    let selected: Vec<String> = preview
        .items
        .iter()
        .filter(|item| item.selected)
        .map(|item| item.operation_id.clone())
        .collect();

    let report = execute_undo(
        &db,
        &root,
        &preview.undo_plan_id,
        &preview.digest,
        preview.undo_token.as_deref().expect("预览必须签发令牌"),
        &selected,
        "undo-request",
        NOW_MS,
    )
    .expect("撤销本身不应报错（被杀是另一回事）");

    println!(
        "survived undo run={} reverted={} conflicted={}",
        report.run_id, report.reverted, report.conflicted
    );
}

fn persist_root_and_scan(
    db: &Database,
    root: &filepilot_lib::safety::root::ApprovedRoot,
    scan: &filepilot_lib::scanner::ScanOutcome,
) {
    use filepilot_lib::storage::repositories as repo;

    repo::insert_root(
        db,
        &repo::RootRow {
            id: root.id().to_string(),
            canonical_path: root.canonical().display().to_string(),
            volume_id: root.volume_id().to_owned(),
            identity: root.identity().file_id_string(),
            session_id: "crash-session".to_owned(),
        },
    )
    .expect("根落库");

    repo::insert_scan(
        db,
        &repo::ScanRow {
            id: scan.scan_id.to_string(),
            root_id: root.id().to_string(),
            status: "completed".to_owned(),
            recursive: true,
            started_at: "2026-09-17T00:00:00.000Z".to_owned(),
        },
    )
    .expect("扫描落库");

    repo::insert_files(db, &scan.records).expect("文件落库");
}

fn join_under(root: &Path, relative: &[&str]) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative {
        path.push(part);
    }
    path
}

fn payload(relative: &[&str]) -> Vec<u8> {
    format!("contents of {}", relative[0]).into_bytes()
}
