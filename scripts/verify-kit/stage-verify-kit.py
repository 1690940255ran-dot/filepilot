"""把 T17 验收包暂存到 ASCII 路径，便于传给虚拟机。

为什么是 ASCII 路径：虚拟机里大概率是没有配好中文环境的干净系统，
非 ASCII 路径在命令行/PowerShell 里最容易出编码问题。
本脚本把安装包 + 收集脚本 + 操作步骤拷到 C:\\fp-verify\\，并按需起一个
只绑本机 LAN 地址的 HTTP 服务（虚拟机不装 VMware Tools 也能取文件）。
"""

import hashlib
import shutil
from pathlib import Path

import sys


# 控制台编码：Windows 上 Python 的 stdout 默认跟随系统代码页
# （CI runner 是 cp1252、中文系统是 GBK），而本脚本会打印中文，
# 未设置时直接抛 UnicodeEncodeError，让整条命令失败。
# 见 docs/POST_RELEASE_TODO.md 的 CI-003。
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

# 相对本脚本定位仓库根（`scripts/verify-kit/` → 上两级）。
# `STAGE` 是**要交给虚拟机的暂存目录**，故意放在仓库外、且是纯 ASCII 路径。
STAGE = Path(r"C:\fp-verify")
PROJECT = Path(__file__).resolve().parents[2]
STAGING = PROJECT / "tmp" / "fp-verify-staging"

INSTALLER = PROJECT / "src-tauri" / "target" / "release" / "bundle" / "nsis" / "FilePilot_0.1.0_x64-setup.exe"
SCRIPT = PROJECT / "scripts" / "verify-clean-machine.ps1"
STEPS = STAGING / "操作步骤.txt"

EXPECTED_SHA256 = "3e1ecb686fc740d212642c38125960fb955d4a75ff9487d32bb12dc1061ee8b0"

STAGE.mkdir(parents=True, exist_ok=True)

# 1) 安装包
target_installer = STAGE / INSTALLER.name
shutil.copy2(INSTALLER, target_installer)
digest = hashlib.sha256(target_installer.read_bytes()).hexdigest()
print(f"安装包: {target_installer}  {target_installer.stat().st_size} 字节")
print(f"  SHA-256: {digest}")
print(f"  与发布说明一致: {digest == EXPECTED_SHA256}")

# 1b) 上一次跑出来的报告不能被带进这一轮：脚本是**追加**写 report.txt 的，
#     留着旧报告会让「这次验收的结果」和上一次混在一起。
ARCHIVE = PROJECT / "tmp" / "verify-kit-reports"
for stale in (STAGE / "report.txt", STAGE / "report.json"):
    if stale.is_file():
        ARCHIVE.mkdir(parents=True, exist_ok=True)
        shutil.move(str(stale), str(ARCHIVE / stale.name))
        print(f"旧报告已归档（不删除）: {ARCHIVE / stale.name}")

# 2) 收集脚本（保持 UTF-8 BOM，PowerShell 5.1 才不会把中文读成乱码）
target_script = STAGE / SCRIPT.name
data = SCRIPT.read_bytes()
if not data.startswith(b"\xef\xbb\xbf"):
    data = b"\xef\xbb\xbf" + data
target_script.write_bytes(data)
print(f"脚本:   {target_script}  {target_script.stat().st_size} 字节  BOM={data[:3] == b'\xef\xbb\xbf'}")

# 3) 操作步骤与顺手用的启动脚本（都带 BOM，任何编辑器都能正常显示中文）
for name in ("操作步骤.txt", "run-env-check.cmd"):
    source = STAGING / name
    if not source.is_file():
        print(f"[skip] 暂存源里没有 {name}")
        continue
    raw = source.read_bytes()
    # .cmd 不要加 BOM：cmd.exe 会把 BOM 当成命令的一部分
    (STAGE / name).write_bytes(raw if name.endswith(".cmd") else b"\xef\xbb\xbf" + raw)
    print(f"文件:   {STAGE / name}  {(STAGE / name).stat().st_size} 字节")

# 4) 哈希文件（给习惯手工核对的人）
(STAGE / "SHA256.txt").write_text(
    f"{digest}  {INSTALLER.name}\n", encoding="utf-8"
)
print(f"哈希文件: {STAGE / 'SHA256.txt'}")

print()
print("=== 目录内容 ===")
for p in sorted(STAGE.iterdir()):
    print(f"  {p.name}  {p.stat().st_size} 字节")
