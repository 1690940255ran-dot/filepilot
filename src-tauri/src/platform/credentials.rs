//! Windows 凭据存储（规格 3.3、5.2）。
//!
//! 规格原文：「模型 API Key 放入 Windows 凭据存储，配置只存 `credentialRef`。
//! 前端保存后不再读回明文；日志和崩溃报告不得包含密钥、文档正文或完整请求体。」
//!
//! ## 这个模块守的是什么
//!
//! 密钥一旦进入 SQLite 或日志，就等于进入了**会被备份、导出、截图、
//! 贴进 issue** 的地方。所以这里的边界不是「尽量别写进去」，而是
//! 「写进去这件事在类型上就做不到」：
//!
//! * [`store`] 是唯一把密钥交给系统的入口，它只接收一个引用名；
//! * [`read`] 是唯一取回明文的入口，返回 [`Secret`]——**它没有 `Display`，
//!   `Debug` 也只输出 `***`**，于是「不小心把它打进日志」需要显式写
//!   `.expose()`，而那是一次看得见的动作；
//! * 数据库里那一列叫 `credentialRef`，存的是引用，不是密钥。
//!
//! ## 为什么用系统凭据存储而不是自己加密
//!
//! 自己做「加密后存进 SQLite」需要一个密钥，而那个密钥又得存在某处——
//! 这是把问题挪一层，不是解决它。Windows 凭据存储由系统按登录会话保管，
//! 并且是用户**已经在用**的那套东西（他可以在「凭据管理器」里看见和删除），
//! 不存在「应用藏了一个只有它认识的口袋」。

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

use crate::domain::errors::{codes, AppError};

/// 所有 FilePilot 凭据的目标名都带这个前缀。
///
/// 凭据存储是**按目标名全局共享**的一个命名空间：不加前缀的话，
/// 一个叫 `default` 的引用会和别的程序同名条目撞在一起。
const TARGET_PREFIX: &str = "FilePilot/";

/// 用户名那一栏。凭据管理器用它做展示，不参与查找。
const USER_NAME: &str = "FilePilot";

/// `CRED_MAX_CREDENTIAL_BLOB_SIZE` 的实际值（5 × 512）。
///
/// 超过它 `CredWriteW` 会返回 `ERROR_INVALID_PARAMETER`，而那个错误码
/// 说明不了「是密钥太长了」。在这个界面上先自己挡住，好给一句准确的话。
const MAX_SECRET_BYTES: usize = 2560;

/// 一段明文密钥。
///
/// **刻意不实现 `Display`**，`Debug` 也只输出 `***`：这两个 trait 正是
/// 格式化宏与日志宏会去用的，堵住它们，「密钥进了日志」就从
/// 「一次疏忽」变成了「一次显式调用 [`Secret::expose`]」。
pub struct Secret(String);

impl Secret {
    /// 从一段明文构造。空串会被拒绝——「保存了一个空密钥」与
    /// 「没有配置密钥」在界面上必须分得开。
    pub fn new(value: impl Into<String>) -> Result<Self, AppError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AppError::new(
                codes::CREDENTIAL_UNAVAILABLE,
                "密钥不能为空。若要清除已保存的密钥，请删除这条提供商配置。",
            ));
        }
        if value.len() > MAX_SECRET_BYTES {
            return Err(AppError::new(
                codes::CREDENTIAL_UNAVAILABLE,
                format!(
                    "密钥过长（{} 字节，上限 {MAX_SECRET_BYTES}）。",
                    value.len()
                ),
            ));
        }
        Ok(Self(value))
    }

    /// 取出明文。
    ///
    /// 名字取得直白是**有意的**：调用点上出现 `expose()` 就是在提醒
    /// 读代码的人「这里有一段明文正在被使用，它不该被写进任何持久化位置」。
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不写长度、不写前几位：那些也是可被利用的信息。
        formatter.write_str("Secret(***)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // 先把字节擦掉再让 `String` 归还内存。
        //
        // 这挡不住「已经被复制到别处的副本」，但能挡住最常见的一种残留：
        // 这段密钥留在**已归还的堆块**里，被同进程的下一次分配读到。
        // `write_volatile` 是为了防止优化器把「写了马上就 free」整个删掉。
        let bytes = unsafe { self.0.as_bytes_mut() };
        for byte in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

/// 把引用名变成凭据存储用的目标名。
///
/// 校验三件事，都是「会静默出错」的类型：
///
/// * **空引用**：会建出一个别人也能撞上的条目；
/// * **含 NUL**：`CredWriteW` 按 NUL 结尾读，名字被**悄悄截断**，
///   于是写入的条目与后来查找的条目不是同一个；
/// * **过长**：超出 `CRED_MAX_GENERIC_TARGET_NAME_LENGTH`。
fn target_name(reference: &str) -> Result<Vec<u16>, AppError> {
    if reference.is_empty() {
        return Err(AppError::new(
            codes::CREDENTIAL_UNAVAILABLE,
            "凭据引用不能为空。",
        ));
    }
    if reference.contains('\0') {
        return Err(AppError::new(
            codes::CREDENTIAL_UNAVAILABLE,
            "凭据引用不能包含空字符：它会让系统按截断后的名字存取。",
        ));
    }

    let full = format!("{TARGET_PREFIX}{reference}");
    let wide: Vec<u16> = full.encode_utf16().chain(std::iter::once(0)).collect();
    // 去掉末尾那个 NUL 再判长度：`CRED_MAX_GENERIC_TARGET_NAME_LENGTH` 数的是字符。
    if wide.len() - 1 > 32_767 {
        return Err(AppError::new(
            codes::CREDENTIAL_UNAVAILABLE,
            "凭据引用过长。",
        ));
    }
    Ok(wide)
}

/// 把错误码翻成一句用户看得懂的话。
///
/// **不能笼统地报「保存失败」**：凭据服务被禁用、权限不足、密钥超长，
/// 三者的下一步动作完全不同。
fn map_error(error: &windows::core::Error, action: &str) -> AppError {
    let detail = format!("{error}（0x{:08X}）", error.code().0 as u32);
    AppError::new(
        codes::CREDENTIAL_UNAVAILABLE,
        format!("无法{action} Windows 凭据存储中的密钥：{detail}。请确认「凭据管理器」服务可用。"),
    )
}

/// 写入（或覆盖）一条密钥。返回后调用方只应保存 `reference`。
pub fn store(reference: &str, secret: &Secret) -> Result<(), AppError> {
    let mut target = target_name(reference)?;
    let mut user: Vec<u16> = USER_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    // 密钥按 UTF-8 存字节。凭据存储的 blob 是**字节**，不是宽字符——
    // 按 `encode_utf16` 存会让读回来时多一层猜测。
    let mut blob = secret.expose().as_bytes().to_vec();

    let credential = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: windows::core::PWSTR(target.as_mut_ptr()),
        Comment: windows::core::PWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: windows::core::PWSTR::null(),
        UserName: windows::core::PWSTR(user.as_mut_ptr()),
    };

    // 同一个目标名再写一次是**覆盖**，不是报错——用户改密钥走的就是这条路。
    unsafe { CredWriteW(&credential, 0) }.map_err(|error| map_error(&error, "写入"))?;

    // 让明文副本尽早离开这个函数。`blob` 在这里被擦掉，
    // 而不是等它被 drop 时留下一段归还的堆内存。
    for byte in blob.iter_mut() {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    let _ = &mut target;
    Ok(())
}

/// 读取一条密钥。**只在发送请求时调用**。
///
/// 返回 `None` 表示这条引用没有对应的密钥（用户没配，或已经删掉），
/// 那不是错误。
pub fn read(reference: &str) -> Result<Option<Secret>, AppError> {
    let mut target = target_name(reference)?;
    let mut handle: *mut CREDENTIALW = std::ptr::null_mut();

    match unsafe {
        CredReadW(
            PCWSTR(target.as_mut_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut handle,
        )
    } {
        Ok(()) => {}
        Err(error) if error.code() == ERROR_NOT_FOUND.into() => return Ok(None),
        Err(error) => return Err(map_error(&error, "读取")),
    }

    // `CredReadW` 给的内存必须用 `CredFree` 还。用一个守卫保证
    // 后面无论从哪条路返回都会还——这是 FFI 里最容易漏的一处。
    struct CredentialGuard(*mut CREDENTIALW);
    impl Drop for CredentialGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CredFree(self.0 as *const c_void) };
            }
        }
    }
    let _guard = CredentialGuard(handle);

    let blob = unsafe {
        let credential = &*handle;
        if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
            return Ok(None);
        }
        std::slice::from_raw_parts(
            credential.CredentialBlob,
            credential.CredentialBlobSize as usize,
        )
    };

    // 存进去的是 UTF-8 字节；读不出来说明这条条目不是我们写的
    // （或者被别的程序改过），如实报错而不是替换字符凑一份。
    let text = std::str::from_utf8(blob).map_err(|_| {
        AppError::new(
            codes::CREDENTIAL_UNAVAILABLE,
            "凭据存储中的密钥不是有效的 UTF-8，无法使用。请在设置里重新保存。",
        )
    })?;

    Secret::new(text).map(Some)
}

/// 删除一条密钥。返回 `true` 表示确实删掉了一条，`false` 表示本来就没有。
pub fn delete(reference: &str) -> Result<bool, AppError> {
    let mut target = target_name(reference)?;
    match unsafe { CredDeleteW(PCWSTR(target.as_mut_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(true),
        Err(error) if error.code() == ERROR_NOT_FOUND.into() => Ok(false),
        Err(error) => Err(map_error(&error, "删除")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的凭据引用。
    ///
    /// 这些用例会**真的往本机凭据管理器里写条目**——这是验证
    /// 「密钥确实进了系统凭据存储」唯一诚实的办法。代价是必须保证
    /// 不留下垃圾，所以用守卫在 `Drop` 里删；断言失败时 `Drop` 照样跑。
    struct TestCredential(String);

    impl TestCredential {
        fn new(label: &str) -> Self {
            Self(format!("test-{label}-{}", uuid::Uuid::new_v4()))
        }

        fn reference(&self) -> &str {
            &self.0
        }
    }

    impl Drop for TestCredential {
        fn drop(&mut self) {
            let _ = delete(&self.0);
        }
    }

    #[test]
    fn a_stored_secret_can_be_read_back_byte_for_byte() {
        let guard = TestCredential::new("roundtrip");
        let secret = Secret::new("sk-测试-ключ-🔑").expect("构造密钥");

        store(guard.reference(), &secret).expect("写入应成功");

        let read_back = read(guard.reference())
            .expect("读取不该失败")
            .expect("刚刚写进去的应当读得回来");
        assert_eq!(
            read_back.expose(),
            "sk-测试-ключ-🔑",
            "非 ASCII 密钥必须原样取回"
        );
    }

    #[test]
    fn an_absent_reference_reads_as_none_not_an_error() {
        // 「没配密钥」是正常状态（本地 Ollama 就不需要），把它做成错误
        // 会逼前端用错误码白名单去区分「没配」与「读坏了」。
        let guard = TestCredential::new("absent");
        // 用 `is_none()` 而不是 `assert_eq!(.., None)`：`Secret` **刻意不实现
        // `PartialEq`**——比较两个密钥是否正确应当由服务端认证去回答，
        // 应用里出现「密钥相等比较」通常意味着有人在写校验逻辑，
        // 而那多半是个坏主意（常量时间、失败计数、日志泄露都会跟着来）。
        assert!(read(guard.reference()).expect("读取不该失败").is_none());
    }

    #[test]
    fn deleting_an_absent_reference_reports_false() {
        let guard = TestCredential::new("delete-twice");
        let secret = Secret::new("x").expect("构造密钥");
        store(guard.reference(), &secret).expect("写入应成功");

        assert!(delete(guard.reference()).expect("删除不该失败"));
        assert!(
            !delete(guard.reference()).expect("再删一次不该失败"),
            "第二次删同一个引用必须如实报「本来就没有」"
        );
    }

    #[test]
    fn storing_again_replaces_the_previous_secret() {
        // 用户改密钥走的就是这条路：同一个引用再写一次。
        let guard = TestCredential::new("replace");
        store(guard.reference(), &Secret::new("旧密钥").expect("构造")).expect("写入应成功");
        store(guard.reference(), &Secret::new("新密钥").expect("构造")).expect("覆盖应成功");

        let read_back = read(guard.reference()).expect("读取").expect("应存在");
        assert_eq!(read_back.expose(), "新密钥");
    }

    #[test]
    fn a_deleted_secret_is_really_gone() {
        let guard = TestCredential::new("gone");
        store(guard.reference(), &Secret::new("待删除").expect("构造")).expect("写入应成功");
        delete(guard.reference()).expect("删除应成功");

        assert!(
            read(guard.reference()).expect("读取不该失败").is_none(),
            "删掉之后必须读不到——否则「删除密钥」是个骗人的按钮"
        );
    }

    #[test]
    fn a_reference_with_a_nul_byte_is_refused() {
        // 系统按 NUL 结尾读名字，带 NUL 的引用会被**悄悄截断**：
        // 存进去的和查出来的不是同一个条目，而两边都不报错。
        let error = store("bad\0name", &Secret::new("x").expect("构造"))
            .expect_err("含 NUL 的引用必须被拒绝");
        assert_eq!(error.code, codes::CREDENTIAL_UNAVAILABLE);
        assert!(error.message.contains("空字符"), "{}", error.message);
    }

    #[test]
    fn an_empty_reference_is_refused() {
        assert!(store("", &Secret::new("x").expect("构造")).is_err());
        assert!(read("").is_err());
        assert!(delete("").is_err());
    }

    #[test]
    fn an_empty_secret_is_refused() {
        // 「保存了一个空密钥」与「没有配置密钥」必须分得开：
        // 前者会让提供商带着空 Authorization 去撞 401。
        let error = Secret::new("").expect_err("空密钥必须被拒绝");
        assert_eq!(error.code, codes::CREDENTIAL_UNAVAILABLE);
    }

    #[test]
    fn an_over_long_secret_is_refused_with_a_clear_message() {
        // 超过上限时系统只回一个含混的「参数错误」，所以我们先自己挡。
        let too_long = "a".repeat(MAX_SECRET_BYTES + 1);
        let error = Secret::new(too_long).expect_err("超长密钥必须被拒绝");
        assert!(error.message.contains("过长"), "{}", error.message);
    }

    #[test]
    fn the_longest_allowed_secret_is_accepted() {
        // 边界另一侧也要钉住：正好等于上限应当可以，否则上限就写错了。
        let at_limit = "a".repeat(MAX_SECRET_BYTES);
        assert!(Secret::new(at_limit).is_ok());
    }

    #[test]
    fn a_secret_never_prints_itself() {
        let secret = Secret::new("sk-绝密-value").expect("构造");
        let printed = format!("{secret:?}");
        assert_eq!(printed, "Secret(***)");
        assert!(!printed.contains("绝密"), "Debug 不得泄露内容");
        // 不写长度：长度本身也是可被利用的信息。
        assert!(!printed.contains("12"));
    }

    #[test]
    fn the_target_name_is_namespaced() {
        // 凭据存储是按目标名全局共享的命名空间；不加前缀的话，
        // 一个叫 `default` 的引用会和别的程序撞名。
        let wide = target_name("abc").expect("构造目标名");
        let text = String::from_utf16(&wide[..wide.len() - 1]).expect("转回字符串");
        assert_eq!(text, "FilePilot/abc");
        assert_eq!(wide[wide.len() - 1], 0, "必须以 NUL 结尾");
    }
}
