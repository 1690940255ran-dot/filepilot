# 测试矩阵

规格 T15 的验收：

> `docs/TEST_MATRIX.md` 包含真实环境、时间、结果、问题与复测证据。
> **UI mock 通过不能替代真机文件执行验收。**

这份文档就是按那句话组织的。每一条都写清楚**用的是什么环境、什么时候跑的、
结果是什么、发现了什么问题、怎么复测的**。

---

## 1. 怎么读这份表

| 列 | 含义 |
|---|---|
| 环境 | 在哪台机器、什么构建 |
| 时间 | 什么时候跑的 |
| 结果 | 通过 / 失败 / **没做** |
| 证据 | 日志、命令、或产出文件的路径 |

**「没做」也要如实写。** 规格 T15 里有几项需要人（安装版走流程、邀请试用者），
它们在这里标成「未做」而不是「通过」——把没做的写成通过，比不做更糟。

**不变量的证据映射不在这份文件里**：`INV-01` ～ `INV-10`（规格 §9.10 的发布门槛项）
逐条列在 `docs/INVARIANTS.md`，可用 `python scripts/check-invariants.py` 当场复跑。

---

## 2. 环境

| 项 | 值 |
|---|---|
| 机型 | 联想 83LT |
| 系统 | Windows 11 家庭中文版 |
| CPU | AMD Ryzen 9 8945HX（16C/32T） |
| 内存 | 16GB DDR5（**单通道**，实际 5200 MT/s） |
| 磁盘 | 954GB NVMe |
| 显卡 | NVIDIA RTX 5060 Laptop 8GB |
| Rust | 见 `rust-toolchain.toml` |
| Node | 22.x（managed） |

> 内存是**单通道**——它会影响扫描这类偏 I/O 的任务，引用数字时要一起说。

---

## 3. 性能基线（规格 T15 第 2、3 条）

**目标**（规格原文）：

> 冷启动 5 秒内可交互；10,000 文件元信息扫描目标 15 秒内；1,000 项命名计划
> 目标 2 秒内（不含哈希、OCR、网络）。**这些是测量目标，未测不能当成事实宣传。**

### 怎么跑

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml \
  --test perf_baseline -- --ignored --nocapture
```

`#[ignore]` 的理由：建一万个文件要几分钟，不该混进每次 `cargo test`。
但它**必须能被一条命令跑起来**，而不是只在某个人的机器上跑过一次。

### 结果

<!-- PERF_RESULT_START -->

**测得时间**：2026-09-23
**构建**：`--release`
**机器**：见第 2 节（16GB **单通道**）

| 指标 | 实测 | 目标 | 达标 |
|---|---|---|---|
| 样本：10,000 个小文件 | 468 KB 合计 | — | — |
| **扫描 10,000 个文件的元信息** | **2.398 s** | ≤ 15 s | ✅ |
| **生成 1,000 项命名计划** | **0.255 s** | ≤ 2 s | ✅ |
| 生成 10,000 项计划（超出规格，供参考） | 2.553 s | — | — |
| 校验 1,000 项计划 | 0.497 s | — | — |
| 峰值工作集 | 49.8 MB | — | — |
| 扫描是否触发上限 | 否 | — | — |

**没有一项接近上限**：扫描快 6 倍，计划生成快 8 倍。

**一条要一起说的**：造这 10,000 个样本本身花了 **401 秒**（6 分 41 秒），
而扫描它们只用 2.4 秒。差别来自杀软对**每一次 `fs::write`** 的实时检查——
创建比扫描慢一百多倍。引用「10,000 个文件要多久」时，这个区别很重要：
**整理别人机器上的既有文件，不会遇到这 401 秒。**

<!-- PERF_RESULT_END -->

### 连续 10 轮不累积（规格 T15 第 4 条）

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml \
  --test perf_baseline ten_consecutive -- --ignored --nocapture
```

判据：第 10 轮的耗时**不得**比第 1 轮慢 5 倍以上。绝对耗时受机器影响，
但「越跑越慢」是缺陷。

<!-- ROUNDS_RESULT_START -->

**测得时间**：2026-09-23（与上面同一次运行）
**样本**：500 个小文件，连续扫描 10 轮

| 轮次 | 耗时 |
|---|---|
| 0 | 0.144 s |
| 1 | 0.136 s |
| 2 | 0.136 s |
| 3 | 0.145 s |
| 4 | 0.137 s |
| 5 | 0.136 s |
| 6 | 0.137 s |
| 7 | 0.139 s |
| 8 | 0.139 s |
| 9 | 0.136 s |

**结论**：无累积趋势——第 10 轮与第 1 轮持平（甚至略快）。10 轮之间的
波动在 0.136–0.145 s 之间，是正常的调度抖动。

<!-- ROUNDS_RESULT_END -->

---

## 4. 功能验证

### 4.1 单元与集成测试（真实文件操作）

| 范围 | 命令 | 结果 | 说明 |
|---|---|---|---|
| Rust 全量 | `cargo test --features failpoints -- --test-threads=1` | **687 通过 / 0 失败 / 2 ignored**（19 个 `test result` 行） | **真实 NTFS 文件操作**，不是 mock |
| 前端 | `pnpm test` | **215 通过 / 17 文件** | 契约 mock |
| 端到端（浏览器） | `pnpm test:e2e` | **4 通过** | **契约 mock**——见下 |

> 前端计数 **215 / 17** 为 2026-09-24 复跑（PR-001~004 批次修复后）。
> 此前记为 182 / 14；两轮增量来自本次为四条缺陷新增的三个测试文件，
> 见 §4.1.3。Rust 侧仍为 687 / 0，与批次前一致
> （本次改的是契约层新增类型与命令，未触既有用例）。

> **`pnpm test:e2e` 没有验证真实文件操作。** 它跑在 Playwright 的 Web 模式下，
> 所有 IPC 都由严格契约 mock 提供（见 `tests/e2e/fixtures.ts`）。它验的是
> **界面在给定后端响应下的行为**，不验后端真的动了文件。规格明确要求
> 「报告必须标明其没有验证真实文件操作」——就是这一句。

#### 4.1.1 「687」这个数字的口径（2026-09-25 核对）

有过一次「558 还是 687」的疑问。答案：**687 是对的计数，558 是我自己数错**——
详见本节末尾那段。但这一轮核对顺带查清了一件更值得写下来的事：
**本地与 CI 的「全绿」不是同一个集合**，虽然两边都在 Windows 上跑。

| 项 | 本地（Windows 11 + MSVC） | CI（`windows-latest`，见 `.github/workflows/ci.yml` 的 `rust` job） |
|---|---|---|
| 运行平台 | Windows 11 | **Windows Server**（不是 Linux，两边都是 Windows） |
| 命令 | `cargo test --features failpoints -- --test-threads=1` | 同上，多一个 `--locked` |
| 可发现的用例数 | **687** | **应当也是 687**，但**从未在 runner 上跑过** |
| `--test-threads=1` | 是 | 是（理由写在 workflow 注释里：绕开 6.2 的偶发失败） |

> **CI 的绿至今是零次**。`rust` job 已经配好在 Windows runner 上跑同一套命令，
> 但它**一次都没有真正跑过**——推送触发前，这条「两边一致」只是**配置一致**，
> 不是**结果一致**。这两者的差别，正是 8.8 那个白屏缺陷教给我们的东西。

**手动核对方式**（不要只信总数行）：

```bash
# 逐个数：把每个 test 文件的用例行数出来，加起来
grep -c '^test .* \.\.\. ok' <每个测试文件>   # 或按 test result 行汇总
```

本地逐文件实测（2026-09-25，`tmp/tmp-rust-serial.txt` / `tmp/gate-rust.log` 两份日志一致）：

| 目标 | 通过 |
|---|---|
| `unittests src\lib.rs` | 406 |
| `tests\ai_contract.rs` | 36 |
| `tests\ai_proxy_isolation.rs` | 1 |
| `tests\analysis.rs` | 29 |
| `tests\execute_windows.rs` | 34 |
| `tests\extract.rs` | 34 |
| `tests\naming.rs` | 18 |
| `tests\recovery_windows.rs` | 27 |
| `tests\root_scope.rs` | 11 |
| `tests\scan.rs` | 13 |
| `tests\storage.rs` | 20 |
| `tests\undo_windows.rs` | 22 |
| `tests\validate_plan.rs` | 36 |
| 其余 bin / doc-tests | 0 |
| **合计** | **687** |

> **一次差点写进文档的错误**：先前把总数记成 **558**，来源是
> `406 + 36 + 1 + 29 + 34 + 34 + 18`——**只取了日志里前 7 个 `test result` 行**，
> 后面 6 个目标（`recovery_windows` / `root_scope` / `scan` / `storage` /
> `undo_windows` / `validate_plan`，共 129 条）被**静默丢掉**了。
> 这个数字**当时并没有被写进任何交付文档**，发现于本轮的逐项复核
> （逐文件统计见上表：13 个目标相加正好 687，与日志的总数行一致）。
>
> **教训**：`grep 'test result' | head -7` 这种写法**永远不会报错**，
> 它只是给你一个看起来很像真的数字。**总数必须逐个目标加，或者用一条
> 会非零退出的脚本判**——「数字看起来合理」不是校验。

#### 4.1.2 为什么本地日志里少了 `tests/extract.rs` 的 `Running` 行（2026-09-25 实验）

核对时发现一个看起来像"用例没跑"的迹象：`tmp/gate-rust.log` 与
`tmp/tmp-rust-serial.txt` 报出的 `Running unittests src\lib.rs …` 里，
**lib 的条目带 `src\lib.rs`，而 `[[test]]` 的条目只有裸名字**
（`Running tests\extract.rs` 而不是 `Running tests\extract.rs (src-tauri\tests\extract.rs)`）。
这让人怀疑「lib 里那 406 条里混着本该属于 `tests/*.rs` 的用例」。

**假设**：`Cargo.toml` 里 `[[bin]]` 之后紧跟 `[profile.release]`，
导致本来写在 `[[bin]]` 与 `[profile.release]` 之间的 `[[test]]`
被**解析成 profile 下的未知键而静默丢弃**。

**决定性实验**：临时加一条锚定

```toml
[[test]]
name = "tmp_extract"
path = "tests/extract.rs"
```

重跑 `cargo test --features failpoints --test tmp_extract -- --test-threads=1`，cargo 报：

```
Running tests\extract.rs (target\debug\deps\tmp_extract-36f41687cffcfa29.exe)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.72s
```

`Running` 行出现、且路径括号随之变成 `tests\extract.rs` —— **假设成立**。
（实验后已复原 `Cargo.toml` 并删除备份，项目保持干净。）

**结论**：这**不是 bug**——`tests/*.rs` 里的用例全都真的在跑，
只是**归属显示**不同（前 5 个按 lib 目标编译 → 计入库的 406；其余按各自集成测试目标）。
因此 687 这个总数是对的，**没有用例被跳过**。

> **要记住的位置陷阱**：`[[bin]]` 必须是文件里**最后一个表**。
> 在它之后再写 `[[test]]` / `[profile.*]` 会被**静默丢弃**——
> `cargo build` 与 `clippy --all-targets` 全绿、零警告，
> 只有 `cargo test` 的 `Running` 行少了几条。**唯一的判别手段就是上面那种锚定实验。**
>
> 本轮**不改** `Cargo.toml`（无 bug，且改动牵动 workspace/harness 等无法在不推送的前提下
> 完整验证的配置）。若将来要新增 `[[test]]`，先把 `[[bin]]` 挪到 `[profile.*]` 之后。

#### 4.1.3 PR-001~004 批次新增用例（2026-09-24）

四条发布后缺陷的回归用例，共 **33 条 / 3 个新文件**，
前端总数因此由 182 升到 215。缺陷描述见 `docs/POST_RELEASE_TODO.md`。

| 文件 | 条数 | 覆盖 | 关键观测条件 |
|---|---|---|---|
| `tests/ui/confirm-feedback.test.tsx` | 6 | PR-004 | `execute_plan` **永不 resolve** |
| `tests/ui/history-items.test.tsx` | 12 | PR-003 | 明细按 runId 懒加载 |
| `tests/ui/presentation-invariants.test.ts` | 15 | PR-001/002/004 | 解析 `styles.css` 源码 |

**PR-004 为什么必须冻结执行**：`execute_plan` 用一个立即完成的 Promise 替身，
`await` 一闪而过，对话框瞬间关闭，断言**永远是绿的**——这正是 C 节与 D 节
验收漏掉这个缺陷的原因（当时只测了 8~10 个文件）。
所以用例固定使用 `new Promise(() => {})` 让执行**永不返回**，
把「执行期间」这个中间态变成可观测的。

**PR-001/002 的诚实边界（重要）**：**jsdom 不实现 CSS 级联与布局**。
任何形如「这两个元素间距是多少」「这个数字有没有右对齐」的断言，
在 jsdom 里都是**真空为真**（vacuously green）——元素没有计算样式，
比较结果永远相等，测试通过但不证明任何事。
因此这些用例**改为解析 `src/styles.css` 源码**，断言**结构性不变量**：

| 断言 | 防的是什么 |
|---|---|
| 五个 `.issue*` 类都有规则 | 退回「类名没有任何样式」的原始缺陷状态 |
| `.issue` 用 `display:flex` + `gap` | 退回「靠相邻 margin 凑间距」的脆弱写法 |
| `.issue-scope` 有等宽字体 + 底色 | 退回「作用域与正文视觉同级、读成一句话」 |
| 三个 `.badge-*` 存在 | 徽章退化成裸文本 |
| `.modal-backdrop` 是 `fixed` + `inset:0` | 遮罩不再覆盖全屏 |
| `.progress-panel` 是 `sticky` + `tabular-nums` | 进度面板被长列表顶出视野 |
| `.info-grid dd` 右对齐 + `tabular-nums` | PR-002 的数字列错位 |
| `.info-grid` 第一列不是 `auto` | 标签列宽随内容抖动 |

> **这证明的是什么**：不会**退回**「完全没有样式」的状态。
> **这不能证明**「看起来好看」——间距是否舒适、颜色对比是否足够、
> 在 125% 缩放下会不会错位，jsdom 一概判不了。
> 后者只能靠 §C2 / §D2 的真机肉眼验收。**两边的结论不可互相替代。**

### 4.2 大文件哈希的进度与取消

| 用例 | 结果 |
|---|---|
| 进度回调单调递增，最后一次正好等于文件大小 | ✅ |
| 中途取消返回 `None`（不是 `Err`），且很快生效 | ✅ |
| 一开始就取消：一块都不读 | ✅ |
| 受控路径与普通路径给出同一个哈希 | ✅ |

### 4.3 无网络启动

| 用例 | 结果 |
|---|---|
| 启动路径不含会联网的命令 | ✅ |
| 启动路径只含那几条本地命令 | ✅ |
| 模型服务不可达时启动仍完成 | ✅ |

---

## 5. 人工项（未做）

规格 T15 里这几项**需要人**，如实标为未做：

| 项 | 状态 | 为什么没做 |
|---|---|---|
| Windows 实际**安装版**走一遍规则 / AI 本地 / AI 云授权 / 取消 / 撤销 / 恢复 | **未做** | 需要先出安装包（T16），并且要在真机上手动点 |
| 邀请 5 位试用者，记录首次完成率与采纳/修改原因 | **未做** | 需要真实的试用者与授权脱敏样本 |

**这两项不做完，T15 就不算完成。** 上面所有自动化测试加在一起，也不能替代
「一个人用安装版把自己的文件整理了一次」。

---

## 6. 已知问题

### 6.1 `platform::child_process` 的偶发失败

见 `PROGRESS.md` 的「已知问题」一节：一条跑真实子进程、检查句柄值的用例在
全量跑时偶发失败。已做的判别实验（单跑通过、`--test-threads=1` 也失败）
指向「句柄值被前序测试复用」，下一步是把它挪到独立的集成测试文件。

**它不是产品缺陷**，但**一条会因为环境而随机变红的测试是不可用的**——
它会把真正的回归淹在噪声里。

### 6.2 `tests/analysis.rs` 假 HTTP 服务的偶发失败（2026-09-24 新发现）

**现象**：跑这个测试文件时，**随机**有 1–4 条用例挂在

```
AppError { code: "MODEL_TIMEOUT", message: "...（WinHTTP 错误 12152 / 12030）" }
```

而期望是 `MODEL_AUTH` / `MODEL_INVALID_OUTPUT` / 「分析应当成功」。
每次挂的用例都不一样。

**验证性实验**（同一命令，只改并发度）：

| 实验 | 结果 |
|---|---|
| `--test analysis`，默认并行，连跑 7 次 | **5 次全绿 / 2 次挂 1–2 条** |
| `--test analysis`，`--test-threads=1` | **29 通过 / 0 失败** |
| 全量 `cargo test --features failpoints --locked`（默认并行） | 挂 3 条 |
| 同一条命令 + `--test-threads=1` | **687 通过 / 0 失败 / 2 ignored，exit 0** |

结论：**与并发相关的偶发失败**，不是确定性缺陷。

**已做的一处夹具改动**（不是修复）：

`tests/support/fake_http.rs` 原先在 **accept 时**就弹出脚本回复，于是
「连上但没发请求」的连接（`Drop` 里唤醒 accept 的那次 connect、或端口回收后
落到新服务上的连接）会**吃掉一条本该给真实请求的回复**，让后续回复整体错位。
改成**先读到请求再取回复**。

> **诚实的话**：这个改动本身语义更正确（连接 ≠ 请求），
> 但**没有证据表明它修掉了本次偶发失败**——改完连跑 5 次仍挂了 2 次。
> 所以它记作「顺带修正的一处语义错误」，**不是**这个问题的解。

**下一步（未做）**：按 `windows-flaky-test-triage` 的路子走——给失败断言加临时
诊断取证（把 WinHTTP 错误码、请求序号、服务收到的请求数一起打出来），
先分清是「客户端复用了一条已死的连接」还是「夹具端口回收导致回复错位」，
再决定改夹具还是改产品代码路径。

**当前处置**：CI 与本地验收对 Rust 侧用 `--test-threads=1`（确定性优先），
并在 workflow 里写明这是**绕开已知偶发失败、不是修复**。
**这件事不能一直挂着**：它让「全绿」的含义变模糊。

---

## 7. 跨层一致性审查（2026-09-23）

用 `cross-layer-coverage-audit` 的手段做了一次收尾审查：

| 手段 | 对象 | 结果 |
|---|---|---|
| 集合对比 | 错误码（Rust ↔ 前端已知集合） | 39 = 39，一致 |
| 集合对比 | 注册命令 ↔ 前端 `CommandName` | 27 = 27，一致 |
| 未接线模块 | Rust 源文件 ↔ `mod` 声明 | 无孤儿 |
| 未接线模块 | 前端模块 ↔ `import` | 无孤儿 |
| 可疑模式 | `unreachable!` / `todo!` / `unwrap` | 见下 |
| 注释一致性 | 「100 MiB 上限」→ 实现 | 一致 |

### 发现并修复

**一处**：`scanner/snapshot.rs` 的 `sha256_of_handle` 里有个 `unreachable!`。
它逻辑上不可达，但这个函数在**执行路径**上——一个 panic 会炸掉整批已完成的
工作。改成返回 `io::Error`。

判据不是「它是否真不可达」，而是**「万一它可达，代价是什么」**。

### 当时的门禁

Rust **687** 通过 / 0 失败、前端 `vitest` **179** 通过 / `playwright` **4** 通过；
`fmt --check`、`clippy -D warnings`、`tsc`、`eslint` 干净。

---

## 8. P8 打包与交付验收（2026-09-24）

规格 T16：「生成 Windows x64 用户级安装包，无需管理员权限；
**打包 OCR/解析 worker 所需资源并验证可找到**。」

### 8.1 本机实跑打包

| 项 | 值 |
|---|---|
| 命令 | `pnpm desktop:build`（`beforeBuildCommand` 内含 `pnpm bundle:workers`） |
| 时间 | 2026-09-24 |
| 环境 | 见第 2 节（开发机，非干净机器） |
| 结果 | **通过** |
| 产出 | `src-tauri/target/release/bundle/nsis/FilePilot_0.1.0_x64-setup.exe` |
| 体积 | 3,110,789 字节（3.0 MB） |
| SHA-256 | `7d0388fed71b54002e6b8222bb95ecfad7123a1748ae59e2bccf56e793a612e5` |

> 这一组数字是**本机最后一轮构建**的产物。**换一次构建就对不上**——
> 同机连续两次构建的哈希与体积都不同，原因与影响见 8.7。
> 发布时要先把产物冻结，再对它计算哈希。

解析工作进程的落点：

| 位置 | 文件 | 体积 |
|---|---|---|
| staging（`externalBin` 要求它先存在） | `src-tauri/binaries/extract_worker-x86_64-pc-windows-msvc.exe` | 1.8 MB |
| tauri 构建脚本拷入的 target 目录 | `src-tauri/target/release/extract_worker.exe` | 1.9 MB |

### 8.2 第一次打包是**失败**的（记录在案）

第一次跑直接失败：

```
resource path `binaries\extract_worker-x86_64-pc-windows-msvc.exe` doesn't exist
[bundle:workers] 编译失败（exit 101）
```

这不是配置写错，是**顺序死锁**：`tauri-build` 的构建脚本会校验 `externalBin`
里的文件存在，而 `extract_worker` 是同一个 crate 的 bin 目标——编译它必然触发
那个构建脚本。修法是用 `TAURI_CONFIG` 覆盖让 **staging 这一次**编译不校验
externalBin（细节见 `docs/PROGRESS.md` 的 T16 一节）。

**证据链**：带覆盖单独编译 → `exit=0`（1m59s）→ 改脚本 → 完整打包 `exit=0`。

### 8.3 「worker 在安装包里」这条**只做到一半**

| 手段 | 结论 |
|---|---|
| staging 文件存在且非空 | ✅ 1.8 MB |
| tauri 构建脚本成功把它拷进 `target/release/`（文件缺失时这一步会失败） | ✅ |
| 打包整体成功 | ✅ |
| **在安装包里直接列出文件名** | ❌ **没做到**：NSIS 的 LZMA 是固实压缩，直接搜安装包字节，`extract_worker.exe` / `filepilot.exe` **都搜不到**——所以这个搜法命中或未命中都不构成证据 |
| **安装后确认它与主程序同级** | ⏳ **未做**：需要真机安装（T17） |

> 规格那句「验证可找到」的完整兑现要等安装版验收。
> 现在的状态是「构建链路上它确实被收进去了」，**不是**「装完之后确实在那儿」。
> CI 里额外加了一条断言（staging 文件必须存在），因为漏掉它时
> **编译和打包都会成功**——只有用户第一次用 AI 模式才会发现解析不了。

### 8.4 CI（写好并本地校验，**尚未在 runner 上跑过**）

| 项 | 状态 |
|---|---|
| `.github/workflows/ci.yml` 五个 job（web / rust / contracts / e2e / bundle） | 已写好 |
| YAML 可解析 | ✅ 用真实解析器（PyYAML）验证过 |
| 断言 staging 存在 + 输出安装包 SHA-256 | ✅ 已加进 `bundle` job |
| 在 GitHub runner 上实跑 | ❌ **未做**：还没推送触发过 |

**未经运行验证的 CI 不能算通过。** 这条留在「未做」。

---

### 8.5 发布物内容扫描（2026-09-24）

规格 T16：「**发布内容不包含真实路径样本、API Key、模型返回原文或用户资料**」。

做成了一条命令 `scripts/scan-release-content.py`（清单第 5 节曾要求「逐项检查」，
而靠人眼逐字节翻二进制不现实）。判定规则：

| 对象 | 判据 |
|---|---|
| 文本（源码/文档/脚本/前端产物） | 出现**本机用户名**构成的用户目录路径，或本机开发目录名 |
| 可执行文件 | 出现**任意** `X:\Users\<名字>\` —— 二进制里不该有编译机的任何用户路径 |

判定基准是**运行时取当前用户名**，不写死在脚本里（写死等于让脚本自己成为泄漏源）。

#### 第一次扫描：命中 3 处，其中 2 处是发布物

| 命中 | 性质 | 处置 |
|---|---|---|
| `scripts/cleanup-temp.py` 写死本机开发目录 | 会进公开仓库的真实路径样本 | 改成从 `__file__` 推导根目录，并**默认只预演**（`--apply` 才真删） |
| `docs/PROGRESS.md` 里一条环境备注写了绝对路径 | 同上 | 改成 `<用户目录>\...` 形态 |
| **`filepilot.exe` / `extract_worker.exe` 内含编译机用户路径** | 见下 | 用 `--remap-path-prefix` 修 |

#### 二进制里的路径是怎么进去的

逐字节搜出来的上下文是：

```
C:\Users\<用户>\.cargo\registry\src\index.crates.io-...\anyhow-1.0.104\src\error.rs
C:\Users\<用户>\.cargo\registry\src\index.crates.io-...\brotli-decompressor-5.0.3\src\decode.rs
```

来源是**依赖 crate 的 panic 位置**（`file!()` 是编译期字面量）。这解释了两件事：
为什么 `strip = true` 去不掉它（它不是调试信息，是字符串字面量）；
为什么泄漏的是 `.cargo\registry` 而不是项目目录（依赖比我们的代码 panic 得更多）。

**修法**：`scripts/build-desktop.py` 统一给 release 构建加
`--remap-path-prefix`（项目目录 → `.`、cargo registry → `/cargo`、
rustup toolchain → `/rust`），用 `CARGO_ENCODED_RUSTFLAGS` 传递——
`RUSTFLAGS` 会被 cargo 按**空白**切分，路径里有空格就会坏。

**代价要说清楚**：RUSTFLAGS 变了 → cargo 指纹随之变化 → **一次全量重编译**
（release 十几分钟）。这是有意的，换来的是产物不含编译机信息。

#### 修复后

| 项 | 结果 |
|---|---|
| `scripts/scan-release-content.py` | **0 类命中**（修复前 2 类：两个 exe） |
| 逐字节复核 | `cj169`、`Desktop` 在 `filepilot.exe` / `extract_worker.exe` 里**都已消失**；`/cargo` 出现，即路径确实被改写成了通用前缀 |
| 安装包 | 重新构建（哈希与体积见 8.1；**每次构建都会变**，见 8.7） |
| **安装包内的压缩负载** | ⚠️ **本脚本覆盖不到**（NSIS 固实 LZMA，搜字符串命中与未命中都不算证据）——留给人工 |

> 一处如实说明：本轮产物是**两条**映射构建的（项目目录 + cargo registry）。
> 脚本里第三条 `~/.rustup/toolchains` 当时因笔误写成单数而被静默跳过；
> 本机产物里逐字节确认**不含** `.rustup` 路径，所以那条映射是**预防性**的，
> 不需要为它重建。脚本已修正为复数。

> **判据**：扫描器只对**它可以证明的东西**下结论。安装包那一格永远写「未做」，
> 直到真的装一遍看过。

---

### 8.6 WebView2 依赖与「卸载保留用户数据」（2026-09-24，配置级证据）

T17 有两项在开发机上测不出来（开发机必然有 WebView2、也不该为了测试去卸载环境），
所以这里给的是**读源码拿到的配置级证据**，真机那一格仍然留给
`docs/CLEAN_MACHINE_ACCEPTANCE.md`。

#### WebView2 安装模式：把默认值写显式

`bundle.windows.webviewInstallMode` 现在显式写成
`{"type": "downloadBootstrapper", "silent": true}`。

依据（**读的是实际链接的版本** tauri-utils 2.9.3 的 `config.rs`）：

```rust
impl Default for WebviewInstallMode {
  fn default() -> Self { Self::DownloadBootstrapper { silent: true } }
}
```

也就是说**行为没变**，写显式的价值在于**钉住语义**：将来若上游改了默认值，
这个项目不会跟着漂移（最坏情形是默认变成 `skip`——缺 WebView2 的用户会装出一个打不开的程序）。

可选值（同上源码）：`skip` / `downloadBootstrapper` / `embedBootstrapper`（+1.8 MB）/ 
`offlineInstaller`（+127 MB）/ `fixedRuntime`（+180 MB）。
选 `downloadBootstrapper` 的理由：目标是 Windows 11，系统通常自带 WebView2，
只在缺失时联网引导；代价是**安装阶段可能需要网络**——这条已写进 README 的安装说明。

| 项 | 状态 |
|---|---|
| 已装 WebView2 的机器：安装不额外下载 | ⏳ 未做（需真机） |
| 移除 WebView2 的机器：安装能引导 | ⏳ 未做（需真机） |
| 断网 + 缺 WebView2：失败提示可理解 | ⏳ 未做（需真机） |

#### 卸载：保留用户数据

依据是**配置面**：`NsisConfig`（同上版本 `config.rs`）里**没有任何**
删除应用数据的字段——卸载器只处理安装目录与快捷方式。

应用数据位置由项目自己决定（`platform/cache.rs::app_data_dir`）：
`%LOCALAPPDATA%\FilePilot\`（数据库 `filepilot.db` 与 `cache\`）。

| 项 | 状态 |
|---|---|
| 卸载只移除程序、`%LOCALAPPDATA%\FilePilot\` 仍在 | ⏳ 未做（需真机） |
| 整理过的用户文件不受卸载影响 | ⏳ 未做（需真机） |

> **为什么不当成「通过」**：「配置里没有删数据的选项」是结构性推理，
> 不是这台机器上的实测。真机上卸载一次只要两分钟，而**数据丢失是不可逆的**——
> 这类结论不值得靠推理省下来。

---

### 8.7 安装包**不是逐字节可复现的**（2026-09-24）

这一节是「顺手验证一个推断」验出来的，而**推断是错的**——它的价值就在这里。

#### 起因

给 `tauri.conf.json` 加了显式的 `webviewInstallMode`（值等于默认值，
源码依据见 8.6）。推断是「行为不变 → 产物不变 → 哈希不变」。
跑一次重建来验证这个推断，结果：

| 构建 | 安装包体积 | SHA-256 |
|---|---|---|
| #1 加路径重映射后 | 3,110,470 | `c9ffe272…923f26` |
| #2 加显式 webviewInstallMode | 3,112,424 | `ee7c418d…5cab77` |
| #3 什么都不改，再建一次 | 3,112,424 | `4bfc6fac…361a08` |

**推断被证伪**：`#2` 与 `#3` 配置完全相同、机器相同、连续构建，
哈希仍然不同；而且 `#1` 与 `#2` 的**体积也不一样**（差 1,954 字节）。

#### 差异定位

把 `#3` 的产物留一份副本，与 `#4` 逐字节比对：

| 观察 | 值 |
|---|---|
| 体积 | 3,112,424 vs 3,111,240（差 **1,184 字节**） |
| 逐字节不同 | 97.8%（压缩流整体位移，属连带效应） |
| **首个差异偏移** | **52,760**，紧跟 NSIS 首头 `ef be ad de "NullsoftInst"` |
| 该处内容 | 后随的长度字段不同（`0xe8af2e` vs `0xe8ab48`）→ **压缩负载本身不同** |
| 主程序 exe 的 PE 头 | `TimeDateStamp = 0x6ab4b41b`（**真实构建时间**，不是固定值） |

结论：至少有一个**每次构建都变**的输入（PE 头的时间戳是最确定的一个），
它让 exe 字节不同 → NSIS 压缩的输入不同 → 安装包不同。
**具体还有没有别的可变输入，本轮没有定位完。**

#### 一次**设计错误**的实验（记在案，因为它差点变成错误结论）

我接着只重编了主程序（`cargo build --release`）再比对，得到「体积从 5,533,184
掉到 5,051,392，少了 481 KB」，看起来像「不确定性比想象的大得多」。

**这个对比不成立**：前一份是 `tauri build` **打过 bundle 信息补丁**的 exe
（构建日志里有 `Info Patching … with bundle type information: nsis`），
后一份是纯 `cargo build` 的产物。**两者不是同一过程的两次运行**，
那 481 KB 是补丁差异，不是不确定性。

> **可复用的判断**：比较「两次构建是否一致」时，**先确认两次的过程完全相同**。
> 一次是 `tauri build`、另一次是 `cargo build`，比出来的任何差异都不能归因于不确定性。

#### 对发布流程的直接影响（写进清单）

- **SHA-256 只能对「冻结的那一份发布文件」计算**，并且要写进发布页。
- **不要**用本地重建的哈希去核对已发布文件——同机连续两次构建的哈希就不一样，
  那样做会得到一个假警报（或者更糟：以为文件被篡改）。
- 因此本项目**不宣称可复现构建**（至少在 NSIS 这一层做不到）。
  想要可复现，需要给链接器加 `/Brepro`（去掉 PE 时间戳）、并逐个排查
  NSIS/makensis 自身的可变输入——**本轮未做**。

---

### 8.8 安装版**整页白屏**（2026-09-24，T17 抓到的第一个真机缺陷）

这一节是 T17 真正存在的理由：**开发机上全绿、安装完之后打开是一片白**。

#### 现象与取证过程

| 步骤 | 观察 |
|---|---|
| 静默安装 + 启动 | 进程存活、窗口标题正确、`msedgewebview2.exe` 有 12 个进程 |
| 静止窗口截图（不挪窗口） | 窗口框在、**内容区纯白**（截图仅 25 KB，近似纯色） |
| 挪窗口后截图 | 仍是白 —— 一度以为是「挪动导致 WebView2 重绘」，其实不是 |
| 主程序字符串扫描 | 前端资源**确实**嵌在 exe 里（能搜到 `index-*.js` / `index-*.css` 名字） |
| **WebView2 的 Chromium 日志** | 拿到根因 ↓ |

打开日志的方式（这条通道值得记住）：

```python
env["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--enable-logging --log-file={log} --v=1"
```

日志里直接写着：

```
INFO:CONSOLE:13] "Error compiling schema, function code: const schema2 = scope.schema[2];…"
INFO:CONSOLE:13] "Uncaught EvalError: Evaluating a string as JavaScript violates the
  following Content Security Policy directive because 'unsafe-eval' is not an allowed
  source of script: script-src 'self' 'sha256-…'"
```

#### 根因

| 环节 | 事实 |
|---|---|
| 前端做契约运行时校验（规格 5.1） | 用 Ajv 消费 `contracts.schema.json` |
| Ajv 的编译方式 | 生成校验函数源码后 `new Function(...)` → **需要 `unsafe-eval`** |
| 生产 CSP | `script-src 'self'`（**不含** `unsafe-eval`）→ 拒绝 |
| 开发 CSP（`devCsp`） | 含 `'unsafe-eval'` → **本地一切正常** |

于是：校验器编译抛异常 → IPC 响应校验崩 → React 启动失败 → 白屏。
**这个缺陷只在安装版出现，因为两层 CSP 不一样。**

#### 修法：把编译从运行期挪到构建期

新增 `scripts/generate-validators.mjs`：用 Ajv 的 `standaloneCode()` 把 29 个契约定义
序列化成普通 ES 模块（`src/api/validators.generated.ts`，10,084 行、318 KB），
运行期只剩静态函数、**不再有 `new Function`**。

**为什么不直接给生产 CSP 加 `'unsafe-eval'`**：那是为了一处便利把 XSS 的门开一条缝，
是安全倒退。预编译同时保住了「运行时按 schema 校验」与「严格 CSP」两个要求。

配套改动：

- `pnpm contracts:generate` 现在同时重新生成 Rust 契约与预编译校验器；
  `pnpm contracts:check`（CI 跑）会在两者不一致时失败；
- `ajv` 从 `dependencies` 退回 `devDependencies`（前端运行时不再引用它）；
- 生成物 `@ts-nocheck`（机器产物，类型由 `validation.ts` 负责）+ eslint 单文件放宽。

#### 守卫：`pnpm test:prod-csp`

| 项 | 内容 |
|---|---|
| 做法 | 用 `dist/`（**生产产物**）+ `tauri.conf.json` 里的**生产 CSP** 起静态服务，Playwright 断言界面渲染 |
| 为什么之前没有它 | `pnpm dev` / `pnpm test:e2e` 用的都是 `devCsp` —— 差异就是缺陷的藏身处 |
| **它真的会红吗** | **会**：把实现临时改回运行时 Ajv 后，两条用例都失败（元素找不到=白屏），与安装版现象一致；改回预编译后 2/2 通过 |

#### 修复后的真机验证

| 项 | 结果 |
|---|---|
| 安装（静默，`/S`） | 退出码 0，安装目录 `%LOCALAPPDATA%\FilePilot` |
| `extract_worker.exe` 与主程序同级 | ✅ 1,945,600 B（**T16 那句「验证可找到」到此结案**） |
| 启动 | 进程存活；stdout 有 `启动核对：没有未决的执行`（启动恢复跑过了） |
| **界面渲染** | ✅ 见 `docs/screenshots/installed-app.png`（首页、导航、「选择文件夹」、当前模式） |
| 卸载 | 主程序/worker/开发工具全部移除；`filepilot.db*` **保留** ✅（符合 T17） |

> 另外记录一个**不是问题但值得说**的观察：安装目录里还有
> `export-contracts.exe`（契约导出器，开发工具，427 KB）。它由 Tauri 打包
> `[[bin]]` 目标时一并带上，**不影响功能**，但用户不需要它。
> 处置：本轮记录，不修（要修得给该 bin 加 `required-features`，
> 并改 `contracts:generate` 的调用方式）。

### 8.9 T17 真机验收实跑（2026-09-25，VMware 虚拟机）

**这台机器不是合格的 T17 验收环境**（Windows 10 18363，且装了 Python 与 VS Code），
所以结论记为**平台探测**。但它仍然证明了许多自动化测不到的事，
**尤其是「安装版在另一台机器上能不能起来」**——即 8.8 那个白屏缺陷的回归确认。

#### 环境

| 项 | 值 |
|---|---|
| 系统 | Microsoft Windows 10 专业工作站版 build 18363，64 位 |
| 内存 / C 盘可用 | 4.0 GB / 81.4 GB |
| WebView2 | 146.0.3856.62（该机**已装**；G 节三条分支另由用户于同日补测，见 §8.9.1） |
| 被测包 | `FilePilot_0.1.0_x64-setup.exe`，3,029,911 B，SHA-256 `3e1ecb68…`（**哈希比对通过**） |
| 与宿主机的隔离 | 独立虚拟机，无开发工具链（仅有 Python 3.9 与 VS Code） |

#### 自动部分（`scripts/verify-clean-machine.ps1`）

| 阶段 | 结果 |
|---|---|
| `install` | 退出码 0；安装目录 `%LOCALAPPDATA%\FilePilot` |
| `extract_worker.exe` 与主程序同级 | ✅ 1,945,600 B |
| **进程存活** | ✅ `pid=1884` |
| **主窗口标题** | ✅ 「文件领航 FilePilot」（窗口已创建） |
| 内存占用 | 22 MB |
| `uninstall` | 退出码 0 |
| 主程序已移除 / worker 已移除 | ✅ / ✅ |
| **用户数据保留** | ✅ `filepilot.db` 存在且 **SHA-256 未变** |

> **8.8 的回归结论**：同一个包在**另一台机器**上装完后**正常渲染**（用户截图确认
> 首页、导航、「选择文件夹」、当前整理模式齐全）。白屏缺陷的修复在真机成立。

#### 人工部分

| 节 | 结果 | 关键证据 |
|---|---|---|
| C 规则全流程 | ✅ 通过 | 8 个文件 → 已移动 8 / 失败 0 / 未执行 0 / 未完成 0；**「文档」「压缩包」两个目标目录都被创建**（一次执行建多目录，此前只有单测覆盖） |
| C2 大批量即时反馈 | ❌ **不通过** | 952 个文件的目录，点「确认执行」后**界面零反馈** → 记为 `POST_RELEASE_TODO.md` PR-004（P1） |
| D 撤销 | ✅ 通过 | 8 个文件全部回原位，含 `新建 WinRAR ZIP 压缩1件.zip` 这类名字也原样还原 |
| D2 历史可辨识性 | ❌ **不通过** | 用户主动反馈：历史只显示计数、明细区只有问题列表 → 记为 PR-003 |
| E 退出与重启 | ✅ 通过 | 历史从数据库恢复；撤销需重新授权根目录；重选同目录后旧操作可撤销 |
| F 边界 | ✅ 通过 | >100 MiB 的文件**不出现在预览列表**（扫描阶段即跳过）；实测 `big.bin` 未入列 |
| G WebView2 | ✅ **通过**（三条分支全过） | 见 §8.9.1 |
| H 卸载 | ✅ 通过 | 见上表 |

#### 本轮澄清的一条误读记录

归档报告 `tmp/verify-kit-reports/report.txt` 里有一条宿主机记录写着
`[失败（应用启动后崩溃）] 进程存活 = 已退出`。**已判定为脚本改到一半时的中间态**，
不是应用缺陷：报告生成时间（`00:51:07`）距脚本最后修改（`00:48:13`）仅 3 分钟；
且用当前脚本在宿主机逐秒采样 20 次，PID 全程存活、标题稳定；
同一安装包在虚拟机上 `pid=1884` 通过。

> **教训**：脚本把历次结果**追加**进同一个 `report.txt`，开发期的中间态会与正式
> 结论混在一起，交回时极易被误读成缺陷。应根据需要给报告加正式/草案标记，
> 或把冒烟写到别的文件名（详见 `POST_RELEASE_TODO.md` CL-001）。

### 8.9.1 G 节 WebView2 三分支（2026-09-25，用户实跑）

**这一节是整个 T17 里唯一完全空白过的一节，现已跑完。**

| 分支 | 结果 | 说明 |
|---|---|---|
| 已装 WebView2 | ✅ 通过 | 安装不额外下载，装完即可运行（宿主机与虚拟机两条记录一致） |
| **移除 WebView2** | ✅ 通过 | 安装过程**能引导安装**，不是静默装出一个打不开的程序 |
| **断网 + 缺 WebView2** | ✅ 通过 | 安装给出**可理解的失败提示**，未产出不可用的程序 |

> **口径（必须一起读）**：G 节三条验的是 `webviewInstallMode = downloadBootstrapper`
> 这条分支的行为，**与「验收机是否干净」正交**——它不依赖机器上有没有 Node/Rust，
> 也不依赖 Windows 版本。所以在非合格验收机上得到的结果**可以直接采纳**。
>
> 但它**不能**让 T17 那一格变成「通过」：T17 的硬前提是
> **干净的 Windows 11 x64**，这一条仍然没有满足。**G 通过 ≠ T17 通过。**
>
> 唯一还值得在干净 Win11 上顺手复验的是第二、三条——Win11 通常自带 WebView2，
> 要造出「缺失」往往得先手工卸载运行时。**不阻断交付，属锦上添花。**

#### 仍未覆盖的（诚实列出）

- **合格的 T17 环境**：还是缺一台**干净的 Windows 11 x64**。虚拟机是 Win10 且有开发工具。
  **这是 T17 现在唯一的硬缺口**——G 节已于 2026-09-25 跑完，见 §8.9.1。
- **Windows CI**：workflow 写好并本地校验过 YAML，**从未在 runner 上跑过**。
- **T15 的两项人工验收**（安装版走 AI 本地/云授权全流程、邀请 5 位试用者）：见第 5 节。

---

## 9. 更新约定

改了性能相关的代码（扫描、哈希、规划器）之后，**重跑第 3 节并把新数字写进来**，
连同日期与环境。**不要**让脚本自动改这份文档：这里的每一条都该是有人看过、
签过字的。

第 5 节那两项人工验收做完之后，把它们从「未做」改成实际的日期与结果——
**不要**因为「自动化测试都过了」就顺手改掉。
