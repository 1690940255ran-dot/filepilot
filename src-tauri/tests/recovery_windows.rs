//! 规格 T08：启动恢复（P4）。
//!
//! 这个文件测的不是「恢复函数算得对」，而是**进程真的被强杀之后，下一次启动
//! 能不能靠磁盘事实把状态判对**。两者的差别是本任务的全部要点：
//!
//! * 不靠内存记忆 —— 判定所需的一切都在磁盘（文件树）和数据库（journal）里；
//! * 不走 `Drop` —— 子进程用 `std::process::exit` 消失，SQLite 连接不会正常关闭，
//!   WAL 留在磁盘上，和用户按「结束任务」一模一样；
//! * 每个注入点都要有**前后文件树 / 哈希断言**——只断言「状态是 ambiguous」
//!   是不够的，那只证明代码走到了分支，没证明数据还在。
//!
//! ## 为什么父测试要用子进程而不是自己 `exit`
//!
//! `cargo test` 默认在**同一个进程**里多线程跑完全部用例。任何一次
//! `process::exit` 都会连带杀掉同进程的其他用例，于是「通过」会变成
//! 「被邻居顺手干掉」。因此注入点只存在于专用可执行目标
//! `recovery_crash_child` 里，父测试通过 [`Command::new`] 拉起它，
//! 并在 `assert_eq!(status.code(), Some(FAILPOINT_EXIT_CODE))` 里
//! **确认它确实是被注入点杀掉的**，而不是因为别的原因挂掉。
//!
//! ## 文件树断言为什么用哈希而不是 mtime
//!
//! 崩溃后的新进程里，`mtime` 可能因为文件系统精度、时钟调整而漂移；
//! 而内容哈希是规格 8.1 认定的内容身份，也是恢复判定的依据本身。
//! 用同一个工具去测它，比引入一套「测试自己的哈希」更不容易互相印证出错。
//!
//! 这些测试只在 Windows 上跑（NTFS 的文件身份、重解析点、rename 语义）。

#![cfg(windows)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use filepilot_lib::domain::types::{OpResolution, OpStatus, RunStatus};
use filepilot_lib::platform::failpoint::{
    FAILPOINT_AFTER_APPLIED, FAILPOINT_AFTER_DIR_CREATED, FAILPOINT_AFTER_RENAME,
    FAILPOINT_BEFORE_RENAME, FAILPOINT_ENV, FAILPOINT_EXIT_CODE,
};
use filepilot_lib::recovery::{acknowledge_items, reconcile_run, Verdict};
use filepilot_lib::safety::root::{approve_root, ApprovedRoot};
use filepilot_lib::storage::db::Database;
use filepilot_lib::storage::repositories::count_rows;
use filepilot_lib::storage::runs::{
    acknowledge_operation, list_created_dirs, list_events, list_operations, load_run,
};

/// 子进程建出来的文件，与 `recovery_crash_child.rs` 里的 `ITEMS` 必须一致。
///
/// 三份而不是一份：要有「被派发并移动的那份」与「从未进入计划、绝不该被动的那两份」。
/// 只放一份文件的话，「恢复流程顺手把整批重跑一遍」这种错误根本看不出来。
///
/// 这份清单只在**断言文案与说明**里用到（实际取值都直接写字面量，
/// 因为断言要读得出来「在说哪个文件」）；保留它的作用是记住这个约束，
/// 改子进程的文件清单时必须一起改这里。
#[allow(dead_code)]
const ITEMS: [&str; 3] = ["a.txt", "b.txt", "c.txt"];

/// 目标目录名（按类型规则，`.txt` 归到这里）。
const TARGET_DIR: &str = "文档";

// ---------------------------------------------------------------------------
// 工作区与子进程
// ---------------------------------------------------------------------------

/// 一次用例的工作区。
///
/// `root/` 与 `filepilot.sqlite` 都放在 `TempDir` 里，父进程与子进程用的是
/// **同一份**——这正是「新进程打开同一测试 DB 与目录」的实现方式。
struct Workspace {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        // 复用共用的 `support::test_root()`：它保证目录**不落在 %TEMP%**，
        // 否则按规格 1.2 这里的根永远无法通过授权，整套用例会先卡在授权上。
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
/// 它是 `[[bin]]` 目标，所以与测试可执行文件**不在同一个目录**：
/// 测试在 `target/debug/deps/`，bin 在 `target/debug/`。
/// 名字还会带平台后缀（Windows 上是 `.exe`），两处都不能写死。
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

/// 拉起子进程。
///
/// 参数后面**不加** `--`：独立 bin 的 `main` 直接读 `std::env::args()`，
/// 没有 libtest 在前面截胡（那是 `[[test]]` 目标才会有的问题）。
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

/// 建文件树，**不碰数据库**。
fn seed_tree(workspace: &Workspace) {
    let output = run_child(workspace, "seed", "none");
    assert!(
        output.status.success(),
        "seed 失败（退出码 {:?}）：{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 在指定注入点跑一次执行，直到进程被杀。
///
/// 返回子进程的退出码，供调用方断言「确实是被注入点杀的」。
fn crash_at(workspace: &Workspace, point: &str) -> i32 {
    let output = run_child(workspace, "crash", point);

    let code = output.status.code();
    assert_eq!(
        code,
        Some(FAILPOINT_EXIT_CODE),
        "注入点 {point} 没有把进程杀掉（退出码 {code:?}）。\
         若子进程打印了 `survived`，说明这个注入点根本没被走到；\
         若退出码是 101，说明它在注入点之前就 panic 了。\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    FAILPOINT_EXIT_CODE
}

// ---------------------------------------------------------------------------
// 磁盘与日志的取证助手
// ---------------------------------------------------------------------------

/// 把文件树拍平成 `相对路径 -> SHA-256`。
///
/// **只比对哈希，不比 mtime**：崩溃恢复关心的就是内容，而 mtime 在这里
/// 既不稳定也不是判定依据。
fn tree_hashes(root: &Path) -> BTreeMap<String, String> {
    support::snapshot_tree(root)
        .into_iter()
        .map(|(relative, bytes)| (relative, sha256_hex(&bytes)))
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read(root: &Path, relative: &[&str]) -> Option<Vec<u8>> {
    std::fs::read(support::join_under(root, relative)).ok()
}

/// 哪个文件现在待在目标目录里。
///
/// 返回有序列表，这样「a 移过去了、b 没动」这类断言可以直接比较。
fn moved_to_target(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = tree_hashes(root)
        .into_keys()
        .filter_map(|key| {
            let normalized = key.replace('\\', "/");
            normalized
                .strip_prefix(&format!("{TARGET_DIR}/"))
                .map(str::to_owned)
        })
        .collect();
    out.sort();
    out
}

/// 打开崩溃留下的那个库。
fn reopen(workspace: &Workspace) -> Database {
    assert!(
        workspace.db_path().is_file(),
        "数据库文件 {:?} 不存在——子进程应当在被杀之前至少建好它",
        workspace.db_path()
    );
    Database::open(&workspace.db_path()).expect("新进程应能打开崩溃留下的库")
}

/// 唯一一条 run 的 id。
fn only_run_id(db: &Database) -> String {
    let runs = filepilot_lib::storage::runs::list_runs(db, 10).expect("列 run");
    assert_eq!(runs.len(), 1, "本用例只应产生一条 run");
    runs[0].id.clone()
}

/// 按**源文件名**取操作状态（如 `"a.txt" -> Applied`）。
///
/// 用文件名而不是 `itemId` 作键：`itemId` 是规划器生成的内部 id
/// （形如 `itm_xxx`），测试里去猜它的拼法就把测试和实现细节绑在一起了；
/// 而源文件名是测试自己造的，稳定且可读。
fn statuses(db: &Database, run_id: &str) -> BTreeMap<String, OpStatus> {
    list_operations(db, run_id)
        .expect("读操作")
        .into_iter()
        .filter_map(|operation| {
            let name = operation.source.last().cloned()?;
            Some((name, operation.status))
        })
        .collect()
}

/// 按源文件名找到 `operations.id` —— **事件查询要用这个**。
///
/// 别把它和 `ReconciledItem.item_id` 混用——那是 `operations.itemId`：
/// `operation_events.operationId` 的外键指向 `operations(id)`，
/// 而 `operations.itemId` 指向的是计划项。
/// 传错不会报错，只会查到空的事件列表，然后让断言以一个看似无关的数字失败。
fn operation_id_of(db: &Database, run_id: &str, file: &str) -> String {
    list_operations(db, run_id)
        .expect("读操作")
        .into_iter()
        .find(|operation| operation.source.last().map(String::as_str) == Some(file))
        .unwrap_or_else(|| panic!("{file} 应当有一条操作记录"))
        .id
}

/// 在核对结果里按源文件名找那一项。
///
/// 比 `items.iter().find(|i| i.item_id == 某个 id)` 少一次来回，
/// 也少一个用错 id 的机会——这类断言里最常见的 bug 就是拿错了 id
/// （本文件早期版本正是把 `operations.id` 当成了 `itemId`，白查了一轮空列表）。
fn reconciled<'a>(
    outcome: &'a filepilot_lib::recovery::ReconcileOutcome,
    file: &str,
) -> &'a filepilot_lib::recovery::ReconciledItem {
    outcome
        .items
        .iter()
        .find(|item| item.source.last().map(String::as_str) == Some(file))
        .unwrap_or_else(|| {
            panic!(
                "{file} 应当出现在核对结果里，实际有：{:?}",
                outcome
                    .items
                    .iter()
                    .map(|item| item.source.join("/"))
                    .collect::<Vec<_>>()
            )
        })
}

// ===========================================================================
// 每个注入点
// ===========================================================================

/// `before-rename`：意图已落盘，重命名**尚未发生**。
///
/// 期望：磁盘一个字都没动，恢复判定为 `NotApplied`。
#[test]
fn crash_before_rename_leaves_every_file_untouched_and_is_not_applied() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    let before = tree_hashes(&workspace.root());

    crash_at(&workspace, FAILPOINT_BEFORE_RENAME);

    assert_eq!(
        tree_hashes(&workspace.root()),
        before,
        "重命名之前崩溃，文件树必须逐字节不变"
    );

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);

    // 崩溃时日志里只有 prepared —— 这恰恰是「不能靠日志判断」的证明。
    let statuses_before_recovery = statuses(&db, &run_id);
    assert_eq!(
        statuses_before_recovery.get("a.txt"),
        Some(&OpStatus::Prepared),
        "日志停在 prepared：磁盘事实与日志不一致，必须靠核对才知道谁对"
    );

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");

    let a = reconciled(&outcome, "a.txt");
    assert_eq!(
        a.verdict,
        Verdict::NotApplied,
        "源在、目标不在 → 记为未应用；实际：{}",
        a.message
    );
    assert!(
        !outcome.blocks_new_runs,
        "未应用是可重新发起的确定状态，不该阻塞新任务"
    );

    // 核对之后日志被修正成 failed（= 「确定没做成，可以再来」）
    assert_eq!(statuses(&db, &run_id).get("a.txt"), Some(&OpStatus::Failed));
    assert_eq!(tree_hashes(&workspace.root()), before, "核对不得改动文件树");
}

/// `after-rename`：**最危险的一个点**。
///
/// 磁盘已经变了，而日志里只有 `prepared`。恢复流程必须靠磁盘事实判定为
/// `Applied`，且**不能重复移动**（重复移动会撞「目标已存在」而报错，
/// 或者更糟——把文件弄丢）。
#[test]
fn crash_after_rename_is_recognised_as_applied_and_never_moves_again() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    let before = tree_hashes(&workspace.root());

    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    // 磁盘事实：a.txt 已经不在原位、已经在目标目录里
    let after_crash = tree_hashes(&workspace.root());
    assert_ne!(after_crash, before, "重命名已经发生，文件树必然变了");
    assert!(
        read(&workspace.root(), &["a.txt"]).is_none(),
        "崩溃后源位置应当已经空了"
    );
    let expected_payload = b"contents of a.txt".to_vec();
    assert_eq!(
        read(&workspace.root(), &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(expected_payload.as_slice()),
        "文件必须在目标位置，且内容逐字节不变"
    );
    assert_eq!(moved_to_target(&workspace.root()), vec!["a.txt".to_owned()]);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    assert_eq!(
        statuses(&db, &run_id).get("a.txt"),
        Some(&OpStatus::Prepared),
        "日志只有 prepared —— 这正是本任务要防的「磁盘动了、日志没说」"
    );

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");

    let a = reconciled(&outcome, "a.txt");
    assert_eq!(
        a.verdict,
        Verdict::Applied,
        "目标在、身份与哈希都对得上 → 记为已应用；实际：{}",
        a.message
    );

    // 状态映射到已有的 `applied`，不制造第二套词汇
    assert_eq!(
        statuses(&db, &run_id).get("a.txt"),
        Some(&OpStatus::Applied),
        "核对出来的「已应用」应当就是 execution 阶段那个 applied"
    );

    // 关键：**不得重复移动**。文件树必须与崩溃后完全一致。
    assert_eq!(
        tree_hashes(&workspace.root()),
        after_crash,
        "核对过程绝不能再来一次移动"
    );

    // 而且核对本身是幂等的：再跑一次结论不变
    let again = reconcile_run(&db, &root, &run_id, 1_700_000_002_000).expect("第二次核对");
    assert_eq!(
        again.state_digest, outcome.state_digest,
        "重复核对不得改变状态"
    );
    assert_eq!(tree_hashes(&workspace.root()), after_crash);
}

/// `after-applied`：结果已落盘。
///
/// 这一项是**确定完成**的，恢复流程不该再去磁盘核对它，也不该把它标成
/// 需要人工确认。
#[test]
fn crash_after_applied_keeps_the_item_done_and_does_not_block_new_runs() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    crash_at(&workspace, FAILPOINT_AFTER_APPLIED);

    let after_crash = tree_hashes(&workspace.root());
    assert_eq!(
        read(&workspace.root(), &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(b"contents of a.txt".as_slice())
    );

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    // 崩溃时日志已经把 a.txt 写成 applied —— 这是可以信任的确定事实。
    assert_eq!(
        statuses(&db, &run_id).get("a.txt"),
        Some(&OpStatus::Applied),
        "结果已落盘，重启后必须看到 applied"
    );

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");

    assert_eq!(
        statuses(&db, &run_id).get("a.txt"),
        Some(&OpStatus::Applied),
        "已完成的项不得被核对改写"
    );
    assert!(
        outcome
            .items
            .iter()
            .all(|item| item.source.last().map(String::as_str) != Some("a.txt")),
        "已 applied 的项不该出现在待核对列表里——它没有不确定的地方"
    );
    assert_eq!(
        tree_hashes(&workspace.root()),
        after_crash,
        "核对不得改动文件树"
    );
}

/// `after-dir-created`：目录刚建好、身份刚记录。
///
/// 这个点要证明的是：**目录不会因为崩溃被当成陌生目录删掉，也不会被
/// 擅自宣称是本次创建的**。恢复只补一条审计事件，保留原状。
#[test]
fn crash_after_dir_created_keeps_the_directory_and_records_why() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    crash_at(&workspace, FAILPOINT_AFTER_DIR_CREATED);

    assert!(
        workspace.root().join(TARGET_DIR).is_dir(),
        "目录已经建出来了，恢复流程必须保留它——删掉才是真的丢数据"
    );

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);

    let dirs = list_created_dirs(&db, &run_id).expect("读目录记录");
    let created: Vec<_> = dirs.iter().filter(|dir| dir.created_by_run).collect();
    assert_eq!(created.len(), 1, "应当恰好记录了一个新建目录");
    assert_eq!(created[0].relative_path, vec![TARGET_DIR.to_owned()]);
    assert_eq!(created[0].state, "created", "崩溃时目录还处于 created 状态");
    assert!(
        !created[0].directory_identity.is_empty(),
        "身份必须在崩溃前就已经落盘，否则恢复时只能猜"
    );

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");

    // 目录还在，且**没有被重新宣称**成别的东西
    assert!(workspace.root().join(TARGET_DIR).is_dir());
    let dirs_after = list_created_dirs(&db, &run_id).expect("读目录记录");
    assert_eq!(dirs_after.len(), dirs.len(), "核对不得新增/删除目录记录");

    // 审计事件挂在第一条操作上——`operation_events.operationId` 有外键，
    // 造一个 `<runId>#dir` 这种合成 id 只会被约束静默拒掉。
    // 所以这里到**真实操作**的事件流里去找那条记录。
    let host_id = list_operations(&db, &run_id)
        .expect("读操作")
        .first()
        .expect("至少有一条操作")
        .id
        .clone();
    let host_events = list_events(&db, &host_id).expect("读事件");
    let kept = host_events
        .iter()
        .find(|event| event.phase == "recoveryKeptDirectory")
        .unwrap_or_else(|| {
            panic!(
                "必须留下「为什么这个目录还在」的审计线索，实际事件：{:?}",
                host_events
                    .iter()
                    .map(|e| e.phase.as_str())
                    .collect::<Vec<_>>()
            )
        });
    // 载荷要带上 runId 与目录路径，否则「是哪次整理的哪个目录」还得再去别处查
    let payload = kept.payload_json.as_deref().unwrap_or_default();
    assert!(
        payload.contains(&run_id),
        "审计载荷必须带上 runId，实际：{payload}"
    );
    assert!(
        payload.contains(TARGET_DIR),
        "审计载荷必须带上目录路径，实际：{payload}"
    );

    // 文件一个都没动（目录建了，但第一项还没派发）
    assert!(
        read(&workspace.root(), &["a.txt"]).is_some(),
        "目录建好就崩溃，文件必须还在原位"
    );
    assert!(outcome
        .items
        .iter()
        .all(|item| item.verdict != Verdict::Applied));
}

// ===========================================================================
// 规格 8.3 决策表的其余分支
// ===========================================================================

/// 两处都有文件 → `Ambiguous`，**两处都保留**。
///
/// 这是「宁可留着两份、也不猜哪份是垃圾」的直接体现。测试要把两份内容都钉住，
/// 因为最坏的结果不是「判错了」，而是「判错之后顺手删了一个」。
#[test]
fn two_copies_at_both_ends_are_both_kept_and_reported_as_ambiguous() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    // 崩在 after-rename（文件已到目标），然后人为在源位置放一个新的同名文件
    // —— 模拟「用户在这期间又存了一份同名文件」这个真实场景。
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);
    let root = approve_root(&workspace.root()).expect("授权根");
    support::write_file(&workspace.root(), &["a.txt"], b"a DIFFERENT copy");

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");
    let a = reconciled(&outcome, "a.txt");

    assert_eq!(a.verdict, Verdict::Ambiguous, "两处都有 → 绝不自动判定");
    assert!(outcome.blocks_new_runs, "有未决项时必须阻塞新任务");

    // 两份都还在，内容都没被改
    assert_eq!(
        read(&workspace.root(), &["a.txt"]).as_deref(),
        Some(b"a DIFFERENT copy".as_slice()),
        "源位置的那一份必须原样保留"
    );
    assert_eq!(
        read(&workspace.root(), &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(b"contents of a.txt".as_slice()),
        "目标位置的那一份也必须原样保留"
    );
}

/// 两处都不存在 → `Ambiguous`，并在消息里给出**两个路径**。
///
/// 规格要求「展示两处路径」：这是唯一能让用户自己去找的线索，
/// 只报一个「状态不明」等于什么都没说。
#[test]
fn both_ends_missing_is_ambiguous_and_reports_both_paths() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    // 崩在 before-rename（文件还在原位），崩完把源文件删掉：
    // 于是磁盘上两处都没有 —— 这正是「用户在这期间把它删了/移走了」。
    crash_at(&workspace, FAILPOINT_BEFORE_RENAME);
    std::fs::remove_file(workspace.root().join("a.txt")).expect("删掉源文件");

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");
    let a = reconciled(&outcome, "a.txt");

    assert_eq!(a.verdict, Verdict::Ambiguous, "两处都没有 → 不猜");

    // 消息里必须同时提到源与目标，否则用户无从下手
    assert!(
        a.message.contains("a.txt") || a.message.contains("源"),
        "报告必须说明是哪两个位置，实际：{}",
        a.message
    );

    // 结构化的路径本身也带在结果里，界面可以直接渲染成可点击的两行
    assert_eq!(a.source, vec!["a.txt".to_owned()]);
    assert_eq!(a.target, vec![TARGET_DIR.to_owned(), "a.txt".to_owned()]);
    assert!(outcome.blocks_new_runs);
}

/// 身份对得上但**内容变了** → `Ambiguous`。
///
/// 规格的同段要求：「识别出文件」与「满足执行条件」是两件事。
/// 这一条要在真实的崩溃现场验证它——不是靠构造一个假指纹。
#[test]
fn same_file_with_changed_content_at_the_source_is_ambiguous() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    crash_at(&workspace, FAILPOINT_BEFORE_RENAME);
    // 原地改写：卷内身份（fileId）不变，内容变了
    support::write_file(&workspace.root(), &["a.txt"], b"contents of a.txt CHANGED");

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");
    let a = reconciled(&outcome, "a.txt");

    assert_eq!(
        a.verdict,
        Verdict::Ambiguous,
        "文件「找得到」但内容已变，仍然不满足执行条件；实际：{}",
        a.message
    );
    assert!(
        a.message.contains("身份") || a.message.contains("内容"),
        "消息必须点明是身份/内容对不上，而不是笼统地说「找不到」：{}",
        a.message
    );

    // 改动后的内容必须原样保留 —— 恢复不能把它「修回去」
    assert_eq!(
        read(&workspace.root(), &["a.txt"]).as_deref(),
        Some(b"contents of a.txt CHANGED".as_slice())
    );
    assert!(outcome.blocks_new_runs);
}

/// 目标位置被**另一个文件**占了 → `Ambiguous`，绝不覆盖。
#[test]
fn a_different_file_at_the_target_is_ambiguous_and_never_overwritten() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    crash_at(&workspace, FAILPOINT_BEFORE_RENAME);

    // 源文件被移走（模拟用户在别处挪过），目标位置放一个同名但不同内容的东西
    std::fs::remove_file(workspace.root().join("a.txt")).expect("移除源");
    support::make_dir(&workspace.root(), &[TARGET_DIR]);
    support::write_file(&workspace.root(), &[TARGET_DIR, "a.txt"], b"SOMEONE ELSE");

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);

    let root = approve_root(&workspace.root()).expect("授权根");
    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");
    let a = reconciled(&outcome, "a.txt");

    assert_eq!(a.verdict, Verdict::Ambiguous);
    assert_eq!(
        read(&workspace.root(), &[TARGET_DIR, "a.txt"]).as_deref(),
        Some(b"SOMEONE ELSE".as_slice()),
        "目标位置那个不是我们要的文件的**绝不能被覆盖**"
    );
}

// ===========================================================================
// 幂等、可重复调用
// ===========================================================================

/// `reconcile_run` 可以重复调用，结论与磁盘都不变。
///
/// 这条不是形式主义：启动恢复会在每次开应用时跑一遍，
/// 一次不幂等就意味着「多用几次之后状态自己会漂」。
#[test]
fn reconcile_run_is_idempotent_across_repeated_calls() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");

    let first = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("第一次");
    let snapshot_after_first = tree_hashes(&workspace.root());
    assert_eq!(first.items.len(), 1, "第一次核对要处理那一项未决的操作");

    // 第二次开始：那一项已经是确定事实（applied），**不该再出现在核对列表里**。
    // 这本身就是幂等的一种表现——「需要核对的东西」清单也应当收敛，
    // 而不是每次启动都重新摊开一遍已经确定的结果。
    for round in 0..3 {
        let repeat = reconcile_run(&db, &root, &run_id, 1_700_000_002_000 + round)
            .unwrap_or_else(|error| panic!("第 {round} 次重复核对失败: {}", error.message));

        assert_eq!(repeat.status, first.status, "终态必须稳定");
        assert_eq!(repeat.state_digest, first.state_digest, "状态摘要必须稳定");
        assert_eq!(
            repeat.blocks_new_runs, first.blocks_new_runs,
            "阻塞判断必须稳定"
        );
        assert!(
            repeat.items.is_empty(),
            "已确定为 applied 的项不该重新进入待核对列表，实际还剩 {} 项",
            repeat.items.len()
        );
        assert_eq!(
            tree_hashes(&workspace.root()),
            snapshot_after_first,
            "重复核对绝不能碰磁盘"
        );
    }

    // 重复调用也不该把事件刷爆 —— 每次都追加一条「已恢复」是噪音
    let a_id = operation_id_of(&db, &run_id, "a.txt");
    let recovered_events = list_events(&db, &a_id)
        .expect("读事件")
        .into_iter()
        .filter(|event| event.phase == "recoveredApplied")
        .count();
    assert_eq!(
        recovered_events, 1,
        "状态没变时不该重复追加恢复事件，实际写了 {recovered_events} 条"
    );
}

/// 已移动的那一项 + 未在计划里的文件：**判定必须各自准确**。
///
/// 崩溃时只有 `a.txt` 被派发（子进程只选中它），`b.txt` / `c.txt`
/// 从来没有进入过计划，因此它们连操作记录都没有。恢复流程的正确行为是：
/// 给 `a.txt` 判 `Applied`，**并且完全不碰另外两个文件**。
///
/// 这条防的是「恢复流程顺手把整批都重新执行一遍」——
/// 那正是规格禁止的「靠内存记忆恢复」。
#[test]
fn recovery_only_touches_items_that_were_actually_dispatched() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    let before = tree_hashes(&workspace.root());
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");

    let operations = list_operations(&db, &run_id).expect("读操作");
    assert_eq!(
        operations.len(),
        1,
        "只有被派发的那一项才有操作记录；未派发的项根本不该出现在日志里"
    );
    assert_eq!(
        operations[0].source,
        vec!["a.txt".to_owned()],
        "被派发的应当是 a.txt"
    );

    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对应成功");
    assert_eq!(outcome.items.len(), 1, "核对结果里也只该有那一项");
    assert_eq!(outcome.items[0].verdict, Verdict::Applied);

    assert!(!outcome.blocks_new_runs, "没有未决项时不该阻塞新任务");

    // 磁盘事实：a 移走、b 与 c 原位且内容一字不差
    assert_eq!(moved_to_target(&workspace.root()), vec!["a.txt".to_owned()]);
    for name in ["b.txt", "c.txt"] {
        assert_eq!(
            read(&workspace.root(), &[name]).as_deref(),
            Some(format!("contents of {name}").as_bytes()),
            "{name} 从未进入计划，恢复流程绝不能动它"
        );
    }

    // 整体只有 a.txt 的位置变了，其余文件哈希与崩溃前完全一致
    let after = tree_hashes(&workspace.root());
    for (path, hash) in &before {
        if path.contains("a.txt") {
            continue;
        }
        assert_eq!(
            after.get(path),
            Some(hash),
            "{path} 的内容不该因为恢复而改变"
        );
    }
}

// ===========================================================================
// journal 不可读时必须阻断
// ===========================================================================

/// `journal` 本身读不出来时，**阻断执行**，而不是「重置数据库再试」。
///
/// 规格明确点名了这条失败方向。测试用一个损坏的库文件来模拟：
/// 把主文件改成随机字节，恢复流程必须报错而不是返回一个「一切正常」的结果。
///
/// 重点在**不返回 `Ok`**：只要它返回了 `Ok`，界面就会显示一份凭空捏造的
/// 「已恢复」报告，而那比直接报错危险得多。
#[test]
fn an_unreadable_journal_blocks_recovery_instead_of_resetting() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    // 崩溃后把库文件破坏掉，同时把 WAL 一起清走 —— 不这样做的话
    // SQLite 可能仅靠 WAL 就把内容还原出来，那就没测到「读不出来」。
    let db_path = workspace.db_path();
    let wal = db_path.with_extension("sqlite-wal");
    let shm = db_path.with_extension("sqlite-shm");
    let _ = std::fs::remove_file(&wal);
    let _ = std::fs::remove_file(&shm);
    std::fs::write(&db_path, b"this is not a sqlite database").expect("破坏库文件");

    // 新进程连打开都不该成功 —— 这是「阻断」的第一道闸。
    let opened = Database::open(&db_path);
    assert!(
        opened.is_err(),
        "损坏的库必须打不开；若这里成功了，说明恢复流程有可能在坏库上「继续工作」"
    );

    // 文件树必须没被动过：忽略错误、继续执行、或者干脆重置，都会在这里露馅
    assert!(
        read(&workspace.root(), &[TARGET_DIR, "a.txt"]).is_some(),
        "阻断恢复的过程中文件绝不能被动"
    );
}

/// 反向保险：**可用的**库必须能正常核对。
///
/// 上一条如果只测「坏库报错」，那么一个「永远报错」的实现在两条用例上
/// 都能通过。这一条是它的对照组。
#[test]
fn a_healthy_journal_is_reconciled_normally() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");

    let outcome =
        reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("健康的库必须能核对出结果");
    assert!(
        !outcome.state_digest.is_empty(),
        "摘要必须非空——空摘要会让「保留现状并确认知晓」的校验形同虚设"
    );
    assert!(!outcome.items.is_empty(), "健康库里那条未决项必须被核对到");
}

// ===========================================================================
// 释放条件
// ===========================================================================

/// 所有未决项都处理掉之后，新的执行才被解除阻塞。
///
/// 这条钉住的是 `blocks_new_runs` 的**双边**语义：有未决项时必须是 `true`，
/// 处理完必须是 `false`。只测一边的话，一个恒为 `true` 的实现也能通过。
#[test]
fn new_runs_stay_blocked_until_every_unresolved_item_is_settled() {
    let workspace = Workspace::new();
    seed_tree(&workspace);

    // 造出「两处都有」的 ambiguous
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);
    support::write_file(&workspace.root(), &["a.txt"], b"a second copy");
    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");

    let blocked = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    assert!(blocked.blocks_new_runs, "有未决项时必须阻塞");
    assert_eq!(blocked.status, RunStatus::RecoveryRequired);

    // 用户自己把源位置那份挪走 —— 未决因素消失了
    std::fs::remove_file(workspace.root().join("a.txt")).expect("用户自行处理掉那份副本");

    let unblocked = reconcile_run(&db, &root, &run_id, 1_700_000_002_000).expect("再核对");
    assert!(
        !unblocked.blocks_new_runs,
        "冲突已消除，必须解除阻塞；否则用户会永远卡在恢复页"
    );
    assert!(
        unblocked
            .items
            .iter()
            .all(|item| item.verdict == Verdict::Applied),
        "消除冲突后应当只剩「已应用」这一种判定"
    );
    assert_ne!(
        unblocked.status,
        RunStatus::RecoveryRequired,
        "不再有未决项时不该停在 recoveryRequired"
    );
}

/// 保留 runId 与 planId：恢复报告要能指回原始计划。
#[test]
fn the_outcome_points_back_at_the_original_run_and_plan() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let row = load_run(&db, &run_id).expect("读 run").expect("run 应存在");
    let root = approve_root(&workspace.root()).expect("授权根");

    // 崩溃留下的 run 是 `running`：进程没走完 `journal.finish`，
    // 而**只有 `reconcile_run` 才能把它推到终态**。
    // 这正是「启动时先扫 `unfinished_runs` 再核对」这个流程存在的理由。
    assert_eq!(
        row.status, "running",
        "崩溃现场应当停在 running——进程没来得及写终态"
    );

    let outcome = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    assert_eq!(outcome.run_id, run_id);
    assert_eq!(outcome.plan_id, row.plan_id);
    assert!(
        !outcome.plan_id.is_empty(),
        "恢复必须能指回计划，否则用户无法确认「这是哪一次整理」"
    );

    // 核对之后 run 才离开非终态
    let after = load_run(&db, &run_id)
        .expect("再读 run")
        .expect("run 应存在");
    assert_ne!(
        after.status, "running",
        "核对必须把崩溃留下的 run 推进到终态，否则下次启动还会再扫到它"
    );
    assert_eq!(
        after.status, "completed",
        "a.txt 已应用、其余两项确定未应用，整批没有未决项 → completed"
    );
}

/// 不存在的 runId 要明确报错，而不是安静地返回一份空报告。
#[test]
fn an_unknown_run_id_is_an_error_not_an_empty_report() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_BEFORE_RENAME);

    let db = reopen(&workspace);
    let root = approve_root(&workspace.root()).expect("授权根");

    let error = reconcile_run(&db, &root, "no-such-run", 1_700_000_001_000)
        .expect_err("未知 runId 必须报错");
    assert!(
        error.message.contains("no-such-run"),
        "错误信息要带上出问题的 id，实际：{}",
        error.message
    );
}

/// 恢复流程不能凭空造出新的 run。
///
/// 一个容易犯的错是：「恢复」被实现成「开一条新 run 再执行一遍」。
/// 那既不幂等，也绕开了审计。
#[test]
fn recovery_never_creates_a_new_run() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");
    let runs_before = count_rows(&db, "runs").expect("计数");
    let ops_before = count_rows(&db, "operations").expect("计数");

    // 第一次核对会写下判定，事件数会涨；之后再跑就不该再涨了。
    reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("第一次核对");
    let events_settled = count_rows(&db, "operation_events").expect("计数");

    for round in 0..3 {
        reconcile_run(&db, &root, &run_id, 1_700_000_002_000 + round).expect("重复核对");
    }

    assert_eq!(
        count_rows(&db, "runs").expect("计数"),
        runs_before,
        "恢复是核对既有事实，不是重新执行一遍"
    );
    assert_eq!(
        count_rows(&db, "operations").expect("计数"),
        ops_before,
        "操作记录条数不该变"
    );
    assert_eq!(
        ops_before, 1,
        "本用例只派发了一项（子进程只选中 a.txt），所以只该有一条操作记录"
    );

    // **收敛**才是关键：状态不再变化时，反复核对必须不再追加事件。
    // 否则审计表会随着开应用次数线性膨胀，真正有用的记录反而被埋掉。
    assert_eq!(
        count_rows(&db, "operation_events").expect("计数"),
        events_settled,
        "状态稳定之后重复核对不得再追加任何事件"
    );

    // 环境变量不会被恢复流程碰到（它只在测试构建里存在）
    assert_eq!(std::env::var(FAILPOINT_ENV).ok(), None);
}

// ===========================================================================
// 「保留现状并确认知晓」（规格 8.2）
// ===========================================================================

/// 造一个「两处都有文件」的未决场景，返回 (workspace, db, run_id, root)。
///
/// 用 `crash_at(FAILPOINT_AFTER_RENAME)` 让 a.txt 落在目标位置，
/// 再在源位置写一份内容不同的同名文件——这正是规格 8.3 里
/// 「两处都有文件 → ambiguous，保留两者」那一行。
fn ambiguous_scene() -> (Workspace, Database, String, ApprovedRoot) {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    crash_at(&workspace, FAILPOINT_AFTER_RENAME);
    support::write_file(&workspace.root(), &["a.txt"], b"a second copy");

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");
    (workspace, db, run_id, root)
}

/// 确认知晓之后解除阻塞，但**原始判定必须原样保留**。
///
/// 这是整个 T08 里最容易被写错的一条：把「用户接受了这个现状」
/// 实现成「把状态改成 skipped / applied」，界面上立刻就好看了，
/// 但审计记录里再也查不到「这一项其实没做成」。
#[test]
fn acknowledging_keeps_the_ambiguous_verdict_and_only_clears_the_block() {
    let (_workspace, db, run_id, root) = ambiguous_scene();

    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    assert!(before.blocks_new_runs, "两处都有文件时必须先阻塞");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");

    let after = acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        "两处我都留着",
        1_700_000_002_000,
    )
    .expect("确认应当成功");

    assert!(
        !after.blocks_new_runs,
        "全部未决项都已核对并接受后，必须解除新任务的阻塞"
    );
    assert_ne!(
        after.status,
        RunStatus::RecoveryRequired,
        "不再有未处置项时不该停在 recoveryRequired"
    );

    let operations = list_operations(&db, &run_id).expect("读操作");
    let operation = operations
        .iter()
        .find(|op| op.id == operation_id)
        .expect("操作应存在");
    assert_eq!(
        operation.status,
        OpStatus::Ambiguous,
        "磁盘上确实还是两处都有文件，状态不能被改写成别的值"
    );
    assert_eq!(
        operation.resolution,
        OpResolution::Acknowledged,
        "处置结果应当被单独记下来"
    );
}

/// 用户给的理由要落到审计事件里。
///
/// 「保留现状」是个决定，不是默认值。半年后回看时必须能回答
/// 「当时为什么就这样算了」——没有理由的记录等于没有记录。
#[test]
fn the_reason_is_written_into_an_audit_event() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");
    let item_id = list_operations(&db, &run_id)
        .expect("读操作")
        .into_iter()
        .find(|op| op.id == operation_id)
        .expect("操作应存在")
        .item_id;
    let events_before = list_events(&db, &operation_id).expect("读事件").len();

    let reason = "源位置那份是要留的，目标位置那份请忽略";
    acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        reason,
        1_700_000_002_000,
    )
    .expect("确认应当成功");

    let events = list_events(&db, &operation_id).expect("读事件");
    assert_eq!(
        events.len(),
        events_before + 1,
        "确认必须追加**一条**审计事件——不多不少"
    );

    let event = events.last().expect("应有事件");
    assert_eq!(
        event.phase, "recoveryAcknowledged",
        "事件名要能被后续版本识别，不能随手起"
    );
    let payload = event.payload_json.as_deref().expect("事件必须带载荷");
    assert!(
        payload.contains(reason),
        "载荷里必须能看到用户写下的理由，实际是：{payload}"
    );
    // 事件挂在 operation 上，所以载荷里带的是 `itemId`（计划项标识），
    // 不是文件名——文件名在 `operations` 那一行里，查得到。
    let parsed: serde_json::Value = serde_json::from_str(payload).expect("载荷必须是合法 JSON");
    assert_eq!(
        parsed.get("itemId").and_then(|value| value.as_str()),
        Some(item_id.as_str()),
        "载荷要指明是哪一项，实际是：{payload}"
    );
}

/// 理由里的引号和反斜杠不能让载荷变成非法 JSON。
///
/// 载荷是手写 `format!` 拼出来的，而理由是自由文本。用 `{:?}` 才能保证转义；
/// 直接内插的话，一个 `"` 就会把整条审计记录变成读不出来的垃圾。
#[test]
fn a_reason_full_of_quotes_still_produces_valid_json() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");

    let nasty = r#"含 "引号" 与 C:\路径\反斜杠 的理由"#;
    acknowledge_items(&db, &run_id, &before.state_digest, nasty, 1_700_000_002_000)
        .expect("确认应当成功");

    let events = list_events(&db, &operation_id).expect("读事件");
    let payload = events
        .last()
        .and_then(|event| event.payload_json.as_deref())
        .expect("事件必须带载荷");

    let parsed: serde_json::Value =
        serde_json::from_str(payload).expect("载荷必须是合法 JSON，否则这条审计记录就废了");
    assert_eq!(
        parsed.get("reason").and_then(|value| value.as_str()),
        Some(nasty),
        "转义之后原文必须能原样取回"
    );
}

/// 空理由要被拒绝。
#[test]
fn an_empty_reason_is_rejected() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");
    let events_before = list_events(&db, &operation_id).expect("读事件").len();

    let error = acknowledge_items(&db, &run_id, &before.state_digest, "   ", 1_700_000_002_000)
        .expect_err("空理由必须被拒绝");
    assert_eq!(
        error.code,
        filepilot_lib::domain::errors::codes::REQUEST_CONFLICT
    );

    let operations = list_operations(&db, &run_id).expect("读操作");
    assert_eq!(
        operations[0].resolution,
        OpResolution::Open,
        "被拒绝的确认不能留下半个处置结果"
    );
    assert_eq!(
        list_events(&db, &operation_id).expect("读事件").len(),
        events_before,
        "被拒绝的确认不该写任何审计事件"
    );
}

/// 摘要对不上时必须拒绝——这是防「确认了一份过期的报告」的唯一机制。
#[test]
fn a_stale_state_digest_is_refused() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");

    // 先合法确认一次，摘要随之改变
    acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        "第一次",
        1_700_000_002_000,
    )
    .expect("第一次确认应当成功");

    // 再拿**旧摘要**提交：此时事实已经变了，必须拒绝
    let error = acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        "第二次",
        1_700_000_003_000,
    )
    .expect_err("过期摘要必须被拒绝");
    assert_eq!(
        error.code,
        filepilot_lib::domain::errors::codes::STALE_PLAN,
        "过期确认的错误码要能让界面说清「请重新核对」"
    );
}

/// 一批里有部分状态变化时，整批都要拒绝。
///
/// 逐项校验会留下「前几条按旧事实确认、后几条按新事实确认」的混合结果，
/// 那比直接报错更糟：用户以为自己核对的是一屏，实际确认的是两屏。
#[test]
fn the_digest_covers_the_whole_batch_not_just_one_item() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");

    // 篡改任意一位，摘要就不再匹配
    let mut tampered = before.state_digest.clone();
    tampered.push('0');

    let error = acknowledge_items(&db, &run_id, &tampered, "试图绕过核对", 1_700_000_002_000)
        .expect_err("摘要不符必须被拒绝");
    assert_eq!(error.code, filepilot_lib::domain::errors::codes::STALE_PLAN);

    let operations = list_operations(&db, &run_id).expect("读操作");
    assert!(
        operations
            .iter()
            .all(|op| op.resolution == OpResolution::Open),
        "整批拒绝意味着一条都不能被标记"
    );
}

/// 重复提交同一份确认是幂等的：不重复写事件，也不报错。
#[test]
fn acknowledging_twice_does_not_duplicate_the_audit_trail() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let first = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");

    let after = acknowledge_items(&db, &run_id, &first.state_digest, "理由", 1_700_000_002_000)
        .expect("第一次确认");
    let events_after_first = list_events(&db, &operation_id).expect("读事件").len();

    // 用**第一次确认返回的**摘要再提交一遍，模拟界面重试
    let again = acknowledge_items(&db, &run_id, &after.state_digest, "理由", 1_700_000_003_000)
        .expect("重复确认不该报错");
    assert_eq!(
        list_events(&db, &operation_id).expect("读事件").len(),
        events_after_first,
        "同一件事已记过就不再记，否则审计表会被重试刷满"
    );
    assert_eq!(again.state_digest, after.state_digest, "重复确认不改变事实");
}

/// 只确认了一部分时，剩下的未决项仍然阻塞。
#[test]
fn the_block_only_lifts_when_every_item_is_settled() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    assert!(before.blocks_new_runs);

    // 手动只把第一项置为已确认，模拟「还有一项没看」
    let operation_id = operation_id_of(&db, &run_id, "a.txt");
    acknowledge_operation(&db, &operation_id).expect("置为已确认");

    let partial = reconcile_run(&db, &root, &run_id, 1_700_000_004_000).expect("再核对");
    assert!(
        !partial.blocks_new_runs,
        "本场景只有一项未决，确认它就应当解除阻塞"
    );

    // 反向验证：把处置结果退回 open，阻塞必须回来
    // （直接改库，模拟「撤销确认」这种未来可能出现的操作）
    set_resolution_for_test(&db, &operation_id, "open");
    let reblocked = reconcile_run(&db, &root, &run_id, 1_700_000_005_000).expect("第三次核对");
    assert!(
        reblocked.blocks_new_runs,
        "处置被退回后必须重新阻塞——阻塞判定读的是库里的现状，不是内存记忆"
    );
}

/// 已经确认过的项不会因为再次核对而被重置。
///
/// `reconcile_run` 每次启动都跑。如果它顺手把 `resolution` 写回 open，
/// 用户每次开应用都要把同一批东西重新确认一遍。
#[test]
fn reconciling_never_resets_an_acknowledgement() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let after_ack = acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        "理由",
        1_700_000_002_000,
    )
    .expect("确认");

    // 确认这一步本身会改变摘要（resolution 进了摘要），所以基准要取
    // **确认之后**的那一份。此后事实不再变化，摘要就必须稳定。
    let mut previous = after_ack.state_digest.clone();
    for round in 0..3 {
        let outcome =
            reconcile_run(&db, &root, &run_id, 1_700_000_010_000 + round).expect("重复核对");
        assert!(
            !outcome.blocks_new_runs,
            "已确认的项不该被重新算成未决（第 {round} 轮）"
        );
        assert_eq!(
            outcome.state_digest, previous,
            "事实没变，摘要就不该变（第 {round} 轮）——否则用户会被要求反复重新核对"
        );
        previous = outcome.state_digest.clone();
    }

    let operations = list_operations(&db, &run_id).expect("读操作");
    assert_eq!(
        operations[0].resolution,
        OpResolution::Acknowledged,
        "重复核对必须保留处置结果"
    );
}

/// `resolution` 必须进摘要：否则「确认了一部分」与「全部确认」的摘要相同，
/// 用户就能拿一份只核对了第一条时看到的摘要去提交整批确认。
#[test]
fn the_digest_changes_when_an_item_is_acknowledged() {
    let (_workspace, db, run_id, root) = ambiguous_scene();
    let before = reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let after_ack = acknowledge_items(
        &db,
        &run_id,
        &before.state_digest,
        "理由",
        1_700_000_002_000,
    )
    .expect("确认");

    assert_ne!(
        after_ack.state_digest, before.state_digest,
        "处置状态变了摘要就必须变，否则过期确认无从检出"
    );
}

/// 对非 ambiguous 的操作写「已确认」必须失败，而不是静默成功。
#[test]
fn acknowledging_a_non_ambiguous_operation_is_refused() {
    let workspace = Workspace::new();
    seed_tree(&workspace);
    // 这个点位上 a.txt 已经稳稳落在目标位置，判定是 applied
    crash_at(&workspace, FAILPOINT_AFTER_APPLIED);

    let db = reopen(&workspace);
    let run_id = only_run_id(&db);
    let root = approve_root(&workspace.root()).expect("授权根");
    reconcile_run(&db, &root, &run_id, 1_700_000_001_000).expect("核对");
    let operation_id = operation_id_of(&db, &run_id, "a.txt");

    let error = acknowledge_operation(&db, &operation_id)
        .expect_err("applied 的操作不该能写「已确认保留」");
    assert_eq!(
        error.code,
        filepilot_lib::domain::errors::codes::REQUEST_CONFLICT
    );
}

/// 测试用：直接把某一项的处置结果写回去。
///
/// 生产代码没有这个入口（也不该有），这里是为了验证「处置被退回后阻塞会回来」
/// 这条不变量——它保证阻塞判定读的是库里的现状。
fn set_resolution_for_test(db: &Database, operation_id: &str, resolution: &str) {
    db.connection()
        .execute(
            "UPDATE operations SET resolution = ?2 WHERE id = ?1",
            rusqlite::params![operation_id, resolution],
        )
        .expect("测试直改处置结果");
}
