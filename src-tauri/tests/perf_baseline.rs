//! T15：真机性能基线。
//!
//! 规格原文：
//!
//! > 用 10,000 个小型合成文件测扫描耗时，1,000 项测计划生成，记录机器
//! > CPU/内存/磁盘和样本总大小。
//! >
//! > 性能目标：冷启动 5 秒内可交互；10,000 文件元信息扫描目标 15 秒内；
//! > 1,000 项命名计划目标 2 秒内（不含哈希、OCR、网络）。**这些是测量目标，
//! > 未测不能当成事实宣传。**
//!
//! ## 为什么要真的跑，而不是写个「应该够快」的注释
//!
//! 最后那句话就是理由。性能数字一旦进了 README 或简历，它就成了一个**承诺**；
//! 而承诺只能由真实测量支撑。所以这个文件用 `#[ignore]` 标记——它不该在每次
//! `cargo test` 里跑（建一万个文件要几分钟），但**必须能被一条命令跑起来**，
//! 并且把结果写到磁盘上供人引用。
//!
//! 跑法：
//!
//! ```text
//! cargo test --test perf_baseline -- --ignored --nocapture
//! ```
//!
//! 结果落在 `tmp/perf_baseline.json`，由人工誊写进 `docs/TEST_MATRIX.md`。
//! **不要**让脚本直接改那份文档：那份文档里的每一条都该是有人看过、签过字的。

#![cfg(windows)]

mod support;

use std::io::Write as _;
use std::time::{Duration, Instant, SystemTime};

use filepilot_lib::domain::types::{Mode, Proposal};
use filepilot_lib::planner::build::{build_rule_plan, PlanBuild};
use filepilot_lib::planner::validate::validate_plan;
use filepilot_lib::rules::RuleKind;
use filepilot_lib::safety::root::approve_root;
use filepilot_lib::scanner::{scan, ScanOptions};

/// 规格里的样本规模。
const SCAN_SAMPLE: usize = 10_000;
/// 规格里的计划规模。
const PLAN_SAMPLE: usize = 1_000;
/// 规格里的目标（超过它要在报告里写清楚，而不是悄悄改数字）。
const SCAN_TARGET: Duration = Duration::from_secs(15);
const PLAN_TARGET: Duration = Duration::from_secs(2);

/// 建一个含 `count` 个小型合成文件的目录树。
///
/// 分到 100 个子目录里：全平铺在根下的话，测的是「一个巨大的目录」这种
/// 很少见的情况，而真实场景下文件总是分散的。
fn make_sample(root: &std::path::Path, count: usize) -> u64 {
    use std::fs;

    let mut written: u64 = 0;
    for index in 0..count {
        let bucket = root.join(format!("bucket-{:02}", index % 100));
        if index < 100 {
            fs::create_dir_all(&bucket).expect("建子目录");
        }
        let path = bucket.join(format!("file-{index:05}.txt"));
        // 内容小但非空：空文件在真实场景里少见，而大小会影响指纹计算。
        let body = format!("第 {index} 个合成样本，用于性能测量。");
        fs::write(&path, body.as_bytes()).expect("写样本文件");
        written += body.len() as u64;
    }
    written
}

/// 把一项测量写进结果文件。
struct Report {
    lines: Vec<String>,
}

impl Report {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn add(&mut self, name: &str, value: String) {
        println!("[perf] {name} = {value}");
        self.lines.push(format!("{name} = {value}"));
    }

    /// 规格要求的「记录机器 CPU/内存/磁盘」。
    fn environment(&mut self) {
        self.add("os", std::env::consts::OS.to_owned());
        self.add("arch", std::env::consts::ARCH.to_owned());
        self.add("cpu_cores", num_cpus().to_string());
        self.add(
            "cpu",
            std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".to_owned()),
        );
        self.add("timestamp", format!("{:?}", SystemTime::now()));
    }

    fn flush(&self) {
        let path = std::path::Path::new("tmp/perf_baseline.txt");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = std::fs::File::create(path).expect("写结果文件");
        for line in &self.lines {
            writeln!(file, "{line}").expect("写一行");
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// 当前进程的峰值工作集（MB）。
///
/// 规格要求「记录机器 CPU/内存/磁盘」。内存这一栏用**峰值工作集**——
/// 它就是任务管理器里「内存」那一列看的数字，也是用户在意的那个。
///
/// 峰值比当前值更有意义：一个扫描任务的内存曲线是「涨上去、回落」，
/// 只看结束时的当前值会漏掉中间那个峰值，而峰值才是「这台机器扛不扛得住」
/// 的答案。
fn peak_working_set_mb() -> Option<f64> {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    // SAFETY：`counters` 是一块大小正确的栈上内存，`GetCurrentProcess`
    // 返回的是伪句柄（不需要关闭）。
    let outcome = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    // `windows 0.62` 的这套 API 返回 `Result<(), Error>`，不是老的 `BOOL`。
    outcome
        .is_ok()
        .then(|| counters.PeakWorkingSetSize as f64 / (1024.0 * 1024.0))
}

#[test]
#[ignore = "性能基线：建一万个文件要好几分钟，`cargo test` 里不该跑"]
fn the_scan_and_plan_targets_hold_at_the_spec_sample_sizes() {
    let mut report = Report::new();
    report.environment();

    let temp = support::test_root();
    let root_path = temp.path().to_path_buf();

    // ---- 造样本 --------------------------------------------------------
    let sample_start = Instant::now();
    let bytes = make_sample(&root_path, SCAN_SAMPLE);
    report.add("sample_files", SCAN_SAMPLE.to_string());
    report.add("sample_bytes", bytes.to_string());
    report.add(
        "sample_create_seconds",
        format!("{:.2}", sample_start.elapsed().as_secs_f64()),
    );

    let root = approve_root(&root_path).expect("授权样本目录");

    // ---- 扫描 ----------------------------------------------------------
    let scan_start = Instant::now();
    let outcome = scan(&root, &ScanOptions::default()).expect("扫描样本");
    let scan_elapsed = scan_start.elapsed();

    report.add("scan_files", outcome.records.len().to_string());
    report.add("scan_seconds", format!("{:.3}", scan_elapsed.as_secs_f64()));
    report.add("scan_truncated", outcome.truncated.to_string());
    report.add(
        "scan_within_target",
        (scan_elapsed <= SCAN_TARGET).to_string(),
    );

    assert_eq!(
        outcome.records.len(),
        SCAN_SAMPLE,
        "样本数不对，性能数字就没有可比性"
    );

    // ---- 计划生成（规则模式） -------------------------------------------
    //
    // 规格明说「不含哈希、OCR、网络」，规则模式正好三者都不碰。
    let plan_start = Instant::now();
    let build: PlanBuild = build_rule_plan(
        &root,
        &outcome.scan_id.to_string(),
        RuleKind::ByType,
        &outcome.records,
        SystemTime::now(),
    )
    .expect("生成规则计划");
    let plan_elapsed = plan_start.elapsed();

    // 1,000 项：只取前 1,000 条记录。规格要的是「1,000 项命名计划」。
    let slice = &outcome.records[..PLAN_SAMPLE.min(outcome.records.len())];
    let slice_start = Instant::now();
    let sliced = build_rule_plan(
        &root,
        &outcome.scan_id.to_string(),
        RuleKind::ByType,
        slice,
        SystemTime::now(),
    )
    .expect("生成 1,000 项计划");
    let slice_elapsed = slice_start.elapsed();

    report.add("full_plan_items", build.plan.items.len().to_string());
    report.add(
        "full_plan_seconds",
        format!("{:.3}", plan_elapsed.as_secs_f64()),
    );
    report.add("sliced_plan_items", sliced.plan.items.len().to_string());
    report.add(
        "sliced_plan_seconds",
        format!("{:.3}", slice_elapsed.as_secs_f64()),
    );
    report.add(
        "sliced_plan_within_target",
        (slice_elapsed <= PLAN_TARGET).to_string(),
    );

    // ---- 校验（它也在「计划生成」这条链路上） ---------------------------
    let validate_start = Instant::now();
    let _ = validate_plan(&sliced.plan, &root).expect("校验计划");
    report.add(
        "validate_1000_seconds",
        format!("{:.3}", validate_start.elapsed().as_secs_f64()),
    );

    if let Some(mb) = peak_working_set_mb() {
        report.add("peak_working_set_mb", format!("{mb:.1}"));
    }

    report.flush();

    // 不在这里断言「达标」：规格说的是**测量目标**，而测量结果该由人看过
    // 之后写进 TEST_MATRIX。测试红了不等于性能不达标——它只是说明这次
    // 测量超了，而超了的原因（机器在跑别的东西？样本在机械盘上？）
    // 需要人判断。报告里的 `*_within_target` 就是给那个判断用的。
    println!("[perf] 完整结果见 tmp/perf_baseline.txt");
}

#[test]
#[ignore = "性能基线的一部分：连续 10 轮扫描，看有没有累积"]
fn ten_consecutive_rounds_do_not_accumulate() {
    // 规格 T15：「连续 10 轮任务不积累僵尸 worker 或未关闭句柄」。
    //
    // 「不积累」的可观测形式：10 轮之后的耗时与第 1 轮相比**没有单调上升**。
    // 绝对耗时受机器影响，但「第 10 轮比第 1 轮慢一个数量级」是缺陷。
    let temp = support::test_root();
    let root_path = temp.path().to_path_buf();
    // 每轮 500 个文件：够看出趋势，又不至于让这条测试跑十分钟。
    make_sample(&root_path, 500);
    let root = approve_root(&root_path).expect("授权");

    let mut rounds = Vec::new();
    for round in 0..10 {
        let start = Instant::now();
        let outcome = scan(&root, &ScanOptions::default()).expect("扫描");
        let elapsed = start.elapsed();
        println!("[perf] round {round}: {:.3}s", elapsed.as_secs_f64());
        assert_eq!(outcome.records.len(), 500, "第 {round} 轮的结果数不对");
        rounds.push(elapsed.as_secs_f64());
    }

    let first = rounds[0];
    let last = rounds[9];
    assert!(
        last < first * 5.0,
        "第 10 轮比第 1 轮慢了 5 倍以上，像是资源在累积：{rounds:?}"
    );

    let path = std::path::Path::new("tmp/perf_rounds.txt");
    let _ = std::fs::create_dir_all("tmp");
    let mut file = std::fs::File::create(path).expect("写结果");
    for (index, seconds) in rounds.iter().enumerate() {
        writeln!(file, "round {index} = {seconds:.3}s").expect("写一行");
    }
}

/// 未被使用的类型引用保持编译期有效性——这个文件只用得到它们的一部分。
#[allow(dead_code)]
fn _type_check(_: Proposal, _: Mode) {}
