# 工具链自检
#
# 规格 T00 要求：读取官方 Tauri Windows 前置要求，检查 Node LTS、pnpm、Rust、
# MSVC Build Tools、WebView2。缺失项记录清楚；需要系统安装权限时才请求具体权限。
#
# 本脚本**只检查，不安装**。理由：安装 MSVC Build Tools 需要管理员权限，
# 修改系统级组件不应该由一个自检脚本静默完成。

$ErrorActionPreference = 'Continue'

function Report($name, $ok, $detail, $fix) {
    $mark = if ($ok) { '[OK]  ' } else { '[缺失]' }
    $color = if ($ok) { 'Green' } else { 'Red' }
    Write-Host ("{0} {1,-24} {2}" -f $mark, $name, $detail) -ForegroundColor $color
    if (-not $ok -and $fix) {
        Write-Host ("        -> {0}" -f $fix) -ForegroundColor Yellow
    }
}

# 「装了但本会话看不见」既不是缺失也不是就绪，单独一类。
# 它不该让自检整体失败——否则用户会以为是自己装错了。
function Warn($name, $detail, $fix) {
    Write-Host ("{0} {1,-24} {2}" -f '[注意]', $name, $detail) -ForegroundColor Yellow
    if ($fix) {
        Write-Host ("        -> {0}" -f $fix) -ForegroundColor Yellow
    }
    $script:sessionStale = $true
}

Write-Host ''
Write-Host 'FilePilot 工具链自检' -ForegroundColor Cyan
Write-Host ('-' * 72)

$missing = 0
# 是否存在「已安装但当前会话看不见」的组件（典型场景：装完 rustup 却复用了旧终端）
$sessionStale = $false

# ---- Node ----
$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
    $nodeVersion = (& node --version) 2>$null
    Report 'Node.js' $true "$nodeVersion  ($($node.Source))" $null
} else {
    Report 'Node.js' $false '未找到' '安装 Node.js LTS 22.x：https://nodejs.org'
    $missing++
}

# ---- pnpm ----
$pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
if ($pnpm) {
    $pnpmVersion = (& pnpm --version) 2>$null
    Report 'pnpm' $true "$pnpmVersion  ($($pnpm.Source))" $null
} else {
    Report 'pnpm' $false '未找到' 'corepack enable pnpm   或   npm i -g pnpm@10'
    $missing++
}

# ---- Rust ----
# 注意：PATH 变更只对**新打开**的终端生效。如果用户在装完 rustup 后直接复用
# 已经开着的终端，`Get-Command cargo` 会是空的，但工具其实已经装好了。
# 这里显式区分「没装」和「装了但本会话 PATH 没刷新」，避免给出误导性的安装建议。
$cargoHome = Join-Path $env:USERPROFILE '.cargo\bin'
$cargoExe = Join-Path $cargoHome 'cargo.exe'
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargo) {
    $cargoVersion = (& cargo --version) 2>$null
    $rustcVersion = (& rustc --version) 2>$null
    Report 'Rust (cargo)' $true "$cargoVersion" $null
    Report 'Rust (rustc)' $true "$rustcVersion" $null
} elseif (Test-Path $cargoExe) {
    Warn 'Rust (cargo)' "已安装于 $cargoHome，但当前会话 PATH 未刷新" '关掉这个终端重新打开（PATH 变更不影响已打开的会话）'
} else {
    Report 'Rust (cargo)' $false '未找到（src-tauri 无法构建）' '安装 rustup：https://rustup.rs  （安装后重开终端）'
    $missing++
}
$rustfmtExe = Join-Path $cargoHome 'rustfmt.exe'
$rustfmt = Get-Command rustfmt -ErrorAction SilentlyContinue
if (-not $rustfmt -and (Test-Path $rustfmtExe)) { $rustfmt = Get-Item $rustfmtExe }
Report 'rustfmt' ([bool]$rustfmt) $(if ($rustfmt) { '已安装' } else { '未找到' }) 'rustup component add rustfmt'
$clippyExe = Join-Path $cargoHome 'cargo-clippy.exe'
$clippy = Get-Command cargo-clippy -ErrorAction SilentlyContinue
if (-not $clippy -and (Test-Path $clippyExe)) { $clippy = Get-Item $clippyExe }
Report 'clippy' ([bool]$clippy) $(if ($clippy) { '已安装' } else { '未找到' }) 'rustup component add clippy'

# ---- MSVC C++ 生成工具 ----
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$msvcPath = $null
if (Test-Path $vswhere) {
    $msvcPath = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
}
if (-not $msvcPath) {
    # 退路：直接找 cl.exe
    $cl = Get-ChildItem 'C:\Program Files\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx64\x64\cl.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cl) { $msvcPath = $cl.FullName }
}
if ($msvcPath) {
    Report 'MSVC C++ 工具集' $true $msvcPath $null
} else {
    Report 'MSVC C++ 工具集' $false '未找到 cl.exe（Rust 的 MSVC 目标无法链接）' @'
以管理员身份运行 PowerShell，执行：
          winget install --id Microsoft.VisualStudio.2022.BuildTools --accept-source-agreements --accept-package-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
'@
    $missing++
}

# ---- Windows SDK ----
$sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\Include' -Directory -ErrorAction SilentlyContinue
if ($sdk) {
    Report 'Windows SDK' $true (($sdk | ForEach-Object { $_.Name }) -join ', ') $null
} else {
    Report 'Windows SDK' $false '未找到（通常随 MSVC 工作负载一起安装）' '安装 MSVC 工作负载时会一并安装'
    $missing++
}

# ---- WebView2 ----
$webviewKeys = @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
)
$webviewVersion = $null
foreach ($key in $webviewKeys) {
    if (Test-Path $key) {
        $webviewVersion = (Get-ItemProperty $key).pv
        if ($webviewVersion) { break }
    }
}
if ($webviewVersion) {
    Report 'WebView2 Runtime' $true $webviewVersion $null
} else {
    Report 'WebView2 Runtime' $false '未找到' '安装：https://developer.microsoft.com/microsoft-edge/webview2/'
    $missing++
}

Write-Host ('-' * 72)
if ($missing -eq 0 -and $sessionStale) {
    Write-Host '依赖都已安装，但当前终端的 PATH 是旧快照。' -ForegroundColor Yellow
    Write-Host '关掉本终端重新打开，即可直接使用 cargo / rustc。' -ForegroundColor Yellow
} elseif ($missing -eq 0) {
    Write-Host '全部依赖就绪。可以运行：pnpm typecheck && cargo check --manifest-path src-tauri/Cargo.toml' -ForegroundColor Green
} else {
    Write-Host "有 $missing 项缺失。上面的 -> 行给出了对应安装方式。" -ForegroundColor Yellow
    Write-Host '注意：本脚本只检查，不安装。MSVC Build Tools 需要管理员权限，请自行确认后再装。' -ForegroundColor Yellow
}
Write-Host ''

if ($missing -gt 0) { exit 1 }
exit 0
