"""CI 静态检查：测试隔离 + 工作流脚本编码。

    python scripts/check-test-isolation.py

命中即非零退出。两项检查：

1. **集成测试不得在进程内直调 WinRT / OCR**（见下面 CI-004 那一节）
2. **用 5.1 版 shell 的 workflow 步骤，脚本必须是纯 ASCII**（见 CI-005 那一节）

两项都是「违反了不会立刻报错、只在特定环境下崩」的规矩 ——
本项目已经把这两条写成注释了，但都**照样被违反过**，所以做成机械检查。

## 为什么需要这条机械检查（CI-004，2026-09-27）

WinRT 的公寓模型**绑定线程**，而 libtest 每条用例跑在**自己的线程**上：

  第一个线程调 `RoInitialize` 初始化公寓 → 用例结束 → 线程退出
  → 第二个线程再调 WinRT → **`STATUS_ACCESS_VIOLATION`（段错误）**

实测表现很有迷惑性：**完整跑整份套件必崩，而单独跑那条用例却正常**
（甚至单独跑整个 image 子集也正常）—— 于是很容易被误判成「某个输入有毒」，
然后在错误的方向上查很久。

这条规矩本来就写在 `src-tauri/src/platform/ocr.rs` 的模块测试注释里：
「真实调用只走**工作进程**，由 `tests/extract.rs` 从外部验证」。
本项目架构也是按这个设计的 —— `extract_batch` 起工作进程，
`service::ocr_availability()` 同样走工作进程。

**但光有注释挡不住**：2026-09-27 修 CI-005 时，为了在「没有中文语言包的机器」
上分两条路，往 `tests/extract.rs` 里加了两处 `platform::ocr::availability()`
的进程内直调 —— 那个文件原本**零进程内 WinRT 调用**，加完就在 CI 上崩了。
所以把它做成机械检查，而不是继续指望注释。

## 允许的写法

需要知道本机 OCR 状态时，用这两条（都不碰进程内 WinRT）：

* 从**已经拿到的提取结果**里读 —— `code_of(extraction) == "OCR_UNAVAILABLE"`（零开销，首选）
* 问工作进程 —— `extractors::service::ocr_availability()`
"""

import re
import sys
from pathlib import Path

# 控制台编码：Windows 上 Python 的 stdout 默认跟随系统代码页
# （CI runner 是 cp1252、中文系统是 GBK），而本脚本会打印中文，
# 未设置时直接抛 UnicodeEncodeError。见 docs/POST_RELEASE_TODO.md 的 CI-003。
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(__file__).resolve().parent.parent
TESTS_DIR = PROJECT / "src-tauri" / "tests"

# 集成测试里**禁止**出现的进程内 WinRT 调用。
# 注意 `service::ocr_availability` 是允许的 —— 它起工作进程，不在这个列表里。
FORBIDDEN = [
    (re.compile(r"\bplatform::ocr::"), "进程内直调 platform::ocr（WinRT 公寓绑定线程）"),
]

# 注释行不算：文档里会引用这些名字来解释规则。
COMMENT = re.compile(r"^\s*//")


def scan(path: Path) -> list[str]:
    problems: list[str] = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if COMMENT.match(line):
            continue
        for pattern, label in FORBIDDEN:
            if pattern.search(line):
                rel = path.relative_to(PROJECT)
                problems.append(f"{rel}:{number}: {label}\n    {line.strip()}")
    return problems


# ---------------------------------------------------------------------------
# 第二项：用 5.1 版 shell 的 workflow 步骤，脚本必须是纯 ASCII
# ---------------------------------------------------------------------------
#
# **为什么**：GitHub 会把 `run:` 块写成临时 `.ps1` 文件；而 Windows PowerShell 5.1
# 在**没有 BOM** 时按系统代码页读它 —— 中文会变成乱码并直接 `ParserError`
# （2026-09-28 实测：`不要放过这一条…` 变成 `ä¸è¦æ”¾è¿‡…`，整步失败）。
#
# 而「确认 OCR 真的可用」那一步又**必须**用 5.1 —— 7 版没有内置 WinRT 类型投影。
# 两个约束合起来，就只能要求它的脚本全 ASCII。
#
# 顺带说：7 版默认按 UTF-8 读脚本，所以**别的步骤**打中文没问题；
# 这条检查只针对用 5.1 的那些步骤，不会误伤。

WORKFLOW = PROJECT / ".github" / "workflows" / "ci.yml"
LEGACY_SHELL = "powershell"


def check_workflow_ascii() -> list[str]:
    try:
        import yaml  # 延迟导入：没有它时只跳过这一项，不影响第一项检查
    except ImportError:
        return [""]  # 空串表示「跳过」，由调用方过滤

    if not WORKFLOW.exists():
        return [f"找不到 {WORKFLOW}"]

    doc = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
    problems: list[str] = []
    for job_name, job in (doc.get("jobs") or {}).items():
        for step in job.get("steps") or []:
            if step.get("shell") != LEGACY_SHELL:
                continue
            body = step.get("run") or ""
            bad = [character for character in body if ord(character) > 127]
            if bad:
                problems.append(
                    f"{job_name} / {step.get('name')}：脚本里有 {len(bad)} 个非 ASCII 字符\n"
                    f"    例：{''.join(bad[:12])!r}"
                )
    return problems


def main() -> int:
    if not TESTS_DIR.is_dir():
        print(f"找不到集成测试目录：{TESTS_DIR}", file=sys.stderr)
        return 2

    files = sorted(TESTS_DIR.rglob("*.rs"))
    problems: list[str] = []
    for path in files:
        problems.extend(scan(path))
    print(f"[1/2] 检查 {len(files)} 个集成测试文件：禁止进程内直调 WinRT")

    workflow_problems = [p for p in check_workflow_ascii() if p]
    print("[2/2] 检查 ci.yml：用 5.1 版 shell 的步骤，脚本必须是纯 ASCII")

    if not problems and not workflow_problems:
        print()
        print("通过：两项都没问题。")
        return 0

    print()
    for item in problems + workflow_problems:
        print(f"[违规] {item}")
    print()
    print(
        "结论：**不通过**。\n"
        "· 进程内直调 WinRT：公寓绑定线程，而 libtest 每条用例一个线程 ——\n"
        "  完整套件必然 STATUS_ACCESS_VIOLATION，单跑那条却正常。\n"
        "  改用：从提取结果读 `code_of(..) == \"OCR_UNAVAILABLE\"`，\n"
        "  或问工作进程 `extractors::service::ocr_availability()`。\n"
        "· 5.1 版 shell 的脚本含非 ASCII：GitHub 把它写成无 BOM 的临时 .ps1，\n"
        "  而 5.1 按系统代码页读 —— 中文会变乱码并 ParserError。\n"
        "  这一步又必须用 5.1（7 版没有 WinRT 类型投影）⇒ 脚本只用 ASCII。\n"
        "详见 src-tauri/src/platform/ocr.rs 的模块测试注释与 POST_RELEASE_TODO 的 CI-004 / CI-005。"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
