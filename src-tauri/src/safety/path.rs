//! 路径组件校验。
//!
//! 规格 7.1 的命名规则。这是**安全边界**的起点：
//! 后续所有目标路径都由「若干个已校验的组件」拼装而成，
//! 一旦这里放进了 `..`、绝对路径或设备名，后面的拼接就不能再算安全。

use crate::domain::{errors::codes, errors::AppError};

/// 单个名称组件的上限，单位是 **UTF-16 code units**（规格 7.1 第 2 条）。
///
/// 注意不是字符数：中文每字 1 个 unit，而 emoji 这类非 BMP 字符每字 2 个。
/// 用 `chars().count()` 会低估，于是「看起来没超」的名字在 Windows 上超限。
pub const MAX_COMPONENT_UTF16: usize = 80;

/// 目标总路径的上限（规格 7.1 第 2 条，v0.1 保守值）。
pub const MAX_TOTAL_PATH_UTF16: usize = 240;

/// Windows 禁止出现在文件名里的字符。
const ILLEGAL_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Windows 保留设备名（不含 COM/LPT 的数字部分）。
const RESERVED_BASE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// 把文件名拆成（主体, 扩展名含前导点）。
///
/// 扩展名**保持原大小写**：规格 7.1 第 1 条要求「程序附加原扩展名，含大小写」，
/// 因此 `.PDF` 不能变成 `.pdf`。边界与 `scanner::walk::extension_of` 一致：
/// 以点开头的名字（`.gitignore`）没有扩展名。
///
/// 放在这里而不是 `planner`：它是「怎么读一个 Windows 文件名」的规则，
/// 和组件校验属于同一族。放在规划器里会让 `rules` 反过来依赖 `planner`
/// （规划器又依赖 rules），形成模块环——环一旦形成，两个模块就没法各自被理解。
pub fn split_file_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(index) => (&name[..index], &name[index..]),
    }
}

/// 校验一个路径组件（单个目录名或文件名）。
///
/// 成功时原样返回输入 —— 规格 7.1 第 1 条要求真实源名称保持原样，
/// **不做 Unicode 归一化**，也不做大小写折叠。
pub fn validate_component(value: &str) -> Result<String, AppError> {
    // 空组件
    if value.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "名称不能为空"));
    }

    // 长度按 UTF-16 code units 计
    if value.encode_utf16().count() > MAX_COMPONENT_UTF16 {
        return Err(AppError::new(
            codes::PATH_TOO_LONG,
            format!("名称超过 {MAX_COMPONENT_UTF16} 个 UTF-16 字符单位"),
        ));
    }

    // 全点名称：`.`、`..`、`...` 一律拒绝。
    // 只拦 `.` 和 `..` 是不够的 —— Windows 会把多余的尾点规范化掉，
    // 于是 `...` 可能落到与预期不同的位置。
    if value.chars().all(|c| c == '.') {
        return Err(AppError::new(codes::INVALID_PATH, "名称不能只由点组成"));
    }

    // 非法字符与控制字符
    for ch in value.chars() {
        if ch.is_control() {
            return Err(AppError::new(codes::INVALID_PATH, "名称不能包含控制字符"));
        }
        if ILLEGAL_CHARS.contains(&ch) {
            return Err(AppError::new(
                codes::INVALID_PATH,
                format!("名称不能包含字符 {ch:?}"),
            ));
        }
    }

    // 前导 / 尾随空格与点。
    // Windows 在多数路径解析路径上会**静默剥离**它们，导致落盘名与预览里
    // 显示的名字不一致 —— 对一个「先预览再执行」的产品来说这是不可接受的。
    if value.starts_with(' ') {
        return Err(AppError::new(
            codes::INVALID_PATH,
            "名称不能以空格开头（Windows 可能将其剥离）",
        ));
    }
    if value.ends_with(' ') || value.ends_with('.') {
        return Err(AppError::new(
            codes::INVALID_PATH,
            "名称不能以空格或点结尾（Windows 会将其剥离）",
        ));
    }

    // 保留设备名
    if is_reserved_device_name(value) {
        return Err(AppError::new(
            codes::RESERVED_NAME,
            format!("{value:?} 是 Windows 保留设备名，不能用作目录或文件名"),
        ));
    }

    Ok(value.to_owned())
}

/// 是否是 Windows 保留设备名。
///
/// 规则来自平台语义而不是一个简单正则：
/// - **大小写不敏感**：`con` 与 `CON` 等价；
/// - **带扩展名同样保留**：`CON.txt` 仍然指向控制台设备；
/// - **上标数字也算数字**：`COM¹` 与 `COM1` 在 Win32 里被同等对待
///   （U+00B9 / U+00B2 / U+00B3）。
fn is_reserved_device_name(name: &str) -> bool {
    // Windows 只把第一个点之前的部分当作设备名主干
    let stem = match name.find('.') {
        Some(idx) => &name[..idx],
        None => name,
    }
    // Win32 会在解析设备名时剥离主干末尾的空格与点。
    // 因此 `COM1 .txt`、`NUL..log` 仍然是设备名。
    .trim_end_matches([' ', '.']);

    // 上标数字先归一化成 ASCII 数字，再按 ASCII 大写比较。
    // 用 to_ascii_uppercase 而不是 to_uppercase：后者对非 ASCII 可能展开成多个字符，
    // 会制造出长度变化的比较目标。
    let normalized: String = stem
        .chars()
        .map(|c| match c {
            '\u{00B9}' => '1',
            '\u{00B2}' => '2',
            '\u{00B3}' => '3',
            other => other,
        })
        .collect::<String>()
        .to_ascii_uppercase();

    if RESERVED_BASE_NAMES.contains(&normalized.as_str()) {
        return true;
    }

    // COM1–COM9 / LPT1–LPT9。COM0 与 LPT0 不是保留名。
    // 不能对任意 UTF-8 字符串 `split_at(3)`：3 可能落在中文或 emoji
    // 的编码中间并触发 panic。只在 ASCII 前缀匹配成功后检查剩余部分。
    if let Some(digit) = normalized
        .strip_prefix("COM")
        .or_else(|| normalized.strip_prefix("LPT"))
    {
        return digit.len() == 1 && matches!(digit.as_bytes()[0], b'1'..=b'9');
    }

    false
}

/// 校验一整条相对路径（组件数组）。
///
/// 逐组件校验，并检查总长度。
pub fn validate_relative_path(components: &[String]) -> Result<(), AppError> {
    if components.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "路径不能为空"));
    }

    let mut total_units = 0usize;
    for component in components {
        validate_component(component)?;
        // 每个组件后面跟一个分隔符
        total_units += component.encode_utf16().count() + 1;
    }

    if total_units > MAX_TOTAL_PATH_UTF16 {
        return Err(AppError::new(
            codes::PATH_TOO_LONG,
            format!("目标路径超过 {MAX_TOTAL_PATH_UTF16} 个 UTF-16 字符单位"),
        ));
    }

    Ok(())
}

/// 校验已经存在于磁盘上的源相对路径。
///
/// 源名称不是应用生成的，可能早于当前 80-unit 产品限制。这里仅阻止路径逃逸
/// 和无法作为单个 Windows 组件的内容，不把“新名称规则”倒套在用户旧文件上。
pub fn validate_existing_relative_path(components: &[String]) -> Result<(), AppError> {
    if components.is_empty() {
        return Err(AppError::new(codes::INVALID_PATH, "路径不能为空"));
    }
    for value in components {
        if value.is_empty() || value == "." || value == ".." {
            return Err(AppError::new(codes::INVALID_PATH, "源路径包含非法组件"));
        }
        if value
            .chars()
            .any(|ch| ch.is_control() || ILLEGAL_CHARS.contains(&ch))
        {
            return Err(AppError::new(codes::INVALID_PATH, "源路径包含非法字符"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_file_name_keeps_the_extension_case() {
        assert_eq!(split_file_name("报告.PDF"), ("报告", ".PDF"));
        assert_eq!(split_file_name("a.tar.gz"), ("a.tar", ".gz"));
        assert_eq!(split_file_name("无扩展名"), ("无扩展名", ""));
        // 前导点是隐藏文件标记，不是扩展名
        assert_eq!(split_file_name(".gitignore"), (".gitignore", ""));
        assert_eq!(split_file_name(""), ("", ""));
    }

    #[test]
    fn device_name_detection_matches_win32_semantics() {
        assert!(is_reserved_device_name("CON"));
        assert!(is_reserved_device_name("con"));
        assert!(is_reserved_device_name("Con.txt"));
        assert!(is_reserved_device_name("COM1"));
        assert!(is_reserved_device_name("lpt9.md"));
        assert!(is_reserved_device_name("COM\u{00B9}"));

        assert!(!is_reserved_device_name("COM0"));
        assert!(!is_reserved_device_name("LPT0"));
        assert!(!is_reserved_device_name("CONSOLE"));
        assert!(!is_reserved_device_name("COM10"));
        assert!(!is_reserved_device_name("我的CON"));
    }

    #[test]
    fn dot_only_names_are_rejected_in_every_length() {
        for n in [".", "..", "...", "...."] {
            assert!(validate_component(n).is_err(), "{n:?} 应被拒绝");
        }
        // 有点但还有别的字符是合法的
        assert!(validate_component(".gitignore").is_ok());
    }

    #[test]
    fn relative_path_checks_total_length() {
        let ok = vec!["学习".to_owned(), "数学".to_owned()];
        assert!(validate_relative_path(&ok).is_ok());

        let too_long = vec!["中".repeat(80), "中".repeat(80), "中".repeat(80)];
        let err = validate_relative_path(&too_long).unwrap_err();
        assert_eq!(err.code, codes::PATH_TOO_LONG);

        assert!(validate_relative_path(&[]).is_err());
    }

    #[test]
    fn illegal_component_inside_a_path_is_rejected() {
        let bad = vec!["学习".to_owned(), "..".to_owned()];
        assert_eq!(
            validate_relative_path(&bad).unwrap_err().code,
            codes::INVALID_PATH
        );
    }
}
