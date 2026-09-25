//! 规格 6.4：「代理环境变量不得偷偷把请求转发到外网」。
//!
//! ## 为什么这一条**必须单独一个测试二进制**
//!
//! 验证它需要 `std::env::set_var`，而那是**进程级**的改动。Rust 之所以把
//! 多线程下的 `set_var` 视为不安全，是因为它改动的是 CRT 的 `environ`
//! 数组——别的线程同时调用 `getenv` 就可能读到损坏的内容。
//!
//! 这不是理论担忧：这条测试最初写在 `ai_contract.rs` 里，结果是**同一
//! 个进程里另外 8 条网络测试开始随机报 WinHTTP 12152 / 12030**，
//! 而串行跑它们全过。把这条挪到这里之后，两个文件各自干净。
//!
//! `cargo test` 会**串行**运行各个测试二进制，所以这里改环境变量的那段时间
//! 没有任何别的测试在跑；而这个文件里只有一条测试，libtest 的多线程
//! 调度也就无从发挥。

#![cfg(windows)]

mod support;

use std::time::Duration;

use filepilot_lib::ai::http::{post_json, Endpoint, NeverCancel, REQUEST_TIMEOUT};

use support::fake_http::{FakeHttpServer, Reply};

#[test]
fn an_http_proxy_environment_variable_does_not_redirect_the_request() {
    // 做法是把三个常见的代理变量都指向一个**假的代理服务**，
    // 然后验证目标服务收到了请求、而代理服务一个都没收到。
    //
    // 这条之所以成立，是因为 WinHTTP 会话是用
    // `WINHTTP_ACCESS_TYPE_NO_PROXY` 开的：它既不读系统代理设置
    // （IE/WinINet 那份），也不读环境变量——不是「我们检查过没有代理」，
    // 而是那条路径不存在。
    let proxy = FakeHttpServer::start();
    let target = FakeHttpServer::start();
    target.set_fallback(Reply::json("{}"));

    let proxy_url = proxy.base_url();
    let saved: Vec<(&str, Option<String>)> = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        .iter()
        .map(|name| (*name, std::env::var(name).ok()))
        .collect();
    for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
        std::env::set_var(name, &proxy_url);
    }

    let endpoint = Endpoint::parse_local(&target.url("/v1/chat/completions"))
        .expect("假服务地址应当是合法的本地端点");
    let outcome = post_json(&endpoint, "{}", &[], REQUEST_TIMEOUT, &NeverCancel);

    // 先把环境恢复回去，再断言——否则一个失败的断言会让这条测试
    // 把环境留给同一进程里后续的代码。
    for (name, value) in saved {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }

    outcome.expect("请求应当直连目标服务");
    assert!(target.wait_for_requests(1, Duration::from_secs(5)));
    assert_eq!(
        proxy.request_count(),
        0,
        "请求被代理转发了：{:?}",
        proxy.requests()
    );
}
