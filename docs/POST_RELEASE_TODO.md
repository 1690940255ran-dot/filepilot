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

（暂无。PR-001 ~ PR-004 已于 2026-09-24 批量修复，见「已修复」。）

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

### PR-005 CI 与本地「全绿」的口径未被验证为同一集合（**待核实，非缺陷**）

- **发现时间**：2026-09-25，核对测试总数时顺带发现
- **现象**：`docs/TEST_MATRIX.md` 4.1.1 记录过一桩「687 还是 558」的疑问。
  核对结论是 **687 正确**（558 是只统计了日志前 7 行造成的计数错误），
  但核对过程中暴露出一条**尚未消解的不确定性**：CI 的 `rust` job 跑在
  `windows-latest` 上，命令与本地一致（`--features failpoints --locked --
  --test-threads=1`），**理论上应当同样是 687**——
  然而这个「应当」**从未被观测过**。

- **为什么不写成缺陷**：目前没有任何证据表明两边不一致。
  本地是 Windows 11 + MSVC、CI 是 Windows Server + MSVC，
  按 `cfg(windows)` 分布的测试不会产生差异。**这是"未验证"，不是"有问题"。**

- **但它值得记一条**：本项目刚刚才因为「配置一致 ≠ 结果一致」吃过一次教训——
  8.8 的白屏缺陷正是"开发机全绿、装完白屏"。同一类推理若再犯一次，
  代价会是**CI 上某个平台专属用例长期红着或长期被跳过而没人知道**
  （`--features failpoints` 的注释里已经写明：不带这个 feature 跑，
  一批用例会被整段跳过而 CI 依然全绿）。

- **消解方式**：推送触发一次 CI，把 `rust` job 的
  `test result:` 行数出来，与本地 687 逐个目标对账。
  **在一次真实运行之前，引用 CI「全绿」时都必须附带「未在 runner 上跑过」。**

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
