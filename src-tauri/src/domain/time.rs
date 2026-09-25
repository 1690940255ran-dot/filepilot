//! 纯日期时间换算。
//!
//! 为什么单独一个模块：规格 5.1 要求时间跨 IPC 使用 UTC RFC3339 字符串，
//! 规格 6.3 要求 byMonth 规则**按本地时区**生成 `YYYY-MM`。两件事都需要
//! 「纳秒 ↔ 公历年月日」的换算，而这段算术是纯函数——不读系统时区、不碰磁盘、
//! 不依赖 Tauri，因此可以放在 domain 层并被穷举测试。
//!
//! 系统时区语义（夏令时、时区偏移）**不在这里**：那属于平台行为，
//! 见 `platform::windows::local_datetime_from_unix_ns`。
//!
//! 刻意不引入 `chrono` / `time` 这类日期库：本模块只需要两个经典算法
//! （civil-from-days 及其逆运算），而它们可以被固定向量与往返性质完整验证。
//! 多一个依赖就意味着多一份「版本升级改变时区语义」的风险。

/// 一天有多少纳秒。
const NS_PER_DAY: i128 = 86_400 * 1_000_000_000;
const NS_PER_SECOND: i128 = 1_000_000_000;

/// 可表示年份的下界：FILETIME 纪元 1601-01-01。
///
/// NTFS 的最早时间戳就是这里，再往前的时间戳在本产品里没有真实来源，
/// 与其猜一个结果，不如明确拒绝。
pub const MIN_YEAR: i32 = 1601;

/// 可表示年份的上界。上限来自 `SYSTEMTIME.wYear` 是 `u16`，
/// 取 9999 是为了让 RFC3339 输出始终是 4 位年份（不会出现 `+10000-01-01`）。
pub const MAX_YEAR: i32 = 9999;

/// 拆开后的公历时间。所有字段都是**已经定好时区**的墙上时间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// 0–999。纳秒精度在这里被丢弃，因为 RFC3339 输出到毫秒已经足够，
    /// 而**指纹用的是原始纳秒字符串**，不受这里影响。
    pub millisecond: u32,
}

impl CivilDateTime {
    pub fn new(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        millisecond: u32,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        }
    }

    /// 年份是否落在可表示范围内。
    pub fn is_representable(&self) -> bool {
        (MIN_YEAR..=MAX_YEAR).contains(&self.year)
            && (1..=12).contains(&self.month)
            && (1..=31).contains(&self.day)
            && self.hour < 24
            && self.minute < 60
            && self.second < 60
            && self.millisecond < 1000
    }

    /// `YYYY-MM`。规格 6.3 的 byMonth 分类键。
    pub fn year_month(&self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    /// UTC RFC3339 字符串，例如 `2026-09-17T07:54:19.123Z`。
    ///
    /// 规格 5.1：跨 IPC 的时间统一用这个格式。
    pub fn to_rfc3339_utc(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.millisecond
        )
    }
}

/// 把「Unix 纪元起的毫秒」格式化成 UTC RFC3339。
///
/// 与 [`CivilDateTime::to_rfc3339_utc`] 的区别只是入口不同：执行日志与令牌
/// 都按毫秒计时，这里省掉调用方自己乘 1_000_000 的重复代码。
///
/// 无法表示的时间戳回退成 Unix 纪元 —— 那会让任何基于它的过期判断立即失效，
/// 是安全的失败方向（宁可比预期早过期，也不要永不过期）。
pub fn rfc3339_from_unix_ms(ms: i64) -> String {
    civil_from_unix_ns(i128::from(ms) * 1_000_000)
        .map(|dt| dt.to_rfc3339_utc())
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_owned())
}

/// 把「Unix 纪元起的纳秒」换算成 UTC 公历时间。
///
/// 超出 [`MIN_YEAR`]..=[`MAX_YEAR`] 时返回 `None`——**不夹取、不猜测**。
/// 调用方必须把它变成一个明确的错误，而不是悄悄落到某个默认日期。
pub fn civil_from_unix_ns(ns: i128) -> Option<CivilDateTime> {
    // 用 div_euclid / rem_euclid 而不是 `/` 与 `%`：
    // 对 1970 年之前的时间戳，Rust 的 `/` 向零取整会得到「-1 天 + 23 小时」这种
    // 自相矛盾的组合（天数与当日秒数符号相反）。
    let days = ns.div_euclid(NS_PER_DAY);
    let ns_of_day = ns.rem_euclid(NS_PER_DAY);

    let (year, month, day) = civil_from_days(days)?;

    let seconds_of_day = (ns_of_day / NS_PER_SECOND) as u32;
    let millis = ((ns_of_day % NS_PER_SECOND) / 1_000_000) as u32;

    let value = CivilDateTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3600,
        minute: (seconds_of_day % 3600) / 60,
        second: seconds_of_day % 60,
        millisecond: millis,
    };

    value.is_representable().then_some(value)
}

/// 公历时间 → Unix 纪元起的纳秒。
///
/// 与 [`civil_from_unix_ns`] 互为逆运算（往返性质有测试守着）。
pub fn unix_ns_from_civil(dt: &CivilDateTime) -> i128 {
    let days = days_from_civil(dt.year, dt.month, dt.day);
    let seconds_of_day = dt.hour as i128 * 3600 + dt.minute as i128 * 60 + dt.second as i128;
    days * NS_PER_DAY + seconds_of_day * NS_PER_SECOND + dt.millisecond as i128 * 1_000_000
}

/// Howard Hinnant 的 `civil_from_days`。
///
/// 输入是「距 1970-01-01 的天数」（可为负），输出公历年月日。
/// 选它是因为它是**无分支的整数算术**：没有查表、没有闰年特例，
/// 因此可以用固定向量把闰年、世纪闰年、公元前区间一次测穿。
fn civil_from_days(days: i128) -> Option<(i32, u32, u32)> {
    // 平移到「0000-03-01 起」的纪元，让闰日落在年末，消除 2 月的特例
    let z = days.checked_add(719_468)?;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]，3 月为 0
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = mp + if mp < 10 { 3 } else { -9 }; // [1, 12]
    let year = y + i128::from(m <= 2);

    let year = i32::try_from(year).ok()?;
    Some((year, m as u32, d as u32))
}

/// Howard Hinnant 的 `days_from_civil`：公历年月日 → 距 1970-01-01 的天数。
fn days_from_civil(year: i32, month: u32, day: u32) -> i128 {
    let y = i128::from(year) - i128::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400); // [0, 399]
    let m = i128::from(month);
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + i128::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_published_vectors() {
        // Unix 纪元
        let epoch = civil_from_unix_ns(0).expect("纪元应可表示");
        assert_eq!(
            epoch,
            CivilDateTime::new(1970, 1, 1, 0, 0, 0, 0),
            "0 纳秒必须是 1970-01-01T00:00:00Z"
        );

        // 已知值：1700000000 秒 = 2023-11-14T22:13:20Z
        let known = civil_from_unix_ns(1_700_000_000 * NS_PER_SECOND).expect("应可表示");
        assert_eq!(known.year_month(), "2023-11");
        assert_eq!(known.to_rfc3339_utc(), "2023-11-14T22:13:20.000Z");
    }

    #[test]
    fn handles_leap_days_and_century_rules() {
        // 2000 是闰年（能被 400 整除），1900 不是（能被 100 整除但不能被 400 整除）
        let leap = civil_from_unix_ns(unix_ns_from_civil(&CivilDateTime::new(
            2000, 2, 29, 12, 0, 0, 0,
        )))
        .expect("应可表示");
        assert_eq!(leap, CivilDateTime::new(2000, 2, 29, 12, 0, 0, 0));

        // 1900-03-01 的前一天必须是 1900-02-28（不是 29）
        let feb28 = unix_ns_from_civil(&CivilDateTime::new(1900, 3, 1, 0, 0, 0, 0)) - NS_PER_DAY;
        let back = civil_from_unix_ns(feb28).expect("应可表示");
        assert_eq!(back, CivilDateTime::new(1900, 2, 28, 0, 0, 0, 0));
    }

    #[test]
    fn dates_before_the_epoch_keep_day_and_second_consistent() {
        // 1969-12-31T23:59:59Z 距离纪元只差 1 秒，且为负数。
        // 用 `/` 取整的实现会在这里得到「-1 天 + 86399 秒」的错误组合。
        let before = civil_from_unix_ns(-NS_PER_SECOND).expect("应可表示");
        assert_eq!(before, CivilDateTime::new(1969, 12, 31, 23, 59, 59, 0));
    }

    #[test]
    fn round_trips_over_a_wide_range() {
        // 覆盖 1601 到 9999 的多个采样点，包括闰年边界与月边界
        for dt in [
            CivilDateTime::new(1601, 1, 1, 0, 0, 0, 0),
            CivilDateTime::new(1899, 12, 31, 23, 59, 59, 999),
            CivilDateTime::new(1970, 1, 1, 0, 0, 0, 0),
            CivilDateTime::new(2024, 2, 29, 12, 34, 56, 789),
            CivilDateTime::new(2100, 3, 1, 1, 2, 3, 4),
            CivilDateTime::new(9999, 12, 31, 23, 59, 59, 999),
        ] {
            let ns = unix_ns_from_civil(&dt);
            assert_eq!(civil_from_unix_ns(ns), Some(dt), "往返换算必须无损：{dt:?}");
        }
    }

    #[test]
    fn out_of_range_timestamps_are_rejected_not_clamped() {
        // 1600 年（FILETIME 纪元之前）与 10000 年都必须被拒绝
        assert!(civil_from_unix_ns(unix_ns_from_civil(&CivilDateTime::new(
            1600, 12, 31, 0, 0, 0, 0
        )))
        .is_none());
        assert!(civil_from_unix_ns(unix_ns_from_civil(&CivilDateTime::new(
            10000, 1, 1, 0, 0, 0, 0
        )))
        .is_none());
    }

    #[test]
    fn year_month_is_zero_padded() {
        assert_eq!(
            CivilDateTime::new(2026, 9, 17, 0, 0, 0, 0).year_month(),
            "2026-09"
        );
        assert_eq!(
            CivilDateTime::new(2026, 12, 1, 0, 0, 0, 0).year_month(),
            "2026-12"
        );
    }

    #[test]
    fn rfc3339_has_millisecond_precision_and_z_suffix() {
        let text = CivilDateTime::new(2026, 1, 2, 3, 4, 5, 6).to_rfc3339_utc();
        assert_eq!(text, "2026-01-02T03:04:05.006Z");
        assert!(text.ends_with('Z'), "必须是 UTC 标记");
    }
}
