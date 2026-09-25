//! byMonth 规则：按文件**最后修改时间**的本地月份分类（规格 6.3）。
//!
//! 规格点名了两件事，这里都必须如实遵守：
//!
//! 1. 用**本地时区**生成 `YYYY-MM`。同一批文件在不同时区的用户看来
//!    可以落在不同的月份目录里，这是预期行为，不是缺陷。
//! 2. 界面文案必须叫「修改月份」。它**不是拍摄日期**，也不是创建日期——
//!    复制、下载、同步都会改写修改时间。理由文案里已经写明依据是修改时间。

use crate::domain::errors::{codes, AppError};
use crate::platform::windows::local_datetime_from_unix_ns;

/// 文件最后修改时间对应的本地月份，形如 `2026-09`。
///
/// 失败时返回 `INVALID_TIMESTAMP`：文件的时间戳落在可表示范围之外
/// （例如被工具写成 1601 年之前或 10000 年之后）。**不夹取、不猜月份**——
/// 猜出来的月份会变成一个用户无法解释的目录名。
pub fn category_for(modified_ns: &str) -> Result<String, AppError> {
    let ns = parse_modified_ns(modified_ns)?;
    let local = local_datetime_from_unix_ns(ns)?;
    Ok(local.year_month())
}

/// 供理由文案使用：能算出来就给月份，算不出来就如实说「未知」。
///
/// 理由文案不能因为时间戳异常而失败——它只是解释用的文字，
/// 真正的失败已经由 [`category_for`] 报告。
pub fn year_month_or_unknown(modified_ns: &str) -> String {
    match category_for(modified_ns) {
        Ok(month) => month,
        Err(_) => "未知".to_owned(),
    }
}

/// 解析指纹里的十进制纳秒字符串。
///
/// 规格 5.1 之所以用字符串传时间戳，是为了避开 JavaScript 的数值精度损失；
/// 在 Rust 侧必须显式解析，不能 `as i128` 地信任输入。
fn parse_modified_ns(value: &str) -> Result<i128, AppError> {
    value.parse::<i128>().map_err(|_| {
        AppError::new(
            codes::INVALID_TIMESTAMP,
            format!("文件修改时间不是合法的十进制纳秒时间戳: {value:?}"),
        )
    })
}

/// 仅用于单元测试的断言辅助：给定本地年月日构造 UTC 纳秒。
#[cfg(test)]
fn utc_ns_of(year: i32, month: u32, day: u32, hour: u32) -> i128 {
    crate::domain::time::unix_ns_from_civil(&crate::domain::time::CivilDateTime::new(
        year, month, day, hour, 0, 0, 0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_non_numeric_timestamp() {
        let error = category_for("不是数字").expect_err("非数字时间戳必须报错");
        assert_eq!(error.code, codes::INVALID_TIMESTAMP);
    }

    #[test]
    fn rejects_a_timestamp_outside_the_representable_range() {
        let beyond_max =
            crate::domain::time::unix_ns_from_civil(&crate::domain::time::CivilDateTime::new(
                crate::domain::time::MAX_YEAR + 1,
                1,
                1,
                0,
                0,
                0,
                0,
            ))
            .to_string();
        assert_eq!(
            category_for(&beyond_max).unwrap_err().code,
            codes::INVALID_TIMESTAMP
        );

        // 连数字都解析不了的输入同样必须报错，而不是当成 0（=1970-01）
        let absurd = "9".repeat(40);
        assert_eq!(
            category_for(&absurd).unwrap_err().code,
            codes::INVALID_TIMESTAMP
        );
    }

    /// 极端输入不得 panic，也不得算出"看起来正常"的月份。
    ///
    /// `i128` 的边界值会经过 `div_euclid` / `checked_add` / `i32::try_from` 一路，
    /// 任何一步用错运算符都可能在 release 下悄悄回绕、在 debug 下直接 panic。
    #[test]
    fn extreme_i128_inputs_are_rejected_without_panicking() {
        for text in [
            i128::MIN.to_string(),
            i128::MAX.to_string(),
            (i128::MAX / 2).to_string(),
            (i128::MIN / 2).to_string(),
            " 1".to_owned(),
            "1e9".to_owned(),
        ] {
            let outcome = std::panic::catch_unwind(move || category_for(&text).map_err(|e| e.code));
            let result = outcome.unwrap_or_else(|_| panic!("极端输入不得 panic"));
            assert_eq!(
                result.err().as_deref(),
                Some(codes::INVALID_TIMESTAMP),
                "超出可表示范围的时间戳必须明确报错"
            );
        }
    }

    /// 纪元附近的负时间戳要落到正确的月份，而不是"回绕"到别的月份。
    #[test]
    fn timestamps_around_the_epoch_are_consistent() {
        assert_eq!(category_for("0").expect("纪元应可分类"), "1970-01");
        // 纪元前 1 纳秒：UTC+8 是 1970-01-01 07:59:59，UTC-5 是 1969-12-31 19:00
        let before = category_for("-1").expect("应可分类");
        assert!(
            before == "1970-01" || before == "1969-12",
            "纪元前 1 纳秒只能落在这两个月之一，实际 {before}"
        );
    }

    #[test]
    fn bucket_is_the_local_month_and_matches_the_documented_format() {
        // 用「当前时刻」做断言：把 UTC 时刻交给生产代码换算，
        // 再和 Windows 直接给出的本地时间比对——两条互相独立的取值路径。
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("本机时间应在纪元之后")
            .as_nanos() as i128;
        let bucket = category_for(&now_ns.to_string()).expect("当前时间应可分类");

        let local = crate::platform::windows::local_datetime_from_unix_ns(now_ns)
            .expect("当前时间应可换算");
        assert_eq!(bucket, format!("{:04}-{:02}", local.year, local.month));

        let bytes = bucket.as_bytes();
        assert_eq!(bytes.len(), 7, "必须是 YYYY-MM 共 7 个字符");
        assert_eq!(bytes[4], b'-');
        assert!(bytes[..4].iter().all(u8::is_ascii_digit));
        assert!(bytes[5..].iter().all(u8::is_ascii_digit));
    }

    #[test]
    fn different_months_land_in_different_buckets() {
        // 相隔 40 天必然跨过至少一个月份边界（最长月份 31 天），
        // 因此这个断言不依赖测试运行的具体日期。
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("本机时间应在纪元之后")
            .as_nanos() as i128;
        let forty_days_ns: i128 = 40 * 86_400 * 1_000_000_000;

        let now = category_for(&now_ns.to_string()).expect("当前时间应可分类");
        let earlier = category_for(&(now_ns - forty_days_ns).to_string()).expect("应可分类");
        assert_ne!(now, earlier, "相隔 40 天必须落在不同月份");
    }

    #[test]
    fn same_instant_maps_to_a_single_bucket_regardless_of_call_order() {
        let ns = utc_ns_of(2026, 5, 17, 6);
        let first = category_for(&ns.to_string()).expect("应可分类");
        let second = category_for(&ns.to_string()).expect("应可分类");
        assert_eq!(first, second, "同一输入必须得到同一分类");
    }

    #[test]
    fn unknown_reason_text_does_not_pretend_to_know_the_month() {
        assert_eq!(year_month_or_unknown("坏数据"), "未知");
        assert_eq!(
            year_month_or_unknown(&utc_ns_of(2026, 5, 17, 6).to_string()),
            "2026-05"
        );
    }

    #[test]
    fn bucket_of_a_utc_instant_is_the_local_calendar_month() {
        // 这条测试在不同时区的机器上都成立：断言的是「生产代码 == 系统时区换算」，
        // 而不是某个固定的月份字符串。
        let ns = utc_ns_of(2026, 3, 15, 12);
        let local = local_datetime_from_unix_ns(ns).expect("应可换算");
        assert_eq!(
            category_for(&ns.to_string()).expect("应可分类"),
            local.year_month()
        );
    }
}
