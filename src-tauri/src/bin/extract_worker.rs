//! `extract_worker` —— 内容解析工作进程（规格 6.2）。
//!
//! 它只做一件事：从父进程继承来的**只读句柄**里读出内容，把结论写成一行 JSON。
//!
//! ## 为什么是一个独立进程
//!
//! 规格写得很直接：**Tauri 前端隔离不等于解析器隔离**，
//! 「不能在 UI 进程直接执行不可信解析」。
//!
//! 用户文件内容是不可信数据。解析器崩一次，代价不该是整个应用连同
//! 用户还没保存的界面状态一起消失——那是把「读一份坏文件」变成
//! 「丢掉正在做的事」。
//!
//! ## 它拿不到什么
//!
//! * **没有路径。** 参数表里只有句柄、fileId 和格式。想读别的文件，
//!   得先有一支别的文件的句柄，而句柄只能由父进程给。
//! * **没有网络。** 这个二进制不链接任何网络代码，也不读任何 URL。
//!   这一条是**构造性**的（代码里就没有那条路径），不是运行时拦截。
//! * **没有子进程。** 由父进程设置的 Job Object 限制为 1 个活动进程，
//!   它自己 spawn 出来的东西会被操作系统直接拒绝。
//! * **内存与时间有上限。** 同样来自 Job Object。

use std::io::Write;
use std::process::ExitCode;

use filepilot_lib::extractors::protocol::{WorkerArgs, PROTOCOL_VERSION, WORKER_USAGE_EXIT_CODE};
use filepilot_lib::extractors::worker;

fn main() -> ExitCode {
    let args = match WorkerArgs::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            // 用法错误走**另一个退出码**：父进程据此区分「参数没传对」
            // 与「解析结果不理想」。两者排查看的是完全不同的地方。
            eprintln!("extract_worker: {message}");
            return ExitCode::from(WORKER_USAGE_EXIT_CODE as u8);
        }
    };

    if args.version != PROTOCOL_VERSION {
        eprintln!(
            "extract_worker: 协议版本不符（收到 {}，本进程支持 {}）",
            args.version, PROTOCOL_VERSION
        );
        return ExitCode::from(WORKER_USAGE_EXIT_CODE as u8);
    }

    // 按模式检查必填项。缺参数是**用法错误**，与「解析结果不理想」
    // 用不同的退出码分开——两者排查看的是完全不同的地方。
    if let Err(message) = args.validate() {
        eprintln!("extract_worker: {message}");
        return ExitCode::from(WORKER_USAGE_EXIT_CODE as u8);
    }

    let output = worker::run(&args);

    // 写成一行 JSON，父进程读一行即可。
    let json = match output.to_json() {
        Ok(json) => json,
        Err(error) => {
            // 序列化失败是我们自己的缺陷，不是用户文件的问题。
            eprintln!("extract_worker: 无法写出结果: {error}");
            return ExitCode::from(WORKER_USAGE_EXIT_CODE as u8);
        }
    };

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if let Err(error) = lock
        .write_all(json.as_bytes())
        .and_then(|()| lock.write_all(b"\n"))
        .and_then(|()| lock.flush())
    {
        eprintln!("extract_worker: 无法写出结果: {error}");
        return ExitCode::from(WORKER_USAGE_EXIT_CODE as u8);
    }

    ExitCode::SUCCESS
}
