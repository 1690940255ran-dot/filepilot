"""带自愈重试的 cargo 调用。

## 为什么需要它

本机杀软会对**刚写出来的文件**短暂加锁，cargo 写 `target/` 下的派生文件
（`deps/*.d`、`.fingerprint/**/dep-*`）时会撞 `os error 5`，
构建就此失败——而**失败点每次都不同**，看起来像随机的。

## 怎么处理

1. **先直接重试。** 锁是瞬时的，多数情况下第二次就过；
2. 仍然失败时，只删**报错里点到的那一个文件**，再试；
3. 每次之间停 1 秒——扫描本身是毫秒级的，拖住它的是排队。

刻意只删单个文件、不删目录：本机的安全删除闸门对「一次删超过 50 个文件」
会要求确认，而 `.fingerprint/<crate>/` 这类目录动辄上千个文件。
删目录会被拦住，整条重试链就死在那里——真正被锁的往往只有一个文件。

## 用法

    python scripts/cargo_retry.py <最多尝试次数> -- <cargo 参数...>

退出码是最后一次 cargo 的退出码。成功时把 cargo 的原始输出透传出来，
调用方仍能照常解析测试结果。注意 stdout/stderr 是分开写的：
测试结果在 stdout，cargo 自己的进度在 stderr，**别把两者拼起来再正则匹配**——
`Running ...`（stderr）与 `test result: ...`（stdout）的先后顺序会错位。
"""

import os
import re
import subprocess
import sys
import time

MSVC = (
    r"C:\Program Files\Microsoft Visual Studio\2022\Community"
    r"\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64"
)


def cargo_env() -> dict:
    env = dict(os.environ)
    env["PATH"] = MSVC + os.pathsep + env["PATH"]
    return env


def locked_files(output: str) -> list[str]:
    """从 cargo 报错里挑出「写入被拒」的具体**文件**。"""
    found: list[str] = []
    for pattern in (r"dep info at:\s*(\S+\.d)", r"failed to write `([^`]+)`"):
        for match in re.finditer(pattern, output):
            found.append(match.group(1))

    seen: set[str] = set()
    unique: list[str] = []
    for path in found:
        if path not in seen:
            seen.add(path)
            unique.append(path)
    return unique


def remove_one(path: str) -> bool:
    """删掉**单个文件**。目录一律不碰——见模块文档。"""
    if not os.path.isfile(path):
        return False
    try:
        os.remove(path)
        return True
    except OSError:
        return False


def main() -> int:
    attempts = int(sys.argv[1])
    cargo_args = sys.argv[sys.argv.index("--") + 1 :]

    env = cargo_env()
    for attempt in range(1, attempts + 1):
        proc = subprocess.run(
            ["cargo", *cargo_args],
            capture_output=True,
            text=True,
            errors="replace",
            env=env,
        )

        stdout = re.sub(r"\x1b\[[0-9;]*m", "", proc.stdout)
        stderr = re.sub(r"\x1b\[[0-9;]*m", "", proc.stderr)

        if proc.returncode == 0:
            print(f"[cargo_retry] 第 {attempt} 次尝试成功", file=sys.stderr)
            sys.stdout.write(stdout)
            sys.stderr.write(stderr)
            return 0

        combined = stdout + stderr
        if "os error 5" not in combined or attempt == attempts:
            # 不是杀软锁，或者已经用完了重试次数：如实返回，别把真错误吞掉。
            sys.stdout.write(stdout)
            sys.stderr.write(stderr)
            return proc.returncode

        removed = sum(1 for path in locked_files(combined) if remove_one(path))
        print(
            f"[cargo_retry] 第 {attempt} 次撞 os error 5，"
            f"删掉 {removed} 个被锁文件后重试",
            file=sys.stderr,
        )
        time.sleep(1)

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
