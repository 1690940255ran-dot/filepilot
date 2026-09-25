param(
    [ValidateSet('env', 'install', 'uninstall')]
    [string]$Phase = 'env',
    [string]$Installer = '',
    [string]$InstallDir = "$env:LOCALAPPDATA\FilePilot",
    [string]$ExpectedSha256 = '',
    [string]$OutDir = '',
    # 在*非干净*机器（例如开发机）上也想跑一遍时用它。
    # 默认闸门是：机器不干净 → 拒绝安装。理由是「在开发机上装成功」这件事
    # 没有证明力；但有时你就想验证这个脚本本身能不能跑通，那就显式认领：
    # 结果是**平台探测/冒烟**，不是 T17 验收。报告里会写明这一点。
    [switch]$AllowDirty,
    # 改脚本的过程中反复试跑时用它：报告写到 report-draft.txt 而不是 report.txt。
    #
    # 为什么需要：报告是**追加**写的，开发期的中间态会与正式结论混在同一个文件里。
    # 2026-09-25 就发生过——一条改到一半时跑出来的「应用启动后崩溃」留在报告里，
    # 交回时被当成真缺陷排查（见 docs/POST_RELEASE_TODO.md CL-001）。
    # 改脚本的人应该主动加这个开关，别让半成品记录混进交付物。
    [switch]$Draft
)

# T17 干净机器验收：能自动化的部分全在这里。
#
# 用法（在**干净机器**上，普通用户权限即可）：
#   powershell -ExecutionPolicy Bypass -File .\verify-clean-machine.ps1 -Phase env
#   powershell -ExecutionPolicy Bypass -File .\verify-clean-machine.ps1 -Phase install
#   ...  人工做界面流程（见同目录 操作步骤.txt）  ...
#   powershell -ExecutionPolicy Bypass -File .\verify-clean-machine.ps1 -Phase uninstall
#
# 每次追加写 report.txt（UTF-8），并把同样的内容 JSON 化到 report.json。
# 加 -Draft 则写 report-draft.txt / report-draft.json，用于开发期试跑。
#
# 三条设计原则：
#   1. **不碰系统设置**：不改 PATH、不关 Defender、不动注册表里跟本应用无关的东西；
#   2. **只报告事实**：每项输出「实测值 + 判定」，判定不出来的写「未能判定」；
#   3. **不改用户的文件**：验收目录由调用方指定，脚本只读它。

$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($Installer)) {
    $Installer = Join-Path $Root 'FilePilot_0.1.0_x64-setup.exe'
}
if ([string]::IsNullOrWhiteSpace($OutDir)) { $OutDir = $Root }
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
$ReportBase = if ($Draft) { 'report-draft' } else { 'report' }
$ReportTxt = Join-Path $OutDir ($ReportBase + '.txt')
$ReportJson = Join-Path $OutDir ($ReportBase + '.json')

$script:Findings = @()
$script:HasFailure = $false

function Say {
    param([string]$Text = '')
    Write-Host $Text
    Add-Content -Path $ReportTxt -Value $Text -Encoding utf8
}

function Record {
    param([string]$Item, [string]$Value, [string]$Verdict)
    $script:Findings += [pscustomobject]@{ item = $Item; value = $Value; verdict = $Verdict }
    if ($Verdict -like '失败*') { $script:HasFailure = $true }
    Say ("  [{0}] {1} = {2}" -f $Verdict, $Item, $Value)
}

function Check-Clean {
    Say ''
    Say '=== 1. 环境审计：这台机器是不是「干净」的 ==='
    $probes = @('node', 'npm', 'cargo', 'rustc', 'rustup', 'python', 'python3', 'git', 'code')
    $found = @()
    foreach ($p in $probes) {
        $cmd = Get-Command $p -ErrorAction SilentlyContinue
        if (-not $cmd) { continue }
        # Windows 11 可能自带 python/python3 的 Microsoft Store 执行别名；
        # 它不是已安装的 Python，不能让一台干净机器被误判为开发机。
        $windowsApps = Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps'
        if ($p -in @('python', 'python3') -and
            [System.IO.Path]::GetDirectoryName($cmd.Source) -eq $windowsApps -and
            [System.IO.Path]::GetFileName($cmd.Source) -in @('python.exe', 'python3.exe')) {
            Record 'Python 执行别名' ("{0} -> {1}" -f $p, $cmd.Source) '记录（非开发工具）'
            continue
        }
        $found += ("{0} -> {1}" -f $p, $cmd.Source)
    }
    if ($found.Count -eq 0) {
        Record '开发工具' '一个都没有' '通过（干净）'
        Record 'T17 前提（无开发环境）' 'node/cargo/python 等均不存在' '满足'
    } else {
        foreach ($f in $found) { Record '开发工具' $f '警告（不干净）' }
        Record 'T17 前提（无开发环境）' ("检测到 {0} 个开发工具" -f $found.Count) '不满足：本次结果只能记为「平台探测」，不计入 T17 验收'
    }

    $os = Get-CimInstance Win32_OperatingSystem
    Record '系统' ("{0} build {1}" -f $os.Caption, $os.BuildNumber) '记录'
    Record '架构' $os.OSArchitecture '记录'

    # T17 的**前提**是「Windows 11 x64 + 没有开发环境」。前提不满足时，
    # 后面所有结果都不该被当成 T17 验收结论——所以在报告里直接判定，
    # 不靠人记着。（Windows 11 的 build 号从 22000 起；Windows 10 是 10240~21999）
    $build = [int]$os.BuildNumber
    $isWin11 = $build -ge 22000
    $is64 = "$($os.OSArchitecture)" -like '*64*'
    if ($isWin11 -and $is64) {
        Record 'T17 前提（Windows 11 x64）' ("{0} / 64 位" -f $os.Caption) '满足'
    } elseif (-not $isWin11) {
        Record 'T17 前提（Windows 11 x64）' ("{0} build {1}" -f $os.Caption, $build) '不满足：本次结果只能记为「平台探测」，不计入 T17 验收'
    } else {
        Record 'T17 前提（Windows 11 x64）' $os.OSArchitecture '不满足：需要 x64'
    }

    $cs = Get-CimInstance Win32_ComputerSystem
    Record '内存' ("{0:N1} GB" -f ($cs.TotalPhysicalMemory / 1GB)) '记录'
    $disk = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='C:'"
    if ($disk) { Record 'C: 可用' ("{0:N1} GB" -f ($disk.FreeSpace / 1GB)) '记录' }

    # WebView2：Windows 11 通常自带，这里记录实际版本（G 节要用）
    # 不看具体 GUID——那是随版本变的，改成「枚举客户端里名字含 WebView2 的」
    # 加「安装目录是否存在」两条独立判据
    $wvVersion = $null
    try {
        $clients = Get-ChildItem 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients' -ErrorAction Stop
        foreach ($c in $clients) {
            $p = Get-ItemProperty -Path $c.PSPath -ErrorAction SilentlyContinue
            if ($p.name -like '*WebView2*' -and $p.pv) { $wvVersion = $p.pv; break }
        }
    } catch { }
    $wvDir = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
    $wvFiles = @()
    if (Test-Path $wvDir) { $wvFiles = Get-ChildItem $wvDir -Directory -ErrorAction SilentlyContinue }
    Record 'WebView2 版本（注册表）' ($(if ($wvVersion) { $wvVersion } else { '未找到' })) '记录'
    Record 'WebView2 安装目录' ($(if ($wvFiles.Count -gt 0) { ($wvFiles | ForEach-Object { $_.Name }) -join ',' } else { '未找到' })) '记录'

    # WebView2 缺失不是缺陷，而是一条**要抓住的测试机会**：
    # 这时安装会走 `downloadBootstrapper` 分支（需要联网）——正是 G 节第 2 条。
    # Windows 11 通常自带 WebView2，所以这一幕在 Win11 上几乎造不出来；
    # 在 Windows 10（往往不带）或手工卸载之后才能看到。
    if (-not $wvVersion -and $wvFiles.Count -eq 0) {
        Record 'WebView2 缺失' '是' '注意：本次安装会走 downloadBootstrapper（**需要联网**），请记录实际提示文案（G 节第 2 条）'
    } else {
        Record 'WebView2 缺失' '否（本机已装）' '说明：G 节「缺失」那一格在 Windows 11 上测不到，需要先卸载 WebView2 或另找一台机器'
    }
    return ($found.Count -eq 0)
}

function Check-Installer {
    Say ''
    Say '=== 2. 安装包来源校验（没有代码签名，这是唯一的来源验证）==='
    if (-not (Test-Path $Installer)) {
        Record '安装包' $Installer '失败（文件不存在）'
        return $false
    }
    $item = Get-Item $Installer
    $hash = (Get-FileHash -Path $Installer -Algorithm SHA256).Hash.ToLower()
    Record '安装包体积' ("{0} 字节" -f $item.Length) '记录'
    Record '安装包 SHA-256' $hash '记录'
    if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        Record '哈希比对' '调用方未提供期望值' '未能判定'
    } elseif ($hash -eq $ExpectedSha256.ToLower()) {
        Record '哈希比对' '与期望一致' '通过'
    } else {
        Record '哈希比对' ("与期望不一致（期望 {0}）" -f $ExpectedSha256) '失败'
        return $false
    }
    return $true
}

function Do-Install {
    Say ''
    Say '=== 3. 静默安装（/S，用户级）==='
    try {
        $proc = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru -Wait -ErrorAction Stop
    } catch {
        Record '安装结果' $_.Exception.Message '失败（安装程序未能启动）'
        return
    }
    Record '安装退出码' $proc.ExitCode '记录'
    if ($proc.ExitCode -ne 0) { Record '安装结果' '非零退出' '失败'; return }

    Start-Sleep -Seconds 3
    if (Test-Path $InstallDir) { Record '安装目录' $InstallDir '记录' } else { Record '安装目录' $InstallDir '失败（不存在）'; return }

    $files = Get-ChildItem $InstallDir -File -ErrorAction SilentlyContinue
    foreach ($f in $files) { Record ('安装文件 ' + $f.Name) ("{0} 字节" -f $f.Length) '记录' }

    $main = Join-Path $InstallDir 'filepilot.exe'
    $worker = Join-Path $InstallDir 'extract_worker.exe'
    Record '主程序' $(if (Test-Path $main) { '存在' } else { '缺失' }) $(if (Test-Path $main) { '通过' } else { '失败' })
    # 规格 T16：「打包解析 worker 并验证可找到」——这一条必须在真机上成立
    Record '解析工作进程与主程序同级' $(if (Test-Path $worker) { ("存在 {0} 字节" -f (Get-Item $worker).Length) } else { '缺失' }) $(if (Test-Path $worker) { '通过' } else { '失败' })

    # 快捷方式与卸载登记项
    $startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $shortcut = Get-ChildItem $startMenu -Recurse -Filter '*FilePilot*' -ErrorAction SilentlyContinue
    Record '开始菜单快捷方式' $(if ($shortcut) { ($shortcut | ForEach-Object { $_.Name }) -join ',' } else { '未找到' }) '记录'

    $reg = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FilePilot'
    if (Test-Path $reg) {
        $u = Get-ItemProperty -Path $reg
        Record '卸载登记项' ("DisplayName={0}; InstallLocation={1}" -f $u.DisplayName, $u.InstallLocation) '记录'
    } else {
        Record '卸载登记项' '未找到' '记录'
    }

    Say ''
    Say '=== 4. 启动检查（进程 + 主窗口标题）==='
    $before = Get-Process -Name 'filepilot' -ErrorAction SilentlyContinue
    if ($before) { Record '启动前已有实例' ($before.Count) '警告' }
    $app = Start-Process -FilePath $main -PassThru
    Start-Sleep -Seconds 20
    $live = Get-Process -Id $app.Id -ErrorAction SilentlyContinue
    if ($live) {
        Record '进程存活' ('pid=' + $app.Id) '通过'
        $live.Refresh()
        Record '主窗口标题' ($(if ($live.MainWindowTitle) { $live.MainWindowTitle } else { '（空）' })) $(if ($live.MainWindowTitle) { '通过（窗口已创建）' } else { '警告（无窗口标题）' })
        Record '内存占用' ("{0:N0} MB" -f ($live.WorkingSet64 / 1MB)) '记录'
    } else {
        Record '进程存活' '已退出' '失败（应用启动后崩溃）'
    }
    Say ''
    Say '注意：脚本**不能**判断界面是不是白屏——那要看人眼。'
    Say '      应用保持运行与安装，请现在按同目录的「操作步骤.txt」做 C/D/E/F/G 节，'
    Say '      验收完再执行 -Phase uninstall（它会先正常关闭应用）。'
}

function Do-Uninstall {
    Say ''
    Say '=== 5. 卸载 ==='
    $uninstaller = Join-Path $InstallDir 'uninstall.exe'
    if (-not (Test-Path $uninstaller)) {
        # 「没装过」不是失败，是「没有可判定对象」。写成失败会让人以为卸载坏了。
        Record '卸载器' ("{0} 不存在" -f $uninstaller) '未能判定（本次没有安装过，或已被卸载）'
        return
    }

    # 卸载前先把应用关掉。**先优雅关闭**（CloseMainWindow），因为：
    #  - install 阶段结束时应用是故意留开着的（方便做人工验收），
    #    所以「装完直接卸载」这条连续流程必然遇到"应用还在运行"；
    #  - 但强杀会跳过应用自己的收尾（写日志/关数据库），那是我们在别处
    #    专门测的路径，不该在验收流程里顺手制造。
    # 关不掉才判定为失败，让人去处理。
    $running = Get-Process -Name 'filepilot' -ErrorAction SilentlyContinue
    if ($running) {
        Record '卸载前应用状态' ("仍在运行 {0} 个实例，尝试正常关闭" -f @($running).Count) '记录'
        foreach ($p in $running) { $null = $p.CloseMainWindow() }
        Start-Sleep -Seconds 6
        $still = Get-Process -Name 'filepilot' -ErrorAction SilentlyContinue
        if ($still) {
            Record '卸载前应用状态' '未能关闭——请手动退出 FilePilot 后重跑卸载阶段' '失败'
            return
        }
        Record '卸载前应用状态' '已正常关闭' '通过'
    } else {
        Record '卸载前应用状态' '应用未在运行' '记录'
    }

    $dbPath = Join-Path $InstallDir 'filepilot.db'
    $dbBefore = Test-Path -LiteralPath $dbPath
    $dbHashBefore = if ($dbBefore) { (Get-FileHash -LiteralPath $dbPath -Algorithm SHA256).Hash } else { $null }
    Record '卸载前用户数据' $(if ($dbBefore) { 'filepilot.db 存在' } else { 'filepilot.db 不存在' }) '记录'

    try {
        $proc = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru -Wait -ErrorAction Stop
    } catch {
        Record '卸载结果' $_.Exception.Message '失败（卸载程序未能启动）'
        return
    }
    Record '卸载退出码' $proc.ExitCode '记录'
    Start-Sleep -Seconds 6

    $main = Join-Path $InstallDir 'filepilot.exe'
    $worker = Join-Path $InstallDir 'extract_worker.exe'
    Record '主程序已移除' $(if (-not (Test-Path $main)) { '是' } else { '否' }) $(if (-not (Test-Path $main)) { '通过' } else { '失败' })
    Record '解析工作进程已移除' $(if (-not (Test-Path $worker)) { '是' } else { '否' }) $(if (-not (Test-Path $worker)) { '通过' } else { '失败' })

    # 规格 T17：「卸载仅移除应用程序；默认保留用户文件和操作历史」
    if ($dbBefore) {
        if (Test-Path -LiteralPath $dbPath) {
            $dbHashAfter = (Get-FileHash -LiteralPath $dbPath -Algorithm SHA256).Hash
            if ($dbHashAfter -eq $dbHashBefore) {
                Record '用户数据保留' 'filepilot.db 存在且 SHA-256 未变' '通过'
            } else {
                Record '用户数据保留' 'filepilot.db 仍在，但卸载期间内容改变' '失败（需调查）'
            }
        } else {
            Record '用户数据保留' '卸载后 filepilot.db 不存在' '失败（数据被删，违背 T17）'
        }
    } else {
        Record '用户数据保留' '卸载前就没有数据库（说明本次验收没跑过执行流程）' '未能判定'
    }
}

# ---------------------------------------------------------------------------

$stamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
Say ''
Say '################################################################'
Say ("# FilePilot T17 干净机器验收  phase={0}  {1}" -f $Phase, $stamp)
if ($Draft) {
    Say '#'
    Say '# 【草案】本次带 -Draft 运行，结果写到 report-draft.txt。'
    Say '#   开发期试跑用，**不是**交付给验收方的记录，不要与正式报告混看。'
}
Say '################################################################'

switch ($Phase) {
    'env' {
        Check-Clean | Out-Null
        # 说清楚这一阶段**不做什么**：它是只读的。
        # 否则"跑完了但应用没打开"会被当成脚本坏了（真实发生过）。
        Say ''
        Say '--- 本次是【只读检查】：没有安装、也没有启动应用 ---'
        Say '    要安装并启动应用，请执行：'
        Say '      powershell -ExecutionPolicy Bypass -File .\verify-clean-machine.ps1 `'
        Say '          -Phase install -ExpectedSha256 <发布页上的哈希>'
        Say '    （在不干净的机器上试跑再加 -AllowDirty，结果只算平台探测。）'
    }
    'install' {
        $clean = Check-Clean
        if (-not $clean -and -not $AllowDirty) {
            Record '干净机器前提' '检测到开发工具，已停止安装阶段' '失败（环境不符合 T17）'
            Say ''
            Say '如果你只是想验证这个脚本能不能跑通（不是 T17 验收），加 -AllowDirty 重跑；'
            Say '那样跑出来的结果会记为「平台探测 / 冒烟」，不计入 T17。'
        } else {
            if (-not $clean) {
                Record '本次性质' '在非干净机器上运行（-AllowDirty）' '平台探测/冒烟：**不计入 T17 验收**'
            }
            if (Check-Installer) { Do-Install }
        }
    }
    'uninstall' {
        if (-not (Test-Path $ReportTxt)) { Say '(提示：之前没跑过 install 阶段)' }
        Do-Uninstall
    }
}

Say ''
Say '=== 本次判定汇总 ==='
# 不用 {0,-28} 对齐：中文字符在等宽字体里占两格，按字符数补空格反而更乱
foreach ($f in $script:Findings) {
    Say ("  - [{0}] {1}：{2}" -f $f.verdict, $f.item, $f.value)
}

try {
    $script:Findings | ConvertTo-Json -Depth 4 | Out-File -FilePath $ReportJson -Encoding utf8
} catch {
    Say ('JSON 写出失败: ' + $_.Exception.Message)
}

Say ''
Say ("报告已追加写入: {0}" -f $ReportTxt)
Say '把 report.txt 和截图一起交回即可。'
if ($script:HasFailure) { exit 1 }
