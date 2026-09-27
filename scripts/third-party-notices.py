"""生成 THIRD_PARTY_NOTICES.md —— 规格 T16：「核查每个直接/传递依赖」。

数据只来自两个 lockfile 对应的**实际安装结果**：

- Rust：`cargo metadata --locked`（解析的就是 Cargo.lock 里那一份）
- npm：`node_modules/.pnpm` 下每个包的 package.json（pnpm 按 lockfile 安装）

做三件事：

1. 汇总所有传递依赖的名称、版本、许可证；
2. 标出 copyleft（GPL/AGPL/LGPL/SSPL 等）——出现一个就**非零退出**，
   让发版流程停下来，而不是在几百行清单里被翻过去；
3. 重写 THIRD_PARTY_NOTICES.md（全文由本脚本生成，不要手改）。

用法：python scripts/third-party-notices.py
"""

import json
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


# 控制台编码：Windows 上 Python 的 stdout 默认跟随系统代码页
# （CI runner 是 cp1252、中文系统是 GBK），而本脚本会打印中文，
# 未设置时直接抛 UnicodeEncodeError，让整条命令失败。
# 见 docs/POST_RELEASE_TODO.md 的 CI-003。
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(__file__).resolve().parent.parent
CARGO_TOML = PROJECT / "src-tauri" / "Cargo.toml"
PNPM_DIR = PROJECT / "node_modules" / ".pnpm"
OUTPUT = PROJECT / "THIRD_PARTY_NOTICES.md"

# 我们自己的包不进清单
SELF_PACKAGES = {"filepilot", "filepilot_lib"}

# 强 copyleft：出现即失败。MIT 项目链接 GPL 会让整个产物受传染。
# LGPL 单独一档：动态链接通常可用，但桌面应用静态链接居多，要人工确认。
STRONG_COPYLEFT = re.compile(r"\b(GPL-[\d.]+|AGPL-[\d.]+|SSPL|EUPL|CDDL|CPAL)\b", re.I)
WEAK_COPYLEFT = re.compile(r"\b(LGPL-[\d.]+|MPL-[\d.]+|EPL-[\d.]+)\b", re.I)


def collect_cargo() -> dict[str, list[tuple[str, str]]]:
    """license 表达式 -> [(name, version)]，按字母序。"""
    cargo = shutil.which("cargo")
    if cargo is None:
        sys.exit("[notices] 找不到 cargo —— 无法核对 Rust 依赖。")
    result = subprocess.run(
        [cargo, "metadata", "--locked", "--format-version", "1",
         "--manifest-path", str(CARGO_TOML)],
        capture_output=True, text=True, encoding="utf-8",
    )
    if result.returncode != 0:
        sys.exit(f"[notices] cargo metadata 失败：\n{result.stderr}")
    meta = json.loads(result.stdout)

    by_license: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for pkg in meta["packages"]:
        if pkg["name"] in SELF_PACKAGES:
            continue
        license_expr = pkg.get("license") or pkg.get("license_file") or "未标注"
        by_license[license_expr].append((pkg["name"], pkg["version"]))
    return dict(sorted(by_license.items()))


def collect_npm() -> dict[str, list[tuple[str, str]]]:
    by_license: dict[str, list[tuple[str, str]]] = defaultdict(list)
    if not PNPM_DIR.is_dir():
        print("[notices] 没有 node_modules/.pnpm，跳过 npm 部分"
              "（先跑 pnpm install 再生成完整清单）", file=sys.stderr)
        return {}
    seen: set[str] = set()
    for pkg_json in PNPM_DIR.glob("*/node_modules/*/package.json"):
        try:
            info = json.loads(pkg_json.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        name = info.get("name", pkg_json.parent.name)
        if name in seen:
            continue
        seen.add(name)
        raw = info.get("license") or info.get("licenses") or "未标注"
        if isinstance(raw, list):  # 旧式 licenses 数组
            raw = " OR ".join(str(item.get("type", item)) for item in raw)
        by_license[str(raw)].append((name, info.get("version", "?")))
    for entries in by_license.values():
        entries.sort()
    return dict(sorted(by_license.items()))


def audit(groups: dict[str, list], strong: list, weak: list, source: str) -> None:
    for license_expr, entries in groups.items():
        if STRONG_COPYLEFT.search(license_expr):
            strong.append((source, license_expr, entries))
        elif WEAK_COPYLEFT.search(license_expr):
            weak.append((source, license_expr, entries))


def render_table(groups: dict[str, list[tuple[str, str]]]) -> list[str]:
    lines: list[str] = []
    for license_expr, entries in groups.items():
        lines.append(f"### {license_expr}（{len(entries)} 个）")
        lines.append("")
        for name, version in entries:
            lines.append(f"- `{name}` {version}")
        lines.append("")
    return lines


def main() -> int:
    cargo_groups = collect_cargo()
    npm_groups = collect_npm()

    strong: list = []
    weak: list = []
    audit(cargo_groups, strong, weak, "Rust")
    audit(npm_groups, strong, weak, "npm")

    total = sum(len(v) for v in cargo_groups.values()) + \
        sum(len(v) for v in npm_groups.values())

    lines = [
        "# 第三方组件与许可证",
        "",
        "<!-- 本文件由 scripts/third-party-notices.py 生成，不要手改。 -->",
        "<!-- 依赖变化后重跑：python scripts/third-party-notices.py -->",
        "",
        "FilePilot 本身以 MIT 发布（见 `LICENSE`）。以下列出全部直接/传递依赖",
        "及其许可证；各自的许可证全文见对应上游仓库。",
        "",
        "## 1. 审计结论",
        "",
    ]
    if strong:
        lines.append("**发现强 copyleft 依赖，发版前必须处理：**")
        lines.append("")
        for source, expr, entries in strong:
            # 带版本号：不带版本的许可证清单没法照着处理
            # （同一个包可能同时存在多个版本，只打名字会出现「`r-efi`, `r-efi`」）
            names = ", ".join(f"`{n} {v}`" for n, v in entries[:5])
            lines.append(f"- [{source}] {expr}：{names}")
        lines.append("")
    else:
        lines.append(f"- 共 {total} 个传递依赖，**未发现强 copyleft**"
                     "（GPL/AGPL/SSPL 等）。")
    if weak:
        lines.append("- 弱 copyleft（动态链接可用，静态链接需人工确认）：")
        for source, expr, entries in weak:
            names = ", ".join(f"`{n} {v}`" for n, v in entries[:5])
            lines.append(f"  - [{source}] {expr}：{names}")
    else:
        lines.append("- 未发现弱 copyleft（LGPL/MPL/EPL）。")
    lines += [
        "",
        "### 弱 copyleft 的人工确认记录",
        "",
        "- `r-efi` 标注 `MIT OR Apache-2.0 OR LGPL-2.1-or-later`——任选其一，",
        "  本项目按 MIT/Apache-2.0 使用，不构成问题（2026-09-22 确认）。",
        "- MPL-2.0（`cssparser`/`lightningcss` 等）是**文件级** copyleft：",
        "  不修改这些依赖的源码时，按其原样链接分发即合规，",
        "  源码在 crates.io / npm 公开可得（2026-09-22 确认）。",
        "",
        "## 2. 不打包的内容",
        "",
        "- **模型权重**：AI 模式的模型由用户自备（本地 Ollama 或云端 API），",
        "  安装包不包含任何模型文件。",
        "- **OCR 资源**：使用 Windows 系统自带的 WinRT OCR 与语言包，",
        "  不随应用分发。",
        "- **图标**：`src-tauri/icons/` 由 `scripts/make-icons.py` 生成，",
        "  与项目同为 MIT。",
        "- **仅在构建期使用的 npm 包**（例如 `ajv`）：契约校验器已经在构建期",
        "  预编译成静态代码（见 `scripts/generate-validators.mjs`），运行时不加载",
        "  这些包。下面第 4 节仍然把它们列出来——宁可多列，不要漏列。",
        "",
        "## 3. Rust 依赖（Cargo.lock）",
        "",
    ]
    lines += render_table(cargo_groups)
    lines += [
        "## 4. npm 依赖（pnpm-lock.yaml）",
        "",
    ]
    if npm_groups:
        lines += render_table(npm_groups)
    else:
        lines.append("（生成时未安装 node_modules，此节为空。）")
        lines.append("")

    OUTPUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"[notices] 已写入 {OUTPUT.name}：共 {total} 个依赖，"
          f"强 copyleft {len(strong)} 组，弱 copyleft {len(weak)} 组")
    return 1 if strong else 0


if __name__ == "__main__":
    sys.exit(main())
