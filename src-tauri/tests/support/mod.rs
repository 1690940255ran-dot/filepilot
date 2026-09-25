//! 集成测试共用支撑。
//!
//! 规格 10.1：**所有集成测试使用独立目录，绝不把真实用户数据复制进测试。**
//!
//! 这里有一个必须显式处理的坑：`tempfile::TempDir::new()` 建在 `%TEMP%`，
//! 而 `%TEMP%` 位于 `%LOCALAPPDATA%\Temp` —— 按规格 1.2 的「拒绝应用数据目录」，
//! 这样的目录**永远无法通过根授权**。所以测试根必须建在一个普通位置。
//! 默认使用项目目录的同级专用目录，既避开源码仓库内部，也不会在用户主目录
//! 留下测试痕迹；CI 可通过 `FILEPILOT_TEST_BASE` 显式覆盖。

#![allow(dead_code)] // 各测试文件只会用到其中一部分

pub mod fake_http;

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, TempDir};

/// 所有测试根都建在这个目录下，便于集中清理。
const TEST_BASE_DIR: &str = "filepilot-test";

/// 建立一个**可通过根授权**的临时目录。
///
/// 返回的 `TempDir` 在 drop 时自行清理。
pub fn test_root() -> TempDir {
    let base = test_base_dir();
    fs::create_dir_all(&base).expect("创建测试基目录应成功");
    Builder::new()
        .prefix("case-")
        .tempdir_in(&base)
        .expect("在测试基目录下建临时目录应成功")
}

fn test_base_dir() -> PathBuf {
    if let Some(configured) = std::env::var_os("FILEPILOT_TEST_BASE") {
        return PathBuf::from(configured);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("src-tauri 应位于项目目录内")
        .join(TEST_BASE_DIR)
}

/// 在 `root` 下按相对片段建文件，父目录自动创建。
///
/// 写完后会等待 Windows Defender / 索引服务对该文件放开句柄
/// （见 [`settle_fs_noise`]）——测试几乎总是"刚建完文件就执行"，
/// 而全量跑时 Defender 的扫描队列正被上游测试激活。
pub fn write_file(root: &Path, relative: &[&str], contents: &[u8]) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative {
        path.push(part);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录应成功");
    }
    fs::write(&path, contents).expect("写文件应成功");
    settle_fs_noise(&path);
    path
}

/// 等待 Windows Defender / 索引服务对**刚创建的文件**放开句柄。
///
/// ## 这个函数为什么存在
///
/// Windows 上 `CreateFile`（`fs::write`）返回后，Defender 的实时扫描与
/// Search Indexer 仍可能对该文件短暂持有一个**拒绝共享写/删**的句柄，
/// 通常几十到几百毫秒。紧接着对这个文件做
/// `open_source_for_rename`（`FILE_SHARE_READ`，不共享写/删）就会撞
/// `ERROR_ACCESS_DENIED`（os error 5）。
///
/// 这是**测试环境固有的噪声**，不是被测代码的缺陷——生产环境里用户
/// 不会刚保存完一个文件立刻整理它。集成测试的全量跑会在极短时间内
/// 建/删大量文件（`lib` 的 150 个单测并发跑），激活 Defender 的
/// 扫描队列，因此只有"全量跑"才偶发、单跑必绿。
/// 连 rustc 自己清理增量编译缓存都撞过同一类 `os error 5`。
///
/// ## 语义边界
///
/// 它只等**瞬时**的 `ACCESS_DENIED`（error 5），且探测成功立即返回。
/// 真正被其他句柄长期占用的文件会在超时后**如实 panic**，而不是
/// 无限等下去——所以它不会把"文件被占用"的用例掩盖成通过：
/// 那类用例的锁是在 `write_file` 返回**之后**才由用例自己持有的。
pub fn settle_fs_noise(path: &Path) {
    use std::time::{Duration, Instant};

    // Defender 的实时扫描通常远低于 500ms；给 2s 上限是为了在
    // 高负载 CI 上也不抖动。超过这个仍撞锁，那就是真的被占用，
    // 应当如实失败，而不是无限等下去。
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // 探测方式必须与生产代码的失败路径一致：`open_source_for_rename`
        // 用的是 `FILE_SHARE_READ`（**不共享写/删**）。用默认的
        // `File::open`（共享读写删）会探测不到 Defender 的排他锁，
        // 于是"等"了一个根本没在等的条件。
        match filepilot_lib::platform::windows::open_source_for_rename(path) {
            Ok(_) => return,
            Err(error) if error.raw_os_error() == Some(5) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                panic!(
                    "等待文件句柄释放超时，但错误不是瞬时的: {error}（路径 {}）",
                    path.display()
                );
            }
        }
    }
}

/// 建立目录。
pub fn make_dir(root: &Path, relative: &[&str]) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative {
        path.push(part);
    }
    fs::create_dir_all(&path).expect("创建目录应成功");
    path
}

/// 把相对片段拼到根之下，返回绝对路径。
///
/// 只给测试构造路径用；生产代码必须走 `ApprovedRoot::resolve_within`，
/// 那条路径带组件校验。
pub fn join_under(root: &Path, relative: &[&str]) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative {
        path.push(part);
    }
    path
}

/// 把整棵目录树拍平成 `相对路径 -> 文件内容` 的有序列表。
///
/// 规格 T02 的验收要求「测试前后比较临时目录文件树与内容，必须完全不变」，
/// 这个函数就是那个比较的基准。
pub fn snapshot_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    collect(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(reader) = fs::read_dir(dir) else {
        return;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            collect(root, &path, out);
        } else if meta.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("路径应位于根之下")
                .to_string_lossy()
                .into_owned();
            let bytes = fs::read(&path).unwrap_or_default();
            out.push((relative, bytes));
        }
    }
}
