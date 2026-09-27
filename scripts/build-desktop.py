"""生产安装包的构建入口：先给路径「去名」，再交给 `tauri build`。

## 为什么需要这个脚本

release 二进制里会留下**编译机的绝对路径**。实测（2026-09-24，对
`filepilot.exe` 逐字节搜用户名）：

```
C:\\Users\\cj169\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\anyhow-1.0.104\\src\\error.rs
C:\\Users\\cj169\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\brotli-decompressor-5.0.3\\src\\decode.rs
```

来源是**依赖 crate 的 panic 位置**（`file!()` 是编译期字面量），
`strip = true` 去不掉它。于是**编译机的用户名**跟着安装包一起发出去。

规格 T16 明写：「发布内容不包含真实路径样本、API Key、模型返回原文或用户资料」。
所以这不是洁癖，是验收项。

## 做法

rustc 的 `--remap-path-prefix`：

| 原前缀 | 映射为 |
|---|---|
| 项目目录 | `.` |
| `<用户目录>\\.cargo\\registry` | `/cargo` |
| `<用户目录>\\.rustup\\toolchain` | `/rust` |

两个前缀都要映射：只映射项目目录时，依赖里的路径照旧带着用户名。

## 两个实现细节

1. **用 `CARGO_ENCODED_RUSTFLAGS` 而不是 `RUSTFLAGS`**：
   cargo 会把 `RUSTFLAGS` 按**空白**切分，而路径里可能有空格
   （`C:\\Users\\Some User\\...`）。Encoded 形式用 `\\x1f` 分隔，天然免疫。
2. **RUSTFLAGS 变了，cargo 指纹就变了** → 第一次会全量重编译一次
   （release 大约十几分钟）。这是有意的：换来的是产物不含编译机信息。
   之后的增量构建不受影响。

CI 里同样走这个脚本（`pnpm desktop:build`），所以 CI 产物也不含 runner 的路径。
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

# 输出重定向到文件时 Python 会整块缓冲，日志顺序就乱了——
# 而构建日志的顺序正是用来定位「哪一步失败」的。
#
# **必须同时指定 encoding**：Windows 上 stdout 默认跟随系统代码页
# （CI runner 是 cp1252、中文系统是 GBK），而本脚本会打印中文
# （如 `print("[release] 路径重映射：")`），未指定时直接抛
# `UnicodeEncodeError: 'charmap' codec can't encode characters...`，
# 让整个打包 job 变红。见 docs/POST_RELEASE_TODO.md 的 CI-003。
sys.stdout.reconfigure(encoding="utf-8", errors="replace", line_buffering=True)
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(__file__).resolve().parent.parent


def remap_flags() -> list[str]:
    """要映射的 (原前缀, 目标) 列表。

    ## 为什么**不能**用 `prefix.exists()` 过滤（2026-09-27 CI-007）

    这里原本是「只映射真实存在的前缀」，本意是防 rustup 用系统工具链时那一层不存在。
    但那是个**会静默失效**的防御：`--remap-path-prefix` 是**纯字符串前缀改写**，
    它根本不要求被映射的路径存在；而反过来，路径**暂时**不存在时跳过映射，
    就会让依赖带着原路径被编译。

    这正是 CI 上发生的事 —— 该步骤在 CI 日志里只打印了两条映射：

        [release] 路径重映射：
          D:\\a\\filepilot\\filepilot=.
          C:\\Users\\runneradmin\\.rustup\\toolchains=/rust      ← 少了 .cargo/registry

    于是 `aes` / `brotli-decompressor` / `anyhow` 这些依赖的 panic 位置
    把 `C:\\Users\\<runner>\\.cargo\\registry\\src\\…` 留在了两个可执行文件里，
    内容扫描正确地拒绝了产物。本地 registry 一直是热的，所以**本机永远复现不出来**。

    ## 顺带修正 CARGO_HOME

    原来写死 `<home>/.cargo`，但 CI 的 workflow 显式设置了 `CARGO_HOME`，
    两者未必是同一个位置。现在优先读环境变量，读不到才回落到 `<home>/.cargo`。
    """
    home = Path.home()
    cargo_home = Path(os.environ.get("CARGO_HOME") or (home / ".cargo"))
    rustup_home = Path(os.environ.get("RUSTUP_HOME") or (home / ".rustup"))

    flags: list[str] = []
    for prefix, target in (
        (PROJECT, "."),
        (cargo_home / "registry", "/cargo"),
        (rustup_home / "toolchains", "/rust"),
    ):
        # **不做 exists() 判断**：见上面的说明。
        # 前缀不存在时这条规则只是匹配不到任何东西，是安全的空操作；
        # 而跳过它会让产物泄漏编译机路径 —— 代价远大于收益。
        flags.extend(["--remap-path-prefix", f"{prefix}={target}"])
    return flags


def main() -> int:
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"):
        if os.environ.get(name):
            print(
                f"[release] {name} 已经设置——本脚本会覆盖它，"
                "所以直接停下来，而不是悄悄丢掉你原本的 flags。\n"
                "请自己把 --remap-path-prefix 加进去（或用 "
                "CARGO_ENCODED_RUSTFLAGS 的 \\x1f 分隔形式），再重跑。",
                file=sys.stderr,
            )
            return 2

    tauri = shutil.which("tauri")
    if tauri is None:
        print(
            "[release] 找不到 tauri 命令。请先 `pnpm install`，"
            "并用 `pnpm desktop:build` 调用本脚本（它会带上 node_modules/.bin 的 PATH）。",
            file=sys.stderr,
        )
        return 1

    env = os.environ.copy()
    flags = remap_flags()
    # \x1f（Unit Separator）是 cargo 官方的 encoded 分隔符
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    print("[release] 路径重映射：")
    # flags 是 [--remap-path-prefix, "从=到", ...]，有价值的是后面那半
    for pair in flags[1::2]:
        print(f"  {pair}")
    print(f"[release] tauri: {tauri}")
    print()

    return subprocess.run([tauri, "build"], cwd=str(PROJECT), env=env).returncode


if __name__ == "__main__":
    sys.exit(main())
