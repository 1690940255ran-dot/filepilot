# 生成 IPC 契约（TypeScript 类型 + JSON Schema）
#
# 真源是 Rust 类型，见 docs/DECISIONS.md ADR-006。
# 生成的产物**不允许手工修改**：git 里改动它会让 check-contracts.ps1 失败。

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot 'src-tauri/Cargo.toml'

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host ''
    Write-Host '错误：找不到 cargo。契约生成需要 Rust 工具链。' -ForegroundColor Red
    Write-Host '安装：https://rustup.rs  （或运行 scripts/setup-toolchain.ps1 查看本机所需步骤）'
    Write-Host '未生成任何文件，且**不得**用未验证的文件冒充生成结果。'
    exit 2
}

if (-not (Test-Path $manifest)) {
    Write-Host "错误：找不到 $manifest" -ForegroundColor Red
    exit 2
}

Push-Location $repoRoot
try {
    Write-Host "正在生成契约（cargo run --bin export-contracts）..."
    & cargo run --manifest-path $manifest --bin export-contracts
    if ($LASTEXITCODE -ne 0) {
        Write-Host "错误：契约生成器退出码 $LASTEXITCODE" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

Write-Host '契约生成完成。' -ForegroundColor Green
exit 0
