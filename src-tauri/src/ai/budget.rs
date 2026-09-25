//! 请求上限、重试与费用（规格 6.4）。
//!
//! 规格把这一整段的数字都写死了，所以它们集中在这里，而不是散在
//! 适配器里各写一遍——散开的后果是「改了一处、漏了一处」，
//! 而漏掉的那处正好是「最多 30 次请求」这种**会真的花钱**的上限。
//!
//! ```text
//! 每次请求最多 10 个文件、总计 12,000 文本字符；并发 1；60 秒超时。
//! 一个任务最多 200 个文件、30 次请求（含重试和 JSON 修复），
//! 达到任何上限停止并显示已完成范围。
//! 429/可恢复 5xx 最多重试 2 次，指数退避并服从上限；401/403 不重试。
//! 最多 1 次输出格式修复，仍失败则返回模型格式错误。
//! 模型费用仅在有可靠计价配置时估算，否则显示请求数/发送字符数，不编造金额。
//! ```

use crate::domain::errors::{codes, AppError};

/// 用户自然语言指令上限（规格 6.4 第一句）。
pub const MAX_INSTRUCTION_CHARS: usize = 1_000;

/// 单个文件的文本摘要上限（规格 6.4）。
///
/// 规格原话还有一句必须记住的：「**文本裁剪不是隐私脱敏保证**」——
/// 截到 2,000 字符只是控制载荷大小，不代表里面就没有敏感信息。
pub const MAX_SUMMARY_CHARS: usize = 2_000;

/// 每次请求最多带几个文件。
pub const MAX_FILES_PER_REQUEST: usize = 10;

/// 每次请求最多带多少文本字符。
pub const MAX_CHARS_PER_REQUEST: usize = 12_000;

/// 一个任务最多处理多少个文件。
pub const MAX_FILES_PER_TASK: u32 = 200;

/// 一个任务最多发多少次请求，**含重试与 JSON 修复**。
///
/// 「含」这两个字是关键：把重试排除在外会让一个卡在 429 的端点
/// 无限重试下去，而每一次都是一次真实的调用。
pub const MAX_REQUESTS_PER_TASK: u32 = 30;

/// 429 / 可恢复 5xx 最多重试几次。
pub const MAX_RETRIES: u32 = 2;

/// 输出格式修复最多做几次。
pub const MAX_REPAIRS: u32 = 1;

/// 指数退避的基数。
const BACKOFF_BASE_MS: u64 = 1_000;

/// 一次请求的载荷规模。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchSize {
    pub files: usize,
    pub characters: usize,
}

impl BatchSize {
    /// 按字符（Unicode 标量）而非字节统计。
    ///
    /// 规格说的是「12,000 文本字符」。用字节数会让一份中文载荷在
    /// 三分之一的大小上就被判超限——那是另一条规则，不是这一条。
    pub fn of(summaries: &[String]) -> Self {
        Self {
            files: summaries.len(),
            characters: summaries.iter().map(|text| text.chars().count()).sum(),
        }
    }

    /// 校验单请求的两个上限。
    pub fn validate(&self) -> Result<(), AppError> {
        if self.files > MAX_FILES_PER_REQUEST {
            return Err(AppError::new(
                codes::BUDGET_EXCEEDED,
                format!(
                    "一次请求最多 {MAX_FILES_PER_REQUEST} 个文件，收到 {}。\
                     应当先分批——这不是让用户去删文件。",
                    self.files
                ),
            ));
        }
        if self.characters > MAX_CHARS_PER_REQUEST {
            return Err(AppError::new(
                codes::BUDGET_EXCEEDED,
                format!(
                    "一次请求的文本总量最多 {MAX_CHARS_PER_REQUEST} 个字符，\
                     收到 {}。应当先分批。",
                    self.characters
                ),
            ));
        }
        Ok(())
    }
}

/// 校验用户指令长度。
///
/// **超长是拒绝而不是截断**：用户的指令被悄悄砍掉一半，他会看到一份
/// 明显不符合要求的建议，却不知道为什么。宁可当场告诉他。
pub fn validate_instruction(instruction: &str) -> Result<(), AppError> {
    let length = instruction.chars().count();
    if length > MAX_INSTRUCTION_CHARS {
        return Err(AppError::new(
            codes::BUDGET_EXCEEDED,
            format!("整理要求最多 {MAX_INSTRUCTION_CHARS} 个字符，当前 {length} 个。"),
        ));
    }
    Ok(())
}

/// 把一个文件的文本摘要裁到上限。
///
/// 返回是否发生了裁剪，好让界面能如实说明「这份摘要被截过」。
pub fn truncate_summary(text: &str) -> (String, bool) {
    let mut out = String::new();
    for (index, character) in text.chars().enumerate() {
        if index >= MAX_SUMMARY_CHARS {
            return (out, true);
        }
        out.push(character);
    }
    (out, false)
}

/// 一个任务级的花费计数器。
///
/// 它**只做记账与拒绝**，不决定要不要停——「达到上限停止并显示已完成范围」
/// 是上层分析服务的事，因为只有它知道范围是什么。
#[derive(Debug, Default, Clone)]
pub struct TaskBudget {
    requests: u32,
    files: u32,
    retries: u32,
    repairs: u32,
}

impl TaskBudget {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requests(&self) -> u32 {
        self.requests
    }

    pub fn files(&self) -> u32 {
        self.files
    }

    pub fn retries(&self) -> u32 {
        self.retries
    }

    pub fn repairs(&self) -> u32 {
        self.repairs
    }

    /// 这次请求是否还能发出。**调用即计数**——先记再发，
    /// 否则一个抛错的分支就会让计数漏掉一次真实调用。
    pub fn begin_request(&mut self) -> Result<(), AppError> {
        if self.requests >= MAX_REQUESTS_PER_TASK {
            return Err(AppError::new(
                codes::BUDGET_EXCEEDED,
                format!(
                    "本次任务已达 {MAX_REQUESTS_PER_TASK} 次请求上限（含重试），\
                     已处理 {} 个文件。剩余文件未获得建议。",
                    self.files
                ),
            ));
        }
        self.requests += 1;
        Ok(())
    }

    /// 登记这次请求带了多少文件。
    pub fn record_files(&mut self, count: u32) -> Result<(), AppError> {
        if self.files + count > MAX_FILES_PER_TASK {
            return Err(AppError::new(
                codes::BUDGET_EXCEEDED,
                format!(
                    "本次任务最多处理 {MAX_FILES_PER_TASK} 个文件，\
                     已处理 {}，本次还要 {count} 个。",
                    self.files
                ),
            ));
        }
        self.files += count;
        Ok(())
    }

    /// 还能不能再重试一次。
    ///
    /// 同时受两个上限约束：重试次数本身，以及**任务总请求数**——
    /// 后者才是防止「重试把额度用光」的那道闸。
    pub fn allow_retry(&mut self) -> bool {
        if self.retries >= MAX_RETRIES || self.requests >= MAX_REQUESTS_PER_TASK {
            return false;
        }
        self.retries += 1;
        true
    }

    /// 还能不能做一次输出格式修复。
    pub fn allow_repair(&mut self) -> bool {
        if self.repairs >= MAX_REPAIRS || self.requests >= MAX_REQUESTS_PER_TASK {
            return false;
        }
        self.repairs += 1;
        true
    }
}

/// 某个 HTTP 状态码该不该重试。
///
/// 规格 6.4：「429/可恢复 5xx 最多重试 2 次…401/403 不重试」。
///
/// `501 Not Implemented` **不在**可重试之列：它说的是「这个端点不支持
/// 这个功能」，重试多少次都是一样的结果，只是多花时间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    Yes,
    No,
}

impl Retry {
    pub fn of_status(status: u16) -> Self {
        match status {
            429 | 500 | 502 | 503 | 504 => Retry::Yes,
            _ => Retry::No,
        }
    }

    pub fn allowed(self) -> bool {
        self == Retry::Yes
    }
}

/// 第 `attempt` 次重试之前该等多久（指数退避）。
///
/// `attempt` 从 1 开始。上限 `ceiling` 用来「服从上限」：当一次退避
/// 就会把剩下的时间预算吃光时，等待本身已经没有意义。
pub fn backoff_delay(attempt: u32, ceiling: std::time::Duration) -> std::time::Duration {
    let shift = attempt.saturating_sub(1).min(16);
    let milliseconds = BACKOFF_BASE_MS.saturating_mul(1u64 << shift);
    std::time::Duration::from_millis(milliseconds).min(ceiling)
}

/// 一份计价配置。
///
/// 规格：「模型费用**仅在**有可靠计价配置时估算」。这个结构体存在的
/// 前提是用户/内置表给了真实的单价——拿不到就宁可只报请求数。
#[derive(Debug, Clone, PartialEq)]
pub struct Pricing {
    pub currency: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
}

/// 费用或用量。
///
/// 刻意做成枚举而不是「一个可能为 null 的金额」：规格禁止编造金额，
/// 而一个 `f64` 字段很容易被下游用 `unwrap_or(0.0)` 变成「费用是 0」——
/// 那正是一句编出来的话。
#[derive(Debug, Clone, PartialEq)]
pub enum CostReport {
    /// 有可靠计价时的估算。
    Estimated {
        currency: String,
        input_characters: u64,
        output_characters: u64,
        amount: f64,
    },
    /// 没有计价配置：只报可观测的量。
    Unpriced { requests: u32, sent_characters: u64 },
}

impl CostReport {
    /// 有计价配置就算，没有就如实报用量。
    pub fn compute(
        pricing: Option<&Pricing>,
        requests: u32,
        input_characters: u64,
        output_characters: u64,
    ) -> Self {
        let Some(pricing) = pricing else {
            return CostReport::Unpriced {
                requests,
                sent_characters: input_characters,
            };
        };

        let amount = (input_characters as f64 / 1_000_000.0) * pricing.input_per_million
            + (output_characters as f64 / 1_000_000.0) * pricing.output_per_million;

        CostReport::Estimated {
            currency: pricing.currency.clone(),
            input_characters,
            output_characters,
            amount,
        }
    }

    /// 给界面用的一句话。**没有计价时绝不出现金额**。
    pub fn summary(&self) -> String {
        match self {
            CostReport::Estimated {
                currency, amount, ..
            } => format!("估算费用 {amount:.4} {currency}"),
            CostReport::Unpriced {
                requests,
                sent_characters,
            } => format!("已发起 {requests} 次请求，共发送 {sent_characters} 个字符"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_at_the_limits_is_accepted() {
        let size = BatchSize {
            files: MAX_FILES_PER_REQUEST,
            characters: MAX_CHARS_PER_REQUEST,
        };
        assert!(size.validate().is_ok());
    }

    #[test]
    fn one_file_over_the_limit_is_refused() {
        let size = BatchSize {
            files: MAX_FILES_PER_REQUEST + 1,
            characters: 10,
        };
        let error = size.validate().expect_err("超出一个文件必须被拒绝");
        assert_eq!(error.code, codes::BUDGET_EXCEEDED);
        assert!(error.message.contains("分批"), "{}", error.message);
    }

    #[test]
    fn one_character_over_the_limit_is_refused() {
        let size = BatchSize {
            files: 1,
            characters: MAX_CHARS_PER_REQUEST + 1,
        };
        assert!(size.validate().is_err());
    }

    #[test]
    fn characters_are_counted_as_characters_not_bytes() {
        // 规格说的是「12,000 文本字符」。用字节数会让一份中文载荷
        // 在三分之一的大小上就被判超限——那是另一条规则。
        let chinese = "汉".repeat(MAX_CHARS_PER_REQUEST);
        assert_eq!(
            chinese.len(),
            MAX_CHARS_PER_REQUEST * 3,
            "前提：UTF-8 中文占 3 字节"
        );
        let size = BatchSize::of(&[chinese]);
        assert_eq!(size.characters, MAX_CHARS_PER_REQUEST);
        assert!(size.validate().is_ok(), "正好在上限内应当通过");
    }

    #[test]
    fn an_over_long_instruction_is_refused_not_truncated() {
        // 指令被悄悄砍掉一半，用户会看到一份不符合要求的建议却不知道为什么。
        let long = "整".repeat(MAX_INSTRUCTION_CHARS + 1);
        let error = validate_instruction(&long).expect_err("超长指令必须被拒绝");
        assert_eq!(error.code, codes::BUDGET_EXCEEDED);
        assert!(error.message.contains("1000"), "{}", error.message);
    }

    #[test]
    fn an_instruction_at_the_limit_is_accepted() {
        let exact = "整".repeat(MAX_INSTRUCTION_CHARS);
        assert!(validate_instruction(&exact).is_ok());
    }

    #[test]
    fn a_summary_is_truncated_at_the_limit_and_says_so() {
        let long = "字".repeat(MAX_SUMMARY_CHARS + 10);
        let (text, truncated) = truncate_summary(&long);
        assert!(truncated);
        assert_eq!(text.chars().count(), MAX_SUMMARY_CHARS);

        let (short, truncated) = truncate_summary("短");
        assert!(!truncated);
        assert_eq!(short, "短");
    }

    #[test]
    fn a_truncated_summary_is_not_split_in_the_middle_of_a_character() {
        // 按字符裁而不是按字节裁：按字节切会切出一个无效的 UTF-8 序列，
        // 而那个字符串进 JSON 时会变成替换字符。
        let text = "汉".repeat(MAX_SUMMARY_CHARS + 1);
        let (cut, _) = truncate_summary(&text);
        assert_eq!(cut.chars().count(), MAX_SUMMARY_CHARS);
        assert!(cut.chars().all(|c| c == '汉'));
    }

    #[test]
    fn the_task_budget_counts_requests_even_when_they_fail() {
        // 「先记再发」：一个抛错的分支不能让计数漏掉一次真实调用。
        let mut budget = TaskBudget::new();
        budget.record_files(7).expect("记录已处理的文件数");

        for _ in 0..MAX_REQUESTS_PER_TASK {
            budget.begin_request().expect("上限之内应当允许");
        }
        assert_eq!(budget.requests(), MAX_REQUESTS_PER_TASK);

        let error = budget.begin_request().expect_err("超出上限必须被拒绝");
        assert_eq!(error.code, codes::BUDGET_EXCEEDED);
        // 规格要求「达到任何上限停止并**显示已完成范围**」——
        // 这个错误消息就是那句话的落点：用户要知道走到了哪里。
        assert!(
            error.message.contains('7'),
            "错误里要带上已完成范围（已处理 7 个文件）：{}",
            error.message
        );
        assert!(error.message.contains("30"), "{}", error.message);
    }

    #[test]
    fn the_task_budget_refuses_more_files_than_allowed() {
        let mut budget = TaskBudget::new();
        budget
            .record_files(MAX_FILES_PER_TASK)
            .expect("正好到上限应当允许");
        assert!(budget.record_files(1).is_err());
    }

    #[test]
    fn retries_are_capped_and_also_bounded_by_the_request_total() {
        let mut budget = TaskBudget::new();
        assert!(budget.allow_retry());
        assert!(budget.allow_retry());
        assert!(!budget.allow_retry(), "最多重试 {MAX_RETRIES} 次");
        assert_eq!(budget.retries(), MAX_RETRIES);
    }

    #[test]
    fn a_retry_is_refused_once_the_request_budget_is_used_up() {
        // 这才是真正防止「重试把额度用光」的那道闸。
        let mut budget = TaskBudget::new();
        for _ in 0..MAX_REQUESTS_PER_TASK {
            let _ = budget.begin_request();
        }
        assert!(!budget.allow_retry(), "额度用完后不该再重试");
    }

    #[test]
    fn only_one_repair_is_allowed() {
        let mut budget = TaskBudget::new();
        assert!(budget.allow_repair());
        assert!(!budget.allow_repair());
        assert_eq!(budget.repairs(), MAX_REPAIRS);
    }

    #[test]
    fn a_repair_is_refused_once_the_request_budget_is_used_up() {
        let mut budget = TaskBudget::new();
        for _ in 0..MAX_REQUESTS_PER_TASK {
            let _ = budget.begin_request();
        }
        assert!(!budget.allow_repair());
    }

    #[test]
    fn only_recoverable_statuses_are_retried() {
        assert!(Retry::of_status(429).allowed());
        for status in [500, 502, 503, 504] {
            assert!(Retry::of_status(status).allowed(), "{status} 应当可重试");
        }
        for status in [400, 401, 403, 404, 422, 501] {
            assert!(
                !Retry::of_status(status).allowed(),
                "{status} 不该重试：重试多少次都是一样的结果"
            );
        }
    }

    #[test]
    fn backoff_grows_and_respects_the_ceiling() {
        let ceiling = std::time::Duration::from_secs(30);
        assert_eq!(backoff_delay(1, ceiling).as_millis(), 1000);
        assert_eq!(backoff_delay(2, ceiling).as_millis(), 2000);

        // 「服从上限」：一次退避就把预算吃光时，等下去已经没有意义。
        let tiny = std::time::Duration::from_millis(500);
        assert_eq!(backoff_delay(3, tiny), tiny);
    }

    #[test]
    fn backoff_does_not_overflow_on_an_absurd_attempt() {
        // 一个写错的重试循环不该让这里 panic（`1u64 << 100` 是 UB 的温床）。
        let ceiling = std::time::Duration::from_secs(60);
        let delay = backoff_delay(u32::MAX, ceiling);
        assert_eq!(delay, ceiling);
    }

    #[test]
    fn without_pricing_only_usage_is_reported() {
        // 规格：没有可靠计价配置时**不编造金额**。
        let report = CostReport::compute(None, 3, 12_000, 800);
        assert_eq!(
            report,
            CostReport::Unpriced {
                requests: 3,
                sent_characters: 12_000
            }
        );
        let summary = report.summary();
        assert!(summary.contains("3"), "{summary}");
        assert!(!summary.contains('¥'), "没有计价就不该出现金额：{summary}");
        assert!(!summary.contains("$"), "没有计价就不该出现金额：{summary}");
    }

    #[test]
    fn with_pricing_the_amount_is_computed() {
        let pricing = Pricing {
            currency: "USD".to_owned(),
            input_per_million: 3.0,
            output_per_million: 15.0,
        };
        let report = CostReport::compute(Some(&pricing), 1, 1_000_000, 1_000_000);

        let CostReport::Estimated {
            currency, amount, ..
        } = report
        else {
            panic!("有计价时应当给出估算");
        };
        assert_eq!(currency, "USD");
        assert!((amount - 18.0).abs() < f64::EPSILON, "{amount}");
    }

    #[test]
    fn the_priced_summary_carries_the_currency() {
        let pricing = Pricing {
            currency: "CNY".to_owned(),
            input_per_million: 1.0,
            output_per_million: 1.0,
        };
        let summary = CostReport::compute(Some(&pricing), 1, 0, 0).summary();
        assert!(summary.contains("CNY"), "{summary}");
    }
}
