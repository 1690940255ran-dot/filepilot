# FilePilot 完成版代码审查（2026-09-26）

以下「已确认的问题」保留 2026-09-26 初次审查时的现场描述和当时的测试结果，便于追溯；它们不是当前代码状态。修复结果和本轮复跑证据见紧接着的表格。T17 的正式验收仍不能从开发机测试推断。

## 修复跟踪（2026-09-26）

| 编号 | 当前状态 | 修复与回归证据 |
|---|---|---|
| CR-001 | 已修复 | 草稿未保存时不能校验或执行；迟到的校验报告需与当前计划 ID、revision 匹配，且不能覆盖新草稿。预览组件和 reducer 回归测试通过。 |
| CR-002 | 已修复 | 保存时同时提交目标草稿与勾选草稿，单独取消勾选也会写回计划；组件回归测试通过。 |
| CR-003 | 已修复 | 每次进入历史页重新读库；切页回归测试通过。 |
| CR-004 | 已修复 | 恢复概况专查全库未决记录，不再借用最多 200 条的历史查询；201 条新记录遮蔽旧未决 run 的数据库回归测试通过。 |
| CR-005 | 已修复 | 连点 E2E 直接在同一事件循环触发两次点击，不再等待已经消失的按钮；全套 E2E 为 4/4。 |
| CR-006 | 已修复 | 历史增加稳定游标分页和「加载更多」；同毫秒 52 条记录的数据库测试、前端超过 50 条的组件测试通过。 |

本轮复跑：`typecheck`、`lint`、前端单测 **220/220**、`build`、生产 CSP **2/2**、E2E **4/4**、Rust 全套测试（含 `failpoints`，退出码 0）、`cargo fmt --check`、`cargo clippy --all-targets --features failpoints -- -D warnings` 均通过。Rust 测试和 Clippy 在本机以 `CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_BUILD_JOBS=2` 运行，以避开本机 MSVC 的调试记录编译错误；这不改变源码或发布配置。契约重新生成检查、桌面打包和内容扫描也通过，细节见 `docs/PROGRESS.md` 本轮记录。新包仍未在合格的干净 Win11 x64 机器验收，不能用开发机测试代替。

## 已确认的问题（按优先级）

### CR-001｜P1｜预览的本地改动可绕过保存而重新取得执行确认

- **现象**：预览页的 `plainPlan` 叠加 `drafts` / `selectionDrafts`，所以表格和选中数量显示的是本地未保存状态；但「校验并获取确认」直接调用后端 `validate_plan(planId)`，没有先保存，也没有在存在草稿时禁用。`reportLoaded` 随后重建有效确认，未清除草稿或检查 `hasPendingEdits`。因此用户取消勾选/修改目标后，仍可重新校验并点击「确认并执行」；后端执行的是**数据库里的旧计划**，可能与屏幕上的勾选/目标不同。
- **证据**：`src/features/preview/PreviewPage.tsx:123-129,142-160,410-434`；`src/features/preview/previewReducer.ts:121-137,177-189`；后端 `src-tauri/src/commands_plan.rs` 的 `validate_plan` 从库中读取计划。
- **影响**：确认对话框里的数量、表格里的目标与实际移动的文件可能不一致。后端仍有路径/令牌校验，但用户确认的内容与执行内容不一致，属于发布阻断级交互安全问题。
- **建议**：有草稿时禁止校验/执行并明确提示先保存或丢弃；后端报告到达时还需核对计划 revision 与当前可见状态。新增 UI 回归：改勾选或目标但不保存 → 不得取得可执行确认。

### CR-002｜P1｜只改勾选状态时「应用修改」不保存勾选

- **现象**：`saveEdits` 只遍历 `Object.entries(state.drafts)` 组装 `edits`。单独点击复选框只写 `selectionDrafts`，请求会带空 `edits`；后端 `update_plan` 即使收到空列表仍把 revision 加一，并返回未改变选中的计划；`planLoaded` 清空本地草稿，勾选恢复原值。用户以为刚才取消勾选已经保存，实际没有。
- **证据**：`src/features/preview/PreviewPage.tsx:168-191,347-353`；`src/features/preview/previewReducer.ts:112-113,193-198`；`src-tauri/src/commands_plan.rs` 中 `update_plan` 仅遍历传入的 `edits`，之后无条件 revision +1。
- **影响**：直接导致用户选择丢失，并可与 CR-001 组合成「显示不选、实际仍移动」的高风险流程。
- **建议**：以 `drafts` 与 `selectionDrafts` 的 itemId **并集**构造编辑；没有任何有效变更时不调用 `update_plan`。新增“只取消一项 → 保存 → 重新读取计划 → 该项仍未选中”的端到端/组件测试。

### CR-003｜P2｜历史页在本次会话中不会自动显示新执行记录

- **现象**：`App` 把所有页面常驻挂载、只用 `hidden` 切换；`HistoryPage` 的 `list_runs` 只在初次挂载或点击「刷新」后执行。整理/撤销完成后切到历史，列表仍是启动时的快照，必须手动刷新才能看到新记录。
- **证据**：`src/App.tsx:231-261`；`src/features/history/HistoryPage.tsx:62-83,91-99`。现有历史组件测试只覆盖加载/展开，没有“执行后进入历史”的刷新场景。
- **建议**：在切入历史页或执行/撤销完成时传入刷新信号；保持按需加载逐文件明细，但列表需重新查询。补一条“完成一次 run → 打开历史即可看到它”的集成测试。

### CR-004｜P2｜恢复概况只看最近 200 条，老的阻塞记录可能从界面消失

- **现象**：`recovery_status` 用 `list_runs(db, MAX_LISTED_RUNS)`；`MAX_LISTED_RUNS=200`。若一条未决 run 后又产生 200 条更新的记录，恢复概况会给出 `blocked=false`，首页不会指向那条未决记录。存储层执行准入仍用全表 `EXISTS` 阻断，因此不会绕过文件安全闸门，但用户会遇到“界面看不见原因、操作却被拒绝”。
- **证据**：`src-tauri/src/commands_recovery.rs:97-121`、`src-tauri/src/storage/runs.rs:185-216`；全表准入在 `src-tauri/src/storage/execution.rs:57`。
- **建议**：恢复概况专查所有未解决 run（或单独分页），不要复用历史列表的展示上限；增加“201 条以上、最老的一条未决”的数据库测试。

### CR-006｜P2｜历史没有分页，超过 50 条的旧记录不可从界面访问

- **现象**：规格 `docs/MASTER_PLAN.md:387` 要求 `list_history(cursor, limit)`；当前前端固定请求 `list_runs(limit: 50)`，后端也没有 cursor/offset 参数，历史页没有“下一页/加载更多”。第 51 条及更早的 run 在界面上不可见；若用户需要撤销那次操作，虽然 `preview_undo(runId)` 接口还在，也没有可供普通用户找到该 `runId` 的入口。
- **证据**：`src/features/history/HistoryPage.tsx:9,62-83`；`src-tauri/src/commands_execute.rs:267-283`；`src-tauri/src/storage/runs.rs:185-216`。
- **建议**：实现稳定游标分页与“加载更多”，并用超过 50 条的持久化记录验证旧 run 可见、可展开、可进入撤销。逐文件明细也应评估分页/窗口化：当前 `get_run_items` 一次返回全部操作、`RunItemsTable` 一次渲染全部行，大批量 run 可能有明显卡顿；这一性能风险尚未量测，不记为已证实缺陷。

### CR-005｜P2（测试）｜重复点击 E2E 用例稳定超时，门禁当前不绿

- **现象**：`tests/e2e/cancel.spec.ts:137` 在第一次确认后，对已随对话框消失的按钮再次执行 Playwright `click({ force: true })`。`.catch()` 要等点击超时才运行；本次全套与单独复跑都卡到整条测试的 30 秒上限。把总上限临时调到 60 秒后仍卡到 60 秒，说明不是单纯的“机器慢”。Playwright trace 进一步确认：第一次点击已完成且对话框已消失，第二次点击停在 `waiting for getByRole('button', { name: '确认执行' })`，没有进入后续断言。失败日志中的 `page.evaluate` 错误是测试超时关闭页面的后果，不是出现第二次执行请求的证据。
- **建议**：不要对已消失的按钮做长时强制点击。用同步双击事件/短超时配合确定性断言，或直接在冻结的 IPC Promise 期间验证只收到一次 `execute_plan`。修复测试后重新跑全套 E2E；在此之前不能说 E2E 门禁全绿。

## 交付与框架缺口（不是已确认的代码缺陷）

1. **T17 正式验收证据缺口**：现有 `docs/PROGRESS.md` 与 `docs/T17_REPORT.md` 明确写 T17 未完成；已有 VMware 验收是 Win10 18363 且装有 Python/VS Code，按规格只能算平台探测。用户若已另在干净 Win11 x64 完成，请把该机的 `report.txt`、截图与环境审计补入仓库并更新阶段状态；在看到证据前，本审查不能改写成“正式通过”。
2. **T15 人工验收与真实 CI**：`docs/PROGRESS.md` 仍记两项 T15 人工验收未做；`docs/POST_RELEASE_TODO.md` PR-005 仍记 Windows CI 未在 runner 上观测。它们是验收/交付证据缺口，不代表对应功能不存在。
3. **前端产物体积**：`pnpm build` 成功，但 Vite 提示单个 JS chunk 约 585.73 kB（压缩后 124.40 kB），超过 500 kB 提示线。不是当前功能错误；后续可量测冷启动后再决定是否拆分设置/历史等非首页模块，避免只为消除警告而改框架。

## 初次审查时实际运行的门禁（历史记录）

| 命令 | 结果 |
|---|---|
| `pnpm.cmd typecheck` | exit 0 |
| `pnpm.cmd lint` | exit 0 |
| `pnpm.cmd test` | 17 文件 / **215 passed**，exit 0 |
| `pnpm.cmd validators:check` | 31 个定义一致，exit 0 |
| `pnpm.cmd build` | exit 0；有 500 kB chunk 提示 |
| `pnpm.cmd test:prod-csp` | **2 passed**，exit 0 |
| `pnpm.cmd test:e2e` | **3 passed / 1 failed**，exit 1；见 CR-005 |
| 单独复跑重复点击 E2E | **1 failed**，exit 1；同样 30 秒超时 |
| 单例临时设 `--timeout=60000` | **1 failed**，exit 1；仍耗尽 60 秒，未改测试文件 |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | exit 0 |
| `cargo test --features failpoints --locked -- --test-threads=1` | **未进入测试阶段**：本机 MSVC 编译 SQLite 时 `cl` 报 D8050（无法执行 `c1.dll` / 无法将命令行放入调试记录），exit 1。不能据此判定 Rust 代码通过或失败；未关闭 Defender、未修改系统安全设置。 |

## 初次审查建议的修复顺序（已执行）

先把 CR-001/002 作为同一批修复并加回归测试，确认“屏幕展示 = 持久化计划 = 最终确认”的一致性；再修 CR-005 使 E2E 恢复有效；随后处理历史/恢复可发现性与分页（CR-003/004/006）。所有改动需复跑相关门禁并重新构建安装包；若安装包改变，原 SHA-256 与 Win10 平台探测结果不能充当新包的验收证据。
