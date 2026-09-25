#!/usr/bin/env bash
# 为 cargo / rustc 准备 MSVC 工具链环境（本机专用，需要 `source` 使用）。
#
# 用法：
#   source scripts/msvc-env.sh && cargo test --locked
#
# 为什么需要这个脚本
# ------------------
# 本机安装了 VS 2022 生成工具的**文件**（cl.exe、link.exe、MSVC 头文件与库、
# Windows SDK 都在），但该组件没有登记进 vswhere 的组件清单：
#
#   vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64
#   （无输出）
#
# 于是 rustc 的 MSVC 探测失败，退回到 PATH 上的同名程序 —— 而 Git for Windows
# 的 /usr/bin 里正好有一个 link.exe（coreutils 的硬链接工具），
# 它会把 rustc 传来的参数当成文件名，报出 "extra operand"。
#
# 正常做法是调用 vcvars64.bat，但从 bash 里调 cmd.exe 被本环境的策略拦下，
# 所以这里显式导出 vcvars64.bat 会设置的那几组变量。
#
# 关键细节：`INCLUDE`、`LIB` 与 linker 路径必须是 **Windows 形式**（反斜杠、
# 分号分隔）。Git Bash 只会替子进程转换 `PATH`，其他变量原样传出去，
# 写成 `/c/Program Files/...` 会让 cl.exe / link.exe 找不到路径。
#
# 只影响当前 shell，不修改系统环境变量。

set -u

# 同一套目录的两种写法：bash 形式用来枚举版本，Windows 形式用来传给 MSVC 工具。
_VC_BASH='/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC'
_VC_WIN='C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC'
_SDK_BASH='/c/Program Files (x86)/Windows Kits/10'
_SDK_WIN='C:\Program Files (x86)\Windows Kits\10'

if [ ! -d "$_VC_BASH" ]; then
  echo "msvc-env: 找不到 MSVC 工具目录 $_VC_BASH" >&2
  return 1 2>/dev/null || exit 1
fi

# 版本号动态取最大，避免把版本写死在脚本里。
#
# **不能用 `ls | sort -V | tail -n 1`**：本机 PATH 上可能既没有 GNU coreutils，
# 又有 Windows 自带的 `sort.exe`（它不认 `-V`）。那样 version 会是空串，
# 路径被拼成 `...\MSVC\\bin\Hostx64\x64\link.exe`，报出极具误导性的
# 「linker not found」。这里改成纯 bash，不依赖任何外部命令。

# 判断 $1 是否比 $2 新（点分数字，逐段比较）。
_version_gt() {
  local IFS=.
  # shellcheck disable=SC2206  # 有意按点拆分
  local -a left=($1) right=($2)
  local i count=${#left[@]}
  [ "${#right[@]}" -gt "$count" ] && count=${#right[@]}
  for ((i = 0; i < count; i++)); do
    # 10# 强制十进制，避免 08 被当成八进制而报错
    local a=10#${left[i]:-0} b=10#${right[i]:-0}
    if ((a > b)); then return 0; fi
    if ((a < b)); then return 1; fi
  done
  return 1
}

# 取目录下版本号最大的子目录名。
_latest_child_dir() {
  local base="$1" best="" candidate name
  for candidate in "$base"/*; do
    [ -d "$candidate" ] || continue
    name="${candidate##*/}"
    if [ -z "$best" ] || _version_gt "$name" "$best"; then
      best="$name"
    fi
  done
  printf '%s' "$best"
}

_MSVC_VERSION="$(_latest_child_dir "$_VC_BASH")"
_SDK_VERSION="$(_latest_child_dir "$_SDK_BASH/Include")"

if [ -z "$_MSVC_VERSION" ]; then
  echo "msvc-env: $_VC_BASH 下没有版本子目录" >&2
  return 1 2>/dev/null || exit 1
fi
if [ -z "$_SDK_VERSION" ]; then
  echo "msvc-env: $_SDK_BASH/Include 下没有版本子目录" >&2
  return 1 2>/dev/null || exit 1
fi

# 提前确认 linker 真的存在。缺了就在**这里**失败并说清楚，
# 而不是让 cargo 报一句指向空路径的 "link.exe not found"。
_LINKER_BASH="$_VC_BASH/$_MSVC_VERSION/bin/Hostx64/x64/link.exe"
if [ ! -x "$_LINKER_BASH" ]; then
  echo "msvc-env: 找不到 linker $_LINKER_BASH" >&2
  return 1 2>/dev/null || exit 1
fi

export PATH="$_VC_BASH/$_MSVC_VERSION/bin/Hostx64/x64:$PATH"
export PATH="$_SDK_BASH/bin/$_SDK_VERSION/x64:$PATH"

export INCLUDE="$_VC_WIN\\$_MSVC_VERSION\\include"
export INCLUDE="$INCLUDE;$_SDK_WIN\\Include\\$_SDK_VERSION\\ucrt"
export INCLUDE="$INCLUDE;$_SDK_WIN\\Include\\$_SDK_VERSION\\um"
export INCLUDE="$INCLUDE;$_SDK_WIN\\Include\\$_SDK_VERSION\\shared"

export LIB="$_VC_WIN\\$_MSVC_VERSION\\lib\\x64"
export LIB="$LIB;$_SDK_WIN\\Lib\\$_SDK_VERSION\\ucrt\\x64"
export LIB="$LIB;$_SDK_WIN\\Lib\\$_SDK_VERSION\\um\\x64"

# 明确指定 linker，避免再次落到 PATH 上的同名程序。
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER="$_VC_WIN\\$_MSVC_VERSION\\bin\\Hostx64\\x64\\link.exe"

# 让 MSVC 工具链输出英文消息。
# 中文系统的 link.exe 会用 OEM 编码打印"正在创建库 …"，rustc 把它当成
# 非 UTF-8 输出并给出 `linker_messages` 警告；在 `-D warnings` 下会直接失败。
export VSLANG=1033

echo "msvc-env: MSVC $_MSVC_VERSION / Windows SDK $_SDK_VERSION 已就绪"
