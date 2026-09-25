//! 本地假 HTTP 服务，供 `tests/ai_contract.rs` 使用。
//!
//! 规格 T12 的验收要求「本地假 HTTP 服务模拟成功、超时、401、429、5xx、
//! 超长响应、跨域跳转及非法 JSON」，并且「常规 CI 不依赖真实付费模型」。
//! 这个文件就是那句话的落点。
//!
//! ## 为什么不引一个 mock 服务器 crate
//!
//! 需要的行为只有五件事：读一个请求、按脚本回一个响应、记录收到了什么、
//! 能挂住不回、能立刻断开。用 `TcpListener` 手写不到 200 行，而每多一个
//! 依赖就多一份「它的默认行为是什么」要查。
//!
//! ## 它**不是**一个通用 HTTP 服务器
//!
//! 只认 `Content-Length`，不支持 chunked、不支持 keep-alive、不解析查询串。
//! 这是刻意的：测试要模拟的是「模型端点的这几种反应」，
//! 而不是去证明我们能实现一个 HTTP 服务器。

#![allow(dead_code)] // 各测试文件只会用到其中一部分

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 一次收到的请求。
#[derive(Debug, Clone)]
pub struct ReceivedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl ReceivedRequest {
    /// 按名字取请求头（大小写不敏感，HTTP 头本来就不区分大小写）。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// 服务端要给出的回应。
#[derive(Debug, Clone)]
pub enum Reply {
    /// 正常回一个响应。
    Response {
        status: u16,
        content_type: String,
        body: Vec<u8>,
    },
    /// 302 到别处。
    ///
    /// 用来验证「禁止跟随跨域重定向携带密钥」：如果客户端跟随了，
    /// 它会把请求（带着 Authorization）发到 `Location` 指向的地址去。
    Redirect { location: String },
    /// 读完请求后**什么都不回**，把连接挂住。
    ///
    /// 用来验证超时。注意它会让客户端一直等到自己的超时，
    /// 所以用它的测试要传一个**短**的超时值，别等 60 秒。
    Hang,
    /// 读完请求后立刻断开，不给任何响应。
    Abrupt,
}

impl Reply {
    /// 200 + 一段 JSON。
    pub fn json(body: impl Into<String>) -> Self {
        Self::Response {
            status: 200,
            content_type: "application/json".to_owned(),
            body: body.into().into_bytes(),
        }
    }

    /// 指定状态码 + 一段 JSON。
    pub fn status_json(status: u16, body: impl Into<String>) -> Self {
        Self::Response {
            status,
            content_type: "application/json".to_owned(),
            body: body.into().into_bytes(),
        }
    }

    /// 200，但内容不是 JSON。
    ///
    /// 「服务端宣称保证 JSON」不等于客户端可以跳过校验——这条用例
    /// 就是拿一个 HTML 错误页当成 200 的响应体。
    pub fn not_json(body: impl Into<String>) -> Self {
        Self::Response {
            status: 200,
            content_type: "text/html".to_owned(),
            body: body.into().into_bytes(),
        }
    }

    /// 一个远超 1 MiB 的 200 响应。
    ///
    /// 规格 6.4 要求响应体上限 1 MiB；超过它必须被**拒绝**，
    /// 而不是先把内存吃掉再报错。
    pub fn oversized(status: u16, bytes: usize) -> Self {
        Self::Response {
            status,
            content_type: "application/json".to_owned(),
            body: vec![b'a'; bytes],
        }
    }
}

/// 一个绑在回环地址上的假服务。
pub struct FakeHttpServer {
    port: u16,
    stop: Arc<AtomicBool>,
    received: Arc<Mutex<Vec<ReceivedRequest>>>,
    script: Arc<Mutex<VecDeque<Reply>>>,
    fallback: Arc<Mutex<Reply>>,
}

impl FakeHttpServer {
    /// 启动。端口由系统分配，绑在 `127.0.0.1` 上。
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应能绑定回环端口");
        let port = listener.local_addr().expect("应能读到本地地址").port();

        let stop = Arc::new(AtomicBool::new(false));
        let received = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(VecDeque::new()));
        let fallback = Arc::new(Mutex::new(Reply::json(r#"{"ok":true}"#)));

        let server = Self {
            port,
            stop: Arc::clone(&stop),
            received: Arc::clone(&received),
            script: Arc::clone(&script),
            fallback: Arc::clone(&fallback),
        };

        let spawned = std::thread::Builder::new()
            .name("fake-http".to_owned())
            .spawn(move || serve(listener, stop, received, script, fallback))
            .expect("应能启动假服务线程");

        // 线程句柄不持有：服务靠 `stop` 标志退出，`Drop` 里置位并唤醒。
        drop(spawned);
        server
    }

    /// 形如 `http://127.0.0.1:PORT` 的基地址。
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 某个路径的完整地址。
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url())
    }

    /// 排队一个回应。按调用顺序消费，用完之后回落到 [`Self::set_fallback`]。
    pub fn push(&self, reply: Reply) {
        self.script.lock().expect("脚本锁").push_back(reply);
    }

    /// 脚本用完之后一律回这个。
    pub fn set_fallback(&self, reply: Reply) {
        *self.fallback.lock().expect("回落锁") = reply;
    }

    /// 到目前为止收到的所有请求。
    pub fn requests(&self) -> Vec<ReceivedRequest> {
        self.received.lock().expect("记录锁").clone()
    }

    /// 收到的请求数。
    pub fn request_count(&self) -> usize {
        self.received.lock().expect("记录锁").len()
    }

    /// 第 `index` 个请求（从 0 开始）。
    pub fn request(&self, index: usize) -> ReceivedRequest {
        self.requests()
            .get(index)
            .unwrap_or_else(|| {
                panic!(
                    "只收到 {} 个请求，取不到第 {index} 个",
                    self.request_count()
                )
            })
            .clone()
    }

    /// 等收到的请求数达到 `count`，最多等 `timeout`。
    ///
    /// 返回是否等到了。**不要用 sleep 猜**：请求何时到达取决于网络栈，
    /// 而轮询一个确定的条件既快又不抖。
    pub fn wait_for_requests(&self, count: usize, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if self.request_count() >= count {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.request_count() >= count
    }
}

impl Drop for FakeHttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // 唤醒可能正阻塞在 `accept` 上的循环。
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    received: Arc<Mutex<Vec<ReceivedRequest>>>,
    script: Arc<Mutex<VecDeque<Reply>>>,
    fallback: Arc<Mutex<Reply>>,
) {
    // 非阻塞 accept：这样停止标志才有机会被看到。
    listener
        .set_nonblocking(true)
        .expect("应能把监听套接字设为非阻塞");

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let received = Arc::clone(&received);
                let script = Arc::clone(&script);
                let fallback = Arc::clone(&fallback);
                // 每个连接一个线程：`Hang` 会在这个连接上停住，
                // 就地处理会把后面所有连接一起堵死。
                std::thread::spawn(move || handle_connection(stream, script, fallback, received));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    script: Arc<Mutex<VecDeque<Reply>>>,
    fallback: Arc<Mutex<Reply>>,
    received: Arc<Mutex<Vec<ReceivedRequest>>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));

    // **先读到请求，再取回复**——顺序反了会得到一个很难查的偶发失败：
    //
    // 连接不等于请求。有两种连接是「连上但什么都不发」的：
    //   1. `Drop` 里为了唤醒阻塞在 accept 上的循环而发起的那次 connect；
    //   2. 端口被回收后，上一次测试的唤醒连接落到了**这一次**的服务上
    //      （端口是系统分配的，回收后立刻可能被下一个测试拿到）。
    //
    // 在 accept 时就弹回复的话，这类连接会**吃掉一条本该给真实请求的回复**，
    // 之后所有请求的回复整体错位：本该回 401 的请求拿到了下一段脚本，
    // 脚本用尽后回落到「不响应」，客户端报 WinHTTP 12030 / 12152，
    // 测试挂在「分析应当成功」或「应当是 MODEL_AUTH」上——而失败用例
    // 每次还不一样，看着像随机。
    //
    // 拿不到请求就用回落回复（默认 `{"ok":true}`），**不消费脚本**。
    let Some(request) = read_request(&mut stream) else {
        let reply = fallback.lock().expect("回落锁").clone();
        respond(stream, reply);
        return;
    };
    received.lock().expect("记录锁").push(request);

    let reply = {
        let mut queue = script.lock().expect("脚本锁");
        match queue.pop_front() {
            Some(reply) => reply,
            None => fallback.lock().expect("回落锁").clone(),
        }
    };
    respond(stream, reply);
}

/// 按脚本给出回应。
fn respond(mut stream: TcpStream, reply: Reply) {
    match reply {
        Reply::Hang => {
            // 挂住但不关闭：客户端会一直等到自己的超时。
            // 停 8 秒足够覆盖测试里用到的短超时，又不会让线程活太久。
            std::thread::sleep(Duration::from_secs(8));
        }
        Reply::Abrupt => {
            // 直接 drop，客户端会看到连接被重置。
        }
        Reply::Redirect { location } => {
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {location}\r\n\
                 Content-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(response.as_bytes());
        }
        Reply::Response {
            status,
            content_type,
            body,
        } => {
            let head = format!(
                "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                reason_phrase(status),
                body.len()
            );
            // 客户端读到上限就会停止并关闭连接，于是这里的写可能拿到
            // EPIPE——那是**预期行为**，不是测试环境的毛病。
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    }

    let _ = stream.flush();
}

/// 读一个请求：先读到头部结束，再按 `Content-Length` 读体。
fn read_request(stream: &mut TcpStream) -> Option<ReceivedRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];

    let header_end = loop {
        let read = match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(read) => read,
            Err(_) => return None,
        };
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_double_crlf(&buffer) {
            break index;
        }
        if buffer.len() > 256 * 1024 {
            // 头部大到这个地步，不是我们要模拟的东西。
            return None;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();

    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect();

    let content_length: usize = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);

    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let read = match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        body.extend_from_slice(&chunk[..read]);
    }

    Some(ReceivedRequest {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}
