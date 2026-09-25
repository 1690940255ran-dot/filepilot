//! 模型请求的 HTTP 传输层（规格 6.4）。
//!
//! 规格对「往外发请求」这件事有四条硬要求，每一条在下面都对应一个
//! **显式设置**，而不是「指望某个库的默认值恰好是对的」：
//!
//! | 规格要求 | 落点 |
//! |---|---|
//! | 本地端点只允许 loopback；代理环境变量不得把请求转发到外网 | [`Endpoint::parse_local`] + `WINHTTP_ACCESS_TYPE_NO_PROXY` |
//! | 兼容云只接受用户设置的 HTTPS | [`Endpoint::parse_cloud`] |
//! | 禁止跟随跨域重定向携带密钥 | `WINHTTP_OPTION_REDIRECT_POLICY_NEVER` |
//! | 响应体上限 1 MiB | [`MAX_RESPONSE_BYTES`]，自己数读到的字节 |
//!
//! ## 为什么不用第三方 HTTP 客户端
//!
//! 上面四条都是**安全边界**。用第三方库就要逐条去查「它的默认值是什么、
//! 哪个开关能改」，而每查错一条都是一次真实的密钥外泄或请求走错地方。
//! WinHTTP 是系统组件，这四条各自有一个直白的 API 设置，且不引入任何
//! 新的依赖树——发出去的载荷里有用户的文件名与文本摘要，这条链路上
//! 能少一个 crate 就少一个。
//!
//! ## 取消的边界（如实写明）
//!
//! [`Cancellation`] 在**发请求前**、**每次重试前**、以及**每读到一个数据块
//! 之后**被检查。服务端迟迟不吐数据时，取消要等到本次请求超时才会生效——
//! 因为 `WinHttpReadData` 是阻塞的，而在这里强杀 socket 需要跨线程关闭句柄，
//! 那会引入「句柄已关、另一个线程还在用」的竞态，代价比它买到的东西大。
//!
//! 需要「立刻停」的调用方应当把 [`post_json`] 放进一个工作线程，
//! 用通道带超时地等结果。

use std::ffi::c_void;
use std::net::IpAddr;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Networking::WinHttp::{
    WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetOption, WinHttpSetTimeouts, ERROR_WINHTTP_NAME_NOT_RESOLVED,
    ERROR_WINHTTP_SECURE_FAILURE, ERROR_WINHTTP_TIMEOUT, WINHTTP_ACCESS_TYPE_NO_PROXY,
    WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_ADDREQ_FLAG_REPLACE, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

use crate::domain::errors::{codes, AppError};

/// 规格 6.4：响应体上限 1 MiB。
///
/// 一份「文件分类建议」的 JSON 不可能接近这个量级；超过它要么是服务端
/// 出了问题，要么是端点被换成了别的东西。两种都不该让内存跟着涨。
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// 规格 6.4：60 秒超时。
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// 一次读取的缓冲区。16 KiB 是个折中：小到取消能及时被检查到，
/// 大到不用为一次普通响应做上百次系统调用。
const READ_CHUNK_BYTES: usize = 16 * 1024;

/// 能否取消。
///
/// 与 `extractors::service::ExtractObserver` 是同一套思路：调用方实现
/// 「该不该停」，传输层在每个能检查的边界上问一次。
pub trait Cancellation {
    fn should_stop(&self) -> bool;
}

/// 永不取消。用于一次性请求（例如「测试连通性」）。
pub struct NeverCancel;

impl Cancellation for NeverCancel {
    fn should_stop(&self) -> bool {
        false
    }
}

/// 一个已经过校验的连接目标。
///
/// **`host` 一定是一个 IP 字面量**：本地端点里的 `localhost` 在解析时
/// 就被换成了 `127.0.0.1`，别的域名一律拒绝。于是「连到哪儿」不经过
/// 任何名字解析，也就没有「hosts 文件把 localhost 指到别处」这种可能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub secure: bool,
}

impl Endpoint {
    /// 解析一个**本地**提供商端点（规格 6.4：本地模式只允许 loopback）。
    pub fn parse_local(url: &str) -> Result<Self, AppError> {
        let endpoint = Self::parse(url)?;
        let address: IpAddr = endpoint.host.parse().map_err(|_| {
            AppError::new(
                codes::INVALID_ENDPOINT,
                format!(
                    "本地提供商端点必须是 IP 地址或 localhost，收到 {:?}。\
                     本地模式不解析域名：一个能被 hosts 文件改写的名字，\
                     意味着请求可以被指到外网去。",
                    endpoint.host
                ),
            )
        })?;
        if !address.is_loopback() {
            return Err(AppError::new(
                codes::INVALID_ENDPOINT,
                format!(
                    "本地提供商端点只允许回环地址（127.0.0.1 或 ::1），\
                     收到 {}。要连远程服务请把它配置成兼容云提供商。",
                    endpoint.host
                ),
            ));
        }
        Ok(endpoint)
    }

    /// 解析一个**兼容云**提供商端点（规格 6.4：只接受用户设置的 HTTPS）。
    pub fn parse_cloud(url: &str) -> Result<Self, AppError> {
        let endpoint = Self::parse(url)?;
        if !endpoint.secure {
            return Err(AppError::new(
                codes::INVALID_ENDPOINT,
                "云端提供商端点必须使用 https://：明文 HTTP 会把 API Key \
                 和文件摘要直接暴露在链路上。",
            ));
        }
        Ok(endpoint)
    }

    /// 拆出 scheme / host / port / path。
    ///
    /// ## 为什么自己拆，而不是用通用 URL 解析器
    ///
    /// 通用解析器（无论哪一家的）都把 `http://127.0.0.1@evil.com/` 当成
    /// **合法** URL——它的主机是 `evil.com`，而人眼很容易把它读成
    /// `127.0.0.1`。对「本地端点只允许 loopback」这条检查来说，
    /// 解析器的宽容就是检查的漏洞。
    ///
    /// 所以这里反过来：**只认识少数几种形状，其余一律拒绝**。
    /// 每一种被接受的形状都能被完全理解，不存在「解析歧义」的余地。
    fn parse(url: &str) -> Result<Self, AppError> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(invalid("端点不能为空。"));
        }

        let (scheme, rest) = trimmed
            .split_once("://")
            .ok_or_else(|| invalid(format!("端点缺少 http:// 或 https:// 前缀：{trimmed:?}")))?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            other => {
                return Err(invalid(format!(
                    "不支持的协议 {other:?}：只支持 http 与 https。"
                )))
            }
        };

        // 查询串与片段一律拒绝。我们不使用它们，而「支持但不理解」的东西
        // 正是各种绕过手法喜欢待的地方。
        if rest.contains('?') || rest.contains('#') {
            return Err(invalid(
                "端点不能包含 ? 或 #：模型接口的所有参数都在请求体里。",
            ));
        }

        let (authority, path) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, "/"),
        };
        if authority.is_empty() {
            return Err(invalid("端点缺少主机名。"));
        }
        // `user:pass@host` 这种写法是我们最容易读错的一种，直接拒绝。
        if authority.contains('@') {
            return Err(invalid(
                "端点不能包含 @（用户名或密码部分）：这种写法常被用来\
                 把真正的主机名伪装成 @ 前面的那一段。",
            ));
        }

        let (host, port_text) = split_authority(authority)?;
        if host.is_empty() {
            return Err(invalid("端点缺少主机名。"));
        }

        let port = match port_text {
            Some(text) => text
                .parse::<u16>()
                .map_err(|_| invalid(format!("端口 {text:?} 不是 0–65535 之间的整数。")))?,
            None if secure => 443,
            None => 80,
        };
        if port == 0 {
            return Err(invalid("端口不能是 0。"));
        }

        // `localhost` 就地换成 IP 字面量：名字会被系统解析，而解析结果
        // 可以被 hosts 文件改到任意地址。本地请求不该有这种可能性。
        let host = if host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_owned()
        } else {
            host
        };

        Ok(Self {
            host,
            port,
            path: path.to_owned(),
            secure,
        })
    }

    /// 完整 URL。只用于错误信息与探测模式回显，**不参与连接**。
    pub fn display_url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}:{}{}", self.host, self.port, self.path)
    }
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(codes::INVALID_ENDPOINT, message)
}

/// 从 `authority` 里拆出主机与端口。
///
/// IPv6 字面量必须写成 `[::1]:11434`——这是 URL 的规矩，也是唯一能让
/// 「哪个冒号是端口分隔符」无歧义的形式。没有中括号的 IPv6 会被当成
/// 「主机里带冒号」，随后在 IP 解析那一关被拒。
fn split_authority(authority: &str) -> Result<(String, Option<&str>), AppError> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (inside, after) = rest
            .split_once(']')
            .ok_or_else(|| invalid("IPv6 地址缺少右中括号。"))?;
        return match after.is_empty() {
            true => Ok((inside.to_owned(), None)),
            false => {
                let port = after
                    .strip_prefix(':')
                    .ok_or_else(|| invalid("IPv6 地址的右中括号后只能是 :端口。"))?;
                Ok((inside.to_owned(), Some(port)))
            }
        };
    }

    match authority.rsplit_once(':') {
        Some((host, port)) => {
            // 没有中括号却出现多个冒号：形状不合法，交给 IP 解析拒绝。
            if host.contains(':') {
                return Ok((authority.to_owned(), None));
            }
            Ok((host.to_owned(), Some(port)))
        }
        None => Ok((authority.to_owned(), None)),
    }
}

/// 一次 HTTP 响应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// 发一个 JSON POST，读回整个响应体。
///
/// `headers` 里的每一项都会被原样加上；`Content-Type: application/json`
/// 由本函数负责，调用方不用重复。
pub fn post_json(
    endpoint: &Endpoint,
    body: &str,
    headers: &[(&str, &str)],
    timeout: Duration,
    cancellation: &dyn Cancellation,
) -> Result<HttpResponse, AppError> {
    if cancellation.should_stop() {
        return Err(cancelled_error());
    }

    // ---- 会话：显式不用代理 ----
    //
    // `WINHTTP_ACCESS_TYPE_NO_PROXY` 不读系统代理设置（IE/WinINet 那份），
    // 也不读 HTTP_PROXY 之类的环境变量。规格 6.4 要求「代理环境变量不得
    // 偷偷把请求转发到外网」——在这一行之后，那不是「我们检查过了」，
    // 而是「这条路径不存在」。
    let agent = wide("FilePilot/0.1");
    let session = unsafe {
        WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    };
    let session = OwnedHandle::new(session, "无法初始化 HTTP 会话")?;

    // ---- 连接 ----
    let host = wide(&endpoint.host);
    let connection =
        unsafe { WinHttpConnect(session.raw(), PCWSTR(host.as_ptr()), endpoint.port, 0) };
    let connection = OwnedHandle::new(connection, "无法建立 HTTP 连接")?;

    // ---- 请求 ----
    let verb = wide("POST");
    let path = wide(&endpoint.path);
    let flags = match endpoint.secure {
        true => WINHTTP_FLAG_SECURE,
        false => Default::default(),
    };
    let request = unsafe {
        WinHttpOpenRequest(
            connection.raw(),
            PCWSTR(verb.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        )
    };
    let request = OwnedHandle::new(request, "无法创建 HTTP 请求")?;

    // ---- 四项设置：每一项都对应规格里的一句话 ----

    let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    unsafe {
        WinHttpSetTimeouts(
            request.raw(),
            timeout_ms,
            timeout_ms,
            timeout_ms,
            timeout_ms,
        )
    }
    .map_err(|error| transport_error(&error, "无法设置请求超时"))?;

    // 规格 6.4：「禁止跟随跨域重定向携带密钥」。
    //
    // 直接**完全不跟随**重定向。这样「密钥跟着 3xx 跑到别的域名」在实现上
    // 不可能发生——不需要去论证「重定向时 WinHTTP 会不会带上 Authorization」，
    // 因为压根没有第二次请求。3xx 会被当成错误如实报出去。
    let policy: u32 = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
    unsafe {
        WinHttpSetOption(
            Some(request.raw() as *const c_void),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&policy.to_ne_bytes()),
        )
    }
    .map_err(|error| transport_error(&error, "无法禁止 HTTP 重定向"))?;

    add_header(
        request.raw(),
        "Content-Type: application/json; charset=utf-8",
    )?;
    for (name, value) in headers {
        add_header(request.raw(), &format!("{name}: {value}"))?;
    }

    // ---- 发送与接收 ----
    unsafe {
        WinHttpSendRequest(
            request.raw(),
            None,
            Some(body.as_ptr() as *const c_void),
            body.len() as u32,
            body.len() as u32,
            0,
        )
    }
    .map_err(|error| transport_error(&error, "无法发送请求"))?;

    unsafe { WinHttpReceiveResponse(request.raw(), std::ptr::null_mut()) }
        .map_err(|error| transport_error(&error, "无法读取响应"))?;

    let status = status_code(request.raw())?;
    let body = read_body(request.raw(), cancellation)?;

    // 3xx：因为我们不跟随重定向，它到这里就是一个**明确的失败**，
    // 而不是一个需要再去别处取的结果。如实告诉用户端点给了一个跳转，
    // 由他去确认那个地址是不是他要的。
    if (300..400).contains(&status) {
        return Err(AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            format!(
                "端点返回了重定向（HTTP {status}）。出于安全考虑不跟随重定向\
                 ——请把端点直接配置成最终地址。"
            ),
        ));
    }

    Ok(HttpResponse { status, body })
}

/// 一个必须被关闭的 WinHTTP 句柄。
///
/// WinHTTP 的句柄是分层的（会话 → 连接 → 请求），任何一层漏关都会让
/// 连接一直挂到进程退出。用 Drop 管住，就不必在每个 `return` 前面
/// 记得写一次关闭——那种写法漏一个分支就是一处泄漏。
struct OwnedHandle(*mut c_void);

impl OwnedHandle {
    fn new(handle: *mut c_void, context: &str) -> Result<Self, AppError> {
        if handle.is_null() {
            // 句柄分配失败的原因在 `GetLastError` 里，而 `windows` crate
            // 只在 `Result` 形式的 API 上替我们取它。这几个返回裸指针的
            // API 拿不到细节，所以给一句方向明确的提示。
            return Err(AppError::new(
                codes::MODEL_TIMEOUT,
                format!("{context}：系统拒绝了这次 HTTP 操作（多半是网络或句柄资源问题）。"),
            ));
        }
        Ok(Self(handle))
    }

    fn raw(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // 关不掉的句柄没有补救手段，而在这里 panic 会把一个正常的
            // 错误路径变成崩溃。忽略返回值。
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn add_header(request: *mut c_void, header: &str) -> Result<(), AppError> {
    let wide = wide(header);
    // 传不含 NUL 的切片：`WinHttpAddRequestHeaders` 另有长度参数（由
    // `windows` crate 从切片长度填），带上 NUL 会把长度算多一个。
    let text = &wide[..wide.len() - 1];
    unsafe {
        WinHttpAddRequestHeaders(
            request,
            text,
            WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE,
        )
    }
    .map_err(|error| transport_error(&error, "无法设置请求头"))
}

fn status_code(request: *mut c_void) -> Result<u16, AppError> {
    let mut status: u32 = 0;
    let mut length = std::mem::size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut length,
            std::ptr::null_mut(),
        )
    }
    .map_err(|error| transport_error(&error, "无法读取响应状态码"))?;

    u16::try_from(status).map_err(|_| {
        AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            format!("服务端返回了非法的状态码 {status}。"),
        )
    })
}

/// 读完响应体，并在每一步检查上限与取消。
fn read_body(request: *mut c_void, cancellation: &dyn Cancellation) -> Result<String, AppError> {
    let mut body: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; READ_CHUNK_BYTES];

    loop {
        if cancellation.should_stop() {
            return Err(cancelled_error());
        }

        let mut read: u32 = 0;
        unsafe {
            WinHttpReadData(
                request,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() as u32,
                &mut read,
            )
        }
        .map_err(|error| transport_error(&error, "读取响应体失败"))?;

        // 0 字节表示响应结束。这是 WinHTTP 约定，不是错误。
        if read == 0 {
            break;
        }

        // **先判上限再追加**：反过来会让一个恶意/故障服务端把内存顶到
        // 上限之上才被拒绝（`extend_from_slice` 会真的分配那么多）。
        if body.len() + read as usize > MAX_RESPONSE_BYTES {
            return Err(AppError::new(
                codes::MODEL_INVALID_OUTPUT,
                format!(
                    "响应体超过 {} MiB 上限，已停止读取。这份响应不可能是\
                     正常的建议数组。",
                    MAX_RESPONSE_BYTES / (1024 * 1024)
                ),
            ));
        }
        body.extend_from_slice(&buffer[..read as usize]);
    }

    String::from_utf8(body).map_err(|_| {
        AppError::new(
            codes::MODEL_INVALID_OUTPUT,
            "响应体不是有效的 UTF-8，无法解析。",
        )
    })
}

fn cancelled_error() -> AppError {
    AppError::new(codes::MODEL_TIMEOUT, "请求已取消。")
}

/// 从 `windows` crate 的错误里取回原始 Win32 错误码。
///
/// `windows` 把 Win32 错误包装成 `HRESULT_FROM_WIN32(code)`，即
/// `0x8007_0000 | code`，所以低 16 位就是原始码。WinHTTP 的错误码
/// 都在这个范围内。
fn raw_win32_code(error: &windows::core::Error) -> u32 {
    error.code().0 as u32 & 0xFFFF
}

/// 把传输层错误翻成用户能据以行动的话。
///
/// 规格 8.5 只给了 `MODEL_AUTH` / `MODEL_TIMEOUT` / `MODEL_INVALID_OUTPUT`
/// 三个模型侧的码，所以这里要做的是**选对**，而不是全都塞进一个：
/// 名字解析不了、TLS 失败、连不上，用户要做的事完全不同。
fn transport_error(error: &windows::core::Error, context: &str) -> AppError {
    let raw = raw_win32_code(error);
    let (code, hint) = match raw {
        ERROR_WINHTTP_TIMEOUT => (
            codes::MODEL_TIMEOUT,
            "请求超时。本地模型可能在加载，或服务未在监听。",
        ),
        ERROR_WINHTTP_NAME_NOT_RESOLVED => (
            codes::INVALID_ENDPOINT,
            "无法解析端点主机名，请检查地址是否写对、网络是否可用。",
        ),
        ERROR_WINHTTP_SECURE_FAILURE => (
            codes::INVALID_ENDPOINT,
            "TLS 握手失败。请检查端点证书是否有效——应用不会忽略证书错误。",
        ),
        _ => (
            codes::MODEL_TIMEOUT,
            "无法连接到端点。请确认服务已启动、端口正确、且没有被防火墙拦下。",
        ),
    };
    AppError::new(code, format!("{context}：{hint}（WinHTTP 错误 {raw}）"))
}

/// Rust 字符串 -> NUL 结尾的 UTF-16。
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 端点校验：这是本模块最重要的部分 ----

    #[test]
    fn a_local_endpoint_accepts_the_ollama_default() {
        let endpoint = Endpoint::parse_local("http://127.0.0.1:11434").expect("默认地址应被接受");
        assert_eq!(endpoint.host, "127.0.0.1");
        assert_eq!(endpoint.port, 11434);
        assert_eq!(endpoint.path, "/");
        assert!(!endpoint.secure, "本地 Ollama 默认是明文 HTTP");
    }

    #[test]
    fn localhost_is_rewritten_to_an_ip_literal() {
        // 这条是**构造性**的：本地端点里不留下任何需要解析的名字，
        // 于是「hosts 文件把 localhost 指到外网」这条路不存在。
        let endpoint = Endpoint::parse_local("http://localhost:11434").expect("应被接受");
        assert_eq!(endpoint.host, "127.0.0.1");
    }

    #[test]
    fn localhost_is_matched_case_insensitively() {
        let endpoint = Endpoint::parse_local("http://LocalHost:11434").expect("应被接受");
        assert_eq!(endpoint.host, "127.0.0.1");
    }

    #[test]
    fn a_local_endpoint_rejects_a_remote_address() {
        // 规格 6.4：「本地模式只允许 loopback 地址」。
        let error =
            Endpoint::parse_local("http://192.168.1.10:11434").expect_err("非回环地址必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
        assert!(error.message.contains("回环"), "{}", error.message);
    }

    #[test]
    fn a_local_endpoint_rejects_a_domain_name() {
        // 域名要靠系统解析，而解析结果可以被 hosts 文件改到任意地址。
        let error = Endpoint::parse_local("http://api.example.com").expect_err("域名必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
        assert!(error.message.contains("IP 地址"), "{}", error.message);
    }

    #[test]
    fn the_userinfo_trick_is_refused_outright() {
        // 这是手写解析最该防的一种输入：`127.0.0.1` 出现在前面，
        // 而真正的主机是 `evil.com`。通用解析器会（正确地）把它解析成
        // evil.com，而人眼很容易读反。
        let error =
            Endpoint::parse_local("http://127.0.0.1@evil.com/").expect_err("含 @ 的端点必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
        assert!(error.message.contains('@'), "{}", error.message);
    }

    #[test]
    fn a_cloud_endpoint_must_be_https() {
        // 规格 6.4：「兼容云端点只接受用户设置的 HTTPS」。
        let error =
            Endpoint::parse_cloud("http://api.example.com/v1").expect_err("明文 HTTP 必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
        assert!(error.message.contains("https"), "{}", error.message);
    }

    #[test]
    fn a_cloud_endpoint_accepts_https_with_a_path() {
        let endpoint =
            Endpoint::parse_cloud("https://api.example.com/v1/chat/completions").expect("应被接受");
        assert_eq!(endpoint.host, "api.example.com");
        assert_eq!(endpoint.port, 443, "没写端口时 https 默认 443");
        assert_eq!(endpoint.path, "/v1/chat/completions");
        assert!(endpoint.secure);
    }

    #[test]
    fn a_cloud_endpoint_accepts_an_explicit_port() {
        let endpoint = Endpoint::parse_cloud("https://api.example.com:8443/v1").expect("应被接受");
        assert_eq!(endpoint.port, 8443);
    }

    #[test]
    fn an_unknown_scheme_is_refused() {
        for hostile in ["ftp://127.0.0.1/", "file:///C:/x", "127.0.0.1:11434"] {
            assert!(
                Endpoint::parse_local(hostile).is_err(),
                "{hostile} 必须被拒绝"
            );
        }
    }

    #[test]
    fn query_and_fragment_are_refused() {
        // 我们不使用它们，而「支持但不理解」的东西正是绕过手法喜欢待的地方。
        assert!(Endpoint::parse_local("http://127.0.0.1:11434/?a=1").is_err());
        assert!(Endpoint::parse_local("http://127.0.0.1:11434/#x").is_err());
    }

    #[test]
    fn an_empty_or_prefix_less_endpoint_is_refused() {
        assert!(Endpoint::parse_local("").is_err());
        assert!(Endpoint::parse_local("   ").is_err());
        assert!(Endpoint::parse_local("127.0.0.1:11434").is_err());
    }

    #[test]
    fn a_bad_port_is_refused() {
        for hostile in [
            "http://127.0.0.1:0",
            "http://127.0.0.1:70000",
            "http://127.0.0.1:abc",
        ] {
            assert!(
                Endpoint::parse_local(hostile).is_err(),
                "{hostile} 必须被拒绝"
            );
        }
    }

    #[test]
    fn an_ipv6_loopback_is_accepted() {
        // IPv6 必须写成 `[::1]:port`——这是唯一能让「哪个冒号是端口分隔符」
        // 无歧义的形式。
        let endpoint = Endpoint::parse_local("http://[::1]:11434").expect("IPv6 回环应被接受");
        assert_eq!(endpoint.host, "::1");
        assert_eq!(endpoint.port, 11434);
    }

    #[test]
    fn an_ipv6_without_brackets_is_refused() {
        assert!(Endpoint::parse_local("http://::1:11434").is_err());
    }

    #[test]
    fn a_non_loopback_ipv6_is_refused_for_local() {
        let error = Endpoint::parse_local("http://[2001:db8::1]:11434")
            .expect_err("非回环 IPv6 必须被拒绝");
        assert_eq!(error.code, codes::INVALID_ENDPOINT);
    }

    #[test]
    fn the_scheme_is_matched_case_insensitively() {
        let endpoint = Endpoint::parse_cloud("HTTPS://api.example.com/v1").expect("应被接受");
        assert!(endpoint.secure);
    }

    #[test]
    fn a_path_without_a_leading_segment_becomes_root() {
        let endpoint = Endpoint::parse_local("http://127.0.0.1:11434").expect("应被接受");
        assert_eq!(endpoint.path, "/");
    }

    #[test]
    fn the_display_url_round_trips_the_parts() {
        let endpoint = Endpoint::parse_cloud("https://api.example.com/v1/chat").expect("应被接受");
        assert_eq!(
            endpoint.display_url(),
            "https://api.example.com:443/v1/chat"
        );
    }

    #[test]
    fn a_host_with_a_double_colon_separator_is_refused() {
        // `127.0.0.1::11434` 这种形状交给 IP 解析拒绝，
        // 而不是被拆成 host=`127.0.0.1:` + port=11434。
        assert!(Endpoint::parse_local("http://127.0.0.1::11434").is_err());
    }

    #[test]
    fn hostile_endpoint_shapes_are_all_refused() {
        // 一批「看起来像回环、实际不是」的写法。
        //
        // 它们能全被拒，靠的是一条**构造性**的性质：本地端点的 host 必须能被
        // `IpAddr::from_str` 接受，而那个解析是严格的——只认标准点分十进制与
        // 标准 IPv6。它不认识 `2130706433`（十进制整数形式的 127.0.0.1）、
        // `0x7f.0.0.1`（十六进制）或 `0177.0.0.1`（八进制），而**别的语言与
        // 别的库是认识这些的**。
        //
        // 这就是「用某个库解析 URL」与「按这里的规则校验」的区别所在：
        // 前者会把 `http://2130706433` 解析成一个合法的回环地址。
        let hostile = [
            ("http://2130706433", "十进制整数形式的回环地址"),
            ("http://0x7f.0.0.1", "十六进制的回环地址"),
            ("http://0177.0.0.1", "八进制的回环地址"),
            ("http://127.1", "缩写形式"),
            ("http://127.0.0.1.attacker.com", "前缀伪装"),
            ("http://127.0.0.1.evil.com", "前缀伪装"),
            ("http://localhost.evil.com", "前缀伪装"),
            ("http://127.0.0.1%00.evil.com", "百分号编码截断"),
            ("http://evil.com#@127.0.0.1", "片段里藏真实地址"),
            ("http://evil.com?x=127.0.0.1", "查询串里藏真实地址"),
            ("http://127.0.0.1\\@evil.com", "反斜杠混淆"),
            ("http://127.0.0.1\t:11434", "主机名里夹制表符"),
            ("http://127.0.0.1 :11434", "主机名里夹空格"),
            ("http://[::ffff:127.0.0.1]:11434", "IPv4 映射地址"),
        ];

        for (input, reason) in hostile {
            assert!(
                Endpoint::parse_local(input).is_err(),
                "{input}（{reason}）必须被拒绝，但它通过了"
            );
        }
    }

    #[test]
    fn the_strict_ip_parser_is_what_makes_those_refusals_work() {
        // 上一条测试的前提是「Rust 的 IP 解析是严格的」。这条把那个前提
        // 单独钉住：它一旦变化（比如换了解析方式），上面的拒绝就会失效，
        // 而那时这里会先红。
        for strict in ["2130706433", "0x7f.0.0.1", "0177.0.0.1", "127.1"] {
            assert!(
                strict.parse::<IpAddr>().is_err(),
                "{strict} 不应当被解析成一个 IP 地址"
            );
        }
        // 反过来：标准写法要能解析，否则本地端点就没法配了。
        for accepted in ["127.0.0.1", "127.0.0.2", "::1"] {
            assert!(accepted.parse::<IpAddr>().is_ok(), "{accepted} 应当可解析");
        }
    }

    // ---- 取消 ----

    #[test]
    fn a_cancelled_request_never_reaches_the_network() {
        struct AlreadyStopped;
        impl Cancellation for AlreadyStopped {
            fn should_stop(&self) -> bool {
                true
            }
        }

        // 用一个**不存在的端口**：如果实现没有在发请求前检查取消，
        // 这次调用会返回连接错误而不是取消错误，测试就会失败。
        let endpoint = Endpoint::parse_local("http://127.0.0.1:1").expect("端点应被接受");
        let error = post_json(&endpoint, "{}", &[], REQUEST_TIMEOUT, &AlreadyStopped)
            .expect_err("已取消的请求必须直接返回");
        assert!(error.message.contains("取消"), "{}", error.message);
    }

    #[test]
    fn never_cancel_never_stops() {
        assert!(!NeverCancel.should_stop());
    }
}
