//! FilePilot 本地核心。
//!
//! 模块边界（规格 3.2）：
//! ```text
//! React 页面 → 类型化 IPC → Rust commands → application services
//!                                       ├─ scanner / extractors
//!                                       ├─ rules / ai / planner
//!                                       ├─ safety / executor / recovery
//!                                       └─ storage / platform
//! ```
//!
//! P0 只建立 `domain`（纯类型与规则）与最小 `commands` 基线。
//! 其余模块按对应阶段创建，**不用成片空函数冒充框架完成**。

pub mod ai;
pub mod app_state;
pub mod commands;
pub mod commands_analysis;
pub mod commands_execute;
pub mod commands_plan;
pub mod commands_recovery;
pub mod commands_undo;
pub mod domain;
pub mod executor;
pub mod extractors;
pub mod planner;
pub mod platform;
pub mod recovery;
pub mod rules;
pub mod safety;
pub mod scanner;
pub mod storage;

use tauri::Manager as _;

/// 启动桌面应用。
///
/// 库名为 `filepilot_lib`（见 `Cargo.toml`），供集成测试与 `main.rs` 共用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 规格 T07：第二个实例不启动第二个执行器。
    // 用命名互斥体而不是锁文件——进程崩溃时内核会释放互斥体，
    // 锁文件则会留下一个「永远锁着」的残留。
    // 这个变量必须活到 run() 结束，因此**不能**写成 `let _ = ...`。
    let _instance_lock = match platform::windows::acquire_single_instance_lock("FilePilot.单实例")
    {
        Some(lock) => lock,
        None => {
            // 到这一步还没有打开数据库、没有加载 Tauri，退出是干净的。
            eprintln!("FilePilot 已经在运行，本次启动不再打开第二个窗口。");
            return;
        }
    };

    let database = open_application_database();
    let app_state = app_state::AppState::new(database);

    // 规格 8.3：**在窗口起来之前**核对上次没走完的执行。
    //
    // 顺序不能反。等窗口起来再做，用户就有一个窗口可以点下「执行」——
    // 而那次执行本该被上一次的未决项挡住。绑定互斥体保证了同一时刻只有一个实例，
    // 所以这里做一次就够了，不需要在命令里反复检查。
    match recovery::startup::recover_unfinished_runs(
        &app_state,
        safety::confirmation::now_unix_ms(),
    ) {
        Ok(startup) => commands_recovery::log_startup(&startup),
        Err(error) => {
            // 日志读不出来就**不能继续**。规格 8.5：此时的动作是
            // 「停止一切文件操作并告知用户」，而不是「重置数据库后再试」——
            // 后者会销毁唯一的审计依据，用户可能有文件已经被移动过，
            // 却再也没人知道移到了哪里。
            panic!(
                "FilePilot 无法读取执行日志，已停止启动：{}\n\
                 请不要删除数据库文件——它记录了上次执行到哪里。",
                error.message
            );
        }
    }

    purge_startup_cache();

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::get_ocr_status,
            commands::save_provider,
            commands::list_providers,
            commands::test_provider,
            commands::choose_root,
            commands::start_scan,
            commands::cancel_task,
            commands::get_task,
            commands::list_files,
            commands_plan::create_plan,
            commands_analysis::preview_disclosure,
            commands_analysis::grant_disclosure,
            commands_analysis::start_analysis,
            commands_plan::get_plan,
            commands_plan::update_plan,
            commands_plan::validate_plan,
            commands_execute::execute_plan,
            commands_execute::get_run,
            commands_execute::list_runs,
            // PR-003：历史明细按 runId 单独拉，不加宽 list_runs
            commands_execute::get_run_items,
            commands_recovery::get_recovery,
            commands_recovery::acknowledge_recovery,
            commands_recovery::recover_pending_runs,
            commands_recovery::recovery_status,
            commands_undo::preview_undo,
            commands_undo::execute_undo,
            commands_undo::get_undo_report,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<app_state::AppState>();
                if state.is_executing() {
                    api.prevent_close();
                    state.request_shutdown();
                    let window = window.clone();
                    std::thread::spawn(move || {
                        while window.state::<app_state::AppState>().is_executing() {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        let _ = window.close();
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("FilePilot 启动失败：Tauri 运行期初始化出错");
}

/// 打开应用数据库。
///
/// 位置遵循 Windows 惯例：`%LOCALAPPDATA%\FilePilot\filepilot.db`。
/// 规格 8.1 要求计划与执行日志持久化，所以打不开数据库属于**致命错误**：
/// 与其让应用在一个「点确认什么都不会发生」的状态下运行，不如直接失败并说清原因。
fn open_application_database() -> storage::db::Database {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "FilePilot 找不到 Windows LOCALAPPDATA，无法确定持久化执行日志的位置。\n\
                 已停止启动，不会退化到临时目录，以免恢复记录被系统清理。"
            )
        });
    let path = base.join("FilePilot").join("filepilot.db");

    storage::db::Database::open(&path).unwrap_or_else(|error| {
        panic!(
            "FilePilot 无法打开数据库 {}: {}\n请确认该目录可写。",
            path.display(),
            error.message
        )
    })
}

/// 启动时清一次应用缓存（规格 6.2：「任务结束与下次启动清理」）。
///
/// ## 为什么它不返回结果、也不报错
///
/// 缓存清不掉**不该阻止应用启动**：那是一堆可以再生的中间产物，
/// 而用户在等的是一个能用的窗口。所以三条失败路径都不往上抛，
/// 只记一行日志——包括路径自查没过（那说明代码有缺陷，
/// 更不该让它影响用户）。
///
/// ## 它与数据库的关系
///
/// 数据库在 `%LOCALAPPDATA%\FilePilot\filepilot.db`，缓存在同一层下的
/// `cache\`。`purge` 只删 `cache\` **里面**的东西，数据库在它的外面——
/// 规格要求「清缓存不清操作日志」，而操作日志是唯一的审计依据。
///
/// `purge` 自己会先确认那个路径**确实是**缓存目录（必须以
/// `FilePilot\cache` 结尾）。这一步不靠调用方传对，所以就算哪天
/// 这里的路径算错了，结果也是一次明确的拒绝，而不是一次误删。
fn purge_startup_cache() {
    let cache_dir = platform::cache::app_cache_dir();

    match platform::cache::purge(&cache_dir) {
        Ok(report) if report.is_empty() => {
            // 第一次启动就是这条路：没有缓存要清，不必刷日志。
        }
        Ok(report) => {
            eprintln!(
                "已清理应用缓存：{} 个文件、{} 个空目录（{}）",
                report.removed_files,
                report.removed_dirs,
                cache_dir.display()
            );
            // 删不掉的如实说出来。用户看到「缓存里还有东西」时，
            // 至少能在日志里找到是哪一条、为什么。
            for skipped in &report.skipped {
                eprintln!("缓存未能清理：{skipped}");
            }
        }
        Err(error) => {
            eprintln!("跳过缓存清理：{}", error.message);
        }
    }
}
