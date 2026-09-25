//! 文件快照与内容哈希。
//!
//! 规格 6.1：**采样哈希不能替代完整哈希**。
//! 规格 INV-01：扫描、解析、AI 分析、预览都不得改变用户文件。

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::domain::{errors::AppError, types::Fingerprint};
use crate::platform::windows;

/// 一次性读取的块大小。64 KiB 在机械盘与 NVMe 上都接近最优。
const HASH_CHUNK_BYTES: usize = 64 * 1024;

/// 规格 6.1：大于 100 MiB 的文件 v0.1 标记超限，不参与执行。
pub const MAX_HASHABLE_BYTES: u64 = 100 * 1024 * 1024;

/// 把一个 `SystemTime` 转成规格 5.1 要求的**十进制纳秒字符串**。
///
/// 之所以不跨 IPC 传数字：JavaScript 的 Number 只有 53 位有效位，
/// 纳秒级时间戳（1.7e18）会直接丢精度，导致「看起来相同的指纹其实不同」。
pub fn modified_ns(meta: &std::fs::Metadata) -> io::Result<String> {
    let modified = meta.modified()?;
    let nanos = match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        // 时间早于 1970 的极端情况：保留符号，不 panic
        Err(e) => -(e.duration().as_nanos() as i128),
    };
    Ok(nanos.to_string())
}

/// 流式计算 SHA-256。
///
/// 返回**小写十六进制**字符串。分块读取是为了不把整个文件读进内存——
/// 100 MiB 的文件一次性读入会明显抬高内存峰值，而扫描是并发度更高的路径。
pub fn sha256_file(path: &Path, limit: u64) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("打开文件失败: {e}"),
        )
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK_BYTES];
    let mut read_total: u64 = 0;

    loop {
        let n = file.read(&mut buffer).map_err(|e| {
            AppError::new(
                crate::domain::errors::codes::SOURCE_CHANGED,
                format!("读取文件时失败: {e}"),
            )
        })?;
        if n == 0 {
            break;
        }
        read_total += n as u64;
        if read_total > limit {
            return Err(AppError::new(
                crate::domain::errors::codes::SOURCE_CHANGED,
                "文件在哈希过程中超过了大小上限，已中止",
            ));
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// 从一个**已打开的句柄**计算 SHA-256。
///
/// 与 [] 的区别：后者按路径重新打开文件，而路径在校验与执行之间
/// 可能被换掉（规格 7.3 点名的问题）。执行路径必须用这个版本，
/// 保证「校验的文件」和「即将被重命名的文件」是同一个句柄。
/// 大文件哈希的进度回调与取消开关（规格 T15）。
///
/// 规格原文：
///
/// > 整理大文件时**哈希有进度和取消**。
///
/// ## 为什么这两件事必须一起给
///
/// 一个几 GB 文件的 SHA-256 要跑好几秒到几十秒。那段时间里界面既没有进度
/// 也不能取消时，用户看到的只是一个**卡住的界面**——而他会合理地以为
/// 程序死了，然后去点关闭窗口。
///
/// 只给进度不给取消同样不够：知道「还要 20 秒」不能让他做任何事。
#[derive(Default)]
pub struct HashControl<'a> {
    /// 已经读过的字节数。每读完一块调一次。
    pub on_progress: Option<&'a mut dyn FnMut(u64)>,
    /// 返回 `true` 表示用户要求停止。
    pub is_cancelled: Option<&'a dyn Fn() -> bool>,
}

/// 算一个句柄的 SHA-256，**带进度与取消**。
///
/// 被取消时返回 `Ok(None)`——**不返回 `Err`**：取消是用户的选择，不是失败。
/// 用错误表达它，会让调用方在错误处理里区分「出错了」与「用户不要了」，
/// 而那个区分本来就该由返回类型表达。
///
/// 两个控制都是可选的：`&mut HashControl::default()` 就是「不报进度、
/// 不取消」。用 `Option` 而不是「必须有回调」，是因为**有些调用方算的确实
/// 是小文件**，逼它们写两个空闭包只会让调用点变吵。
///
/// ## 它从**当前位置**读到末尾
///
/// 不是「从头算一遍」：那个句柄可能已经被读过一部分（提取流程就是这样），
/// 而 `try_clone` 出一份独立文件指针会绕开「身份取自同一个句柄」这条
/// （规格 7.3：否则会出现「验证 A、执行 B」的窗口）。
///
/// 代价是**调用方要自己保证位置**：同一个句柄上连算两次，第二次会从末尾
/// 读到零字节，于是两个哈希必然不同。这一条是从一条写错的测试里发现的
/// ——那一版测试复用了同一个句柄，红了之后才发现这个语义从没被写下来。
pub fn sha256_of_handle_controlled(
    file: &File,
    control: &mut HashControl<'_>,
) -> io::Result<Option<String>> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK_BYTES];
    //  实现了 Read，直接借用即可，不要 try_clone 出一份独立文件指针
    let mut reader = file;
    let mut done: u64 = 0;

    loop {
        // **先查取消再读**：反过来的话，一次已经开始的读无法被打断，
        // 用户点了「停止」还得等当前这一块读完。
        if control.is_cancelled.is_some_and(|check| check()) {
            return Ok(None);
        }

        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        done += n as u64;
        if let Some(report) = control.on_progress.as_mut() {
            report(done);
        }
    }

    Ok(Some(format!("{:x}", hasher.finalize())))
}

pub fn sha256_of_handle(file: &File) -> io::Result<String> {
    // 旧签名保留（调用方很多），内部走**同一条路径**——不是两份实现。
    // 两份实现迟早会在某个边界上不一致，而「同一个文件的哈希」不一致
    // 是最难查的一类问题。有一条测试专门比对这两条路。
    match sha256_of_handle_controlled(file, &mut HashControl::default())? {
        Some(digest) => Ok(digest),
        // 走不到这里：`HashControl::default()` 的两个控制都是 `None`，
        // 而取消只在 `is_cancelled` 为 `Some` 时才可能为真。
        //
        // **但它返回 `Err` 而不是 `panic!`**：这个函数在**执行路径**上
        // （`safety/fingerprint.rs` 会调它），而一个 panic 会炸掉整批
        // 已经完成的工作。一个不可能发生的分支，不值得用那种代价表达。
        None => Err(io::Error::other("哈希在没有取消的情况下返回了空结果")),
    }
}

/// 为一个已存在的普通文件建立完整指纹。
///
/// `expect_sha256 = true` 时计算内容哈希；初扫传 `false`（规格 6.1：
/// 初扫只记 metadata，进入选中计划前才算 SHA-256）。
///
/// 关键：**身份取自打开的那个句柄**，而不是再按路径打开一次。
/// 否则会出现「验证 A、执行 B」的窗口（规格 7.3）。
pub fn fingerprint(
    path: &Path,
    volume_id: &str,
    expect_sha256: bool,
) -> Result<Fingerprint, AppError> {
    let initial = std::fs::symlink_metadata(path).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::SOURCE_MISSING,
            format!("读取文件属性失败: {e}"),
        )
    })?;
    if !initial.is_file() {
        return Err(AppError::new(
            crate::domain::errors::codes::INVALID_PATH,
            "目标不是普通文件",
        ));
    }
    let file = windows::open_source_for_snapshot(path).map_err(|e| {
        let code = if e.kind() == io::ErrorKind::NotFound {
            crate::domain::errors::codes::SOURCE_MISSING
        } else {
            crate::domain::errors::codes::PERMISSION_DENIED
        };
        AppError::new(code, format!("打开文件失败: {e}"))
    })?;
    let identity = windows::file_identity(&file).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("读取文件身份失败: {e}"),
        )
    })?;
    if identity.volume_id_string() != volume_id {
        return Err(AppError::new(
            crate::domain::errors::codes::SOURCE_CHANGED,
            "文件卷身份与批准根不一致",
        ));
    }
    if windows::file_link_count(&file).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("读取硬链接数量失败: {e}"),
        )
    })? > 1
    {
        return Err(AppError::new(
            crate::domain::errors::codes::INVALID_PATH,
            "多硬链接文件不参与整理",
        ));
    }
    let (size, modified_ns) = windows::size_and_modified(&file).map_err(|e| {
        AppError::new(
            crate::domain::errors::codes::PERMISSION_DENIED,
            format!("读取文件快照失败: {e}"),
        )
    })?;
    if size > MAX_HASHABLE_BYTES {
        return Err(AppError::new(
            crate::domain::errors::codes::INVALID_PATH,
            format!(
                "文件超过 v0.1 的 {} MiB 上限，不参与执行",
                MAX_HASHABLE_BYTES / 1024 / 1024
            ),
        ));
    }

    let sha256 = if expect_sha256 {
        Some(sha256_of_handle(&file).map_err(|e| {
            AppError::new(
                crate::domain::errors::codes::SOURCE_CHANGED,
                format!("读取文件内容失败: {e}"),
            )
        })?)
    } else {
        None
    };

    Ok(Fingerprint {
        volume_id: identity.volume_id_string(),
        file_id: identity.file_id_string(),
        size: size.to_string(),
        modified_ns,
        sha256,
    })
}

/// 供测试与诊断使用：把 `SystemTime` 直接转成同样的字符串。
pub fn system_time_to_ns(t: SystemTime) -> String {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos().to_string(),
        Err(e) => format!("-{}", e.duration().as_nanos()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_known_vectors() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"abc").expect("写文件");

        // SHA-256("abc") 的公开测试向量
        let got = sha256_file(&path, MAX_HASHABLE_BYTES).expect("哈希应成功");
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empty_file_hashes_to_the_empty_digest() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("empty.bin");
        std::fs::write(&path, b"").expect("写空文件");
        assert_eq!(
            sha256_file(&path, MAX_HASHABLE_BYTES).expect("哈希应成功"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn content_change_is_detected_even_when_size_is_unchanged() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("x.txt");
        std::fs::write(&path, b"aaaa").expect("写文件");
        let before = sha256_file(&path, MAX_HASHABLE_BYTES).expect("哈希");

        std::fs::write(&path, b"bbbb").expect("改写");
        let after = sha256_file(&path, MAX_HASHABLE_BYTES).expect("哈希");

        assert_ne!(before, after, "同长度不同内容必须得到不同哈希");
    }

    #[test]
    fn exceeding_the_limit_is_an_explicit_error() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("big.bin");
        std::fs::write(&path, vec![0u8; 4096]).expect("写文件");

        let err = sha256_file(&path, 1024).expect_err("超过上限应报错");
        assert_eq!(err.code, crate::domain::errors::codes::SOURCE_CHANGED);
    }

    #[test]
    fn fingerprint_carries_volume_and_file_identity() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("f.txt");
        std::fs::write(&path, b"hello").expect("写文件");

        let handle = windows::open_source_for_snapshot(&path).expect("打开测试文件");
        let volume_id = windows::file_identity(&handle)
            .expect("读取卷身份")
            .volume_id_string();
        drop(handle);
        let fp = fingerprint(&path, &volume_id, true).expect("取指纹应成功");
        assert_eq!(fp.volume_id, volume_id);
        assert_eq!(fp.size, "5");
        assert_eq!(fp.sha256.as_deref().map(str::len), Some(64));
        assert!(!fp.file_id.is_empty());
        assert!(
            fp.modified_ns.parse::<i128>().is_ok(),
            "modifiedNs 必须是十进制字符串"
        );
    }

    #[test]
    fn fingerprint_leaves_sha_empty_when_not_required() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("g.txt");
        std::fs::write(&path, b"x").expect("写文件");

        let handle = windows::open_source_for_snapshot(&path).expect("打开测试文件");
        let volume_id = windows::file_identity(&handle)
            .expect("读取卷身份")
            .volume_id_string();
        drop(handle);
        let fp = fingerprint(&path, &volume_id, false).expect("取指纹应成功");
        assert!(fp.sha256.is_none(), "初扫不应计算内容哈希");
    }

    #[test]
    fn directory_is_not_a_valid_fingerprint_target() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let err = fingerprint(dir.path(), "V", false).expect_err("目录不应有文件指纹");
        assert_eq!(err.code, crate::domain::errors::codes::INVALID_PATH);
    }

    #[test]
    fn fingerprint_rejects_a_volume_id_not_matching_the_open_handle() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let path = dir.path().join("wrong-volume.txt");
        std::fs::write(&path, b"x").expect("写文件");
        let err = fingerprint(&path, "FFFFFFFF", false).expect_err("卷身份不符必须拒绝");
        assert_eq!(err.code, crate::domain::errors::codes::SOURCE_CHANGED);
    }

    #[test]
    fn missing_file_reports_source_missing() {
        let dir = tempfile::tempdir().expect("建临时目录");
        let err =
            fingerprint(&dir.path().join("nope.txt"), "V", false).expect_err("缺失文件应报错");
        assert_eq!(err.code, crate::domain::errors::codes::SOURCE_MISSING);
    }

    // -----------------------------------------------------------------------
    // 大文件哈希的进度与取消（规格 T15）
    // -----------------------------------------------------------------------

    /// 造一个多块的文件，并打开它。
    ///
    /// 它必须**大于一块**，否则「进度」只会被回调一次，测不出单调性，
    /// 也测不出「取消发生在块与块之间」。
    fn multi_chunk_file(dir: &std::path::Path) -> (std::fs::File, u64) {
        let path = dir.join("big.bin");
        let size = (HASH_CHUNK_BYTES as u64) * 3 + 1234;
        let bytes = vec![0x5Au8; size as usize];
        std::fs::write(&path, &bytes).expect("写大文件");
        let file = std::fs::File::open(&path).expect("打开大文件");
        (file, size)
    }

    #[test]
    fn hashing_reports_monotonic_progress_that_ends_at_the_file_size() {
        // 规格 T15：「整理大文件时哈希有**进度**」。
        let dir = tempfile::tempdir().expect("建临时目录");
        let (file, size) = multi_chunk_file(dir.path());

        let mut seen: Vec<u64> = Vec::new();
        let mut control = HashControl {
            on_progress: Some(&mut |done| seen.push(done)),
            is_cancelled: None,
        };

        let digest = sha256_of_handle_controlled(&file, &mut control)
            .expect("算哈希")
            .expect("没有取消就该有结果");

        assert_eq!(digest.len(), 64, "SHA-256 是 64 个十六进制字符");
        assert!(seen.len() >= 3, "多块文件应当回调多次，实际 {seen:?}");
        assert!(
            seen.windows(2).all(|pair| pair[0] < pair[1]),
            "进度必须严格递增——它要用来画百分比条：{seen:?}"
        );
        assert_eq!(
            *seen.last().expect("至少回调一次"),
            size,
            "最后一次回调必须正好是文件大小，否则百分比到不了 100%"
        );
    }

    #[test]
    fn cancelling_stops_the_hash_and_returns_no_digest() {
        // 规格 T15：「整理大文件时哈希有进度和**取消**」。
        let dir = tempfile::tempdir().expect("建临时目录");
        let (file, _) = multi_chunk_file(dir.path());

        // 第一次回调之后就要求停止。
        //
        // 用 `Cell` 而不是普通的 `usize`：两个闭包都要读它，而其中一个
        // 还要写——一个 `&mut` 加一个 `&` 会同时借用同一个变量。`Cell`
        // 让两者都只**不可变**借用它。
        let calls = std::cell::Cell::new(0usize);
        let mut control = HashControl {
            on_progress: Some(&mut |_| calls.set(calls.get() + 1)),
            is_cancelled: Some(&|| calls.get() >= 1),
        };

        let outcome = sha256_of_handle_controlled(&file, &mut control).expect("不该是 io 错误");

        assert!(
            outcome.is_none(),
            "取消必须返回 None——把它表达成 Err 会让调用方分不清\
             「出错了」与「用户不要了」"
        );
        // 取消在**读下一块之前**检查，所以最多多回调一次。
        assert!(
            calls.get() <= 2,
            "取消应当很快生效，实际回调了 {} 次",
            calls.get()
        );
    }

    #[test]
    fn cancelling_before_the_first_read_does_no_work_at_all() {
        // 「一开始就取消」也要能立刻返回：它对应「用户在弹窗里点了取消，
        // 而任务刚好开始」。
        let dir = tempfile::tempdir().expect("建临时目录");
        let (file, _) = multi_chunk_file(dir.path());

        let calls = std::cell::Cell::new(0usize);
        let mut control = HashControl {
            on_progress: Some(&mut |_| calls.set(calls.get() + 1)),
            is_cancelled: Some(&|| true),
        };

        let outcome = sha256_of_handle_controlled(&file, &mut control).expect("不该是 io 错误");

        assert!(outcome.is_none());
        assert_eq!(calls.get(), 0, "一块都不该读");
    }

    #[test]
    fn the_controlled_and_plain_paths_agree() {
        // 两条路必须给出**同一个**哈希。它们共用同一段实现，但这条断言
        // 是防止将来有人「顺手优化」成两份——而「同一个文件的哈希不一致」
        // 是最难查的一类问题。
        //
        // 注意**两次都要开新的 `File`**：这个函数从**当前位置**读到末尾，
        // 复用一个句柄的话第二次会从末尾开始读到零字节，于是两个哈希
        // 必然不同——而那是测试的错，不是实现的错。
        // （第一版就是这么写的，红了才发现这个语义没被写下来。）
        let dir = tempfile::tempdir().expect("建临时目录");
        let (first_handle, _) = multi_chunk_file(dir.path());

        let plain = sha256_of_handle(&first_handle).expect("普通路径");

        let (second_handle, _) = multi_chunk_file(dir.path());
        let controlled = sha256_of_handle_controlled(&second_handle, &mut HashControl::default())
            .expect("受控路径")
            .expect("没有取消就该有结果");

        assert_eq!(plain, controlled);
    }
}
