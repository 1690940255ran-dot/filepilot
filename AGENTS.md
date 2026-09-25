# AGENTS.md — FilePilot 执行约束

本文件只规定**执行约束**，不重复产品需求。唯一规格源是 `docs/MASTER_PLAN.md`。

## 开工前必读顺序

1. `docs/MASTER_PLAN.md` —— 规格全文，冲突时以它为准
2. `docs/PROGRESS.md` —— 找最早的未完成任务
3. `docs/DECISIONS.md` —— 已定型的技术决策（ADR）
4. 本文件

## 绝对禁止

### 文件安全

- 禁止对**真实用户文件**执行删除、覆盖、移动、重命名。所有文件操作测试只能使用 `tempfile::TempDir` 建立的临时目录。
- 禁止把桌面、下载、文档、源码仓库根等真实目录作为测试目标。
- 禁止用 `exists()` 后 `rename()` 实现「不覆盖」——两次调用之间别的进程可以创建目标。必须走 `SetFileInformationByHandle(FileRenameInfo)` 且 `ReplaceIfExists = FALSE`。
- 禁止复制后删除、禁止改变 ACL、禁止移除只读属性、禁止提权绕过失败、禁止回退成覆盖模式。
- 禁止在未取得与当前计划摘要绑定的有效 `validationToken` 时执行文件操作（INV-02）。

### 模型与隐私

- 模型只能产出 `Proposal`（不含绝对路径）。禁止把模型输出直接当作路径、命令或执行指令。
- 云端模式只发送：随机 fileId、文件名、扩展名、≤2000 字符文本摘要、用户要求。禁止发送绝对路径、用户名、完整文件、原图。
- 禁止在日志、崩溃报告、SQLite、前端中写入 API Key。密钥只进 Windows 凭据存储。
- 模型故障不得触发文件写入，不得静默把云模式切换成其他提供商（INV-09）。

### 前端边界

- 禁止前端持有通用 filesystem/shell 权限。
- 禁止加载远程网页与远程脚本；CSP 必须阻止远程导航。
- 安全校验必须在 Rust 侧成立，不能只依赖前端按钮禁用或 capabilities 文件。

### 工程纪律

- 禁止用 `passWithNoTests`、恒成功空函数、假数据演示伪造「通过」。
- 禁止把「未运行的测试」写成「通过」。环境无法执行时如实记为「未运行」。
- 禁止擅自增加登录、云同步、计费、插件市场、自动删除、全盘扫描、浏览器自动化、多智能体框架。
- 禁止用删除数据库的方式掩盖迁移错误。
- 不向用户承诺「任意文件都能理解」「一定可以撤销」「完全离线」，除非条件实际成立。

## 验收命令

在 `filepilot/` 根目录执行。**本机 bash 环境缺少 coreutils（无 `ls`/`head`/`sed`/`date`），必须用 PowerShell 调用 `pnpm.cmd` 与 cargo。**

**Rust 命令必须先加载 MSVC 环境**（见下方环境事实与 `scripts/msvc-env.sh`）：

```bash
source scripts/msvc-env.sh && cd src-tauri && cargo test --locked
```

```powershell
pnpm install --frozen-lockfile
pnpm contracts:check
pnpm typecheck
pnpm lint
pnpm test
pnpm test:e2e
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm desktop:build
```

任一失败都不得声称通过。

## 每轮任务的固定动作

1. 读 PROGRESS，找最早未完成任务，核对前置阶段
2. 读规格对应章节 + 直接依赖模块 + 现有测试
3. 写高风险行为的**失败测试**，确认失败原因正确（不是环境故障）
4. 实现最小完整模块，保持 `docs/MASTER_PLAN.md` 第 5 节的接口
5. 运行目标测试；接口有变更则额外跑契约与调用方测试
6. 自检不变量、错误流程、取消流程、数据保留
7. 更新 `docs/PROGRESS.md`（实际修改文件、命令、退出码、通过与跳过数、未解决问题、下一任务）

## 本机环境事实（2026-09-17 实测）

| 组件 | 状态 |
|---|---|
| Node | v22.22.2 |
| pnpm | 10.34.5（只能用 `pnpm.cmd`，bash shim 缺 coreutils 会崩） |
| git | 2.52.0 |
| WebView2 Runtime | 153.0.4234.32 ✅ |
| Windows SDK | 10.0.26100.0 ✅ |
| MSVC 工具链文件 | ✅ 存在（`VC\Tools\MSVC\14.44.35207`，`cl.exe`/`link.exe`/头文件/库齐全） |
| MSVC 组件登记 | ❌ **未登记**：`vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64` 无输出 |
| Rust | 1.98.1（stable-x86_64-pc-windows-msvc） |

**MSVC 未登记的直接后果**：rustc 的 MSVC 探测失败，退回到 PATH 上的同名程序——
Git for Windows 的 `/usr/bin/link.exe` 是 coreutils 的硬链接工具，会把 rustc 传来的参数
当成文件名并报 `extra operand`。因此**任何 cargo 命令都要先 `source scripts/msvc-env.sh`**，
它显式导出 vcvars64.bat 会设置的 `PATH`/`INCLUDE`/`LIB` 与 linker 路径。

注意 `INCLUDE`/`LIB`/linker 路径必须是 **Windows 形式**（反斜杠、分号分隔）：
Git Bash 只替子进程转换 `PATH`，其他变量原样传出。

bash 工具现状（与上一版记录不同，已实测）：`ls`/`sort`/`head`/`tail`/`sed` 可用；
`cmd.exe` 从 bash 调用被安全策略拦截（不能用它间接跑 `vcvars64.bat`）。
