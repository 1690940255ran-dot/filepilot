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
    if (-not (Test-Path -LiteralPath $path)) {
        return $null
    }
    # **不要用 `Get-FileHash`。**
    #
    # 它属于 `Microsoft.PowerShell.Utility` 模块。本脚本由
    # `pnpm contracts:check` 以 `powershell -NoProfile -ExecutionPolicy Bypass -File`
    # 启动，而从 node/pnpm 派生的进程环境里 `PSModulePath` 没有正确继承到
    # 系统模块目录 —— 于是 cmdlet 自动加载失败：
    #
    #   Get-FileHash : The term 'Get-FileHash' is not recognized as the name of a
    #   cmdlet, function, script file, or operable program.
    #   At scripts/check-contracts.ps1:29 char:17
    #
    # 2026-09-27 实测：这个失败在 **CI 上必现**（也因此让整个契约 job 变红），
    # 而它此前被误判为「本机命令环境问题」。直接调 .NET 就没有这个依赖：
    # `System.Security.Cryptography` 属于基础框架，不经过模块自动加载。
    #
    # 返回值与 `Get-FileHash` 保持一致：大写十六进制、无分隔符。
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $stream = [System.IO.File]::OpenRead($path)
        try {
            $bytes = $sha.ComputeHash($stream)
        }
        finally {
            $stream.Dispose()
        }
    }
    finally {
        $sha.Dispose()
    }
    return ([System.BitConverter]::ToString($bytes)).Replace('-', '')
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
