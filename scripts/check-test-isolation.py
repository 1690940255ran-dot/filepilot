"""静态检查：集成测试不得在**进程内**直调 WinRT / OCR。

    python scripts/check-test-isolation.py

命中即非零退出。

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


def main() -> int:
    if not TESTS_DIR.is_dir():
        print(f"找不到集成测试目录：{TESTS_DIR}", file=sys.stderr)
        return 2

    files = sorted(TESTS_DIR.rglob("*.rs"))
    problems: list[str] = []
    for path in files:
        problems.extend(scan(path))

    print(f"检查 {len(files)} 个集成测试文件（禁止进程内直调 WinRT）")
    if not problems:
        print("通过：没有进程内直连 WinRT 的调用。")
        return 0

    print()
    for item in problems:
        print(f"[违规] {item}")
    print()
    print(
        "结论：**不通过**。\n"
        "WinRT 公寓绑定线程，而 libtest 每条用例一个线程 —— 进程内直调会让\n"
        "**完整套件必然 STATUS_ACCESS_VIOLATION**，而单跑那条用例却正常。\n"
        "改用：从提取结果读 `code_of(..) == \"OCR_UNAVAILABLE\"`，\n"
        "或问工作进程 `extractors::service::ocr_availability()`。\n"
        "详见 src-tauri/src/platform/ocr.rs 的模块测试注释与 POST_RELEASE_TODO 的 CI-004。"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
