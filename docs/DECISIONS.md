# FilePilot 技术决策记录（ADR）

格式：编号 / 状态 / 日期 / 背景 / 决策 / 后果。
状态取值：`已接受` / `待定` / `已废弃`。

---

## ADR-001 架构固定为 Tauri 2 + Rust + React

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：规格第 3.1 节要求把文件权限、路径验证、数据库和执行器集中在本地核心，界面只处理交互。
- **决策**：采用 Tauri 2 + Rust 业务核心 + React 界面。不采用 Electron + TypeScript（首版不同时维护两套实现）；不引入 Python（首版不增加 Python 安装与打包链）。
- **后果**：需要 Rust 与 MSVC 构建工具链。本地安全原语可用 Windows API 直接实现。更换架构必须先修改 `docs/MASTER_PLAN.md`，不能混搭后端形成重复实现。

---

## ADR-002 首版只支持 Windows 11 x64 + 本地固定 NTFS

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：不覆盖、不越界的保证依赖 Windows 原生文件身份（fileId）与目录句柄语义，这些语义在其他平台不等价。
- **决策**：v0.1 只对 Windows 11 x64 本地固定磁盘 NTFS 声明支持文件执行。拒绝 UNC/网络盘、可移动盘、非 NTFS、已知同步根。其他平台允许编译部分逻辑，但不声明支持文件执行。
- **后果**：平台不支持时返回 `UNSUPPORTED_STORAGE`，**不回落**到普通覆盖式 `rename`。macOS/Linux 支持需先做独立安全设计。

---

## ADR-003 不删除、不覆盖

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：删除与覆盖是数据丢失的主要来源，且难以在崩溃后恢复。
- **决策**：v0.1 支持的操作为分类、重命名、同卷移动、创建分类目录。禁止删除文件、覆盖文件、跨卷移动、目录整体重命名、修改正文与扩展名、解压或执行文件。现有文件绝不被覆盖（INV-03）。
- **后果**：目标名冲突时采用 `名称 (2).扩展名` 稳定分配，并在预览里呈现；最终确认后发生的新冲突返回错误并停止，不自动另取名字。

---

## ADR-004 Windows 安全移动原语

- **状态**：已接受（T03，2026-09-16）
- **背景**：规格 7.3 要求「不覆盖」由内核保证，而不是 `exists()` 之后再 `rename()`——两次调用之间别的进程可以创建目标。
- **决策**：`safety::fingerprint::move_no_replace` 走以下组合，每一环都有对应测试：

| 环节 | 具体做法 | 依据 |
|---|---|---|
| 打开源文件 | `CreateFileW`，访问权限 `GENERIC_READ \| DELETE`，共享模式 **仅 `FILE_SHARE_READ`** | 重命名需要 `DELETE`；不共享 WRITE/DELETE，使校验期间别的进程无法改动或替换该文件。已被写入方占用时打开失败 → `FILE_BUSY` |
| 校验身份/大小/时间 | 在该句柄上 `GetFileInformationByHandle` | 句柄不会因路径被替换而指向别的文件 |
| 校验内容 | `sha256_of_handle`（`&File: Read`，64 KiB 分块） | 规格 E01：size 与 mtime 恰好相同的篡改只能靠内容哈希检出 |
| 保护目录链 | 根、源父目录链、目标父目录链逐级用 `FILE_FLAG_OPEN_REPARSE_POINT` 打开；拒绝 `FILE_ATTRIBUTE_REPARSE_POINT`，且不共享 `FILE_SHARE_DELETE` | 防止已有联接越界，并在校验至重命名期间阻止目录被替换 |
| 目录语义 | 对每个保护句柄查询 `FileCaseSensitiveInfo`，开启大小写敏感时拒绝 | v0.1 的冲突规则按 Windows 默认大小写不敏感语义设计 |
| 执行重命名 | `SetFileInformationByHandle(FileRenameInfo)` + **`ReplaceIfExists = FALSE`** | 规格 7.3 明确点名的原语；内核层拒绝覆盖 |
| 复核结果 | 同一个句柄再取身份，与 `expected.file_id` 比对 | 避免「按路径重新打开」带来的偏差 |

- **不使用的做法**（规格明令禁止）：`std::fs::rename`（会覆盖）、复制后删除、改变 ACL、移除只读属性、提权、回退成覆盖模式。
- **测试证据**（`tests/execute_windows.rs`，17 条全通过）：

| 用例 | 验证内容 |
|---|---|
| `kernel_refuses_to_overwrite_a_target_created_after_the_precheck` | 打开源句柄（= 预检查通过）后由「竞争方」创建目标，再执行重命名 → **内核拒绝**，源与目标字节均完好 |
| `std_fs_rename_would_have_overwritten_the_target` | **对照组**：同一场景下 `std::fs::rename` 确实会覆盖，证明上一条不是侥幸 |
| `detects_source_content_changed_with_identical_size_and_mtime` | 改写内容后把 mtime 恢复成原值 → `SOURCE_CHANGED` |
| `detects_source_replaced_by_a_different_file` | 删除后重建同名同内容文件 → 卷内身份不同 → `SOURCE_CHANGED` |
| `refuses_when_target_already_exists_and_leaves_both_files_intact` | 两个文件的字节都未被触碰 |
| `content_hash_is_identical_before_and_after_move` | 路径改变、内容哈希与身份不变（INV-04） |
| `rejects_a_target_path_that_crosses_a_directory_junction` | 目标祖先为指向根外的 junction 时返回 `REPARSE_POINT`，源与根外目标均不变 |
| `total_target_path_limit_includes_the_approved_root` | 240 UTF-16 units 按完整目标路径计算，而非只算相对部分 |

- **后果**：目标父目录**必须已存在**；本原语不创建目录（那是 T06 的职责），不存在时返回 `INVALID_PATH` 并说明原因。
- **错误映射**：保留 Win32 原始错误码，分别映射共享冲突、目标已存在与权限不足；不得把所有打开失败称为 `SOURCE_MISSING`，也不得把所有 rename 失败称为 `TARGET_EXISTS`。
- **本阶段边界**：按规格 T03，该原语**只用于测试，不接通 UI 执行**。


---

## ADR-005 依赖版本锁定策略

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：规格 T00 要求「选择相互兼容的稳定版本…记录选择日期与来源，不用浮动 latest 作为复现依据」。凭记忆填写版本号不可接受。
- **决策**：全部版本在当日从 `registry.npmjs.org` 与 `crates.io` API 实测取得，并逐条核对 `engines` / `peerDependencies`。

关键结论：

| 包 | 锁定版本 | 依据 |
|---|---|---|
| **typescript** | **6.0.3**（不是 latest 7.0.2） | `typescript-eslint@8.70.0` 的 peer 声明 `typescript: ">=4.8.4 <6.1.0"`，7.0.2 越界。6.0.3 是满足约束的最新稳定版。 |
| react / react-dom | 19.3.0 | latest |
| vite | 8.3.0 | `@vitejs/plugin-react@6.1.1` peer 要求 `vite ^8.0.0`；`vitest@5.0.1` peer 接受 `^8.0.0` |
| vitest | 5.0.1 | engines 要求 node `^22.12.0 \|\| ^24.0.0 \|\| >=26.0.0`，本机 22.22.2 满足 |
| jsdom | 30.0.1 | engines 要求 node `^22.22.2`，本机恰好满足（这是最紧的一条约束） |
| eslint | 10.10.0 | engines 要求 node `^20.19.0 \|\| ^22.13.0 \|\| >=24`，本机满足 |
| @vitejs/plugin-react | 6.1.1 | 其余 peer（oxc-transform-react / @rolldown/plugin-babel / babel-plugin-react-compiler）均标记 `optional: true` |
| tauri | 2.11.5 | crates.io max_stable，license = Apache-2.0 OR MIT |
| tauri-build | 2.6.3 | crates.io max_stable |

- **后果**：升级 TypeScript 到 7.x 之前，必须先确认 `typescript-eslint` 放宽 peer 范围，否则 `pnpm install` 会出现 peer 冲突。版本变更需在此表追加记录，不允许直接改 `package.json` 而不留痕。

---

## ADR-006 契约唯一真源与运行时校验方式

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：规格 5.1 要求「Rust 类型是唯一真源…生成 TypeScript 类型及 JSON Schema，并在前端用 Zod 或等价运行时校验验证 IPC/模型边界」；同时明确禁止「手写第二套类型长期维护」。规格 3.1 的候选依赖里列的是 Zod。
- **决策**：
  1. Rust 侧契约类型同时 derive `serde`（camelCase 序列化）+ `schemars`（输出 JSON Schema）+ `ts-rs`（输出 TypeScript 类型）。
  2. `export-contracts` 二进制生成 `src/api/contracts.generated.ts` 与 `src/api/contracts.schema.json`。
  3. 前端运行时校验使用 **Ajv 直接消费生成的 JSON Schema**，不手写 Zod schema。
- **理由**：手写 Zod schema 等于维护第二套类型定义，直接违反规格 5.1。若走「JSON Schema → Zod」自动转换，会多一个可能失真的中间环节，却没有任何安全收益。Ajv 直接吃 JSON Schema，是链路最短的等价方案（规格原文允许「Zod 或等价」）。
- **后果**：`pnpm contracts:check` 会重新生成并与仓库内文件比对，任何手工修改生成文件都会导致校验失败。

---

## ADR-007 契约生成管道的引导版本策略

- **状态**：已接受（引导版本已于 2026-09-16 被真实生成结果取代）
- **日期**：2026-09-16
- **背景**：T00 执行时本机缺 Rust 工具链与 MSVC C++ 生成工具，`export-contracts` 无法运行。若坚持「生成物不存在」，前端代码写完也无法验证；若直接手写一份生成物当真品，则违反 ADR-006 与规格 5.1。
- **决策**：工具链就绪前采用「引导版本 + 强制比对」：契约文件以严格对应 Rust 定义的内容存在并带 `BOOTSTRAP` 标记；`scripts/check-contracts.ps1` **强制重新生成后再比对哈希**，生成器不可用时非零退出，绝不把现有文件当成通过。
- **结果**：工具链就绪后运行 `pnpm contracts:check`，生成结果与仓库内文件哈希一致，引导版本已被真实产物完整取代：

  ```
  contracts.generated.ts  4BB7A36BC16A6DF209486758DD9E1AC87015E9C7C2183FF23AD646C4E1FE39DB
  contracts.schema.json   C2917463DA3A55F86954C587CCAB782EB81F166B6AF00A9CB25AAB9A83116B4D
  ```

  上表是**引导版本被取代当时**的哈希。契约随后随每个任务扩展，最新一次是
  T11（新增 `OcrAvailabilityReport`）：

  ```
  contracts.generated.ts  5CA29D238CE6CE3DCADA85BD15CD27C81DF620B1BD23772C9843809146405ED9
  contracts.schema.json   60503DD3913A31E2A3A41FD84D67A4FE3DD985E5149C2580B6BF47CB08634DA9
  ```

  > `check-contracts.ps1` **不硬编码**这些值：它先记下当前哈希、重新生成、
  > 再比对前后是否一致。所以哈希变化本身不会让门禁失败——**手工改生成物**才会。

- **不变量**：契约文件一旦由 `pnpm contracts:generate` 生成，**不允许再手工修改**。任何手工改动都会让 `contracts:check` 失败。

- **已知缺口（T11 记录）**：`src/api/client.ts` 里的 `KNOWN_ERROR_CODES` 是**手写**的
  错误码集合，没有和 Rust 的 `codes` 模块做自动比对。T11 新增 `OCR_UNAVAILABLE`
  时就漏同步了一次——前端会把它当成「未知错误」展示。
  目前只有「Rust 侧错误码与规格表一致」的测试（`error_codes_match_the_spec_table`），
  **前端那一份没有测试守着**。要根治应当把 codes 也纳入生成物，而不是继续靠人记得改两处。

---

## ADR-008 CSP 只由 `tauri.conf.json` 单点管理

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：最初在 `index.html` 写了 `<meta http-equiv="Content-Security-Policy">`，同时 `tauri.conf.json` 也配置了 `app.security.csp`。
- **决策**：**CSP 只在 `tauri.conf.json` 声明**，HTML 里不写 meta CSP，并显式提供 `devCsp`。
- **理由**：两处 CSP 会被浏览器取**最严格的交集**，而 `index.html` 里的那份不会随 Tauri 的 dev 模式放宽。结果是 dev 模式下 Vite / React Refresh 需要的内联脚本被 `script-src 'self'` 拦掉，界面无法挂载。把策略收敛到一处，dev 与生产两种模式的差异（内联脚本、eval、HMR WebSocket）都在 `devCsp` 里显式写出，而不是靠隐式放宽。
- **注意**：`devCsp` 放宽**只作用于开发模式**；生产构建仍使用严格的 `csp`（无 `'unsafe-inline'` / `'unsafe-eval'`），不会削弱发布产物的安全边界。

---

## ADR-009 WebView2 崩溃的处理边界

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：本机运行 `pnpm dev` 时桌面窗口只有标题栏、内容区全黑，`filepilot.exe` 没有 `msedgewebview2.exe` 子进程，且每次启动都在 `EBWebView/Crashpad/reports/` 留下约 8.7 MB 转储。加 `--disable-gpu` 后不再崩溃。
- **决策**：**不把 `--disable-gpu` 写进项目配置**。仅在文档中记录该 workaround 与诊断方法，由使用者在自己的环境验证是否存在同样问题。
- **理由**：证据只能说明「在当前的沙箱环境下 WebView2 渲染进程会崩」，不足以断定这是所有机器的普遍现象。给所有用户默认关闭 GPU 加速，是用确定的性能损失去换一个不确定的收益。
- **若后续确认是普遍问题**，应通过 `tauri.conf.json` 窗口配置的 `additionalBrowserArgs` 添加，而不是要求用户设置环境变量。

---

## ADR-010 路径比较前必须剥掉 verbatim 前缀

- **状态**：已接受
- **日期**：2026-09-16
- **背景**：T02 实现根目录授权时发现，`std::fs::canonicalize` 在 Windows 上返回的是
  **扩展长度路径** `\\?\C:\Users\me`，而 `USERPROFILE` / `SystemRoot` / `LOCALAPPDATA`
  等环境变量给出的是普通形式 `C:\Users\me`。团队最初按组件逐项比较这两者，
  于是**所有**归属判断（是否在系统目录下、是否就是用户主目录、是否在应用数据目录下）
  全部返回 false —— 拒绝清单看似实现了，实际上一条也没生效。
- **决策**：所有进入比较的路径统一经过 `safety::root::strip_verbatim_prefix()`。
  该函数同时处理 `\\?\UNC\server\share` → `\\server\share`。
- **代价与边界**：去掉前缀后不再享有 MAX_PATH 豁免。由于规格 7.1 已把目标总路径
  限制在 240 个 UTF-16 单位以内，这个代价可以接受。
- **为什么值得单独立一条 ADR**：这个缺陷不会被普通的「正常路径能授权成功」测试发现——
  它只在**负向**用例（该拒绝的没拒绝）里暴露。后续任何涉及路径比较的模块都必须遵守本条。

---

## ADR-011 T04 的两项依赖与时间处理

- **状态**：已接受（T04，2026-09-17）
- **背景**：T04 需要 SQLite（规格 8.1）与「按本地时区生成 `YYYY-MM`」（规格 6.3）。
  两件事都涉及"要不要引入新依赖"，而规格 0.12 要求安全保证、支持平台、
  删除策略或产品范围的变动必须先记录理由。

### 决策 1：SQLite 用 `rusqlite 0.40.2` + `bundled`

| 项 | 取值 | 依据 |
|---|---|---|
| rusqlite | **0.40.2** | 2026-09-17 由 `index.crates.io/ru/sq/rusqlite` 实测取得的最高非 yank 版本 |
| libsqlite3-sys | 0.38.2（传递依赖） | rusqlite 0.40.2 的 `bundled` 特性指向它 |
| 特性 | `bundled` | 编译随包发布的 SQLite 源，不依赖系统 `sqlite3.dll` |

- **为什么不用系统的 `winsqlite3.dll`**：它是 Windows 自带的 SQLite，版本与编译选项
  随系统更新而变。规格 8.1 要求确定的 `foreign_keys`、`WAL`、`synchronous=FULL`
  语义，把"迁移与事务语义"交给用户机器的系统组件会引入无法复现的差异。
- **代价**：构建需要 C 编译器（本机为 MSVC）。本机 MSVC 组件未在 vswhere 登记，
  因此新增 `scripts/msvc-env.sh`，见 ADR-012 的后果一节。
- **附带事实**：`rusqlite` 的默认特性里含 `ffi-sqlite-wasm-rs`，但它只在
  `wasm32-unknown-unknown` 目标下生效（索引里的 `target` 条件已核对），
  不影响 Windows 构建。

### 决策 2：本地时区换算用 Win32，不引入日期库

- 使用 `SystemTimeToTzSpecificLocalTime`（`windows` crate 的 `Win32_System_Time` 特性）。
- **为什么不自己拿 `GetTimeZoneInformation` 的 Bias 做减法**：Bias 只描述**当前生效**
  的那个偏移。对一个半年前的 UTC 时间戳，正确偏移还要看那一天的夏令时规则；
  自己维护一份时区规则是必然出错的方向。
- **为什么不引入 `chrono` / `time`**：本项目只需要「纳秒 ↔ 公历年月日」这一小段算术
  （Howard Hinnant 的 civil-from-days 及其逆运算），它可以被固定向量与往返性质完整验证。
  多一个日期库意味着多一份"版本升级改变时区语义"的风险，以及一个与本机平台语义
  可能不一致的第二真源。
- **新增 `domain::time`**：上述算术是纯函数（不读系统时区、不碰磁盘），因此放在 domain 层。
  可表示范围限制为 1601–9999：下界是 FILETIME 纪元，上界让 RFC3339 输出始终是 4 位年份。
  **越界一律返回 `None` → 结构化错误**，不夹取、不猜日期。

### 决策 3：新增错误码 `INVALID_TIMESTAMP`

规格 8.5 的错误表是**处理约定**，不是封闭集合。文件的时间戳无法换算成本地时间时，
用 `INTERNAL` 会把用户的坏数据报成程序缺陷，用 `INVALID_PATH` 会误导用户去改文件名。
因此新增一个能区分「文件有问题」与「我们代码有问题」的码。

---

## ADR-012 T04 的计划、校验与持久化语义

- **状态**：已接受（T04，2026-09-17）
- **背景**：规格 7.1、7.2、8.1 给出了规则，但若干处需要确定"具体怎么落地"，
  否则不同实现会做出互相矛盾的取舍。

### 决策 1：批内源路径一律视为"已占用"

规格 7.1 第 9 条要求「目标与本批其他源路径相同视为冲突并在预览前重新分配」。
实现方式：`TargetLedger` 同时持有三个集合——磁盘现有条目、本批已保留目标、
**本批全部源路径**。分配候选名时三者都算占用（本项自己的源路径除外）。

- 后果：`a.txt → b.txt`、`b.txt → a.txt` 这种互换被降级成
  `a.txt → b (2).txt`、`b.txt → a (2).txt`，而不是按用户想象的方式交换。
  v0.1 明确不支持循环交换，这是**降级**而非"装作支持"。
- 为什么必须这样做：执行顺序不能当作前提——一旦某一项失败，后面项的目标位置
  可能永远不会空出来。

### 决策 2：冲突比较用 `to_lowercase` 近似 Windows 语义，偏差方向安全

Windows 的 upcase 表与 Rust 的 `to_lowercase` 在极少数字符上不一致。
选择近似的理由与边界：

- 把「其实不同」的两个名字判为相同 → 多分配一个 ` (2)`，**无害**；
- 把「其实相同」的两个名字判为不同 → 内核的 `ReplaceIfExists = FALSE` 仍会拒绝覆盖
  （INV-03 由内核保证，不由这里的字符串比较保证）。
- 比较键还会剥离尾随空格与点（Windows 会忽略它们），否则
  「磁盘上已有 `a.txt`」这件事会在比较时漏掉。

### 决策 3：`PlanItem` 的 `fileId`/`action` 与 `FileRecord` 的 `extractionStatus` 进数据库

规格 8.1 的字段清单里没有这三列。但 IPC 契约（规格 5.1）的 `FileRecord`/`PlanItem`
**有**这些字段——少了它们，从库里读回来的对象与用户确认过的对象不是同一个东西，
而执行器只认库里的计划。

- 决策：`files.extractionStatus`、`plan_items.fileId`、`plan_items.action` 作为
  **补充列**加入（规格原文说"必要字段"是最小集）。分析结果本身仍在 `analyses` 表。
- 拒绝的做法：靠"读回来再推导"（例如由 `source == target` 推出 `action`）。
  那是把不变量藏在隐式耦合里，一旦有人手改数据库就会得到自相矛盾的计划。

### 决策 4：迁移本体与版本记录同事务；`schema_migrations` 不由迁移创建

- `migrate()` 先建 `schema_migrations`（`CREATE TABLE IF NOT EXISTS`），
  再逐条把「执行迁移 SQL」与「写入版本记录」放进**同一个事务**。
  中途失败 → 整份回滚 → 既不留下半张表，也不留下"已升版本"。
- 因此 `001_initial.sql` 里**不能**再建 `schema_migrations`：本文件第一版就是这么写的，
  `cargo test --test storage` 直接以 `table schema_migrations already exists` 失败。
  这条缺陷被测试挡在提交之前，记录在此以免有人"顺手补回去"。
- 迁移按**版本升序**执行，与数组顺序无关；已记录的版本不会重复执行。

### 决策 5：问题的严重度约定与 `executable_count` 的含义

| 场景 | 严重度 | 原因 |
|---|---|---|
| 构建期某一项不可选（指纹失败、路径超长、父路径是文件…） | `Warning` | 这一项不能执行，不影响其他项 |
| 校验期**选中项**的问题 | `Block` | 选中了却不能执行 |
| 全局问题（根不符、已密封、选中集合为空） | `Block` | 规格 7.2：全局问题始终阻断 |

- `executable_count` = **当前状态下真的能执行的项数**。存在任何全局阻断问题时它一定是 0。
  这样前端只需看一个数字就能判断能不能进入执行（配合规格 10.3 的
  `isConfirmationCurrent` 也成立）。
- 目标重复属于**批次一致性**问题：涉及的所有选中项一律阻断，而不是"先到先得"。

### 决策 6：摘要的规范序列化

- 输入串格式：`长度前缀:内容\n`，以 `filepilot.plan.digest.v1` 开头。
  只用分隔符拼接时，`["a", "b"]` 与 `["a\u{1f}b"]` 这类输入可能拼出同一个字符串；
  长度前缀让字段边界由内容自己决定。
- 参与字段严格按规格 7.2：planId、revision、根卷身份、根**文件身份**与规范真实路径、
  选中项（按 itemId 排序）的 source/target/expected 与操作类型。
  加上根的文件身份是刻意的加强：同一路径上的目录被换成另一个目录时，
  只比路径的摘要不会变化。
- 选中项按 itemId 排序，因此摘要与计划项在数组里的物理顺序无关。

### 决策 7：校验阶段不重算内容哈希

`validate_plan` 只比对源文件的**身份、大小、修改时间**（走
`scanner::snapshot::fingerprint(path, volume, false)`，不读内容）。
理由：哈希在生成计划时算过、执行前还会再算一次（规格 8.2 的"重新计算并核对源身份/哈希"）。
在校验阶段对 10,000 个文件重算哈希，代价与收益不成比例；而"预览后文件被改"这类问题里，
大小或时间变化的占绝大多数，三者都不变却内容变了的，执行阶段的内容哈希一定会拦住。

### 决策 8：本阶段不签发 `validationToken`

规格把令牌的签发与消费放在 T05（`safety/confirmation.rs`）。
T04 的 `validate_plan` 返回 `validation_token: None`、`expires_at: None`。
**不填一个看起来可用的假令牌**——那会让后续阶段的接口看起来已经实现，
实际上没有任何一次性语义。

### 后果

- 需要 C 编译器才能构建（`bundled` SQLite）。本机 MSVC 组件未登记进 vswhere，
  因此新增 `scripts/msvc-env.sh`（`source` 后 cargo 可用）。
- `plans.digest` 在计划被编辑时会被 `save_plan` 清空：旧摘要绑定的是旧版本，
  留着它等于允许用旧校验结果确认新计划。

---

## ADR-013 T04 二次审查后的持久化与校验加固

- **状态**：已接受（2026-09-17）
- **背景**：T04 首轮验收覆盖了正常迁移、事务回滚与乐观锁，但没有覆盖已有 WAL
  数据库的迁移前备份、计划身份被更新请求夹带替换、revision 跳号，以及预览编辑后
  改扩展名/重复引用源文件的情况。

### 决策

1. 已有数据库确有待执行迁移时，先用 SQLite Online Backup API 生成唯一备份；
   不复制主数据库文件。新建空库和无需迁移的重开不制造多余备份。
2. 新计划 revision 固定为 1；每次更新只能是 `current + 1`。rootId、scanId、mode、
   createdAt 是计划不可变身份，更新请求不得改变。
3. 已应用迁移版本必须是当前程序迁移目录的前缀；未来版本、缺口或未知版本均停止打开，
   返回 `DB_UNAVAILABLE`，不尝试猜测兼容。
4. 最终校验独立复核“扩展名完全不变”和“同一源只能出现一次”。不能只依赖规划器
   首次生成正确，因为 T05 会允许用户编辑，数据库也可能损坏。
5. 目录事实必须完整：枚举条目或读取重解析点属性失败时阻断该项，不把错误吞成
   “不存在/不是链接”。

### 后果

- `rusqlite` 启用 `backup` 特性；备份路径可由 `Database::migration_backup_path()` 获取，
  后续恢复页面可据此提供诊断信息。
- T05 的 `update_plan` 只能修改规格允许的可编辑字段，并必须让 revision 恰好加一；
  后端校验仍是最终边界，前端按钮状态不能替代这些检查。

## ADR-014 一次性确认令牌的设计

- **状态**：已接受（T05，2026-09-17）
- **背景**：规格 7.4 要求「确认令牌只有后端能生成，最多 5 分钟有效，只能用一次」，
  并且令牌要绑定用户确认过的那份计划。

### 决策 1：令牌用操作系统 CSPRNG 生成 32 字节，不用 UUID

`uuid::Uuid::new_v4()` 只有 122 位随机性，且格式固定。令牌需要的是**不可猜测**，
不是"不重复"。改用 `getrandom` 直接对接 OS 的 CSPRNG，取 32 字节再十六进制编码。

**拿不到安全随机数时必须失败**，绝不能退化成时间戳或计数器——那会产出可预测的令牌，
等于没有令牌。这条在 `random_token()` 里是显式的 `map_err`，不是 `unwrap_or_default`。

### 决策 2：只存 `SHA-256(token)`，不存明文

即使有人读到进程内存或拿到一份内存转储，也拿不到可用的令牌。
`tests` 里有一条直接断言仓库的键里不出现明文。

### 决策 3：令牌绑定 `planId` + `revision` + `digest` 三者

只绑 `planId` 不够：同一份计划被编辑后，`planId` 不变而内容已经不同了。
`revision` 挡住"编辑过"，`digest` 挡住"内容被改但版本没动"（例如库里被手工改动）。

### 决策 4：三类拒绝用三个错误码

| 情形 | 错误码 | 用户该做什么 |
|---|---|---|
| 超过 5 分钟 | `TOKEN_EXPIRED` | 重新校验即可，计划本身没问题 |
| 已经用过 | `TOKEN_USED` | 说明重复提交了；不该盲目再点 |
| 不存在 / 版本不符 / 摘要不符 | `STALE_PLAN` | 计划已不是看过的那份，需要重新预览 |

合成一个「确认无效」会让用户失去判断依据。规格 8.5 的错误表里本来就有前两个码，
这里只是把它们用起来。

### 决策 5：令牌签发放在命令层，不放 `planner`

`planner::validate_plan` 是**纯函数**：同样的输入永远得到同样的输出，可复算、可测试。
令牌带过期时间和随机数，是运行期状态，一旦混进去 planner 就不再是纯的。
因此分层是：planner 产出 `digest` 与问题列表 → `commands_plan::validate_plan`
在"确实可执行"时才签发令牌。

**推论**：`planner` 产出的 `ValidationReport` 里 `validationToken` 永远是 `None`
（T04 的测试 `the_validation_report_reports_no_token_in_this_phase` 仍然成立且有意义）。

### 决策 6：可执行项为 0 或存在阻断项时**不签发**令牌

签发一个"反正用不了"的令牌会让前端误判为可以执行。宁可让 `validationToken` 是 `None`，
让界面明确说出原因。

## ADR-015 执行日志的四步闸门

- **状态**：已接受（T06，2026-09-17）
- **背景**：规格 8.2 要求「持久化意图 → 移动 → 核对 → 持久化结果」，
  规格 8.3 的崩溃恢复决策表完全依赖日志能回答「这个文件到底动了没有」。

### 决策 1：`prepared` 必须写在真实的 rename **之前**

代码上体现为 `move_no_replace_with` 的 `before_rename` 回调：
核对通过后、调用 `SetFileInformationByHandle` 之前，先落 `prepared`。
回调失败则整个移动放弃——文件一个字节都还没动，报错是安全的。

**反过来的写法（先移动再记日志）会在两步之间留下一个无法判定的窗口**：
进程在那个瞬间死掉，恢复时只能猜，而规格 8.3 明确禁止猜测。

### 决策 2：`move_no_replace` 保留原签名，新增 `_with` 变体

回调版本会让所有既有调用点都要写一个 `|_| Ok(())`。
保留一个无回调的便捷入口，T03 的测试与将来的撤销路径都不用改。

### 决策 3：幂等交给数据库的 UNIQUE 约束，不靠应用层先查后写

`runs.requestId` 上有 UNIQUE 索引。`Journal::begin` 先查一次（快路径），
撞约束再查一次（并发路径）。**两次查询都不是正确性的来源**——
正确性来自那个索引。应用层的「查了再写」中间有竞争窗口，两个请求会都以为自己是第一个。

### 决策 4：`BeginOutcome` 用枚举而不是 `Option<String>`

两种情形语义完全不同（「刚建好、去执行」vs「早就跑过、去读结果」），
共用一个 `None` 很容易让调用方写反——写反的后果是**把已经移动过的文件再移动一次**。

### 决策 5：`applied` 不可被改回

`set_operation_status` 的 SQL 带 `WHERE status NOT IN ('applied')`。
审计事实一旦写下就不能被后续调用抹掉，即使那是一次重试。

### 决策 6：操作状态与事件分两步写，且**先状态后事件**

反过来的话，可能出现「一条 applied 事件配着 prepared 状态」的组合，
恢复逻辑读到只能当成 ambiguous——白白损失一条确定信息。

### 决策 7：`state_digest` 绑定操作事实

`RunReport.state_digest` 对 `(operationId, itemId, status)` 序列取 SHA-256。
规格 8.3 用它防止用户确认一份过期的恢复报告：两次读取之间任何一项状态变了，
摘要就变，界面据此要求重新核对。

## ADR-016 单实例锁用命名互斥体，不用锁文件

- **状态**：已接受（T07，2026-09-17）
- **背景**：规格 T07 要求「第二实例打开已有窗口或退出，不启动第二执行器」。
  两个进程同时整理同一个目录会把计划互相踩掉。

### 决策 1：`CreateMutexW` + `Local\` 命名空间，不引入 `tauri-plugin-single-instance`

- 已有的 `windows` crate 就够了，只多开一个 `Win32_System_Threading` feature。
- 官方插件会注册一堆我们当前用不到的钩子（聚焦已有窗口、深链接转发等）。
- **关键细节**：`CreateMutexW` 在「已存在」时**也会返回一个有效句柄**，
  只是 `GetLastError` 是 `ERROR_ALREADY_EXISTS`（183）。
  不显式判断就会让第二个实例以为自己拿到了锁——那正是这个特性要防的事。

### 决策 2：为什么不用锁文件

锁文件在进程崩溃后会留下一个「永远锁着」的残留，用户只能手动删。
命名互斥体由内核持有，进程一死（包括被强杀）自动释放。

### 决策 3：放在 `Local\` 而不是 `Global\`

`Global\` 跨用户会话互斥，会让同一台机器上的另一个用户完全打不开应用。
桌面应用期望的是「同一用户开一个」，因此用 `Local\`。

### 决策 4：退出而不是激活已有窗口

激活已有窗口需要进程间通信（管道或窗口消息），而当前阶段并没有
「带参数启动第二实例」的需求。直接退出最简单，也没有歧义：
用户看到的不是「什么都没发生」，而是一行明确的原因。

---

## ADR-017 全局执行锁与取消的检查点

- **状态**：已接受（T07，2026-09-17）

### 决策 1：执行锁用 RAII guard

`begin_execution` 返回 `ExecutionGuard`，drop 即释放。
显式的 acquire/release 一对调用很容易在某条提前返回的分支上漏掉释放，
而那会把应用锁死到下次重启。有一条测试专门验证 **panic 展开也会释放**。

### 决策 2：取消检查放在「派发之后、记录意图之前」的**下一次迭代开头**

取消是**协作式**的：`cancel_task` 只置位 `AtomicBool`，
执行器在下一项开始处理前看到它才停。

**已经派发的那一项会走完**（移动 + 落日志）——半途而废才是真正危险的状态：
文件可能已经移动，而日志里还写着 `pending`，恢复流程只能判成 ambiguous。

### 决策 3：取消请求与取消完成是两个状态，界面上也要分开说

后端的 `cancel_task` 返回的是**当前状态**（可能还是 `running`），
真正的 `cancelled` 由执行器在安全点写入。
前端因此显示「已发出停止请求。正在处理的那一项会先完成……」，
而不是立刻说「已停止」——后者会让用户以为文件已经不再变动。

### 决策 4：停止入口只在 `status === 'running'` 时显示

用 `busy` 判断是不够的：收到 `cancelled` 之后执行器已经停了，
但 `execute_plan` 要等整个循环走完才返回，那段时间里按钮会一直挂着，
用户以为还能再停一次。

## ADR-018 e2e 只验证界面行为，文件事实由 Rust 集成测试负责

- **状态**：已接受（T07，2026-09-17）
- **背景**：规格 T14 要求说清 e2e 的能力边界，避免「e2e 全绿」被误读成
  「文件操作正确」。

### 决策 1：Playwright 跑在 Web 模式 + 严格契约 mock 下

`tests/e2e/cancel.spec.ts` 注入一个最小的 `window.__TAURI_INTERNALS__` 替身
（`isTauriAvailable()` 只检查这个字段是否存在），让整条 IPC 链路在真实浏览器里跑起来。
`invoke` 按命令名返回**字段完整**的契约数据——缺字段会被前端的 Ajv 运行时校验挡下，
这本身也是一条防线。

### 决策 2：e2e 断言的是「请求次数」与「界面状态」，不是磁盘

- 「连点确认只发出一次执行请求」——断言 `execute_plan` 的**调用次数**与 `requestId`；
- 「停止入口在收到终态后消失」——断言按钮可见性；
- 「乱序进度不会让界面倒退」——断言序号守卫生效。

**真实文件行为不在这里验证**。「已完成项保留、未派发项原位、内容哈希不变」
由 `tests/execute_windows.rs` 覆盖：它跑在真实 NTFS 上，逐个比对移动前后的内容哈希。

### 决策 3：报告里必须写明这一限制

任何基于 e2e 的结论都要带上「Web 模式 + mock」这个前提。
把两者混为一谈会让「测试通过」失去意义——一个移动错文件却界面正确的实现，
在 e2e 里是绿的。

## ADR-019 契约里的 `Option<T>` 是「必填可空」，不要用 `#[schemars(required)]`

- **状态**：已接受（2026-09-18，修复 T02 遗留缺陷）
- **背景**：`contracts.schema.json` 曾把 `FilePage.nextCursor` 生成为
  `{"type":"string"}`、把 `TaskSummary` 的 `scanId` / `total` / `error`
  生成为各自的非空类型，而 TS 类型是 `string | null`。

  **后果是真实的**：后端返回 `null` 时前端的运行时校验会拒绝整个响应。
  扫描进行中的任务 `scanId` 就是 `null` —— 这条路径在真机上根本走不通。
  之所以一直没暴露，是因为 jsdom 单测不调真实 IPC。

### 决策：响应与请求统一按「必填可空」生成

`export-contracts` 的 `schema_value()` 已经用 `SchemaSettings::for_serialize()`。
在这个模式下 `Option<T>` 会自动进入 `required` **且保留 `null`**，
正是前端 TS 类型 `field: T | null` 的语义。

**因此不要在字段上加 `#[schemars(required)]`**：它把 `Option<T>` 当成 `T` 处理，
把 `null` 从契约里吃掉。（这一点在 `export-contracts.rs` 的注释里原本就写着，
只是当时还有 6 处标注没删干净。）

### 决策：给 e2e fixtures 加一条契约自检

`tests/ui/contract-fixtures.test.ts` 用 Ajv 校验 e2e 的 fixtures。

这不是多余的重复测试，而是**故障定位工具**：契约一改，`pnpm test:e2e` 里
所有用例会一起挂在某个 DOM 断言上，真正的原因（某个字段少了一个或类型不对）
要看 Playwright 的 DOM 快照才能猜到。有了这条自检，失败信息直接是
`/nextCursor must be string`。

本次缺陷正是被它抓出来的 —— 它同时暴露了 fixtures 与**真实契约**两个问题。

## ADR-020 撤销采用原子请求绑定，恢复读取与目录删除统一走句柄安全边界

- **状态**：已接受（2026-09-19，T09 审查修复）

### 决策 1：撤销的幂等单位是“请求及其全部参数”，不是 token

新增 `undo_requests`：以 `requestId` 为主键，绑定 undoPlanId、originalRunId、
tokenHash、digest 和规范化后的 selected 集合，并保存完整 UndoReport。
全局恢复检查、创建 undo run、登记请求与消费 token 在同一 SQLite 事务完成。
因此同参数重试返回第一次的同一报告；相同 requestId 改参数返回 REQUEST_CONFLICT；
只有新 requestId 复用已消费 token 才返回 TOKEN_USED。

### 决策 2：未决恢复是全应用闸门

apply 与 undo 都查询全库未决状态，不能只检查当前原 run。检查必须发生在执行锁内、
确认消费前。扫描与文件变更也共享一个内存工作流闸门：扫描可以彼此并行，任何整理或
撤销与扫描互斥，防止扫描快照和真实移动在同一时间交错。

### 决策 3：读取已有路径与创建新名称使用不同策略

新目标继续受 80/240 UTF-16 产品限制；数据库已记录的源路径和撤销原路径只做防逃逸、
根身份、祖先重解析点和句柄身份校验。否则应用会拒绝撤销本来就存在的长目录名。
恢复与撤销预览统一使用受保护指纹入口，只有 NotFound 表示不存在。

### 决策 4：目录删除必须删除刚刚核对的同一个对象

撤销清理逐级锁定祖先，目标目录用不共享 DELETE 且拒绝重解析点的句柄打开；在该句柄上
核对目录文件 ID，再用 `SetFileInformationByHandle(FileDispositionInfo)` 删除。
不再采用“按路径检查身份→释放句柄→按路径 remove_dir”的竞态窗口。

### 决策 5：状态与对应审计事件原子提交

ADR-015 的“状态与事件分两步写”被本决策替代。新增事务接口同时更新 operation 状态并
追加事件，任一步失败整体回滚；payload 一律由 serde_json 生成。文件系统 rename 仍不可能
进入 SQLite 事务，因此 rename 后数据库提交失败必须进入 recoveryRequired，由磁盘事实核对。

### 决策 4 的更正（2026-09-20，T10 期间实测发现）

决策 4 写的“目标目录用**不共享 DELETE**的句柄打开”**与 API 相矛盾**，
会让删除**永远失败**——不是偶发，是 100% 确定性失败。

实测（四种组合各跑一次，删一个刚建好的空目录）：

| 打开标志 | 共享模式 | `SetFileInformationByHandle(FileDispositionInfo)` |
|---|---|---|
| `BACKUP_SEMANTICS \| OPEN_REPARSE_POINT` | `READ \| WRITE` | **失败** `ERROR_ACCESS_DENIED` |
| `BACKUP_SEMANTICS \| OPEN_REPARSE_POINT` | `READ \| WRITE \| DELETE` | 成功 |
| `BACKUP_SEMANTICS` | `READ \| WRITE` | **失败** |
| `BACKUP_SEMANTICS` | `READ \| WRITE \| DELETE` | 成功 |

**结论**：`FILE_FLAG_OPEN_REPARSE_POINT` 与成败无关（当时的直觉是反的）；
**必须**在共享模式里带上 `FILE_SHARE_DELETE`——Windows 要求“用这支句柄删除它自己”
时，句柄自身的共享模式必须允许删除。

**为什么加宽共享模式不削弱决策 4 想要的保证**：共享模式描述的是“**别人**可以做什么”。
真正关上“核对 A、删掉 B”那个窗口的，是**身份核对与删除作用在同一支句柄上**——
句柄绑定的是一个具体的内核对象，中途改名换不掉它。反过来，缺 `FILE_SHARE_DELETE`
只是让这个函数恒失败，那才是真的没有保护。

**教训**：这条缺陷是 `cargo test` 报出来的（`created_directory_cleanup_is_bound_to_the_open_
directory_identity` 稳定失败）。两个候选原因（标志位、共享模式）无法靠推理分辨，
用四组对照实验一次定音。**别在“两个都说得通”的解释里挑一个信。**

### 决策 4 的第二次更正：`FileDispositionInfo` 会删掉**非空**目录

修完上面那条之后，另一条测试立刻失败，现场实测（打印整棵文件树）：
撤销后目录连同**用户自己放进去的文件**一起消失，而且**没有告警**——
删除被当成了成功。

**根因**：`SetFileInformationByHandle(FileDispositionInfo, DeleteFile = TRUE)`
在本机（Windows 11）对**非空目录**也会成功，并在句柄关闭时把目录连同里面的
文件一并删掉。这直接违反规格 8.4 第 7 条的“非递归清理”。

**为什么之前没暴露**：前一条缺陷（共享模式缺 `FILE_SHARE_DELETE`）让这个调用
**恒失败**，而那层恒失败把“非空也照删”这个真问题**掩盖**了。

> **可复用的教训**：修好一个“让功能恒失败”的缺陷之后，**必须重新审视它掩盖了什么**。
> 恒失败的功能从来没有真正执行过，它的正确性一次都没被验证过。

**修法**：删除前，在**同一支句柄**上确认目录为空。

1. `open_directory_for_delete` 的访问权限加 `FILE_LIST_DIRECTORY`，
   共享模式改为 `FILE_SHARE_READ | FILE_SHARE_DELETE`——
   **刻意不带 `FILE_SHARE_WRITE`**：只要句柄活着，别人就没法往里放东西，
   于是“查它是空的 → 删它”之间不存在竞态窗口。
2. 新增 `windows::directory_is_empty_by_handle`，用
   `GetFileInformationByHandleEx(FileIdBothDirectoryRestartInfo)` 在**句柄上**枚举，
   跳过 `.` / `..`；缓冲装不下（`ERROR_MORE_DATA`）也算非空。
   用句柄而不是按路径 `read_dir`，避免“查的是 A、删的是 B”。
3. `remove_directory_by_handle` **自己再挡一次**：非空直接返回
   `DirectoryNotEmpty`。这个函数可以被单独调用，而“顺手删掉用户东西”的代价
   不允许它依赖调用顺序。

**回归测试**（永久保留）：
`safety::fingerprint::tests::created_directory_cleanup_never_removes_a_directory_that_still_has_files`。
它断言的是**数据丢失边界**，而且**先取证再断言**——把“目录还在吗 / 文件还在吗 /
内容是什么 / 返回值是什么”全部取出来再 assert：第一个断言失败会让后面的永远不执行，
而这一条要回答的是“丢了什么”，不是“是不是丢了”。

### 决策 4 的第三次更正：没有项被撤销时不做目录清理

`execute_undo` 原本无条件调用 `clean_created_dirs`，而它要通过
`host_operation_id` 把审计事件挂到一条**真实操作**上。当本次没有任何项被撤销
（全是冲突、或全部已撤销）时，undo run 里一条操作都没有 → 报
`INTERNAL: 撤销记录没有任何操作，无法挂载审计事件`，**整个撤销失败**。

改为只在 `reverted > 0` 时清理。两个理由：

- **语义上**：分类目录只可能因为“文件被搬回原位”而变空。一项都没搬，
  就没有“本次造成的空目录”这回事。
- **结构上**：审计事件必须挂在真实操作上（`operation_events` 有外键），
  这时根本没有可挂的地方；硬写只会造出一条永远插不进去的假记录。

## ADR-021 T10 的解析依赖与「不传路径、只传句柄」的隔离设计

- **状态**：已接受（2026-09-20）

### 决策 1：解析器依赖只被 `extract_worker` 链接

`zip 8.6.0` / `quick-xml 0.42.0` / `lopdf 0.45.0` / `encoding_rs 0.8.41`，
版本于 2026-09-20 由 crates.io API 实测取得。

规格 6.2 要求「**不能在 UI 进程直接执行不可信解析**」，而
「Tauri 前端隔离不等于解析器隔离」。把这几个 crate 放在同一个 Cargo 包里
是**构建期**的简化；**运行期**的隔离靠下面两条：

- 真正去碰它们的代码只在 `extractors::{text,pdf,docx}` 里，而这三个模块
  只被 `bin/extract_worker.rs` 调用；
- 界面进程侧（`extractors::mod` 的协调器）不引用任何解析器类型。

### 决策 2：不传路径，只传继承的只读句柄

工作进程的命令行参数里**没有路径字段**（`WorkerArgs`），
它只拿到一个整数句柄。这样「去读一个未授权的路径」在协议层面
**无从表达**，而不是「需要被检查的约定」。

- 界面进程用 `CreateFileW(..., FILE_SHARE_READ, ...)` 打开：从这一刻起
  别的进程不能再写这个文件——比「读完再核对没变」更强，它让「读期间被改」发生不了。
- 用 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 把可继承清单收紧到**恰好那几支**。
  直接 `bInheritHandles = TRUE` 会把父进程**所有**可继承句柄复制给子进程，
  等于把「父进程碰巧开着什么」变成子进程的攻击面。
- `WorkerArgs` 与载荷都设 `deny_unknown_fields`：多传一个字段（比如有人
  「顺手加个 `--path` 方便报错」）是**明确拒绝**，不是忽略。

### 决策 3：作业对象管资源与子进程，管不了网络

| 规格要求 | 落实方式 | 性质 |
|---|---|---|
| 256 MiB 内存上限 | `JOB_OBJECT_LIMIT_PROCESS_MEMORY` | 内核执行 |
| 禁止创建子进程 | `JOB_OBJECT_LIMIT_ACTIVE_PROCESS = 1` | 内核执行 |
| 超时终止 | `TerminateJobObject` + `KILL_ON_JOB_CLOSE` 兜底 | 内核执行 |
| 只接收批准的只读句柄 | 可继承句柄清单 | 构造性 |
| **禁止联网** | **不链接网络代码，输入里没有 URL** | **构造性** |

**最后一行必须如实写明**：Windows 没有「这个进程不许联网」的作业限制。
把它写成「作业对象包办了隔离」会让后来的人以为网络那条已经有机制在挡，
而那实际上靠的是「代码里根本没有那条路径」。

### 决策 4：先读输出再等进程

工作进程把一行 JSON 写在 stdout 上，父进程用匿名管道收。
**必须先读到 EOF 再 `wait`**：响应里带着最多 12,000 个字符的正文，
装不下管道缓冲区时，先等会让双方互等——一个教科书式的管道死锁。

### 决策 5：PDF 用 `extract_text_with_limit`，不用 `extract_text`

`lopdf 0.45` 同时提供两者。后者对页面内容流不设上限，一个构造出来的小文件
就能在解压时把内存吃光。前者按页给上限，正是规格 6.2「前 10 页」+
「压缩炸弹」两条要求需要的形状。加载阶段另用
`LoadOptions::max_decompressed_size` 挡住对象流/xref 流的膨胀。

### 后果

- 「文件 ≤ 20 MiB」这条**管不到**解压之后的大小，因此 PDF 与 DOCX 各自还有
  「解压上限」这一层。两道闸缺一不可：前者防「把 2 GiB 读进内存」，
  后者防「读进来之后膨胀」。
- 负向测试（未列进清单的句柄读不到、作业限 1 进程时开不了子进程）
  都跑**真实子进程**，不是 mock——协议层面「没传路径」只说明我们**没传**，
  说明不了**没继承**。

---

## ADR-022 T11 的 OCR 接线、汉字空格与缓存清理

- **状态**：已接受（2026-09-21）

### 决策 1：OCR 状态独立成命令，不塞进 `get_settings`

探测 OCR 要**起一个子进程**去问系统装了哪些识别语言。把它塞进 `get_settings`
会让每次读设置都付这个代价，而设置不只在设置页被读。

于是：命令 `get_ocr_status` 单独存在，前端**只在用户真的打开设置页时**才调用它
（`App.tsx` 里按 `page === 'settings'` 触发）。注意不能靠「组件挂载」来判断——
所有页面都是常挂载的（切换导航只切 `hidden`），靠挂载等于启动时就跑。

代价是多一个状态与一次请求；换来的是「不看设置页就不起子进程」。
界面上两处加载（设置 + OCR）**各自独立**，OCR 查询失败有自己的重试入口。

### 决策 2：汉字之间的空格必须折叠——这是检索正确性，不是显示问题

实测：一张写着「会议纪要」的图，Windows OCR 交回来的是 `会 议 纪 要`
（它按字符切分 CJK 词边界）。

这份正文**是要拿去检索的**。用户在结果里搜「会议」，在 `会 议 纪 要` 里
一个都搜不到——而 T11 的验收原文写的正是「OCR 有文字时返回可搜索片段」。
所以 `platform::ocr::collapse_cjk_spaces` 把**两侧都是 CJK 的空白**吞掉。

判据刻意保守：

| 输入 | 输出 | 为什么 |
|---|---|---|
| `会 议 纪 要` | `会议纪要` | 汉字之间不写空格 |
| `项目进度报告 2026` | 原样 | 那个空格在分隔中文与数字，有用 |
| `Hello World` | 原样 | 拉丁字母不在折叠范围内 |
| `안녕 하세요` | 原样 | **韩文用空格分词**，动了就错 |

最后一行是反向边界：折叠范围一旦「顺手」扩到所有非 ASCII，就会破坏另一种语言。
有一条单测专门钉住它。

### 决策 3：junction 必须用 `remove_dir` 删，不是 `remove_file`

清理缓存会删文件，而缓存目录里可能出现重解析点。三条**实测**事实：

| 事实 | 实测值 |
|---|---|
| `symlink_metadata(junction)` | `is_dir()==false`、`is_symlink()==true`、`attrs==0x410` |
| `remove_file(junction)` | `Err(ACCESS_DENIED)`，链接与目标都在 |
| `remove_dir(junction)` | `Ok(())`，**只删链接**，目标文件与目录原样保留 |

第一行是好消息：Rust 把 junction 判成**链接而非目录**（没有直接用
`FILE_ATTRIBUTE_DIRECTORY` 位），所以不会误递归进去删掉目标——**数据是安全的**。

第二行是坏消息：用 `remove_file` 删不掉它。后果不是数据丢失，而是
**永远清不干净**：每轮清理都留一条 `skipped`，用户没有任何办法自己修好。

即便如此，代码里仍**显式**判一次 `is_symlink()` 再决定是否递归，
而不是只信 `is_dir()`——那个保证就不依赖 std 的内部细节了。

### 决策 4：图片的错误码分流必须与文本 / PDF / DOCX 一致

第一版把图片的所有错误一律映射成 `Unsupported`，于是同一个
`EXTRACTION_CORRUPT` 在 DOCX 上是 `Failed`、在 PNG 上是 `Unsupported`。
用户会看到「不支持此格式」，而那份文件本该能读、只是坏了。

分流规则（与其他格式对齐）：

| 码 | 状态 | 理由 |
|---|---|---|
| `UNSUPPORTED_FORMAT` | `Unsupported` | 图里确实没有文字，这是照片不是坏文件 |
| `OCR_UNAVAILABLE` | `Unsupported` | **这台电脑**还没准备好，去装语言包就能用 |
| `EXTRACTION_TOO_LARGE` / `EXTRACTION_CORRUPT` | `Failed` | 本该能读却没读成 |

`unsupported()` 的文档里早就写了「把两者混为一谈，会让一个坏文件看起来
像一类正常文件」——图片分支当时正是这么干的。

### 决策 5：OCR 的 WinRT 调用不进进程内单测

`RoInitialize` 是**每线程**的，而 WinRT 公寓绑定线程；libtest 每个用例一个线程。
把初始化结果缓进进程级 `OnceLock` 会让第二个线程调用时**段错误**
（实测 `STATUS_ACCESS_VIOLATION`）。

结论有两条：

1. 每次都调 `RoInitialize`，**不缓存**；
2. 真实 OCR 调用整体移出进程内单测——`platform::ocr` 的单测**只测纯逻辑**
   （原因文案、报告映射、空格折叠、缓冲区长度校验），WinRT 那条路
   由工作进程（`--mode ocrAvailability`）承担，`tests/extract.rs` 从外部验证。

### 后果与已知缺口

- **「OCR 语言包缺失」没有被端到端覆盖**：本机装了 `zh-Hans-CN`，
  无法真实构造这条路径，而目前没有注入开关。已覆盖的是原因文案、
  状态映射、界面分支，以及「不可用时绝不返回编造正文」。
  缺的是「工作进程真的返回 `OCR_UNAVAILABLE`」那一段——**如实记为未覆盖**。
- **图片测试的夹具是一张真渲染出来的 PNG**（`tests/fixtures/ocr-chinese.png`）。
  Rust 侧（`image` crate）只能生成像素、没有字体渲染，造不出「有中文文字的图」。
  生成脚本留在 `tmp/make_ocr_fixture.py`。
- 「超大像素图」在集成测试里用一张**只声明尺寸、不含真实像素**的 PNG
  （`png_declaring`）。实测发现只给 IHDR 会被解析器判为「不是图」，
  必须补齐空的 IDAT 与 IEND。这反而带来一层额外保护：实现一旦被改成
  「先解码再判像素数」，这条用例会立刻变红。
- `KNOWN_ERROR_CODES`（`src/api/client.ts`）是手写的，T11 新增
  `OCR_UNAVAILABLE` 时漏同步了一次。见 ADR-006 的「已知缺口」。

---

## ADR-023 T12 的 HTTP 传输、密钥边界与端点校验

- **状态**：已接受（2026-09-21）

### 决策 1：用 WinHTTP，不引第三方 HTTP 客户端

规格 6.4 对「往外发请求」有四条硬要求。用第三方库就要逐条去查「它的默认值
是什么、哪个开关能改」，而每查错一条都是一次真实的密钥外泄或请求走错地方。
用 WinHTTP 之后，**四条各自对应一个直白的 API 设置**：

| 规格要求 | 设置 |
|---|---|
| 代理环境变量不得把请求转发到外网 | `WINHTTP_ACCESS_TYPE_NO_PROXY` |
| 禁止跟随跨域重定向携带密钥 | `WINHTTP_OPTION_REDIRECT_POLICY_NEVER` |
| 响应体上限 1 MiB | 自己数读到的字节 |
| 60 秒超时 | `WinHttpSetTimeouts` |

附带的好处是**零新增依赖**：这条链路上发的是用户的文件名与文本摘要，
能少一个 crate 就少一个。代价是 WinHTTP 的绑定要自己写，
但那是**一次性**的，而上面四条是每个请求都要遵守的。

### 决策 2：`Secret` 刻意不实现 `Display`，`Debug` 只输出 `***`

「密钥进了日志」最常见的发生方式不是有人故意写 `log(key)`，
而是有人写了 `log(context)`，而 `context` 里恰好有一个 `secret` 字段。

所以 `Secret` **没有** `Display`，`Debug` 也不带内容、不带长度；
要用明文必须写 `.expose()`。`Debug` 在调用点上看得见，而
「格式化一个结构体」看不见——这就是差别。

它同样**不实现 `PartialEq`**：应用里出现「密钥相等比较」通常意味着
有人在写校验逻辑，而那多半是个坏主意（常量时间、失败计数、日志泄露都会跟着来）。
校验密钥正确与否应当由服务端认证去回答。

`Drop` 里会先把字节清零再归还内存：这挡不住「已经被复制到别处的副本」，
但能挡住最常见的一种残留——密钥留在**已归还的堆块**里被下一次分配读到。

### 决策 3：`save_provider` 的存储函数**不接收密钥参数**

规格 3.3：「配置只存 `credentialRef`」。让 `repositories::save_provider`
连密钥都拿不到，「把密钥写进 SQLite」这件事在这条路径上就**无从表达**，
而不是「记得别传」。

同类的构造性设计还有几处：

- `ProviderSummary::new` 只接受一个 `bool has_credential`，
  它的签名里没有地方可以放密钥；
- `probe_batch()` **不接受参数**，所以「测试连通性时误发用户文件」
  连表达的方式都没有；
- `SuggestionItem` 里没有路径字段（与 T10 的协议同源）。

验证不是读代码，而是三层断言：类型里没字段、`PRAGMA table_info` 逐列检查、
**把数据库文件整个读成字节搜密钥**（含 `-wal`/`-shm`）。

### 决策 4：端点解析只认识少数几种形状

通用 URL 解析器会（正确地）把 `http://127.0.0.1@evil.com/` 解析成
`evil.com`——而人眼很容易把 `127.0.0.1` 当成主机。对「本地端点只允许
loopback」这条检查来说，**解析器的宽容就是检查的漏洞**。

所以 `Endpoint::parse` 反过来做：只认识 `http`/`https`、无 `@`、无 `?`/`#`
的形状，其余一律拒绝。每一种被接受的形状都能被完全理解，
不存在「解析歧义」的余地。

**`localhost` 在解析时被就地换成 `127.0.0.1`**。理由不是省一次解析，
而是：名字要靠系统解析，而解析结果可以被 hosts 文件改到任意地址。
换掉之后，**本地端点的连接目标里不存在任何需要解析的名字**。

### 决策 5：新增两个错误码

规格 8.5 的错误表是**处理约定**而不是封闭集合（T04 新增
`INVALID_TIMESTAMP` 时已按同一理由扩过一次）：

| 新码 | 为什么不能复用现有的 |
|---|---|
| `CREDENTIAL_UNAVAILABLE` | 报 `MODEL_AUTH` 会误导用户去改密钥，而密钥本身没问题；报 `INTERNAL` 会把环境问题（凭据服务被禁用）说成程序缺陷 |
| `INVALID_ENDPOINT` | `INVALID_PATH` 会让人以为这是文件路径问题，而它要用户改的是设置页上的地址栏 |

### 决策 6：`ai_contract` 的网络用例之间加一道串行闸门

**这是本 ADR 里唯一一条「因为环境而妥协」的决定**，所以要写清楚。

现象：27 条用例在默认并发度（= CPU 线程数）下会随机失败，
错误是 WinHTTP 12152（无效响应）/ 12030（连接错误），**每次失败的用例都不同**；
而 `--test-threads` 取 1 / 2 / 4 / 8 时**全部通过**。

判定的过程本身值得记一笔：先怀疑 `set_var` 污染（把那条测试挪到独立二进制后
问题依旧），再怀疑假服务器，最后用**并发度梯度实验**定位到「与并发度相关」。
这符合「先判别性质，再找根因」——猜了两次都错，实验一次就对。

为什么接受闸门而不是去修：

1. **产品本身就是串行的**。规格 6.4 写的是「并发 1」。让二十多个测试同时
   建立 TCP 连接，测的是一个产品不会进入的状态。
2. **一条会因为环境而随机变红的测试是不可用的**——它会把真正的回归
   淹没在噪声里。而验收命令保持规格里那一条原样（`cargo test --test ai_contract`）。

代价是这个文件的用例不并行，跑完约 15 秒（并行时 4 秒）。

### 后果与已知缺口

- **兼容云适配器的端到端没有被覆盖**：规格要求兼容云端点必须是 HTTPS，
  而本地假服务是明文 HTTP。本地能验证的是**校验本身生效**
  （`the_compatible_adapter_refuses_a_plain_http_endpoint`），
  适配器的解析逻辑由单测覆盖，真实的 HTTPS 路径留给设置页的
  「测试连通性」在真机上验。这一条如实记为未覆盖。
- **`test_provider` 会真的发一次请求**（到用户配置的端点），
  这是规格 5.2 要求的；它不发送任何用户文件（载荷由 `probe_batch()`
  在编译期写死，那个函数不接受参数）。
- **设置页不提供「显示密钥」的开关**，这是有意的：能显示就意味着能读回，
  而规格 3.3 明确排除了那个能力。界面拿到的 `ProviderSummary` 里
  只有一个 `hasCredential: bool`，也没有地方可以装密钥。
