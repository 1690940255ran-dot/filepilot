"""把 `extract_worker` 放到 Tauri 打包器找得到的地方。

规格 T16：「打包 OCR/解析 worker 所需资源**并验证可找到**。」

Tauri 的 `bundle.externalBin` 要求文件**预先存在**、且文件名带 target triple
（如 `extract_worker-x86_64-pc-windows-msvc.exe`），打包时它会去掉 triple
后缀、把文件放进安装目录的**主程序同级**——而 `worker_binary()` 正是在
那里找它的。

所以这个脚本做三件事：

1. 单独编译 `extract_worker`（`--release`，与安装包同一优化级别）；
2. 复制为带 triple 的名字，放进 `src-tauri/binaries/`；
3. **验证它真的在那里**——名字写错时 Tauri 的报错是一句很含糊的
   「找不到文件」，与其到时候读它，不如在这里就挡住。

## 为什么要给这一步加 `TAURI_CONFIG` 覆盖（踩过的坑）

`tauri-build` 的**构建脚本**会读 `bundle.externalBin`，并把列出的文件
拷到 target 目录——文件不存在就直接让构建脚本失败：

    resource path `binaries\\extract_worker-x86_64-pc-windows-msvc.exe` doesn't exist

而 `extract_worker` 是**同一个 crate** 的 bin 目标，编译它必然触发这个构建
脚本。于是形成一个死锁：**staging 那一步的编译，恰恰被它自己要产出的文件挡住了。**

这个死锁的表现很容易被误读：报错说「资源不存在」，看起来像配置写错了，
而实际上配置是对的——只是顺序不可能成立。

解法是让 staging 这次编译**不校验** externalBin（`TAURI_CONFIG` 是
tauri 官方的配置覆盖入口，会 merge 到 `tauri.conf.json` 之上）：

- staging 编译：`externalBin = []` → 构建脚本不校验 → worker 编译出来；
- 之后的 `tauri build`：不带覆盖 → 正常校验并把它收进安装包。

两次构建不会互相污染：构建脚本自己声明了
`cargo:rerun-if-env-changed=TAURI_CONFIG`，覆盖变了它会重跑。
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# 输出重定向到文件时 Python 会整块缓冲，日志顺序就乱了：
# 实测出现过「编译失败」打印在「编译 extract_worker…」**之前**，
# 而构建日志的顺序正是用来判断「哪一步失败」的。
#
# **必须同时指定 encoding**（同 scripts/build-desktop.py 的理由）：
# Windows 上 stdout 默认跟随系统代码页（CI runner 是 cp1252、
# 中文系统是 GBK），打印中文会抛 UnicodeEncodeError 让命令失败。
sys.stdout.reconfigure(encoding="utf-8", errors="replace", line_buffering=True)
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(__file__).resolve().parent.parent
CARGO = PROJECT / "src-tauri" / "Cargo.toml"
TARGET_TRIPLE = "x86_64-pc-windows-msvc"

built = PROJECT / "src-tauri" / "target" / "release" / "extract_worker.exe"
staged_dir = PROJECT / "src-tauri" / "binaries"
staged = staged_dir / f"extract_worker-{TARGET_TRIPLE}.exe"


def main() -> int:
    print("[bundle:workers] 编译 extract_worker（release，跳过 externalBin 校验）…")
    env = os.environ.copy()
    env["TAURI_CONFIG"] = json.dumps({"bundle": {"externalBin": []}})
    result = subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(CARGO), "--bin", "extract_worker"],
        cwd=str(PROJECT),
        env=env,
    )
    if result.returncode != 0:
        print("[bundle:workers] 编译失败", file=sys.stderr)
        return result.returncode

    if not built.is_file():
        print(
            f"[bundle:workers] 编译成功了但找不到 {built} —— 路径假设错了，"
            "请检查这个脚本。",
            file=sys.stderr,
        )
        return 1

    staged_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(built, staged)
    print(f"[bundle:workers] 已放到 {staged.relative_to(PROJECT)}")

    # 验证：这是规格里那句「并验证可找到」的兑现。验证的是**文件存在**，
    # 而「安装后主程序能不能找到它」由 `worker_binary()` 的查找逻辑保证
    # （它就在主程序同目录找）——那一条有集成测试守着。
    if not staged.is_file():
        print("[bundle:workers] 复制后验证失败", file=sys.stderr)
        return 1
    size_mb = staged.stat().st_size / (1024 * 1024)
    print(f"[bundle:workers] 验证通过（{size_mb:.1f} MB）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
