# 契约差异检查
#
# 规格 T01 要求「建立 Rust→TypeScript/Schema 生成和差异检查」。
#
# 本脚本**强制重新生成后再比对**：
#   - 生成器不可用 → 非零退出并报告原因，绝不把现有文件当成「通过」
#   - 重新生成的内容与仓库内文件不一致 → 非零退出，并保留差异供人工查看
#
# 这条设计是刻意的：仓库里的 contracts.generated.ts / contracts.schema.json
# 可能是引导版本（见 docs/DECISIONS.md ADR-007），只有真正跑过生成器才算验证。

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$tsPath = Join-Path $repoRoot 'src/api/contracts.generated.ts'
$schemaPath = Join-Path $repoRoot 'src/api/contracts.schema.json'

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host ''
    Write-Host '契约检查失败：找不到 cargo，无法重新生成契约。' -ForegroundColor Red
    Write-Host '仓库内的契约文件因此**未经生成器验证**。'
    Write-Host '这不是「通过」——请先安装 Rust 工具链（https://rustup.rs）后重跑本命令。'
    exit 2
}

function Get-FileHashOrNull([string]$path) {
    if (Test-Path $path) {
        return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    }
    return $null
}

$beforeTs = Get-FileHashOrNull $tsPath
$beforeSchema = Get-FileHashOrNull $schemaPath

Write-Host '重新生成契约...'
& (Join-Path $PSScriptRoot 'generate-contracts.ps1')
if ($LASTEXITCODE -ne 0) {
    Write-Host "契约检查失败：生成器退出码 $LASTEXITCODE" -ForegroundColor Red
    exit $LASTEXITCODE
}

$afterTs = Get-FileHashOrNull $tsPath
$afterSchema = Get-FileHashOrNull $schemaPath

$failed = $false

if ($beforeTs -ne $afterTs) {
    Write-Host ''
    Write-Host '契约不一致：src/api/contracts.generated.ts' -ForegroundColor Red
    Write-Host "  生成前: $beforeTs"
    Write-Host "  生成后: $afterTs"
    Write-Host '  说明：仓库内的文件不是当前 Rust 类型生成的结果（或有人手工改过它）。'
    Write-Host '  处理：确认 Rust 类型后运行 pnpm contracts:generate，并把结果一并提交。'
    $failed = $true
}

if ($beforeSchema -ne $afterSchema) {
    Write-Host ''
    Write-Host '契约不一致：src/api/contracts.schema.json' -ForegroundColor Red
    Write-Host "  生成前: $beforeSchema"
    Write-Host "  生成后: $afterSchema"
    Write-Host '  处理：运行 pnpm contracts:generate，并把结果一并提交。'
    $failed = $true
}

if ($failed) {
    exit 1
}

Write-Host ''
Write-Host '契约检查通过：生成结果与仓库内文件一致。' -ForegroundColor Green
Write-Host "  contracts.generated.ts  $afterTs"
Write-Host "  contracts.schema.json  $afterSchema"
exit 0
