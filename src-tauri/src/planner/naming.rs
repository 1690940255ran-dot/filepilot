//! 目标名称分配与冲突消解（规格 7.1 第 6、7、9、10 条）。
//!
//! 这个模块只做一件事：**给定「想放到哪个目录、想叫什么名字」，回答「实际可以用哪个名字」**。
//! 它不碰文件、不读内容，只做两件事实核对：
//!
//! 1. 磁盘上那个目录里现在有哪些名字；
//! 2. 本批计划里已经保留了哪些目标名。
//!
//! ## 三条容易被做错的规则
//!
//! - **批内源路径视为占用**（规格 7.1 第 9 条）：不能靠执行顺序假定
//!   「A 先搬走，位置就空出来了」。因此 `a.txt → b.txt`、`b.txt → a.txt`
//!   这种互换不会按用户想象的方式执行，而是各自分配一个新名字。
//!   v0.1 明确不支持循环交换，这里做的是**降级**而不是装作支持。
//! - **大小写不敏感比较**（规格 7.1 第 5 条）：Windows 默认把 `A.txt` 与 `a.txt`
//!   视为同一个名字，因此冲突判断必须按该语义做。
//! - **不静默截断**（规格 7.1 第 2 条）：加了 ` (2)` 之后超长就报错，
//!   不能截断成一个可能与他人碰撞的新名字。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::errors::{codes, AppError};
use crate::domain::types::RelPath;
use crate::platform::windows;
use crate::safety::path::{validate_component, MAX_COMPONENT_UTF16, MAX_TOTAL_PATH_UTF16};
use crate::safety::root::ApprovedRoot;

/// 冲突候选的尝试上限。
///
/// 规格 7.1 第 7 条要求稳定分配 `名称 (2).扩展名`。理论上一个目录里可能有
/// 上千个同名项，但真到了那个量级，说明用户的问题不是「叫第几个」，
/// 而是「这个目录不该被这样整理」。宁可报错也不做无界循环。
pub const MAX_ALLOCATION_ATTEMPTS: u32 = 1000;

/// 组件之间的比较分隔符：单元分隔符 U+001F。
///
/// 规格 7.1 第 3 条禁止组件包含控制字符，所以它不可能出现在任何合法组件里，
/// 拼出来的比较键因此不会出现「A\B」与「A」+「B」混淆这类歧义。
const KEY_SEPARATOR: char = '\u{1f}';

/// 单个名字的比较键：小写化，并剥离 Windows 会忽略的尾随空格与点。
///
/// 尾随空格与点必须剥掉：`CreateFileW("a.txt.")` 打开的是 `a.txt`，
/// 只做小写化会让「磁盘上已有 a.txt」这件事在比较时漏掉。
///
/// 大小写折叠用的是 Rust 的 `to_lowercase`，与 Windows 的 upcase 表在极少数
/// 字符上（如 `İ`）不完全一致。**偏差方向是安全的**：把「其实不同」的两个名字
/// 当成相同，只会多分配一个 ` (2)`；真正的「绝不覆盖」由内核的
/// `ReplaceIfExists = FALSE` 保证，不依赖这里的字符串比较。
pub fn normalize_name(name: &str) -> String {
    name.trim_end_matches([' ', '.']).to_lowercase()
}

/// 整条相对路径的比较键。
pub fn comparison_key(components: &[String]) -> String {
    components
        .iter()
        .map(|component| normalize_name(component))
        .collect::<Vec<_>>()
        .join(&KEY_SEPARATOR.to_string())
}

/// 一个相对路径当前在磁盘上的形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// 不存在。执行阶段会创建目录，因此这**不是**错误。
    Missing,
    Directory,
    /// 存在，但是普通文件。
    File,
    /// 重解析点（符号链接、目录联接、云占位符）。
    ReparsePoint,
    /// 打不开（权限等）。
    Unreadable,
}

/// 观察一个相对路径当前在磁盘上的形态。
///
/// 用 `symlink_metadata` 而不是 `metadata`：后者会跟随链接，
/// 于是「目标父目录是一个指向别处的联接」会被看成普通目录。
pub fn observe(root: &ApprovedRoot, relative: &[String]) -> Result<Observed, AppError> {
    let path = root.resolve_within(relative)?;

    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Observed::Missing),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Ok(Observed::Unreadable)
        }
        Err(error) => Err(AppError::new(
            codes::PERMISSION_DENIED,
            format!("无法读取路径属性: {error}"),
        )),
        Ok(meta) => {
            if windows::is_reparse_point(&path).map_err(|error| {
                AppError::new(
                    codes::PERMISSION_DENIED,
                    format!("无法确认路径是否为重解析点: {error}"),
                )
            })? {
                return Ok(Observed::ReparsePoint);
            }
            if meta.is_dir() {
                Ok(Observed::Directory)
            } else if meta.is_file() {
                Ok(Observed::File)
            } else {
                Ok(Observed::Unreadable)
            }
        }
    }
}

/// 检查目标父目录链：任一段存在但不是目录，这项就不能执行。
///
/// 规格 7.1 第 10 条点名的三种阻断情形里，这里覆盖两种
/// （目录名与现有文件同名、任意路径祖先是普通文件）；第三种
/// 「不能写入目标父目录」由 `Observed::Unreadable` 表达。
///
/// 链上**缺失**的段不是错误：执行阶段会逐级创建目录（规格 8.2）。
pub fn check_target_parent_chain(root: &ApprovedRoot, dir: &[String]) -> Result<(), AppError> {
    for depth in 1..=dir.len() {
        let prefix = &dir[..depth];
        let shown = prefix.join("\\");
        match observe(root, prefix)? {
            Observed::Directory | Observed::Missing => {}
            Observed::File => {
                return Err(AppError::new(
                    codes::TARGET_PARENT_IS_FILE,
                    format!("路径 {shown} 上已经有一个同名文件，不能作为目录"),
                ))
            }
            Observed::ReparsePoint => {
                return Err(AppError::new(
                    codes::REPARSE_POINT,
                    format!("路径 {shown} 是一个链接（重解析点），v0.1 不支持穿过它整理"),
                ))
            }
            Observed::Unreadable => {
                return Err(AppError::new(
                    codes::PERMISSION_DENIED,
                    format!("无法确认路径 {shown} 是否可写"),
                ))
            }
        }
    }
    Ok(())
}

/// 一个目录在磁盘上的现有条目。
///
/// 目录不存在时 `entries` 为空集合——这不是错误，执行阶段会创建它。
#[derive(Debug)]
struct DirFacts {
    /// 现有条目的比较键（`normalize_name` 之后）。
    entries: HashSet<String>,
}

/// 目标名占用台账。
///
/// 同时承担两个职责：缓存目录列举（避免每个文件都重新读一次目录），
/// 以及记录本批已保留的目标名。它**不修改任何文件**。
pub struct TargetLedger<'a> {
    root: &'a ApprovedRoot,
    /// 冲突候选的尝试上限。生产固定为 [`MAX_ALLOCATION_ATTEMPTS`]。
    max_attempts: u32,
    /// 本批已分配出去的目标路径比较键。
    reserved: HashSet<String>,
    /// 本批所有源路径的比较键（规格 7.1 第 9 条：视为占用）。
    batch_sources: HashSet<String>,
    /// 目录列举缓存。键是相对目录的比较键。
    ///
    /// 用 `Arc` 而不是直接放值：`allocate` 需要在持有目录快照的同时
    /// 往 `reserved` 里写。返回引用会让借用检查器不允许那次写入，
    /// 而每次分配都克隆一份上千条的目录列表又太贵。
    dirs: HashMap<String, Arc<DirFacts>>,
    /// 已观察过的路径形态缓存，键同 `dirs`。
    observed: HashMap<String, Observed>,
}

impl<'a> TargetLedger<'a> {
    pub fn new(root: &'a ApprovedRoot) -> Self {
        Self::with_attempt_limit(root, MAX_ALLOCATION_ATTEMPTS)
    }

    /// 指定冲突候选的尝试上限。
    ///
    /// 生产路径一律走 [`TargetLedger::new`]。这个入口存在的唯一理由是
    /// **测试「候选被占满」这条路径**：默认上限是 1000，端到端复现它需要真的
    /// 造 1000 个文件，在带文件过滤驱动的机器上要几十秒——而这条路径要验证的是
    /// 「循环有界且报错明确」，与上限的具体数值无关。
    pub fn with_attempt_limit(root: &'a ApprovedRoot, attempts: u32) -> Self {
        Self {
            root,
            max_attempts: attempts.max(1),
            reserved: HashSet::new(),
            batch_sources: HashSet::new(),
            dirs: HashMap::new(),
            observed: HashMap::new(),
        }
    }

    /// 登记本批的一个源路径。分配目标时它会被视为已占用。
    pub fn reserve_source(&mut self, relative: &[String]) {
        self.batch_sources.insert(comparison_key(relative));
    }

    /// 目标父目录链是否可执行（同 [`check_target_parent_chain`]，走缓存）。
    pub fn ensure_parent_chain(&mut self, dir: &[String]) -> Result<(), AppError> {
        for depth in 1..=dir.len() {
            let prefix = &dir[..depth];
            match self.observe_cached(prefix)? {
                Observed::Directory | Observed::Missing => {}
                Observed::File => {
                    return Err(AppError::new(
                        codes::TARGET_PARENT_IS_FILE,
                        format!(
                            "路径 {} 上已经有一个同名文件，不能作为目录",
                            prefix.join("\\")
                        ),
                    ))
                }
                Observed::ReparsePoint => {
                    return Err(AppError::new(
                        codes::REPARSE_POINT,
                        format!(
                            "路径 {} 是一个链接（重解析点），v0.1 不支持穿过它整理",
                            prefix.join("\\")
                        ),
                    ))
                }
                Observed::Unreadable => {
                    return Err(AppError::new(
                        codes::PERMISSION_DENIED,
                        format!("无法确认路径 {} 是否可写", prefix.join("\\")),
                    ))
                }
            }
        }
        Ok(())
    }

    /// 在 `dir` 下为 `stem + extension` 分配一个不被占用的目标。
    ///
    /// `own_source` 是这一项自己的源路径：它允许出现在目标位置上
    /// （那是「目标与当前位置相同」的情形，由调用方先判成 noop），
    /// 但**其他**项的源路径一律视为占用。
    pub fn allocate(
        &mut self,
        dir: &[String],
        stem: &str,
        extension: &str,
        own_source: &[String],
    ) -> Result<RelPath, AppError> {
        self.ensure_parent_chain(dir)?;

        let own_key = comparison_key(own_source);
        let existing = self.dir_entries(dir)?;

        // 目录部分的路径长度只算一次；每个候选只需要再加上文件名。
        let base = self.root.resolve_within(dir)?;
        use std::os::windows::ffi::OsStrExt as _;
        let base_units = base.as_os_str().encode_wide().count();

        for attempt in 0..self.max_attempts {
            let file_name = candidate_name(stem, extension, attempt)?;

            // 完整目标路径的上限（规格 7.1 第 2 条）按**整条路径**算，
            // 只算相对部分会低估。放在这里而不是调用方：超限时不该占用目标名。
            let units = base_units + 1 + file_name.encode_utf16().count();
            if units > MAX_TOTAL_PATH_UTF16 {
                return Err(AppError::new(
                    codes::PATH_TOO_LONG,
                    format!(
                        "目标完整路径将达到 {units} 个 UTF-16 字符单位，\
                         超过 v0.1 的 {MAX_TOTAL_PATH_UTF16} 上限"
                    ),
                ));
            }

            let mut target = dir.to_vec();
            target.push(file_name.clone());

            let key = comparison_key(&target);
            if self.reserved.contains(&key) {
                continue;
            }
            if self.batch_sources.contains(&key) && key != own_key {
                // 规格 7.1 第 9 条：别人现在占着这个名字，执行顺序不能当作前提。
                continue;
            }
            if existing.entries.contains(&normalize_name(&file_name)) {
                continue;
            }

            self.reserved.insert(key);
            return Ok(target);
        }

        Err(AppError::new(
            codes::TARGET_EXISTS,
            format!(
                "在 {}{} 下尝试了 {attempts} 个候选名称仍未找到可用目标，请先整理该目录",
                self.root.display(),
                if dir.is_empty() {
                    String::new()
                } else {
                    format!("\\{}", dir.join("\\"))
                },
                attempts = self.max_attempts
            ),
        ))
    }

    /// 检查一个完整目标路径当前是否已被占用（磁盘）。
    ///
    /// 与 `allocate` 的区别：这里不分配替代名，只回答「现在还在不在」。
    /// 校验阶段用它报 `TARGET_EXISTS`——规格 7.1 第 8 条：
    /// 最终确认后出现的新冲突要**报错并停止**，不自动另取名字。
    pub fn target_exists_on_disk(&mut self, target: &[String]) -> Result<bool, AppError> {
        let Some((file_name, dir)) = target.split_last() else {
            return Err(AppError::new(codes::INVALID_PATH, "目标路径不能为空"));
        };
        if self.observe_cached(target)? != Observed::Missing {
            return Ok(true);
        }
        let entries = self.dir_entries(dir)?;
        Ok(entries.entries.contains(&normalize_name(file_name)))
    }

    fn observe_cached(&mut self, relative: &[String]) -> Result<Observed, AppError> {
        let key = comparison_key(relative);
        if let Some(state) = self.observed.get(&key) {
            return Ok(*state);
        }
        let state = observe(self.root, relative)?;
        self.observed.insert(key, state);
        Ok(state)
    }

    /// 取一个目录的现有条目；目录不存在时返回空集合（执行阶段会创建它）。
    ///
    /// 注意用的是**原始组件**而不是比较键去读盘：比较键经过小写化与剥离
    /// 尾随点，拿它拼路径会读到另一个位置（在区分大小写的目录上尤其危险）。
    fn dir_entries(&mut self, dir: &[String]) -> Result<Arc<DirFacts>, AppError> {
        let key = comparison_key(dir);
        if let Some(facts) = self.dirs.get(&key) {
            return Ok(Arc::clone(facts));
        }

        let usable = matches!(self.observe_cached(dir)?, Observed::Directory);
        let mut entries = HashSet::new();
        if usable {
            let path: PathBuf = self.root.resolve_within(dir)?;
            let reader = std::fs::read_dir(&path).map_err(|error| {
                AppError::new(
                    codes::PERMISSION_DENIED,
                    format!("无法读取目标目录的现有内容: {error}"),
                )
            })?;
            for entry in reader {
                let entry = entry.map_err(|error| {
                    AppError::new(
                        codes::PERMISSION_DENIED,
                        format!("目标目录枚举不完整，无法安全分配名称: {error}"),
                    )
                })?;
                entries.insert(normalize_name(&entry.file_name().to_string_lossy()));
            }
        }

        let facts = Arc::new(DirFacts { entries });
        self.dirs.insert(key, Arc::clone(&facts));
        Ok(facts)
    }
}

/// 第 `attempt` 个候选文件名。
///
/// 0 → `名称.扩展名`；1 → `名称 (2).扩展名`；2 → `名称 (3).扩展名`……
/// 与规格 7.1 第 7 条给出的形式逐字一致。
fn candidate_name(stem: &str, extension: &str, attempt: u32) -> Result<String, AppError> {
    if attempt == 0 {
        return validate_component(&format!("{stem}{extension}"));
    }

    let numbered = format!("{stem} ({}){extension}", attempt + 1);
    match validate_component(&numbered) {
        Ok(name) => Ok(name),
        Err(error) if error.code == codes::PATH_TOO_LONG => Err(AppError::new(
            codes::PATH_TOO_LONG,
            format!(
                "为避开同名冲突需要用到 {numbered:?}，但名称超过 {MAX_COMPONENT_UTF16} 个 UTF-16 字符单位；\
                 请先缩短原名或整理该目录（不会静默截断成可能再次碰撞的名字）"
            ),
        )),
        Err(error) => Err(error),
    }
}

/// 完整目标路径（含批准根）的 UTF-16 长度。
///
/// 规格 7.1 第 2 条：v0.1 保守限制 240 个 UTF-16 字符单位，且按**完整路径**算。
/// T03 已确认只算相对部分会低估，于是「预览里合法」的路径在执行时超限。
pub fn total_path_units(root: &ApprovedRoot, target: &[String]) -> Result<usize, AppError> {
    let path = root.resolve_within(target)?;
    use std::os::windows::ffi::OsStrExt as _;
    Ok(path.as_os_str().encode_wide().count())
}

/// 完整目标路径是否在长度上限之内。
pub fn check_total_path_length(root: &ApprovedRoot, target: &[String]) -> Result<(), AppError> {
    let units = total_path_units(root, target)?;
    if units > MAX_TOTAL_PATH_UTF16 {
        return Err(AppError::new(
            codes::PATH_TOO_LONG,
            format!(
                "目标完整路径为 {units} 个 UTF-16 字符单位，超过 v0.1 的 {MAX_TOTAL_PATH_UTF16} 上限"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_strips_what_windows_ignores() {
        assert_eq!(normalize_name("A.TXT"), "a.txt");
        assert_eq!(
            normalize_name("a.txt. "),
            "a.txt",
            "Windows 会剥离尾随的点与空格，比较时必须先剥掉"
        );
        assert_eq!(normalize_name("报告 "), "报告");
    }

    #[test]
    fn comparison_keys_cannot_collide_across_component_boundaries() {
        let a = comparison_key(&["学习".to_owned(), "数学".to_owned()]);
        let b = comparison_key(&["学习数学".to_owned()]);
        assert_ne!(a, b, "组件边界必须体现在比较键里");
    }

    #[test]
    fn candidate_names_follow_the_spec_form() {
        assert_eq!(candidate_name("报告", ".pdf", 0).unwrap(), "报告.pdf");
        assert_eq!(candidate_name("报告", ".pdf", 1).unwrap(), "报告 (2).pdf");
        assert_eq!(candidate_name("报告", ".pdf", 2).unwrap(), "报告 (3).pdf");
        // 无扩展名文件保持无扩展名
        assert_eq!(candidate_name("LICENSE", "", 1).unwrap(), "LICENSE (2)");
    }

    #[test]
    fn candidate_names_never_get_silently_truncated() {
        let stem = "中".repeat(80);
        assert_eq!(
            candidate_name(&stem, ".txt", 0).unwrap_err().code,
            codes::PATH_TOO_LONG
        );
        // 加后缀后超长必须报错，而不是截断
        let stem = "中".repeat(78);
        assert_eq!(
            candidate_name(&stem, ".txt", 1).unwrap_err().code,
            codes::PATH_TOO_LONG
        );
    }

    #[test]
    fn candidate_names_reject_illegal_components() {
        assert_eq!(
            candidate_name("a/b", ".txt", 0).unwrap_err().code,
            codes::INVALID_PATH
        );
        assert_eq!(
            candidate_name("CON", ".txt", 0).unwrap_err().code,
            codes::RESERVED_NAME
        );
    }
}
