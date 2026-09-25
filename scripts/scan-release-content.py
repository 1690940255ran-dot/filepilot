"""发布物内容扫描（规格 T16：「发布内容不包含真实路径样本、API Key、模型返回原文或用户资料」）。

这是 `RELEASE_CHECKLIST.md` 第 5 节里**可自动化的部分**：清单上写着「逐项检查」，
而靠人眼逐字节翻二进制是不现实的——所以做成一条命令。

## 判定规则（为什么这样定）

| 对象 | 判据 | 理由 |
|---|---|---|
| **文本**（源码 / 文档 / 脚本 / 前端产物） | 出现**本机用户名**构成的用户目录路径，或本机的开发目录名 | 这些文件会进公开仓库；夹具里的 `C:\\Users\\示例\\…` 之类占位符不算 |
| **可执行文件** | 出现**任意** `X:\\Users\\<名字>\\` | 二进制里不该有编译机的任何用户路径——它一定来自构建环境 |

用户名**不写死在本脚本里**（写死等于让这个脚本自己成为泄漏源），
而是运行时取当前用户名的目录名。代价：换一台机器跑，判定基准随之变化——
这对「发布前在本机检查」正是想要的语义。

## 它**证明不了**什么（必须一起说）

NSIS 安装包是**固实 LZMA 压缩**的，直接在包里搜字符串，命中与未命中**都不算证据**。
所以本脚本不扫安装包；「安装包里不含真实路径」这一条在清单里仍然留给人工
（装完之后看安装目录）。

## 用法

    python scripts/scan-release-content.py            # 汇总
    python scripts/scan-release-content.py --verbose  # 每个命中都列出来

命中即以非零码退出，发版流程应当因此停下。
"""

import argparse
import re
import sys
from collections import Counter
from pathlib import Path

PROJECT = Path(__file__).resolve().parent.parent
CURRENT_USER = Path.home().name

# 会在文本里正常出现的占位名：夹具、文档示例
PLACEHOLDER_NAMES = {"示例", "someone", "me", "user", "username", "public", "default", "all users"}

USER_PATH = re.compile(r"[A-Za-z]:\\Users\\([^\\\s\"']+)\\")
DEV_DIR = re.compile(r"Desktop[\\/]开发")

SECRET_PATTERNS = {
    "OpenAI 形状密钥": re.compile(r"sk-[A-Za-z0-9]{16,}"),
    "Bearer 令牌": re.compile(r"Bearer\s+[A-Za-z0-9._-]{20,}"),
    "AWS 形状密钥": re.compile(r"AKIA[0-9A-Z]{16}"),
    "私钥块": re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
}

TREES = [
    ("源码", PROJECT / "src"),
    ("Rust 源码", PROJECT / "src-tauri" / "src"),
    ("测试", PROJECT / "tests"),
    ("脚本", PROJECT / "scripts"),
    ("文档", PROJECT / "docs"),
    ("前端产物", PROJECT / "dist"),
]

BINARIES = [
    PROJECT / "src-tauri" / "target" / "release" / "filepilot.exe",
    PROJECT / "src-tauri" / "target" / "release" / "extract_worker.exe",
]

SKIP_DIR_NAMES = {"node_modules", "target", ".git", "screenshots"}
SKIP_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".woff", ".woff2", ".pdb", ".rlib"}


def read_text(path: Path) -> str | None:
    try:
        return path.read_bytes().decode("utf-8", errors="ignore")
    except OSError as error:
        print(f"[skip] {path}: {error}")
        return None


def check_text(path: Path, hits: Counter[str]) -> None:
    text = read_text(path)
    if text is None:
        return
    rel = path.relative_to(PROJECT)

    for match in USER_PATH.finditer(text):
        name = match.group(1)
        if name.lower() in PLACEHOLDER_NAMES:
            continue
        # 只有「本机用户名」在文本里才算泄漏；别人机器的路径出现在
        # 文档示例里是可以的（例如解释某个报错长什么样）
        if name == CURRENT_USER:
            hits[f"{rel} -> 本机用户目录路径: {match.group(0)}"] += 0
            hits[f"{rel} -> 本机用户目录路径"] += 1

    for match in DEV_DIR.finditer(text):
        hits[f"{rel} -> 本机开发目录名: {match.group(0)}"] += 1

    for label, pattern in SECRET_PATTERNS.items():
        if pattern.search(text):
            hits[f"{rel} -> {label}"] += 1


def check_binary(path: Path, hits: Counter[str]) -> None:
    if not path.exists():
        print(f"[skip] 不存在（还没构建？）: {path}")
        return
    text = read_text(path)
    if text is None:
        return
    names = {match.group(1) for match in USER_PATH.finditer(text)}
    if names:
        sample = ", ".join(sorted(names)[:3])
        hits[f"{path.name} -> 编译机用户路径（{len(names)} 个用户：{sample}）"] += 1
    for label, pattern in SECRET_PATTERNS.items():
        if pattern.search(text):
            hits[f"{path.name} -> {label}"] += 1


def main() -> int:
    parser = argparse.ArgumentParser(description="发布物内容扫描")
    parser.add_argument("--verbose", action="store_true", help="列出全部命中行")
    args = parser.parse_args()

    hits: Counter[str] = Counter()
    checked = 0

    for _, tree in TREES:
        if not tree.exists():
            print(f"[skip] 不存在: {tree}")
            continue
        for path in sorted(tree.rglob("*")):
            if not path.is_file():
                continue
            if any(part in SKIP_DIR_NAMES for part in path.parts):
                continue
            if path.suffix.lower() in SKIP_SUFFIXES:
                continue
            checked += 1
            check_text(path, hits)

    for path in BINARIES:
        checked += 1
        check_binary(path, hits)

    print(f"当前用户名（判定基准）：{CURRENT_USER}")
    print(f"扫描 {checked} 个文件，命中 {len(hits)} 类\n")

    if not hits:
        print(
            "结论：未命中。源码 / 文档 / 脚本 / 前端产物 / 可执行文件都干净。\n"
            "⚠️  NSIS 安装包内的压缩负载**未被本脚本覆盖**——"
            "「装完之后不含真实路径」仍要人工看一眼（RELEASE_CHECKLIST 第 2、3 节）。"
        )
        return 0

    for entry, count in sorted(hits.items()):
        print(f"[HIT ] {entry}" + (f"（{count} 处）" if count > 1 else ""))
    if not args.verbose:
        print("\n（同一条会合并计数；要看逐条明细加 --verbose）")
    print(
        "\n结论：**不通过**。发布物里出现了不该有的内容。\n"
        "可执行文件里的用户路径通常来自依赖 crate 的 panic 位置（`file!()`），\n"
        "`strip = true` 去不掉；用 `--remap-path-prefix` 去掉——见 scripts/build-desktop.py。"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
