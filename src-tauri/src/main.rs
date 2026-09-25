// Release 构建不弹出控制台窗口；debug 构建保留控制台以便看日志。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    filepilot_lib::run()
}
