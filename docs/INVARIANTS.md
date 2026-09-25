# 不变量证据（INV-01 ～ INV-10）

规格 §9.10 的**发布门槛**：

> P0–P8 所有必需项完成，**INV-01 至 INV-10 全部有测试证据**，
> 未解决的数据丢失/越界/覆盖/恢复缺陷为 0。不能用「已完成大部分功能」替代。

这份文档就是「全部有测试证据」的兑现。每一条给出：

- 规格原文（`docs/MASTER_PLAN.md` §1.3）
- **判定行为**：这条不变量在代码里到底约束什么
- **证据用例**：挑 2 条最能代表该行为的用例（不是全部——按行为关键词能匹配到
  17~69 条不等，这里只列最能证伪的那两条）
- **本次复跑结果**（2026-09-24）

复跑方式（**每次发版都要当场跑，不要只信本文档里的历史结果**）：

```bash
python scripts/check-invariants.py        # 逐条复跑下面这 20 条证据，有失败即非零退出
```

单条手工复跑（串行；`tests/analysis.rs` 并行时会因为在同一进程里抢回环端口
偶发失败，见 `docs/TEST_MATRIX.md` 6.2）：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --features failpoints \
  --test <测试文件> -- --exact <用例名> --test-threads=1
```

---

## INV-01 扫描、解析、AI 分析、预览不改变用户文件、名称或目录结构

| 项 | 内容 |
|---|---|
| 判定行为 | 扫描/提取只读；不建目录、不改名、不动内容 |
| 证据 1 | `scan::s01_scans_plain_chinese_and_emoji_names_without_changing_anything` —— 中文/emoji 名扫描后**整棵树**不变 |
| 证据 2 | `extract::the_whole_tree_is_unchanged_by_extraction` —— 提取（含 OCR）后整棵树不变 |
| 复跑 | **通过** |

## INV-02 没有与当前计划摘要绑定的有效确认，执行器不能工作

| 项 | 内容 |
|---|---|
| 判定行为 | 执行入口要求 digest 与当前计划一致且未被消费 |
| 证据 1 | `execute_windows::executing_a_sealed_plan_is_refused` —— 已封存计划的旧确认被拒 |
| 证据 2 | `recovery_windows::a_stale_state_digest_is_refused` —— 过期的 digest 被拒 |
| 复跑 | **通过** |
| 边界 | 令牌的**密码学强度**不在这些用例里；安全性由后端持有令牌、绑定 revision/digest 保证 |

## INV-03 现有文件绝不被覆盖，即使目标在预览后才被其他进程创建

| 项 | 内容 |
|---|---|
| 判定行为 | 目标已存在 → 拒绝；且**内核层面**的「不覆盖」语义（RENAME_NOREPLACE 等价物）挡住竞态 |
| 证据 1 | `execute_windows::refuses_when_target_already_exists_and_leaves_both_files_intact` —— 两份文件都完好 |
| 证据 2 | `execute_windows::kernel_refuses_to_overwrite_a_target_created_after_the_precheck` —— **预检查之后**才创建的目标也挡得住 |
| 复跑 | **通过** |

## INV-04 任何文件内容字节和扩展名不因整理而改变

| 项 | 内容 |
|---|---|
| 判定行为 | 移动前后内容哈希一致；只改路径不改字节；扩展名不变 |
| 证据 1 | `execute_windows::content_hash_is_identical_before_and_after_move` |
| 证据 2 | `execute_windows::moves_a_file_without_changing_its_bytes` |
| 复跑 | **通过** |

## INV-05 源、目标及其真实父路径均位于批准根目录内；不通过链接绕过范围

| 项 | 内容 |
|---|---|
| 判定行为 | 相对路径不得逃逸；目录联接/符号链接不得把写操作带出根；根被替换要能发现 |
| 证据 1 | `root_scope::relative_path_cannot_escape_the_root` |
| 证据 2 | `execute_windows::rejects_a_target_path_that_crosses_a_directory_junction` |
| 复跑 | **通过** |
| 相关 | `root_scope::detects_root_replaced_by_another_directory`、`validate_plan::a_target_directory_behind_a_junction_is_refused`、`scan::s02_reparse_points_are_skipped_and_marked` |

## INV-06 模型无法直接调用系统命令、文件执行器或传入任意绝对路径

| 项 | 内容 |
|---|---|
| 判定行为 | 模型输出里的绝对路径/未知 fileId 一律丢弃；注入文本不会变成命令 |
| 证据 1 | `ai_contract::a_proposal_carrying_an_absolute_path_is_dropped` |
| 证据 2 | `naming::rejects_absolute_paths_and_device_paths_in_every_form` |
| 复跑 | **通过** |
| 相关 | `ai_contract::a_proposal_naming_an_unknown_file_is_dropped`、`ai_contract::an_injection_in_the_reason_text_does_not_become_a_command` |

## INV-07 每次变更前有持久化意图记录；变更后有结果记录。进程中断后不会盲目重复执行

| 项 | 内容 |
|---|---|
| 判定行为 | 每个被派发的项先写 `prepared`（意图），后写 `applied`（结果）；崩溃后按事实核对，不重放 |
| 证据 1 | `execute_windows::the_journal_records_prepared_before_applied_for_every_moved_file` |
| 证据 2 | `recovery_windows::crash_after_rename_is_recognised_as_applied_and_never_moves_again` |
| 复跑 | **通过** |
| 相关 | `recovery_windows` 的 crash_* 系列 4 条 + `an_unreadable_journal_blocks_recovery_instead_of_resetting` |

## INV-08 撤销不覆盖后来创建的文件，不擅自搬动整理后被修改的文件

| 项 | 内容 |
|---|---|
| 判定行为 | 原位置出现新文件 → 冲突、两份都留；文件被改过 → 默认不撤销 |
| 证据 1 | `undo_windows::a_new_file_at_the_original_path_blocks_the_undo_and_keeps_both_files` |
| 证据 2 | `undo_windows::a_modified_file_is_not_moved_back_by_default` |
| 复跑 | **通过** |
| 相关 | `a_file_replaced_by_another_file_with_the_same_bytes_is_still_a_conflict`（**同字节也算冲突**——按身份而不是按内容判断） |

## INV-09 模型故障不会触发文件写入，也不会静默把云模式换成其他提供商

| 项 | 内容 |
|---|---|
| 判定行为 | 4xx/5xx/超时/坏 JSON 都变成**明确的失败**，不产生任何文件写入；换了提供商/模式 → 旧授权失效 |
| 证据 1 | `ai_contract::a_401_comes_back_as_a_response_not_a_transport_error` |
| 证据 2 | `analysis::a_model_that_keeps_returning_garbage_gives_up_after_one_repair` —— 反复返回垃圾时**只修一次**就明确失败 |
| 复跑 | **通过**（**必须串行跑**：这条属于 `tests/analysis.rs` 的并行偶发失败集合） |
| 相关 | `analysis::changing_the_provider_invalidates_the_grant`、`analysis::an_unknown_provider_is_refused`、`analysis::an_invalid_output_is_repaired_exactly_once_and_never_changes_the_content` |

## INV-10 重试、重复点击、双窗口不得让同一操作执行两次

| 项 | 内容 |
|---|---|
| 判定行为 | 同一个 `requestId` 只产生一个 run；撤销令牌消费一次；重复调用幂等 |
| 证据 1 | `execute_windows::a_repeated_request_id_does_not_create_a_second_run` |
| 证据 2 | `undo_windows::the_same_request_id_undoes_only_once` |
| 复跑 | **通过** |
| 相关 | `recovery_windows::reconcile_run_is_idempotent_across_repeated_calls`、前端 `tests/ui/undo.test.tsx`。**「双窗口」这一半**由全局执行锁（`app_state` 的 `only_one_execution_can_hold_the_global_lock`）+ 请求幂等共同覆盖 |

---

## 汇总

| 不变量 | 复跑的两条证据 |
|---|---|
| INV-01 | 通过 |
| INV-02 | 通过 |
| INV-03 | 通过 |
| INV-04 | 通过 |
| INV-05 | 通过 |
| INV-06 | 通过 |
| INV-07 | 通过 |
| INV-08 | 通过 |
| INV-09 | 通过（串行） |
| INV-10 | 通过 |

**口径说明**（不要把这句省掉）：

- 这里证明的是「**每条不变量的核心判定行为有可复现的自动化证据**」。
- CI 的 `rust` job 跑的是**同一套全量测试**（`cargo test --features failpoints
  --locked -- --test-threads=1`），所以这 20 条也在 CI 覆盖范围内；
  `scripts/check-invariants.py` 的价值在于**发版时能单独、快速地把「这一条还在不在」
  摆到台面上**，并且失败即非零退出。
- 它**不**等于「永远不会出问题」：`docs/TEST_MATRIX.md` 第 5 节那两项人工验收
  （安装版走完整流程、邀请试用者）仍然未做；6.1 / 6.2 两条测试基础设施的
  偶发失败也仍未根除。
- 发布门槛那句「未解决的数据丢失/越界/覆盖/恢复缺陷为 0」的判定依据，
  就是本表 + `docs/TEST_MATRIX.md` 的已知问题清单。
