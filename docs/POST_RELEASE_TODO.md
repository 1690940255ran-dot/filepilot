# FilePilot 发布后待修清单

本文件收集**真机验收 / 使用中发现的、暂缓修复的问题**。

暂缓的理由统一是：改动需要重新构建 + 重出安装包 + 重走一遍 T17。
因此同类小问题**攒成一批**一次性处理，避免反复重打包。

记录规则：
- 每条必须写清「现象 / 证据 / 影响面 / 为什么暂缓 / 建议改法」。
- 只记录**有实测证据**的问题，不记猜测。
- 修掉后移到「已修复」并注明版本。

---

## 待修

PR-001 ~ PR-004 已于 2026-09-24 批量修复（见「已修复」）。

**2026-09-27 CI 首次真实运行新增 3 条**（详见下文「已知问题」一节）：

| 编号 | 严重度 | 一句话 |
|---|---|---|
| **CI-001** | P1 | `extractors::image` 的 OCR 用例在 CI runner 上 `STATUS_ACCESS_VIOLATION`，打崩整个测试进程 |
| **CI-002** | P1 | `check-contracts.ps1` 依赖 `Get-FileHash`，CI 上必然找不到 → 契约 job 必红 |
| **CI-003** | P2 | `build-desktop.py` 打印中文时 cp1252 `UnicodeEncodeError` → 打包 job 必红 |

**这三条都是「本地全绿、CI 必红」**：本地开发机的 WinRT 组件、PowerShell 模块
解析、Python 默认编码与 GitHub runner 不同。其中 CI-002 本可更早发现 ——
`PROGRESS.md` 记过同一个报错，但被判为「本机命令环境问题」而绕过。

---

## 已修复

### PR-001 ~ PR-004 批量修复

- **修复时间**：2026-09-24
- **修复提交**：见同批次 commit
- **批次理由**：四条都属于「改完要重新构建 + 重出安装包」的类型，
  按本文件开头的规则攒成一批一次性处理。
- **修复范围**：13 个文件修改 + 2 个新组件 + 4 个新测试文件。

#### PR-001 问题列表「作用域」标签与正文无视觉间隔 —— 已修复

**修的过程中发现根因比原记录更深一层**：

原记录判断为「三个相邻 `<span>` 之间没有可见间距」，隐含前提是
「有样式但间距不够」。实际扫描 `src/styles.css` 后发现：

```
.issue*        → 零条规则
.modal*        → 零条规则
.badge-*       → 零条规则
.disabled-reasons → 零条规则
```

也就是说这些类名**从来没有对应的 CSS**，不是「间距不够」而是
**完全没有样式**。原记录的「加左间距 + 弱化颜色」方案若照做，
只会补上一条孤立规则，其余类名继续无样式。

- **实际改法**：为 `IssueList` 的整组类名补齐样式——
  `.issue-list`（列向 + gap）、`.issue`（`display:flex` + `align-items:baseline`
  + `gap:10px` + `flex-wrap`）、`.issue-block` / `.issue-warning`（左侧色条）、
  `.issue-badge` 与 `.badge-info` / `.badge-warning` / `.badge-block`、
  `.issue-message`（`flex:1 1 auto; min-width:0`，长文案换行不撑破）、
  `.issue-scope`（等宽字体、弱化色、自带底色与边框、`max-width:34ch` 截断）。
- **顺带补齐**：`.modal-backdrop`（`position:fixed; inset:0; z-index:50`）
  此前也不存在，`ConfirmDialog` 的遮罩依赖它。
- **未改**：`issue.itemId`（UUID）的语义问题保留原样——原建议方案 2
  （换成「第 3 项」）需要动后端 `Issue` 结构，收益低于代价；
  本次用视觉手段把它降级为次要信息已足够解决「读起来像一句话」的问题。

#### PR-002 统计数字列未对齐 —— 已修复

- `.info-grid` 的 `grid-template-columns` 由 `200px 1fr` 改为
  `max-content minmax(0, 1fr)`：标签列按内容宽度收缩，
  不再因为 `200px` 固定宽度导致短标签后的数值起点漂移。
- `.info-grid dd` 增加 `text-align: right` 与 `min-width: 4ch`，
  保留既有的 `font-variant-numeric: tabular-nums`，位数变化时列宽不跳。

#### PR-003 整理历史看不到逐文件明细 —— 已修复（采用原建议方案 3）

- **契约层**：新增 `RunItem` / `RunItems` 两个类型
  （`src-tauri/src/domain/types.rs`），均为 `deny_unknown_fields` + camelCase。
  **没有加宽 `RunReport`** —— 遵循 MASTER_PLAN:387「不将正文带回前端」
  的同一克制原则，历史列表保持轻量。
- **命令层**：新增 `get_run_items(runId) -> IpcResult<Option<RunItems>>`
  （`src-tauri/src/commands_execute.rs`），run 不存在时返回 `null`，
  与既有 `get_run` 的分工一致。
- **契约再生成**：`cargo run --bin export-contracts` →
  `contracts.generated.ts` / `contracts.schema.json` →
  `node scripts/generate-validators.mjs`（31 个定义 / 345.9 KB）。
  **三步都跑了**，避免 TEST_MATRIX §8.8 的生产白屏复发。
- **前端**：新增 `src/features/history/RunItemsTable.tsx`，
  按 `runId` **按需**拉取；三态（加载 / 失败 / 空）互相可区分——
  `null` 响应被当作**错误**而非空态（「记录不在了」≠「没有明细」）。
- **列表可辨识度**：列表行追加 `planId` 前缀。
  这是零额外成本的选择——`planId` 本来就随 `RunReport` 返回，
  无需为每行再查一次 operations。

#### PR-004 点「确认执行」后无反馈 —— 已修复（P1）

- `PreviewPage.tsx`：`setConfirming(false)` 从 `await execute_plan` **之后**
  提到**之前**，与「令牌已消费」解耦；执行期新增独立的
  `.progress-panel`（`position:sticky; bottom:0; z-index:5`）渲染在对话框之外，
  因此不再被遮罩挡住。
- `ConfirmDialog.tsx`：新增 `busy` prop。刻意**不**把 `busy` 并进
  `disabledReasons` —— 两者语义不同：后者说「你现在还不能确认」，
  前者说「你已经确认过了，正在做」。按钮文案在执行期变为「正在整理…」并禁用。
- **进度面板加了可访问名**（`aria-label="整理进度"`）。
  修的过程中发现：同一个页面上有**三处** `role="status"`
  （进度面板、禁用理由列表、停止请求提示），读屏用户听到的是三段无名公告，
  分不清在说什么；测试也无法定位。加 `aria-label` 后它才是一个可点名的区域。

#### 测试证据（本次新增 33 条）

| 文件 | 条数 | 覆盖 |
| --- | --- | --- |
| `tests/ui/confirm-feedback.test.tsx` | 6 | PR-004 |
| `tests/ui/history-items.test.tsx` | 12 | PR-003（含 2 条契约形状） |
| `tests/ui/presentation-invariants.test.ts` | 15 | PR-001/002/004 的 DOM 契约 |

**PR-004 的用例必须用「永不 resolve 的 Promise」冻结执行**——
文件少时 `await` 一闪而过，断言永远是绿的，这正是 C/D 节验收漏掉它的原因。
这组用例同时把那个漏掉的观测条件固定下来：以后谁把关闭时机挪回 `await` 之后，
测试就会红。

**PR-001/002 的诚实边界**：jsdom 不实现 CSS 级联与布局，
任何关于「间距/对齐好不好看」的断言在 jsdom 里都是**真空为真**。
所以这些用例改为解析 `src/styles.css` 源码、断言**结构性不变量**
（类名有规则、用了 flex+gap 而非相邻 margin、用了等宽字体、
第一列不是 `auto`……）。**这证明的是「不会退回无样式状态」，
不是「看起来好看」** —— 后者只能靠真机肉眼验收。

#### 验收侧同步

- `docs/CLEAN_MACHINE_ACCEPTANCE.md` C 节新增 PR-004 与 PR-003 的人工检查项。
- `docs/TEST_MATRIX.md` 补记本次新增用例。

---

## 已知问题

### PR-005 CI 与本地「全绿」的口径未被验证为同一集合 —— **已观测（2026-09-27），结论与当初的推断相反**

- **发现时间**：2026-09-25，核对测试总数时顺带发现
- **当时的推断**：本地 Windows 11 + MSVC、CI Windows Server + MSVC，
  按 `cfg(windows)` 分布的测试不会产生差异 → 判为「未验证，不是有问题」。

- **2026-09-27 实测结果：推断错了，而且错得比预期更严重。**

  触发 CI 的过程本身先暴露了一个**前置缺陷**（见 CI-000）：这条 workflow
  **一次都不会被触发** —— `push.branches` 写的是 `main`，而仓库默认分支是
  `master`。所以此前不是「没观测」，是「根本不会跑」。

  修正触发分支后第一次真实运行（run `36310782246`，commit `cf1eb18`）：
  **5 个 job，3 红 2 绿。**

  | job | 结果 |
  |---|---|
  | 前端 / 契约（ubuntu-latest） | ✅ success |
  | 端到端（浏览器模式，mock IPC） | ✅ success |
  | **Rust（Windows，真实文件操作）** | ❌ `STATUS_ACCESS_VIOLATION`，见 CI-001 |
  | **契约一致性** | ❌ `Get-FileHash` 找不到，见 CI-002 |
  | **桌面打包（NSIS，用户级）** | ❌ `UnicodeEncodeError`，见 CI-003 |

- **对「689 还是不是同一个集合」的回答**：`rust` job **根本没跑完** ——
  测试二进制在 `extractors::image` 那组用例上直接崩掉（0xc0000005），
  连 `test result:` 汇总行都没产出。所以**总数无法对账**，
  而这不代表「两边不一致」，代表**CI 上存在本地没有的平台专属崩溃**。

- **为什么当初的推断会错**：它默认「同一份代码 + 同一个 OS 家族 + 同样命令
  ⇒ 同样结果」。但 CI runner 与开发机在**WinRT 组件、PowerShell 模块解析、
  Python 默认编码**这三件事上并不相同 —— 而这三件正好各对应上面一条失败。
  「配置看起来一致」再次不等于「结果一致」，**这是本项目第三次栽在同一个假设上**
  （前两次：8.8 白屏、ADR-024 的干净重建 E0107）。

- **状态**：不再标为「待核实」。拆成 CI-000 ~ CI-003 四条具体缺陷处理。

---

### CI-000（**已修复**）CI 的 push 触发器分支名写错，workflow 根本不会触发

- **发现时间**：2026-09-27
- **现象**：4 次推送到 `master` 后，`GET /actions/runs` 与 `/actions/workflows`
  的 `total_count` 都是 **0** —— GitHub **连这条 workflow 都没注册**。
- **根因**：`ci.yml` 的 `push.branches` 是 `main / feature/** / fix/** / chore/**`，
  **不含 `master`**；而本仓库是用 API 以 `auto_init: false` 建的**空仓**，
  默认分支由**第一次推送**决定，第一次推的就是 `master`。
  文件里那句注释「本仓库的默认分支就是 `main`（初始提交即在其上）」是**基于印象的推断**。
- **排查路径（可复用）**：
  - `GET /repos/...` → `default_branch = master`
  - `GET /contents/.github/workflows/ci.yml` → 200（文件确实在默认分支上）
  - `GET /actions/permissions` → `enabled: true`（**Actions 本来就开着**）
  - `POST /actions/workflows/ci.yml/dispatches` → **404**（确认未注册）
  - 文件本身干净：无 BOM、无制表符、LF、5 个 job、无重复键
- **修法**：`master` 与 `main` 并列；注释改为实测结论 + 核对命令
  `curl -s https://api.github.com/repos/<owner>/<repo> | grep default_branch`。
- **验证**：推送 `cf1eb18` 后立即 `workflows=1`（`state = active`）、`runs=1`。

### CI-001｜P1｜`extractors::image` 的 OCR 用例在 CI runner 上把测试进程打崩

- **发现时间**：2026-09-27，CI 首次真实运行
- **现象**：`rust` job 失败，`exit code: 0xc0000005, STATUS_ACCESS_VIOLATION`。
  崩溃前最后启动、且**从未打印 `ok`** 的用例是：

  ```
  test extractors::image::tests::an_image_at_the_engine_dimension_limit_is_not_called_too_large ...
  ```

  它上面一条 `an_empty_byte_slice_is_corrupt_not_a_panic ... ok` 正常通过，
  说明**崩的是这一条**，而且是**进程级崩溃**（不是断言失败）——
  连 `test result:` 汇总行都没产出，`--no-capture` 也没用上。
- **影响面**：`rust` job 因此完全无法给出总数，PR-005 的「689 对账」无法进行。
  这一组用例走的是 **WinRT 系统 OCR**（T11），
  推测与 GitHub `windows-latest`（Windows Server）上 OCR 组件/语言包的行为差异有关。
  **注意：这是 CI 专属崩溃，本地 Win11 上 689 条全过。**
- **为什么是 P1**：它让整条 CI 的 Rust 门禁失效——正是 CI 存在的意义。
  且这类崩溃**恰好落在 T17 要验的系统 OCR 路径上**，不能当作测试环境噪音忽略。
- **待做**：在 runner 上定位是哪一步崩（构造该尺寸图片 → 调用 `OcrEngine`），
  再决定是「CI 上跳过 OCR 用例并显式标注」还是修实现。**不要在没定位前就加 skip。**

### CI-002｜P1｜`scripts/check-contracts.ps1` 依赖 `Get-FileHash`，在 CI 上必然失败

- **发现时间**：2026-09-27，CI 首次真实运行
- **现象**：

  ```
  > pnpm contracts:check
  Get-FileHash : The term 'Get-FileHash' is not recognized as the name of a cmdlet...
  At D:\a\filepilot\filepilot\scripts\check-contracts.ps1:29 char:17
  ```

- **根因**：脚本用 `powershell -NoProfile -ExecutionPolicy Bypass -File` 启动
  **Windows PowerShell 5.1**，而 `Get-FileHash` 属于 `Microsoft.PowerShell.Utility`，
  需要 `PSModulePath` 能解析到系统模块目录。从 pnpm/node 派生的进程环境里
  **`PSModulePath` 没有正确继承**，于是 cmdlet 找不到。

- **⚠️ 这一条本可以更早发现**：`docs/PROGRESS.md` 已经记过同一个报错，
  但当时判为「**本机命令环境问题**，不能把失败入口写成通过」，
  并用「临时给这条命令设 `PSModulePath`」绕过。
  **它不是本机问题 —— CI 上一模一样地复现。**
  把可复现的失败归因成「本机环境特殊」，等于把缺陷留到了 CI 上。

- **修法方向**：脚本内自己做哈希（不依赖 `Get-FileHash`），
  或在入口显式设置 `PSModulePath`；并让 `pnpm contracts:check` 这条**入口本身**
  成为被验证的对象，而不是绕过它去跑底层脚本。

### CI-003｜P2｜`scripts/build-desktop.py` 打印中文时因 cp1252 崩溃

- **发现时间**：2026-09-27，CI 首次真实运行
- **现象**：

  ```
  File "D:\a\filepilot\filepilot\scripts\build-desktop.py", line 99, in main
      print("[release] 路径重映射：")
  UnicodeEncodeError: 'charmap' codec can't encode characters in position 10-15
  ```

  runner 上的 Python 3.12 stdout 编码是 **cp1252**，打印中文直接抛异常。
- **同类已知项**：`scripts/scan-release-content.py` 在本机也因默认编码
  （中文系统是 GBK）失败过，当时的处置是**在命令行前设 `PYTHONIOENCODING=utf-8`**——
  但 CI 的 workflow 里**没有设**，所以同一类问题换个脚本又出现一次。
- **修法方向**：不要在每条命令外挂环境变量，而应让脚本自己健壮 ——
  在脚本开头 `sys.stdout.reconfigure(encoding="utf-8", errors="replace")`；
  workflow 里也用 `env: PYTHONIOENCODING: utf-8` 兜一层。
  **逐个命令打补丁的方式已经被证明会漏。**

---

## 已澄清（曾疑似缺陷，实为工具/记录问题）

### CL-001 归档报告里有一条「应用启动后崩溃」，实为脚本改到一半时的中间态记录

- **位置**：`tmp/verify-kit-reports/report.txt` 第 3 行起的
  `phase=install 2026-09-25 00:51:07`（UTC），宿主机、`-AllowDirty` 冒烟
- **该记录写**：`[失败（应用启动后崩溃）] 进程存活 = 已退出`
- **澄清依据**：
  1. **时间戳差 3 分钟**：`scripts/verify-clean-machine.ps1` 修改于
     `00:48:13`，该报告生成于 `00:51:07` —— 正落在改动"启动检查"那段的窗口内。
  2. **同包实测复现不出来**：2026-09-25 下午在宿主机用**当前**脚本逐秒采样 20 次，
     `Start-Process -PassThru` 返回的 PID 全程存活、`MainWindowTitle` 稳定为
     「文件领航 FilePilot」，无任何退出。
  3. **同包在虚拟机上通过**：同一安装包（SHA-256 `3e1ecb68…`）在 Win10 虚拟机上
     `[通过] 进程存活 = pid=1884`、`[通过（窗口已创建）] 主窗口标题 = 文件领航 FilePilot`。
- **结论**：**不是应用缺陷，也不是当前脚本的缺陷。**
- **教训**：改脚本的过程中跑出来的记录，必须与正式验收记录分开存放。
  当前脚本把历次结果**追加**进同一个 `report.txt`，历史中间态会与正式结论混在
  一起，交回时极易被误读为缺陷。
- **已处置（2026-09-25）**：`scripts/verify-clean-machine.ps1` 新增 **`-Draft`** 开关——
  结果写到 `report-draft.txt` / `report-draft.json`，并在报告头明确标注
  「【草案】开发期试跑用，不是交付给验收方的记录」。改脚本时应当主动加上它。
  已实测：`report.txt` 不再被污染，草案与正式报告并存互不干扰。

---


## 明确不改的

记录那些"看起来像问题、实际是设计如此"的点，避免以后重复讨论。

- **`-Phase env` 不启动应用**：只读检查，设计如此。
- **卸载不删用户数据**：`%LOCALAPPDATA%\FilePilot\filepilot.db` 保留操作历史
  是 T17 的硬要求，不是遗漏。
- **撤销前要求重新授权根目录**：安全设计，重启后目录身份需重新确认。
- **目标目录"尚不存在"不是错误**：`Observed::Missing` 属正常情形，
  执行阶段会逐级创建（规格 8.2）。
