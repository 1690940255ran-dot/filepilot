"""复跑 INV-01..INV-10 的证据用例（发布门槛检查）。

规格 §9.10 的发布门槛：**INV-01 至 INV-10 全部有测试证据**。
文档里写着「通过」不够——每次发版都要**当场复跑**一遍，因为
「用例叫这个名字」和「用例现在通过」是两件事。

    python scripts/check-invariants.py

命中失败会以非零码退出。每条不变量挑 2 条最能代表判定行为的用例；
完整映射与说明见 docs/INVARIANTS.md。

串行跑（--test-threads=1）：tests/analysis.rs 里的几条例外，
并行时会因为在同一进程里抢回环端口偶发失败（见 docs/TEST_MATRIX.md 6.2）。

注意：这是**集成测试**，需要能给 NTFS 建真实文件的环境；
没有该环境时应当报「无法运行」，而不是假装通过。
"""

import re
import subprocess
import sys
from pathlib import Path


# 控制台编码：Windows 上 Python 的 stdout 默认跟随系统代码页
# （CI runner 是 cp1252、中文系统是 GBK），而本脚本会打印中文，
# 未设置时直接抛 UnicodeEncodeError，让整条命令失败。
# 见 docs/POST_RELEASE_TODO.md 的 CI-003。
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(__file__).resolve().parent.parent
CARGO = ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml",
         "--features", "failpoints", "--test-threads=1"]

# 每个不变量挑 2 条最能代表判定行为的用例（文件, 用例名）
EVIDENCE = {
    "INV-01": [
        ("scan", "s01_scans_plain_chinese_and_emoji_names_without_changing_anything"),
        ("extract", "the_whole_tree_is_unchanged_by_extraction"),
    ],
    "INV-02": [
        ("execute_windows", "executing_a_sealed_plan_is_refused"),
        ("recovery_windows", "a_stale_state_digest_is_refused"),
    ],
    "INV-03": [
        ("execute_windows", "refuses_when_target_already_exists_and_leaves_both_files_intact"),
        ("execute_windows", "kernel_refuses_to_overwrite_a_target_created_after_the_precheck"),
    ],
    "INV-04": [
        ("execute_windows", "content_hash_is_identical_before_and_after_move"),
        ("execute_windows", "moves_a_file_without_changing_its_bytes"),
    ],
    "INV-05": [
        ("root_scope", "relative_path_cannot_escape_the_root"),
        ("execute_windows", "rejects_a_target_path_that_crosses_a_directory_junction"),
    ],
    "INV-06": [
        ("ai_contract", "a_proposal_carrying_an_absolute_path_is_dropped"),
        ("naming", "rejects_absolute_paths_and_device_paths_in_every_form"),
    ],
    "INV-07": [
        ("execute_windows", "the_journal_records_prepared_before_applied_for_every_moved_file"),
        ("recovery_windows", "crash_after_rename_is_recognised_as_applied_and_never_moves_again"),
    ],
    "INV-08": [
        ("undo_windows", "a_new_file_at_the_original_path_blocks_the_undo_and_keeps_both_files"),
        ("undo_windows", "a_modified_file_is_not_moved_back_by_default"),
    ],
    "INV-09": [
        ("ai_contract", "a_401_comes_back_as_a_response_not_a_transport_error"),
        ("analysis", "a_model_that_keeps_returning_garbage_gives_up_after_one_repair"),
    ],
    "INV-10": [
        ("execute_windows", "a_repeated_request_id_does_not_create_a_second_run"),
        ("undo_windows", "the_same_request_id_undoes_only_once"),
    ],
}

ANSI = re.compile(r"\x1b\[[0-9;]*m")
lines: list[str] = []


def say(text: str = "") -> None:
    lines.append(text)
    print(text, flush=True)


results = {}
for inv, cases in EVIDENCE.items():
    say(f"=== {inv} ===")
    ok_all = True
    for test_file, test_name in cases:
        cmd = ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml",
               "--features", "failpoints", "--test", test_file, "--", "--exact", test_name]
        proc = subprocess.run(cmd, cwd=str(PROJECT), capture_output=True, text=True,
                              errors="replace")
        text = ANSI.sub("", (proc.stdout or "") + (proc.stderr or ""))
        passed = re.search(r"test result: ok\. 1 passed", text) is not None
        not_found = "0 passed" in text and "filtered out" in text
        verdict = "通过" if passed else ("未找到该用例" if not_found else "失败")
        if not passed:
            ok_all = False
        say(f"  [{verdict}] {test_file}::{test_name}")
        if not passed and not not_found:
            for line in text.splitlines():
                if "panicked" in line or "assertion" in line:
                    say(f"      {line.strip()[:160]}")
                    break
    results[inv] = ok_all
    say()

say("=== 汇总 ===")
failed = [inv for inv, ok in results.items() if not ok]
for inv, ok in results.items():
    say(f"  {inv}: {'全部通过' if ok else '**有失败**'}")

report = PROJECT / "tmp" / "inv-evidence-report.txt"
report.parent.mkdir(parents=True, exist_ok=True)
report.write_text("\n".join(lines), encoding="utf-8")
print(f"\n[报告] {report}")

if failed:
    # 发布门槛项：任何一条不变量没有通过，发版流程都必须停下来。
    # 只是打印「有失败」而不改变退出码，等于把判定交给读者——那是装饰。
    print(f"\n[失败] {len(failed)} 条不变量没有通过：{', '.join(failed)}", file=sys.stderr)
    sys.exit(1)

print(f"\n[通过] {len(results)} 条不变量的证据用例全部复跑通过")
sys.exit(0)
