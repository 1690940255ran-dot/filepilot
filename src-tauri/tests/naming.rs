//! 规格 10.1 的 N01 与 10.2：路径组件校验。
//!
//! 这些是**安全边界**的一部分：组件校验不严，就会让 `..`、绝对路径或设备名
//! 混进目标路径，而后续所有拼装都建立在「组件是安全的单个名称」这个前提上。

mod support;

use filepilot_lib::domain::errors::codes;
use filepilot_lib::safety::path::{
    validate_component, validate_relative_path, MAX_COMPONENT_UTF16,
};

// ---------------------------------------------------------------------------
// 规格 10.2 直接给出的四组用例，逐字保留
// ---------------------------------------------------------------------------

#[test]
fn rejects_traversal_and_separators() {
    for name in ["..", ".", "", "a/b", "a\\b", "C:\\outside", "a:stream"] {
        assert!(validate_component(name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn rejects_windows_device_names_and_trailing_chars() {
    for name in [
        "CON", "con.txt", "AUX", "COM1", "LPT9.txt", "name.", "name ",
    ] {
        assert!(validate_component(name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn keeps_legal_chinese_name() {
    let actual = validate_component("高等数学 第三章").unwrap();
    assert_eq!(actual, "高等数学 第三章");
}

#[test]
fn rejects_long_utf16_component() {
    let name = "中".repeat(81);
    let error = validate_component(&name).unwrap_err();
    assert_eq!(error.code, "PATH_TOO_LONG");
}

// ---------------------------------------------------------------------------
// 边界与语义：这些才是真正容易漏掉的部分
// ---------------------------------------------------------------------------

#[test]
fn rejects_every_windows_device_name_case_insensitively() {
    // 规格 7.1 第 4 条：保留设备名**大小写不敏感**，且带扩展名也算
    for base in ["CON", "PRN", "AUX", "NUL", "COM1", "COM9", "LPT1", "LPT9"] {
        for variant in [base.to_owned(), base.to_lowercase(), format!("{base}.txt")] {
            assert!(
                validate_component(&variant).is_err(),
                "应拒绝保留设备名 {variant:?}"
            );
        }
    }
    // COM0 / LPT0 不是保留名（Windows 只认 1-9）
    assert!(validate_component("COM0").is_ok(), "COM0 不是保留设备名");
    assert!(validate_component("LPT0").is_ok(), "LPT0 不是保留设备名");
}

#[test]
fn rejects_superscript_device_names() {
    // Windows 把上标数字 ¹²³ 也当作数字：COM¹ 同样解析为 COM1
    for name in ["COM\u{00B9}", "COM\u{00B2}", "COM\u{00B3}", "LPT\u{00B9}"] {
        assert!(
            validate_component(name).is_err(),
            "应拒绝上标数字设备名 {name:?}"
        );
    }
}

#[test]
fn rejects_control_characters() {
    for name in ["a\u{0}b", "tab\there", "new\nline", "del\u{7F}"] {
        assert!(validate_component(name).is_err(), "应拒绝控制字符 {name:?}");
    }
}

#[test]
fn rejects_the_full_set_of_illegal_characters() {
    for ch in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
        let name = format!("a{ch}b");
        assert!(validate_component(&name).is_err(), "应拒绝含 {ch:?} 的名称");
    }
}

#[test]
fn rejects_trailing_space_or_dot_but_keeps_inner_ones() {
    // Windows 会静默剥掉尾随空格与点，导致落盘名与用户看到的不一致 —— 必须拒绝
    assert!(validate_component("报告 ").is_err());
    assert!(validate_component("报告.").is_err());
    assert!(validate_component("报告..").is_err());
    // 中间的空格和点是合法的
    assert!(validate_component("高等数学 第三章.pdf").is_ok());
    assert!(validate_component("a. b").is_ok());
}

#[test]
fn rejects_leading_space_because_windows_may_strip_it() {
    // 前导空格同样可能被 shell / 资源管理器处理掉，且极易与相邻项混淆
    assert!(validate_component(" 报告").is_err(), "应拒绝前导空格");
}

#[test]
fn component_limit_counts_utf16_code_units_not_chars() {
    // 规格 7.1 第 2 条写的是 80 个 **UTF-16 code units**。
    // 中文每字 1 个 unit，emoji（非 BMP）每字 2 个 —— 用 char count 会算错。
    let sixty_cjk = "中".repeat(60);
    assert!(
        validate_component(&sixty_cjk).is_ok(),
        "60 个中文应在上限内"
    );

    // 每个 emoji 2 个 unit，40 个 = 80，刚好卡在上限
    let forty_emoji = "🎓".repeat(40);
    assert_eq!(forty_emoji.encode_utf16().count(), 80);
    assert!(
        validate_component(&forty_emoji).is_ok(),
        "正好 80 个 UTF-16 unit 应当被接受"
    );

    // 41 个 emoji = 82 个 unit，超限
    let forty_one_emoji = "🎓".repeat(41);
    let err = validate_component(&forty_one_emoji).unwrap_err();
    assert_eq!(err.code, codes::PATH_TOO_LONG);
}

#[test]
fn boundary_at_exactly_eighty_units_is_accepted() {
    let name = "a".repeat(MAX_COMPONENT_UTF16);
    assert!(validate_component(&name).is_ok(), "正好到上限应当接受");

    let too_long = "a".repeat(MAX_COMPONENT_UTF16 + 1);
    assert_eq!(
        validate_component(&too_long).unwrap_err().code,
        codes::PATH_TOO_LONG
    );
}

#[test]
fn returns_the_name_unchanged_for_legal_input() {
    // 规格 7.1 第 1 条：真实源名称保持原样，不做 Unicode 归一化
    for legal in [
        "报告.pdf",
        "高等数学 第三章",
        "a-b_c（副本）",
        "MixedCase.TXT",
        "😀",
        "a中",
        "éé",
    ] {
        assert_eq!(
            validate_component(legal).unwrap(),
            legal,
            "合法名称必须原样返回，不得做归一化或大小写转换"
        );
    }
}

#[test]
fn rejects_absolute_paths_and_device_paths_in_every_form() {
    for bad in [
        r"C:\outside",
        r"\\server\share",
        r"\\?\C:\x",
        r"\\.\PhysicalDrive0",
        "/absolute",
        "a:stream",
        "a:",
    ] {
        assert!(validate_component(bad).is_err(), "应拒绝 {bad:?}");
    }
}

#[test]
fn error_codes_match_the_spec_table() {
    // 规格 8.5：非法路径用 INVALID_PATH，超长用 PATH_TOO_LONG，保留名用 RESERVED_NAME
    assert_eq!(
        validate_component("..").unwrap_err().code,
        codes::INVALID_PATH
    );
    assert_eq!(
        validate_component("CON").unwrap_err().code,
        codes::RESERVED_NAME
    );
    assert_eq!(
        validate_component(&"a".repeat(200)).unwrap_err().code,
        codes::PATH_TOO_LONG
    );
}

// ---------------------------------------------------------------------------
// 主动排查时补的边界：Windows 的路径解析比「一眼看上去」更宽松
// ---------------------------------------------------------------------------

#[test]
fn rejects_device_name_separated_from_extension_by_spaces() {
    // Win32 会剥离文件名主干后的尾随空格，于是 `COM1 .txt` 与 `COM1.txt` 等价。
    // 只判断「第一个点之前的部分」是拦不住这种写法的。
    for name in ["COM1 .txt", "NUL .txt", "AUX  .log"] {
        assert!(
            validate_component(name).is_err(),
            "应拒绝「设备名 + 空格 + 扩展名」形式：{name:?}"
        );
    }
}

#[test]
fn rejects_device_names_with_extra_dots_before_extension() {
    // `CON..txt` 同样会被 Win32 归一化到设备名
    for name in ["CON..txt", "NUL...log"] {
        assert!(validate_component(name).is_err(), "应拒绝 {name:?}");
    }
}

#[test]
fn relative_path_alone_can_be_short_while_the_full_path_is_too_long() {
    // 规格 7.1 限制的是**目标总路径**（含根），只校验相对部分是漏的。
    // 这里构造一个「相对部分很短、但加上根就超限」的场景。
    let relative = vec!["sub".to_owned(), "file.txt".to_owned()];
    assert!(
        validate_relative_path(&relative).is_ok(),
        "相对部分本身是合法的"
    );

    // 根前缀由 ApprovedRoot 提供，所以总长度必须由 execute 层校验，
    // 见 tests/execute_windows.rs 的 total_path_length_includes_the_root。
}
