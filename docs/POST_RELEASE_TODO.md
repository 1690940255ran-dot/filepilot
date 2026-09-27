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

**2026-09-27 CI 首次真实运行新增 4 条**（详见下文「已知问题」一节）：

| 编号 | 严重度 | 一句话 | 状态 |
|---|---|---|---|
| **CI-000** | P1 | push 触发器分支名写错（`main` vs 实际默认分支 `master`），workflow 根本不触发 | ✅ 已修复并在 CI 上验证 |
| **CI-001** | P1 | `extractors::image` 的边界用例在 CI runner 上 `STATUS_ACCESS_VIOLATION`，打崩测试进程 | ✅ 已修复并在 CI 上验证（lib 408 通过） |
| **CI-002** | P1 | `check-contracts.ps1` 依赖 `Get-FileHash`，CI 上必然找不到 | ✅ 已修复并在 CI 上验证（现在能算哈希） |
| **CI-003** | P2 | `build-desktop.py` 打印中文时 cp1252 `UnicodeEncodeError` | ✅ 已修复并在 CI 上验证（中文正常打印） |
| **CI-004** | P2 | WinRT OCR 处理 `10000 × 1` 这类极端宽高比图片时进程崩溃（Windows Server） | 待查，**不要在没定位前加 skip** |
| **CI-005** | P2 | **CI 上验不到真实 OCR 识别结果**（runner 没有中文语言包） | 已如实记录，不当作已覆盖 |
| **CI-006** | P1 | 契约文件被 `check-contracts.ps1` 报不一致 —— **实为 CRLF/LF 假警报** | ✅ 已加 `.gitattributes` 修复 |
| **CI-007** | P1 | `--remap-path-prefix` 在 CI 上未盖住 `runneradmin` 路径，打包 job 的内容扫描正确拒绝产物 | ✅ **根因已定位并修复**（`exists()` 静默跳过映射） |

**这些都是「本地全绿、CI 必红」**：本地开发机的 WinRT 组件、PowerShell 模块
解析、Python 默认编码、行尾设置与 GitHub runner 不同。

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

### CI-004｜P2｜WinRT OCR 在 runner 上让测试进程崩溃（**已缩小范围，仍待定位**）

- **发现时间**：2026-09-27，从 CI-001 里分离出来
- **现象**：测试进程以 `STATUS_ACCESS_VIOLATION (0xc0000005)` 退出。
- **2026-09-27 第二次观测（run 36316850987）：崩溃点会「漂移」**

  | 轮次 | 崩溃所在二进制 | 崩溃前最后启动的用例 |
  |---|---|---|
  | 第一次 | `filepilot_lib`（单测） | `extractors::image::tests::an_image_at_the_engine_dimension_limit_is_not_called_too_large` |
  | 第二次 | `extract`（集成测试） | `the_spaces_ocr_inserts_between_chinese_characters_are_gone` |

  两轮里**同一个进程内其他用例都正常通过**。第二次的日志还显示
  `an_image_over_the_engine_dimension_limit_reports_too_large_not_corrupt ... ok`
  —— 也就是说 **`max_image_dimension()` 并不崩**，此前把它列为嫌疑是错的。

- **当前判断**：崩溃与「某个具体输入」无关，更像是**同一进程内多次触碰 WinRT/OCR
  之后的累积状态问题**（COM 公寓、OCR 引擎对象生命周期之类）。
  这也解释了为什么它会随测试顺序/二进制变化而漂移。
- **影响面（不变）**：图片提取跑在独立的 `extract_worker` 子进程里并受 Job Object
  约束，崩溃会被父进程观测为一次提取失败，不会带走 UI。但用户会看到
  「这个文件读不出来」。
- **仍未定位**：手上没有 Windows Server 环境，无法在崩溃点取栈。
- **处置口径（不变）**：**不加 `#[ignore]`、不加平台 cfg 跳过**。
  在没定位之前跳过，等于把一个问题变成「CI 全绿」的假象 ——
  正是 `--features failpoints` 注释里警告过的那种情形。
- **下一步（留给能上 Server 的人）**：在 runner 上按 `--no-capture` 跑
  `cargo test --test extract the_spaces_ocr` 单独复现；若单跑不复现，
  说明确实是累积状态，那就该考虑**把 OCR 相关用例隔离到独立测试进程**。

- **✅ 2026-09-27 性质已判定：确认是「累积状态」，不是「输入有毒」。**
  加了一步诊断（`if: failure()` + `continue-on-error`，平常零成本、不做断言），
  在 CI 上单独跑三组，**全部通过**：

  | 组 | 命令 | 结果 |
  |---|---|---|
  | A | `--test extract the_spaces_ocr` | **ok**（1 passed，33 filtered） |
  | B | `--test extract a_chinese_image`（对照） | **ok** |
  | C | `--test extract image`（全部 image 用例） | **ok**（6 passed） |

  而**完整跑整份套件**才崩。⇒ 崩溃与某个具体输入无关，
  是**同一进程内多次触碰 WinRT/OCR 之后的累积状态**。

  这次诊断还顺带确认了两件事：
  - 平台上 `availability()` 确实报告**不可用**，CI-005 的分平台分支按预期生效；
  - 那些 `println!` 在 `--nocapture` 下**真的能看到**（日志里出现了
    `[跳过空格折叠检查] 本机 OCR 不可用，读不到文本`），
    所以「跳过」在 CI 上是**可见的**，不是静默的。

- **下一步（需要拍板，因为这是一次设计取舍）**：
  1. **把 OCR 相关用例隔离到独立测试进程** —— 本项目对「一个崩溃带走邻居」
     已有先例（`recovery_crash_child` 就是为此拆出来的 bin）。
     收益：CI 报的会是**精确的那一个能力**，而不是「Rust 全红」；
     其余 30+ 条用例不再被一个平台崩溃连坐。
     **但注意：隔离本身不会让它变绿** —— 隔离后那个目标大概率仍然崩，
     只是崩得精确。**不要把「隔离」当成「修好」。**
  2. 或者继续挖 WinRT 初始化路径（COM 公寓 / 引擎对象生命周期），
     需要一个能复现的环境来取栈。
  3. **不做的事**：加 `#[ignore]` 或平台 cfg 跳过。那会让 CI 变绿，
     但绿的是假象 —— 与 `--features failpoints` 注释里警告的
     「一批用例被整段跳过而 CI 依然全绿」是同一类。

### 2026-09-27 修复与验证记录

| 编号 | 改法 | 验证方式与结果 |
|---|---|---|
| **CI-000** | `push.branches` 加 `master`；修正注释里的错误前提 | 推送 `cf1eb18` 后 `workflows=1`（`state=active`）、`runs=1` —— **已在真实 runner 上观测到** |
| **CI-001** | 抽出纯函数 `exceeds_engine_dimension(width, height, limit)`，`extract` 调用它；边界用例改为直接断言该函数（不再把 `limit × 1` 的图送进 OCR） | 本机 7 条图片用例全过；**待下一次 CI 验证** |
| **CI-002** | `check-contracts.ps1` 不再用 `Get-FileHash`，改用 `System.Security.Cryptography.SHA256` | 对照测试证明**与 `Get-FileHash` 输出逐字符相同**；本机 3 种 `PSModulePath` 取值（缺失／只指向 pwsh7／只指向 5.1）**都复现不了失败**，故本机无法验证修复效果，**只能靠 CI 验证** |
| **CI-003** | 6 个脚本内部设 `sys.stdout/stderr.reconfigure(encoding="utf-8")`；workflow 加 `PYTHONIOENCODING: utf-8` 兜底 | **本机复现并验证**：`PYTHONIOENCODING=cp1252` 下，无保护的 `print` 报 `UnicodeEncodeError: ... position 10-15`（与 CI 日志**逐字相同**）；加上保护后 exit 0，真实脚本 `scan-release-content.py` 也 exit 0 |

> **CI-002 这条要如实说明**：我**没能**在本机复现 `Get-FileHash` 找不到的失败。
> 一个容易想到的解释是「`PSModulePath` 没正确继承」，但实测把 `PSModulePath`
> 删掉、或指向 pwsh7、或指向 5.1 系统模块，`Get-FileHash` **都能用** ——
> 所以那个解释**没有依据**，不作为结论写在文档里。
> 换成 .NET 之后不再依赖 cmdlet 自动加载，**无论真实触发条件是什么都绕开了**，
> 但它是否真的解决问题，要以 CI 结果为准。

### 第一次修复后的 CI 复跑（run 36314541588 / commit 25ee5c3）

**三个 job 仍然是红的，但失败内容全变了** —— 说明上面的修复都生效了，
并且**暴露出了原本被它们掩盖的问题**：

| job | 修复前 | 修复后 |
|---|---|---|
| Rust | `STATUS_ACCESS_VIOLATION`，lib 崩掉、无汇总行 | **lib 408 通过**；改为 2 条 `tests/extract.rs` 断言失败（见 CI-005） |
| 契约一致性 | `Get-FileHash` 找不到 | 现在能算哈希，于是**第一次真正跑出比对结果**（见 CI-006） |
| 桌面打包 | `UnicodeEncodeError` | 中文正常打印，脚本**真的跑到了内容扫描**并正确地拒绝了产物（见 CI-007） |

> **这轮的价值在于**：前一轮的修复并不是「让 CI 变绿」，而是**让 CI 能说话**。
> 三个 job 各从一个环境故障，变成了一个真实结论。

### CI-005｜P2｜CI 上验不到真实的 OCR 识别结果（runner 没有中文语言包）

- **现象**：`tests/extract.rs` 里 2 条用例在 CI 上失败：
  - `a_chinese_image_comes_back_as_searchable_text`（`extract.rs:869`）
    —— 断言 `status == Ok` 并要求正文含「会议」「2026」；
  - `an_image_without_text_is_unsupported_and_never_invents_content`（`extract.rs:943`）
    —— 断言错误码**恰好**是 `UNSUPPORTED_FORMAT`。
- **根因**：前者要求「这台机器能读中文」，而 GitHub 的 `windows-latest`
  没有中文 OCR 语言包；后者**与自己的注释自相矛盾** ——
  它上一行注释写着「`OCR_UNAVAILABLE` 也会落到 unsupported（这台机器没装 OCR）」，
  紧接着却把码写死成 `UNSUPPORTED_FORMAT`。装没装 OCR 会给两个不同的码，
  这条断言在任何「没装 OCR」的机器上都必红。
- **改法**：
  - 后一条：码允许 `UNSUPPORTED_FORMAT | OCR_UNAVAILABLE` 两者之一，
    **保留真正的不变量断言**（状态必须是 unsupported、正文必须为空 ——
    「绝不编造正文」才是这条用例存在的理由）。
  - 前一条：按 `platform::ocr::availability().is_usable()` 分两条路 ——
    有语言包就验识别结果；没有就验「如实报告 `OCR_UNAVAILABLE` 且正文为空」
    （这同样是规格要求：「语言包缺失明确提示」）。
  - 顺带修 `the_spaces_ocr_inserts_between_chinese_characters_are_gone`：
    它只断言「不含 `会 议`」这类模式，**在 OCR 不可用时是真空为真** ——
    能过但什么都没验。现在先确认读到了非空文本再检查折叠。
- **⚠️ 覆盖缺口（如实记录）**：改完之后，**CI 上验不到真正的识别结果**，
  只验「缺失时如实报告」。规格 T11 那句「OCR 有文字时返回可搜索片段」
  目前只在装了中文语言包的机器上被验证过。这一条**不当作已覆盖**。

### CI-006｜P1｜契约「不一致」实为 CRLF/LF 假警报（**已修复**）

- **现象**：`contracts:check` 报两个生成物都不一致：
  ```
  契约不一致：src/api/contracts.generated.ts
    生成前: 2707CC163AC4B894…      ← 仓库检出的
    生成后: 0E5F72481A883A71…      ← 生成器写出的
  ```
- **根因**：仓库**没有 `.gitattributes`**，而 Windows 上 `core.autocrlf` 默认 `true`。
  CI 的 Windows runner 检出成 **CRLF**，而生成器写的是 **LF**；
  而 `check-contracts.ps1` 比对的是**字节 SHA-256**。
- **决定性验证**：把 LF 内容整体转成 CRLF 后算 SHA-256，
  **正好等于上面那个「生成前」值，两个文件都对得上**
  → 契约本来就是一致的，红的只是一个换行符差异。
- **这个假警报被掩盖了两次**：本地先因为 `Get-FileHash` 拿不到而没跑到比对；
  修好 Get-FileHash 之后才在 CI 上第一次真正跑出结果。
- **修法**：新增 `.gitattributes`，只钉住会被逐字节比对的三个生成物
  （`contracts.generated.ts` / `contracts.schema.json` / `validators.generated.ts`）
  为 `eol=lf`。**不做全仓库 `* text=auto`** —— 那会改动大量文件的检出字节
  （`.cmd` / `.bat` 反而需要 CRLF），风险远大于收益。

### CI-007｜P1｜`--remap-path-prefix` 在 CI 上没盖住编译机用户路径（**待查**）

- **现象**：CI 的打包 job 里，内容扫描正确地拒绝了产物：
  ```
  [HIT] extract_worker.exe -> 编译机用户路径（1 个用户：runneradmin）
  [HIT] filepilot.exe     -> 编译机用户路径（1 个用户：runneradmin）
  结论：**不通过**。发布物里出现了不该有的内容。
  ```
- **为什么这是**正确**的行为**：`scripts/build-desktop.py` 用 `--remap-path-prefix`
  去掉编译机路径，而这一条扫描就是那个保护的回归测试。
  在 runner 上红，说明**保护没有完全生效** —— 这正是它存在的意义。
- **目前的困难：没有证据指向是哪一条映射没盖住。** 重映射一共三条：
  项目目录 / `<home>/.cargo/registry` / `<home>/.rustup/toolchains`。
  扫描只报「有个用户名」，无法判断是哪一条漏的。
  **因此本轮不去猜、也不改重映射逻辑**，而是先拿到证据：
  把 workflow 里这一步改成 `--verbose`（0 命中时不额外输出，常开无副作用），
  下一次 CI 运行会逐条打出命中的原文。
- **下一次要看的**：命中串的前缀是
  `C:\Users\<runner>\.cargo\registry\…`（说明 registry 那条没生效）、
  `.cargo\git\…`（说明只映射 registry 不够）、
  还是 `.rustup\…` / 别的目录（说明要新增一条映射）。
  > 这里刻意用 `<runner>` 占位而不是写真实用户名：本文件**本身就在扫描范围内**，
  > 第一版把真名原样写进来，扫描器立刻把这份文档也判成了泄漏源
  > （`[HIT] docs\POST_RELEASE_TODO.md -> 本机用户目录路径`）。
  > **记录泄漏的文档自己会变成泄漏** —— 写这类内容一律用占位符。
- **影响面**：**不阻断功能，但阻断发布。** 规格 T16 明写发布内容
  「不包含真实路径样本」，所以在这一条变绿之前不能发布安装包。
  另外注意本地扫描是通过的（0 命中）—— 这条**只有 CI 能发现**。

- **✅ 2026-09-27 根因已定位（证据决定性）。** 加上 `--verbose` 之后，CI 打出了
  命中的**完整路径**，全都是同一个形状：

  ```
  C:\Users\<runner>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\aes-0.9.3\src\…
  C:\Users\<runner>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\brotli-decompressor-5.0.3\src\…
  ```

  即**正好是映射到 `/cargo` 的那一条前缀**。而同一份 CI 日志里，
  `build-desktop.py` 打印出来的映射只有**两条**：

  ```
  [release] 路径重映射：
    D:\a\filepilot\filepilot=.
    C:\Users\<runner>\.rustup\toolchains=/rust      ← 少了 .cargo/registry
  ```

  **根因**：`remap_flags()` 里有个 `if prefix.exists()` 的「防御」判断，
  本意是防 rustup 用系统工具链时那一层不存在。但
  **`--remap-path-prefix` 是纯字符串前缀改写，根本不要求路径存在**；
  而反过来，路径**暂时**不存在时跳过映射，就让依赖带着原路径被编译了 ——
  CI 上 `.cargo/registry` 在那一刻尚未创建（依赖是这次构建才下载/解压的），
  于是这条映射被**静默跳过**。本地 registry 一直是热的，所以**本机永远复现不出来**。

  > 讽刺的是 `remap_flags()` 自己的注释早就写过
  > 「写成单数时 `exists()` 为假、这条映射被静默跳过」——
  > 同一个坑换了个触发条件又中一次。**「静默跳过」这类防御要格外小心。**

  **修法**：去掉 `exists()` 过滤（前缀不存在时规则匹配不到任何东西，是安全空操作），
  并顺带改为优先读 `CARGO_HOME` / `RUSTUP_HOME` 环境变量 ——
  workflow 里显式设了 `CARGO_HOME`，而原来写死 `<home>/.cargo` 未必是同一处。
  验证：把 `CARGO_HOME` 指向不存在的目录，映射仍是 3 条（旧实现会掉到 2 条）。

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
