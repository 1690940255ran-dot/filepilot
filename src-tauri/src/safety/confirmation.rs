//! 一次性确认令牌（规格 7.4、T05）。
//!
//! 用户点「确认执行」时，后端签发一个令牌；执行器只认这个令牌。
//! 它把「用户看过并同意的那份计划」与「即将执行的那份计划」绑在一起：
//! 令牌里带了 `planId` + `revision` + `digest`，任何一项对不上都拒绝。
//!
//! ## 三条不可退让的性质
//!
//! 1. **只存哈希**：内存里保存的是 `SHA-256(token)`，不保存明文。
//!    即使有人读到进程内存，也拿不到可用的令牌。
//! 2. **一次性**：消费即作废。重复提交同一个令牌必须失败——
//!    否则「双击确认」会执行两次。
//! 3. **会过期**：默认 5 分钟。确认的是「此刻磁盘上的状态」，
//!    放太久就失去意义了。
//!
//! ## 为什么不用 `uuid::Uuid::new_v4()` 直接当令牌
//!
//! v4 UUID 只有 122 位随机性且格式可预测，而这里需要的是**不可猜测**。
//! 用 32 字节 CSPRNG 输出再做十六进制编码。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::domain::{errors::codes, errors::AppError};

/// 令牌有效期。规格 7.4：最多 5 分钟。
pub const TOKEN_TTL_MS: i64 = 5 * 60 * 1000;

/// 生成 32 字节随机数并编码成十六进制。
///
/// 用 `getrandom` 而不是自造随机源：它直接对接操作系统的 CSPRNG。
pub(crate) fn random_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        // 拿不到安全随机数时**必须失败**，绝不能退化成时间戳或计数器——
        // 那会产出可预测的令牌，等于没有令牌。
        AppError::internal(format!("无法获取安全随机数: {error}"))
    })?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// 令牌的哈希。存这个，不存明文。
pub(crate) fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 当前 Unix 毫秒时间戳。
pub fn now_unix_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        // 系统时间早于 1970 只可能是时钟被改坏了；返回 0 会让所有令牌立即过期，
        // 这是安全的失败方向。
        Err(_) => 0,
    }
}

/// 一条已签发的确认。
#[derive(Debug, Clone)]
struct Issued {
    plan_id: String,
    revision: u32,
    digest: String,
    expires_at_ms: i64,
    consumed: bool,
}

/// 令牌仓库。
///
/// 规格 8.1：运行期状态可以放内存。令牌本来就不该跨进程存活——
/// 应用重启后用户必须重新确认，这是期望行为而不是缺陷。
#[derive(Default)]
pub struct ConfirmationStore {
    issued: Mutex<HashMap<String, Issued>>,
}

/// 签发结果。`token` 是明文，**只在这一次返回**，后端不保留。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedConfirmation {
    pub token: String,
    /// UTC RFC3339，用于回填 `ValidationReport.expiresAt`。
    pub expires_at: String,
}

impl ConfirmationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 为一份已校验的计划签发令牌。
    ///
    /// 只有 `validate_plan` 通过（`executable_count > 0` 且无阻断项）才应该调用它；
    /// 这个前置条件由调用方负责，本函数不重复判断——它只保证令牌本身的性质。
    pub fn issue(
        &self,
        plan_id: &str,
        revision: u32,
        digest: &str,
        now_ms: i64,
    ) -> Result<IssuedConfirmation, AppError> {
        let token = random_token()?;
        let expires_at_ms = now_ms.saturating_add(TOKEN_TTL_MS);

        self.lock().insert(
            hash_token(&token),
            Issued {
                plan_id: plan_id.to_owned(),
                revision,
                digest: digest.to_owned(),
                expires_at_ms,
                consumed: false,
            },
        );

        Ok(IssuedConfirmation {
            token,
            expires_at: crate::domain::time::rfc3339_from_unix_ms(expires_at_ms),
        })
    }

    /// 校验并**消费**一个令牌。
    ///
    /// 三类拒绝，各有独立错误码，因为用户能做的事不同：
    /// - 已过期 → `TOKEN_EXPIRED`（重新预览即可，不用改计划）
    /// - 已被消费 → `TOKEN_USED`（说明重复提交了）
    /// - 不存在 / 计划版本变了 / 摘要对不上 → `STALE_PLAN`（计划已不是用户看过的那个）
    ///
    /// 消费是**先检查后置位**的原子操作（整个函数持锁），
    /// 因此两个并发请求里只有一个能成功。
    pub fn consume(
        &self,
        token: &str,
        plan_id: &str,
        revision: u32,
        digest: &str,
        now_ms: i64,
    ) -> Result<(), AppError> {
        let key = hash_token(token);
        let mut issued = self.lock();

        let Some(entry) = issued.get_mut(&key) else {
            return Err(stale("确认已失效，请重新预览并确认"));
        };

        if entry.consumed {
            // 重用：这是最需要明确拒绝的一种。若放行，双击就会执行两次。
            return Err(AppError::new(
                codes::TOKEN_USED,
                "该确认已被使用过，不能重复执行",
            ));
        }
        if now_ms >= entry.expires_at_ms {
            return Err(AppError::new(
                codes::TOKEN_EXPIRED,
                "确认已超过 5 分钟有效期，请重新预览并确认",
            ));
        }
        if entry.plan_id != plan_id || entry.revision != revision {
            return Err(stale("计划在此确认之后被修改过，请重新预览"));
        }
        if entry.digest != digest {
            return Err(stale("计划内容与确认时不一致，请重新预览"));
        }

        entry.consumed = true;
        Ok(())
    }

    /// 清掉已过期或已消费的记录，避免长时间运行后无限增长。
    pub fn prune(&self, now_ms: i64) -> usize {
        let mut issued = self.lock();
        let before = issued.len();
        issued.retain(|_, entry| !entry.consumed && now_ms < entry.expires_at_ms);
        before - issued.len()
    }

    /// 当前未消费且未过期的令牌数。仅供测试与诊断。
    pub fn outstanding(&self, now_ms: i64) -> usize {
        self.lock()
            .values()
            .filter(|entry| !entry.consumed && now_ms < entry.expires_at_ms)
            .count()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Issued>> {
        // 与 app_state 同样容忍中毒：表里都是独立字段，没有跨字段不变量
        self.issued
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn stale(message: &str) -> AppError {
    AppError::new(codes::STALE_PLAN, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_token() -> (ConfirmationStore, IssuedConfirmation) {
        let store = ConfirmationStore::new();
        let issued = store
            .issue("plan-1", 2, "digest-abc", 1_000)
            .expect("签发应成功");
        (store, issued)
    }

    #[test]
    fn a_fresh_token_is_accepted_once() {
        let (store, issued) = store_with_token();
        store
            .consume(&issued.token, "plan-1", 2, "digest-abc", 1_500)
            .expect("首次消费应成功");
        assert_eq!(store.outstanding(1_500), 0, "消费后不应再是未使用状态");
    }

    #[test]
    fn replaying_a_consumed_token_is_rejected() {
        let (store, issued) = store_with_token();
        store
            .consume(&issued.token, "plan-1", 2, "digest-abc", 1_500)
            .expect("首次应成功");

        let err = store
            .consume(&issued.token, "plan-1", 2, "digest-abc", 1_600)
            .expect_err("重放必须被拒绝");
        assert_eq!(
            err.code,
            codes::TOKEN_USED,
            "重用必须报 TOKEN_USED 而不是笼统的 STALE_PLAN"
        );
        assert!(
            err.message.contains("重复"),
            "理由要说明是重复使用，实际：{}",
            err.message
        );
    }

    #[test]
    fn expiry_boundary_is_exclusive() {
        let (store, issued) = store_with_token();
        let expires = 1_000 + TOKEN_TTL_MS;

        // 恰好到期的那一刻就必须失效：`>=` 而不是 `>`
        let err = store
            .consume(&issued.token, "plan-1", 2, "digest-abc", expires)
            .expect_err("恰好到期应被拒绝");
        assert_eq!(err.code, codes::TOKEN_EXPIRED);

        // 差 1 毫秒仍然有效
        let (store2, issued2) = store_with_token();
        store2
            .consume(&issued2.token, "plan-1", 2, "digest-abc", expires - 1)
            .expect("未到期应通过");
    }

    #[test]
    fn changing_the_plan_revision_invalidates_the_token() {
        let (store, issued) = store_with_token();
        let err = store
            .consume(&issued.token, "plan-1", 3, "digest-abc", 1_500)
            .expect_err("版本变了必须拒绝");
        assert_eq!(err.code, codes::STALE_PLAN);
    }

    #[test]
    fn changing_the_digest_invalidates_the_token() {
        let (store, issued) = store_with_token();
        let err = store
            .consume(&issued.token, "plan-1", 2, "digest-CHANGED", 1_500)
            .expect_err("摘要不符必须拒绝");
        assert_eq!(err.code, codes::STALE_PLAN);
        assert!(err.message.contains("内容"));
    }

    #[test]
    fn a_forged_token_is_rejected() {
        let (store, _issued) = store_with_token();
        for forged in ["", "not-a-token", &"0".repeat(64), &"f".repeat(64)] {
            let err = store
                .consume(forged, "plan-1", 2, "digest-abc", 1_500)
                .expect_err("伪造令牌必须拒绝");
            assert_eq!(err.code, codes::STALE_PLAN);
        }
    }

    #[test]
    fn tokens_are_unpredictable_and_not_stored_in_plaintext() {
        let store = ConfirmationStore::new();
        let a = store.issue("p", 1, "d", 0).expect("签发");
        let b = store.issue("p", 1, "d", 0).expect("签发");

        assert_ne!(a.token, b.token, "两次签发的令牌必须不同");
        assert_eq!(a.token.len(), 64, "32 字节的十六进制编码");

        // 仓库的键是哈希，不是明文
        let keys: Vec<String> = store.lock().keys().cloned().collect();
        assert!(
            !keys.iter().any(|key| key == &a.token),
            "仓库里不得出现令牌明文"
        );
        assert!(keys.contains(&hash_token(&a.token)), "键应当是令牌哈希");
    }

    #[test]
    fn consumed_tokens_do_not_block_other_outstanding_ones() {
        let store = ConfirmationStore::new();
        let first = store.issue("p", 1, "d", 0).expect("签发");
        let second = store.issue("p", 1, "d", 0).expect("签发");

        store
            .consume(&first.token, "p", 1, "d", 10)
            .expect("第一次");
        assert_eq!(store.outstanding(10), 1, "另一个令牌仍然有效");
        store
            .consume(&second.token, "p", 1, "d", 20)
            .expect("第二次");
        assert_eq!(store.outstanding(20), 0);
    }

    #[test]
    fn prune_removes_expired_and_consumed_entries() {
        let store = ConfirmationStore::new();
        let used = store.issue("p", 1, "d", 0).expect("签发");
        let _fresh = store.issue("p", 1, "d", 0).expect("签发");
        store.consume(&used.token, "p", 1, "d", 10).expect("消费");

        assert_eq!(store.prune(10), 1, "只应清掉已消费的那条");
        assert_eq!(store.prune(1_000_000), 1, "过期的那条也会被清掉");
        assert_eq!(store.prune(1_000_000), 0, "再清一次不应有变化");
    }

    #[test]
    fn expires_at_is_a_valid_utc_rfc3339_instant() {
        let store = ConfirmationStore::new();

        // 锚点用外部算出的准确值（Python: datetime.strptime(...).timestamp()），
        // 而不是手写一个「看起来差不多」的数字——那样测的只是自己的算术。
        let issued = store
            .issue("p", 1, "d", 1_789_603_200_000) // 2026-09-17T00:00:00Z
            .expect("签发");
        // domain::time::to_rfc3339_utc 带毫秒（规格 5.1 的时间格式统一如此）
        assert_eq!(issued.expires_at, "2026-09-17T00:05:00.000Z");
        assert!(
            issued.expires_at.ends_with('Z'),
            "必须是 UTC 时刻而不是本地时间"
        );
    }

    #[test]
    fn rfc3339_conversion_handles_the_epoch_and_known_instants() {
        assert_eq!(
            crate::domain::time::rfc3339_from_unix_ms(0),
            "1970-01-01T00:00:00.000Z"
        );
        assert_eq!(
            crate::domain::time::rfc3339_from_unix_ms(1_000),
            "1970-01-01T00:00:01.000Z"
        );
        // 1_789_646_400_000 = 2026-09-17T12:00:00Z
        assert_eq!(
            crate::domain::time::rfc3339_from_unix_ms(1_789_646_400_000),
            "2026-09-17T12:00:00.000Z"
        );
        // 1_709_164_800_000 = 2024-02-29T00:00:00Z —— 闰日，日期算术最容易错的地方
        assert_eq!(
            crate::domain::time::rfc3339_from_unix_ms(1_709_164_800_000),
            "2024-02-29T00:00:00.000Z"
        );
        // 紧接着的一天必须是 3 月 1 日而不是 2 月 30 日
        assert_eq!(
            crate::domain::time::rfc3339_from_unix_ms(1_709_251_200_000),
            "2024-03-01T00:00:00.000Z"
        );
    }
}
