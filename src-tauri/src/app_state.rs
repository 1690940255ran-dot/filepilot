//! 应用运行期状态。
//!
//! 规格 8.1：任务和授权的**运行期**状态可以保存在内存；
//! 但凡涉及文件变更的计划、操作、撤销计划与确认**必须持久化**。
//! 目前还没有存储层（T04），所以：
//! - **根授权、扫描结果、任务状态**存在内存里——它们是运行期状态，进程重启后需要重来；
//! - **设置**同样只在内存中，并且**不假装**已经落到磁盘。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use crate::domain::errors::{codes, AppError};
use crate::domain::types::{
    AppSettings, FilePage, FileRecord, RootSummary, ScanSummary, TaskError, TaskStatus, TaskSummary,
};
use crate::safety::confirmation::ConfirmationStore;
use crate::safety::root::ApprovedRoot;
use crate::storage::db::Database;

/// 分页的默认与上限。规格 5.2：`limit` 取值范围 1–200。
pub const DEFAULT_PAGE_LIMIT: u32 = 50;
pub const MAX_PAGE_LIMIT: u32 = 200;

/// 一次扫描在内存中的留存形态。
struct StoredScan {
    root_id: String,
    records: Vec<FileRecord>,
    truncated: bool,
}

/// 一个任务在内存中的留存形态。
struct StoredTask {
    status: TaskStatus,
    processed: u32,
    total: Option<u32>,
    scan_id: Option<String>,
    error: Option<TaskError>,
    cancellation: Arc<AtomicBool>,
}

#[derive(Default)]
struct WorkflowGate {
    execution: Option<String>,
    active_scans: usize,
}

/// 全局执行锁的持有凭证。drop 即释放。
pub struct ExecutionGuard {
    gate: Arc<Mutex<WorkflowGate>>,
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        self.gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution = None;
    }
}

/// 扫描占用凭证。允许扫描彼此并行，但与任何文件变更互斥。
pub struct ScanGuard {
    gate: Arc<Mutex<WorkflowGate>>,
}

impl std::fmt::Debug for ScanGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ScanGuard")
    }
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        let mut gate = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        gate.active_scans = gate.active_scans.saturating_sub(1);
    }
}

/// 手写 `Debug` 而不是派生。
///
/// 派生要求 `AppState` 也实现 `Debug`，而它内部握着数据库连接与若干锁——
/// 给那些东西打印内容既没有意义，也容易在将来不小心把敏感信息写进日志。
/// 这里只需要一个名字：它唯一的用途是让 `Result::expect_err` 之类的调用能编译。
impl std::fmt::Debug for ExecutionGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ExecutionGuard")
    }
}

pub struct AppState {
    settings: Mutex<AppSettings>,
    /// rootId -> 授权对象。**前端只持有 rootId**，换不回路径就伪造不出授权。
    roots: Mutex<HashMap<String, ApprovedRoot>>,
    scans: Mutex<HashMap<String, StoredScan>>,
    tasks: Mutex<HashMap<String, StoredTask>>,
    /// 持久层。规格 8.1：涉及文件变更的计划**必须落库**，
    /// 因此它不是可选的「以后再接」项，而是构造时就要求传入。
    db: Database,
    /// 一次性确认令牌。运行期状态，重启即失效（这是期望行为）。
    confirmations: ConfirmationStore,
    /// 待发送载荷与它们的授权（T13）。
    ///
    /// 与 `confirmations` 一样是**运行期状态**，而且理由是规格明写的：
    /// 「缓存失效或**程序重启后重新预览授权**」。落库会让「上次授权过」
    /// 跨越重启，而用户对一个月前那次授权的记忆早就没了——那时他看到的
    /// 是一份自己完全不记得的载荷被发了出去。
    disclosures: crate::ai::disclosure::ConsentStore,
    /// 会话内的提取缓存（T11）。
    ///
    /// 规格 8.1：`Extraction.text` 是「**仅内存或会话临时缓存**，不写持久日志」。
    /// 它同时是「预览一次、授权后发送」之间不重复提取的依据——
    /// 没有它，用户每改一次 instruction 都要把整批文件重新提取一遍。
    extraction_cache: Mutex<crate::platform::cache::SessionCache>,
    /// 全局执行锁。
    ///
    /// 规格 T07 要求「全局执行锁落到实处」：同一时间只允许一个 run 在改动文件。
    /// 即使界面被绕过（两个窗口、两次 IPC），第二个执行也必须被挡住。
    /// 存的是正在执行的计划 id，便于把冲突说清楚。
    workflow_gate: Arc<Mutex<WorkflowGate>>,
    /// 用户在执行中关闭窗口时置位；新登记的任务也会立即看到取消。
    shutdown_requested: AtomicBool,
    /// 本次应用会话的标识。
    ///
    /// 规格 8.1 要求根授权记录它：重启后必须重新授权同一根目录。
    /// 每次启动换一个值，于是「上一次会话授权的根」在库里一眼可辨。
    session_id: String,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            roots: Mutex::new(HashMap::new()),
            scans: Mutex::new(HashMap::new()),
            tasks: Mutex::new(HashMap::new()),
            db,
            confirmations: ConfirmationStore::new(),
            disclosures: crate::ai::disclosure::ConsentStore::new(),
            extraction_cache: Mutex::new(crate::platform::cache::SessionCache::new()),
            workflow_gate: Arc::new(Mutex::new(WorkflowGate::default())),
            shutdown_requested: AtomicBool::new(false),
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// 持久层句柄，供计划读写使用。
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// 待发送载荷的授权存储。
    pub fn disclosures(&self) -> &crate::ai::disclosure::ConsentStore {
        &self.disclosures
    }

    /// 会话内的提取缓存。
    ///
    /// 返回 `MutexGuard` 而不是 `&SessionCache`：它的 `get` 与 `put` 都要
    /// `&mut self`，所以调用方必须拿着锁。返回 guard 让「这里取得了锁、
    /// 锁会持有到什么时候」在调用点看得见——而不是藏在某个内部方法里。
    pub fn extraction_cache(
        &self,
    ) -> std::sync::MutexGuard<'_, crate::platform::cache::SessionCache> {
        // 毒化不影响正确性：缓存最坏是少一份正文，而那会退化成「重新提取一次」。
        self.extraction_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 一次性令牌仓库。
    pub fn confirmations(&self) -> &ConfirmationStore {
        &self.confirmations
    }

    /// 本次应用会话的标识，写进 `roots.sessionId`。
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    // ---------- 全局执行锁 ----------

    /// 尝试取得全局执行锁。
    ///
    /// 返回的 guard 在 drop 时自动释放——**包括 panic 展开**。
    /// 用显式的 acquire/release 一对调用很容易在提前 return 的分支上漏掉释放，
    /// 而那会把应用锁死到下次重启。
    pub fn begin_execution(&self, plan_id: &str) -> Result<ExecutionGuard, AppError> {
        let mut gate = self.lock(&self.workflow_gate);
        if let Some(holder) = gate.execution.as_ref() {
            return Err(AppError::new(
                codes::TASK_BUSY,
                format!("已经有一个整理任务正在执行（计划 {holder}），请等它结束"),
            ));
        }
        if gate.active_scans > 0 {
            return Err(AppError::new(
                codes::TASK_BUSY,
                "扫描仍在进行，请等扫描结束后再执行文件变更",
            ));
        }
        gate.execution = Some(plan_id.to_owned());
        drop(gate);
        Ok(ExecutionGuard {
            gate: Arc::clone(&self.workflow_gate),
        })
    }

    /// 扫描与整理/撤销共用同一闸门，不能只靠界面按钮避免并发。
    pub fn begin_scan(&self) -> Result<ScanGuard, AppError> {
        let mut gate = self.lock(&self.workflow_gate);
        if let Some(holder) = gate.execution.as_ref() {
            return Err(AppError::new(
                codes::TASK_BUSY,
                format!("计划 {holder} 正在执行，请等文件变更结束后再扫描"),
            ));
        }
        gate.active_scans += 1;
        drop(gate);
        Ok(ScanGuard {
            gate: Arc::clone(&self.workflow_gate),
        })
    }

    /// 当前是否有执行在跑。仅供诊断与测试。
    pub fn is_executing(&self) -> bool {
        self.lock(&self.workflow_gate).execution.is_some()
    }

    // ---------- 设置 ----------

    /// 读取当前设置的一份副本。
    ///
    /// 返回值**不含任何密钥**：规格 3.3 要求 API Key 只进 Windows 凭据存储。
    pub fn settings(&self) -> AppSettings {
        self.lock(&self.settings).clone()
    }

    // ---------- 根授权 ----------

    /// 登记一个刚授权的根，返回给前端的摘要。
    pub fn register_root(&self, root: ApprovedRoot) -> RootSummary {
        let summary = RootSummary {
            root_id: root.id().to_string(),
            display_path: root.display().to_owned(),
            volume_id: root.volume_id().to_owned(),
        };
        self.lock(&self.roots).insert(summary.root_id.clone(), root);
        summary
    }

    /// 按 rootId 换回授权对象。
    ///
    /// 这是**唯一**把 rootId 变成可用路径的入口。找不到就返回 `None`，
    /// 调用方必须把它转成 `ROOT_NOT_AUTHORIZED`，绝不能退化成「用返回的路径继续」。
    pub fn root(&self, root_id: &str) -> Option<ApprovedRoot> {
        self.lock(&self.roots).get(root_id).cloned()
    }

    // ---------- 扫描 ----------

    pub fn store_scan(
        &self,
        outcome: crate::scanner::ScanOutcome,
    ) -> (String, u32, u32, u32, bool) {
        let usable = outcome.usable_count() as u32;
        let skipped = outcome.skipped_count() as u32;
        let total = outcome.records.len() as u32;
        let truncated = outcome.truncated;
        let scan_id = outcome.scan_id.to_string();

        self.lock(&self.scans).insert(
            scan_id.clone(),
            StoredScan {
                root_id: outcome.root_id.to_string(),
                records: outcome.records,
                truncated,
            },
        );

        (scan_id, total, usable, skipped, truncated)
    }

    /// 按 scanId 组装一页文件。
    ///
    /// 游标用的是**上一页最后一条记录的 id**，不是偏移量：
    /// 偏移量在底层列表变化时会漏项或重复，而 id 定位对内容变化更稳。
    pub fn page(&self, scan_id: &str, cursor: Option<&str>, limit: u32) -> Option<FilePage> {
        let scans = self.lock(&self.scans);
        let scan = scans.get(scan_id)?;

        let limit = limit.clamp(1, MAX_PAGE_LIMIT) as usize;
        let total = scan.records.len();

        let start = match cursor {
            None => 0,
            Some(c) => match scan.records.iter().position(|r| r.id == c) {
                Some(i) => i + 1,
                // 游标指向的记录已经不在（scan 被替换或数据被清），
                // 视为越过末尾而不是从头开始——从头开始会让界面无限翻同一页。
                None => total,
            },
        };

        let end = (start + limit).min(total);
        let items = scan.records[start..end].to_vec();
        let next_cursor = if end < total {
            items.last().map(|r| r.id.clone())
        } else {
            None
        };

        Some(FilePage {
            items,
            next_cursor,
            total: total as u32,
        })
    }

    /// 取某次扫描的记录，供规划器构建计划。
    ///
    /// 返回克隆而不是引用：扫描表由 `Mutex` 保护，把引用借出去会把锁的生命周期
    /// 泄漏到调用方，而规划过程本身可能又回来查别的表，容易造成死锁。
    pub fn scan_records(&self, scan_id: &str) -> Option<Vec<FileRecord>> {
        self.lock(&self.scans)
            .get(scan_id)
            .map(|scan| scan.records.clone())
    }

    /// 扫描结果是否被截断（界面需要据此提示缩小范围）。
    pub fn scan_truncated(&self, scan_id: &str) -> Option<bool> {
        self.lock(&self.scans).get(scan_id).map(|s| s.truncated)
    }

    /// 该 scan 对应的根目录 id。
    pub fn scan_root_id(&self, scan_id: &str) -> Option<String> {
        self.lock(&self.scans)
            .get(scan_id)
            .map(|s| s.root_id.clone())
    }

    // ---------- 任务 ----------

    /// 登记一个新任务，初始为 `running`。
    pub fn start_task(&self, total: Option<u32>) -> String {
        let task_id = uuid::Uuid::new_v4().to_string();
        let cancellation = Arc::new(AtomicBool::new(
            self.shutdown_requested.load(Ordering::Acquire),
        ));
        self.lock(&self.tasks).insert(
            task_id.clone(),
            StoredTask {
                status: TaskStatus::Running,
                processed: 0,
                total,
                scan_id: None,
                error: None,
                cancellation,
            },
        );
        task_id
    }

    /// 把任务标记为完成，并挂上扫描结果。
    pub fn finish_task(&self, task_id: &str, scan_id: &str, summary: &ScanSummary) {
        if let Some(task) = self.lock(&self.tasks).get_mut(task_id) {
            task.status = if summary.truncated {
                TaskStatus::Partial
            } else {
                TaskStatus::Completed
            };
            task.processed = summary.total;
            task.total = Some(summary.total);
            task.scan_id = Some(scan_id.to_owned());
            task.error = None;
        }
    }

    /// 按给定状态收尾。
    ///
    /// 扫描与执行共用任务表，但收尾状态不同（执行可能是 `partial` 或 `cancelled`），
    /// 因此提供一个通用入口，而不是给每种情形各写一个方法。
    pub fn finish_task_with_status(&self, task_id: &str, status: TaskStatus) {
        if let Some(task) = self.lock(&self.tasks).get_mut(task_id) {
            task.status = status;
        }
    }

    /// 把任务标记为失败。规格 0.8：失败必须带结构化错误，不能吞异常。
    pub fn fail_task(&self, task_id: &str, error: TaskError) {
        if let Some(task) = self.lock(&self.tasks).get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.error = Some(error);
        }
    }

    pub fn cancellation_token(&self, task_id: &str) -> Option<Arc<AtomicBool>> {
        self.lock(&self.tasks)
            .get(task_id)
            .map(|task| Arc::clone(&task.cancellation))
    }

    /// 只发出取消请求；扫描线程观察到以后才进入 `cancelled`。
    pub fn request_cancel(&self, task_id: &str) -> bool {
        let tasks = self.lock(&self.tasks);
        let Some(task) = tasks.get(task_id) else {
            return false;
        };
        if task.status != TaskStatus::Running && task.status != TaskStatus::Queued {
            return false;
        }
        task.cancellation.store(true, Ordering::Release);
        true
    }

    /// 关闭窗口时停止派发所有任务；当前原子文件操作仍会先安全落日志。
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Release);
        for task in self.lock(&self.tasks).values() {
            if matches!(task.status, TaskStatus::Running | TaskStatus::Queued) {
                task.cancellation.store(true, Ordering::Release);
            }
        }
    }

    pub fn finish_cancelled(&self, task_id: &str) {
        if let Some(task) = self.lock(&self.tasks).get_mut(task_id) {
            task.status = TaskStatus::Cancelled;
            task.total = None;
            task.scan_id = None;
            task.error = None;
        }
    }

    /// 规格 5.2：`get_task` 支持重新打开界面查询。
    pub fn task(&self, task_id: &str) -> Option<TaskSummary> {
        let tasks = self.lock(&self.tasks);
        let task = tasks.get(task_id)?;
        Some(TaskSummary {
            task_id: task_id.to_owned(),
            status: task.status,
            processed: task.processed,
            total: task.total,
            scan_id: task.scan_id.clone(),
            error: task.error.clone(),
        })
    }

    // ---------- 内部的带毒容忍锁 ----------

    /// 拿锁时容忍中毒。
    ///
    /// 这里保存的都是若干个独立字段，没有跨字段不变量，
    /// 因此前一个持有者 panic 之后继续使用不会读到自相矛盾的状态。
    /// 反过来，若在这里 `unwrap()`，一次无关的 panic 会让整个应用永久不可用。
    fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的内存态。规格 10.1：测试一律使用独立数据库，不碰真实数据。
    fn test_state() -> AppState {
        AppState::new(Database::open_in_memory().expect("应能初始化内存库"))
    }

    use crate::domain::types::{ExtractionStatus, Fingerprint, Mode, RelPath};

    fn record(id: &str) -> FileRecord {
        FileRecord {
            id: id.to_owned(),
            scan_id: "scan-1".to_owned(),
            root_id: "root-1".to_owned(),
            relative_path: RelPath::from(vec![format!("{id}.txt")]),
            extension: ".txt".to_owned(),
            fingerprint: Fingerprint {
                volume_id: "V".to_owned(),
                file_id: "F".to_owned(),
                size: "1".to_owned(),
                modified_ns: "0".to_owned(),
                sha256: None,
            },
            extraction_status: ExtractionStatus::Pending,
            skip_code: None,
        }
    }

    #[test]
    fn defaults_are_rule_mode_and_spec_limits() {
        let state = test_state();
        let settings = state.settings();
        assert_eq!(
            settings.mode,
            Mode::Rules,
            "默认必须是规则模式：未配置模型时产品仍需完整可用"
        );
        assert_eq!(settings.scan_max_files, 10_000);
        assert_eq!(settings.scan_max_depth, 20);
        assert_eq!(settings.selected_provider_id, None);
    }

    #[test]
    fn settings_snapshot_is_a_copy() {
        let state = test_state();
        let mut snapshot = state.settings();
        snapshot.scan_max_files = 1;
        assert_eq!(
            state.settings().scan_max_files,
            10_000,
            "调用方改动返回值不应影响应用状态"
        );
    }

    #[test]
    fn unknown_root_id_yields_nothing_rather_than_a_fallback_path() {
        let state = test_state();
        assert!(
            state.root("never-registered").is_none(),
            "未登记的 rootId 必须换不回授权对象"
        );
    }

    #[test]
    fn unknown_task_returns_none() {
        let state = test_state();
        assert!(state.task("nope").is_none());
    }

    #[test]
    fn page_walks_the_list_once_without_repeating_or_skipping() {
        let state = test_state();

        // 直接塞一个 StoredScan，绕开真实磁盘扫描
        let records: Vec<FileRecord> = (0..10).map(|i| record(&format!("f{i:02}"))).collect();
        state.lock(&state.scans).insert(
            "scan-1".to_owned(),
            StoredScan {
                root_id: "root-1".to_owned(),
                records,
                truncated: false,
            },
        );

        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = state
                .page("scan-1", cursor.as_deref(), 3)
                .expect("页应可取");
            seen.extend(page.items.iter().map(|r| r.id.clone()));
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        assert_eq!(seen.len(), 10, "翻页必须恰好覆盖 10 条，不重不漏");
        let unique: std::collections::HashSet<_> = seen.iter().collect();
        assert_eq!(unique.len(), 10, "不应有重复条目");
    }

    #[test]
    fn page_clamps_absurd_limits_instead_of_panicking() {
        let state = test_state();
        state.lock(&state.scans).insert(
            "s".to_owned(),
            StoredScan {
                root_id: "r".to_owned(),
                records: (0..5).map(|i| record(&format!("f{i}"))).collect(),
                truncated: false,
            },
        );

        // limit = 0 若不做钳制会出现空页 + cursor 前进不了的死循环
        let page = state.page("s", None, 0).expect("页应可取");
        assert_eq!(page.items.len(), 1, "limit 应被钳到至少 1");

        let page = state.page("s", None, 99_999).expect("页应可取");
        assert_eq!(page.items.len(), 5, "limit 应被钳到列表长度");
    }

    #[test]
    fn stale_cursor_ends_the_pagination_instead_of_restarting() {
        let state = test_state();
        state.lock(&state.scans).insert(
            "s".to_owned(),
            StoredScan {
                root_id: "r".to_owned(),
                records: (0..3).map(|i| record(&format!("f{i}"))).collect(),
                truncated: false,
            },
        );

        let page = state
            .page("s", Some("已经不存在的-id"), 10)
            .expect("页应可取");
        assert!(
            page.items.is_empty(),
            "游标失效时必须结束翻页，而不是从头开始（那会让界面无限循环）"
        );
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn task_lifecycle_records_scan_and_reports_failure() {
        let state = test_state();

        let task_id = state.start_task(None);
        assert_eq!(
            state.task(&task_id).expect("任务应存在").status,
            TaskStatus::Running
        );

        state.fail_task(
            &task_id,
            TaskError {
                code: "PERMISSION_DENIED".to_owned(),
                message: "根目录不可读".to_owned(),
                retryable: false,
            },
        );
        let summary = state.task(&task_id).expect("任务应存在");
        assert_eq!(summary.status, TaskStatus::Failed);
        assert!(summary.error.is_some());
        assert!(summary.scan_id.is_none(), "失败任务不应挂扫描结果");
    }

    #[test]
    fn cancellation_is_a_request_before_it_becomes_a_terminal_state() {
        let state = test_state();
        let task_id = state.start_task(None);
        let token = state.cancellation_token(&task_id).expect("任务应有令牌");
        assert!(state.request_cancel(&task_id));
        assert!(token.load(Ordering::Acquire));
        assert_eq!(
            state.task(&task_id).expect("任务存在").status,
            TaskStatus::Running,
            "发出请求不等于已取消"
        );
        state.finish_cancelled(&task_id);
        assert_eq!(
            state.task(&task_id).expect("任务存在").status,
            TaskStatus::Cancelled
        );
        assert!(!state.request_cancel(&task_id), "终态任务不能再次请求取消");
    }

    /// 这条是给「不给 `AppSettings` 引入未使用 import」兜底的编译期检查。
    #[test]
    fn app_settings_is_reachable_from_tests() {
        let _ = AppSettings::default();
    }

    // ---------- 全局执行锁（规格 T07） ----------

    #[test]
    fn only_one_execution_can_hold_the_global_lock() {
        let state = test_state();
        let first = state.begin_execution("plan-1").expect("首次应能取到锁");
        assert!(state.is_executing());

        let err = state
            .begin_execution("plan-2")
            .expect_err("第二个执行必须被拒——两个 run 同时改文件会互相踩");
        assert_eq!(err.code, codes::TASK_BUSY);
        assert!(
            err.message.contains("plan-1"),
            "错误里要说清是谁占着，实际：{}",
            err.message
        );

        drop(first);
        assert!(!state.is_executing(), "guard 释放后不应再是占用状态");
        state.begin_execution("plan-3").expect("释放后应能重新取到");
    }

    #[test]
    fn scans_and_file_changes_share_one_workflow_gate() {
        let state = test_state();
        let scan_a = state.begin_scan().expect("首个扫描应能开始");
        let scan_b = state.begin_scan().expect("扫描之间可以并行");
        assert_eq!(
            state
                .begin_execution("plan-during-scan")
                .expect_err("扫描时不能开始文件变更")
                .code,
            codes::TASK_BUSY
        );

        drop(scan_a);
        assert!(state.begin_execution("still-scanning").is_err());
        drop(scan_b);

        let execution = state
            .begin_execution("plan-after-scan")
            .expect("全部扫描结束后应能执行");
        assert_eq!(
            state.begin_scan().expect_err("文件变更时不能开始扫描").code,
            codes::TASK_BUSY
        );
        drop(execution);
        state.begin_scan().expect("执行结束后应能扫描");
    }

    #[test]
    fn the_lock_is_released_even_when_the_holder_panics() {
        // 用显式的 acquire/release 一对调用时，panic 展开会跳过 release，
        // 把应用锁死到下次重启。RAII guard 的意义就在这里。
        let state = test_state();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.begin_execution("plan-boom").expect("应能取到锁");
            panic!("模拟执行过程中崩溃");
        }));

        assert!(result.is_err(), "这里必须真的 panic 过");
        assert!(
            !state.is_executing(),
            "panic 之后锁必须已经释放，否则用户只能重启应用"
        );
    }

    #[test]
    fn the_lock_is_not_held_before_any_execution() {
        let state = test_state();
        assert!(!state.is_executing());
    }
}
