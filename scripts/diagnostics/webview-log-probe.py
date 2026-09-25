"""用 WebView2 的 Chromium 日志拿到页面内部的报错（CSP 拒绝 / JS 异常 / 加载失败）。

安装版白屏但看不到控制台——`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 可以把
Chromium 的 `CONSOLE(...)` 与网络错误写进日志文件。这是唯一能"问页面"的通道。

一次性的验收探针。
"""

import os
import subprocess
import time
from pathlib import Path

# 相对本脚本定位仓库根（`scripts/diagnostics/` → 上两级）。
# 安装目录走环境变量，不写死用户名——`%LOCALAPPDATA%` 在每台机器上都对。
PROJECT = Path(__file__).resolve().parents[2]
INSTALLER = PROJECT / "src-tauri" / "target" / "release" / "bundle" / "nsis" / "FilePilot_0.1.0_x64-setup.exe"
INSTALL_DIR = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local")) / "FilePilot"
LOG = PROJECT / "tmp" / "webview-debug.log"

lines: list[str] = []


def say(text: str = "") -> None:
    lines.append(text)
    print(text, flush=True)


say("=== 安装 ===")
r = subprocess.run([str(INSTALLER), "/S"], capture_output=True, text=True, errors="replace")
say(f"退出码: {r.returncode}")

if LOG.exists():
    LOG.unlink()

say("\n=== 带 Chromium 日志启动 ===")
env = os.environ.copy()
env["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = (
    f"--enable-logging --log-file={LOG} --v=1"
)
app = subprocess.Popen(
    [str(INSTALL_DIR / "filepilot.exe")],
    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace", env=env,
)
say(f"pid={app.pid}，等待 20 秒")
time.sleep(20)

app.terminate()
try:
    out, _ = app.communicate(timeout=10)
except subprocess.TimeoutExpired:
    app.kill()
    out, _ = app.communicate()
say(f"\n应用 stdout: {(out or '').strip()[:400] or '（无）'}")

say(f"\n=== Chromium 日志（{LOG}）===")
if LOG.is_file():
    text = LOG.read_text(encoding="utf-8", errors="replace")
    say(f"日志大小: {len(text)} 字符")
    keep = [
        line for line in text.splitlines()
        if any(k in line for k in ("CONSOLE", "ERROR", "Refused", "Failed", "csp", "CSP",
                                    "net::", "404", "Uncaught", "blocked"))
    ]
    for line in keep[:60]:
        say("  " + line.strip()[:260])
    if not keep:
        say("（没有 CONSOLE/ERROR 行——看原始日志尾部）")
        for line in text.splitlines()[-25:]:
            say("  " + line.strip()[:200])
else:
    say("没有生成日志文件")

say("\n=== 卸载 ===")
r = subprocess.run([str(INSTALL_DIR / "uninstall.exe"), "/S"], capture_output=True, text=True, errors="replace")
time.sleep(5)
say(f"退出码: {r.returncode}")

(PROJECT / "tmp" / "webview-log-report.txt").write_text("\n".join(lines), encoding="utf-8")
print("\n[报告] tmp/webview-log-report.txt")
