"""清理工作区根目录下的中转文件（`tmp_*.txt`、`.tools/`）。

**默认只打印、不删除**；要真删得显式加 `--apply`。
一个默认就会删目录的脚本不该放在仓库里。

工作区根目录**不再写死**（那会把编译机的用户名一起发到公开仓库里）：
默认取本脚本所在位置往上两级（`filepilot/scripts/` → 工作区根），
也可以用 `--root` 指定。

用法：
    python scripts/cleanup-temp.py                # 预演
    python scripts/cleanup-temp.py --apply        # 真删
    python scripts/cleanup-temp.py --root D:\\x --apply
"""

import argparse
import pathlib
import shutil
import sys

# `scripts/` 的上一级是项目根，再上一级是工作区根
DEFAULT_ROOT = pathlib.Path(__file__).resolve().parents[2]

TARGETS = ("tmp_*.txt",)
TARGET_DIRS = (".tools",)


def main() -> int:
    parser = argparse.ArgumentParser(description="清理工作区中转文件")
    parser.add_argument(
        "--root",
        type=pathlib.Path,
        default=DEFAULT_ROOT,
        help=f"工作区根目录（默认 {DEFAULT_ROOT}）",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="真的删除；不加这个参数只预演",
    )
    args = parser.parse_args()

    root: pathlib.Path = args.root
    if not root.is_dir():
        print(f"根目录不存在：{root}", file=sys.stderr)
        return 1

    print(f"根目录：{root}")
    print("模式：" + ("**真删**" if args.apply else "预演（加 --apply 才真删）"))
    print()

    found: list[pathlib.Path] = []
    for pattern in TARGETS:
        found.extend(sorted(root.glob(pattern)))
    for name in TARGET_DIRS:
        candidate = root / name
        if candidate.is_dir():
            found.append(candidate)

    if not found:
        print("没有需要清理的内容。")
        return 0

    print(f"命中 {len(found)} 项：")
    for path in found:
        kind = "DIR " if path.is_dir() else "FILE"
        print(f"  [{kind}] {path.name}")

    if not args.apply:
        print("\n以上仅为预演，未删除任何内容。")
        return 0

    for path in found:
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
    print(f"\n已删除 {len(found)} 项。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
