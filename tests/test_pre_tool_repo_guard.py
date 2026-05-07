"""Regression tests for the thin repository pre-tool guard hook."""

from __future__ import annotations

import importlib
import json
import subprocess
import sys
from pathlib import Path


guard = importlib.import_module("scripts.hooks.pre_tool_repo_guard")


def command_payload(command: str) -> dict[str, object]:
    return {
        "cwd": str(guard.REPO_ROOT),
        "tool_name": "powershell",
        "tool_input": {"command": command},
    }


def delete_payload(path: Path) -> dict[str, object]:
    return {
        "cwd": str(guard.REPO_ROOT),
        "tool_name": "delete_file",
        "tool_input": {"file_path": str(path)},
    }


def test_denies_windows_drive_root_delete() -> None:
    reason = guard.evaluate_pre_tool_use(
        command_payload(r"Remove-Item -Recurse -Force C:\ ")
    )

    assert reason is not None
    assert "整盘" in reason


def test_denies_nested_powershell_drive_root_delete() -> None:
    for command in (
        r'powershell -Command "Remove-Item -Recurse -Force C:\"',
        r'pwsh -Command "Remove-Item -LiteralPath:C:\"',
    ):
        reason = guard.evaluate_pre_tool_use(command_payload(command))
        assert reason is not None
        assert "整盘" in reason


def test_denies_posix_root_delete() -> None:
    reason = guard.evaluate_pre_tool_use(command_payload("rm -rf /"))

    assert reason is not None
    assert "根目录" in reason


def test_denies_repository_wipe_command() -> None:
    reason = guard.evaluate_pre_tool_use(command_payload("Remove-Item -Recurse -Force ."))

    assert reason is not None
    assert "当前仓库" in reason


def test_denies_dangerous_git_cleanup() -> None:
    for command in (
        "git reset --hard HEAD",
        "git clean -fdx",
        "git restore -- .",
        "git -C . clean -fdx",
        "git -C . reset --hard HEAD",
        "git -C . restore -- .",
    ):
        reason = guard.evaluate_pre_tool_use(command_payload(command))
        assert reason is not None
        assert "git" in reason


def test_denies_runtime_database_delete_command() -> None:
    for command in (
        r"Remove-Item data\bills.db",
        r"Remove-Item data\bills.db*",
        r"rm -f data/bills.db*",
        r"Remove-Item data\bills.db-journal",
        r'powershell -Command "Remove-Item -LiteralPath:data\bills.db"',
    ):
        reason = guard.evaluate_pre_tool_use(command_payload(command))
        assert reason is not None
        assert "runtime" in reason


def test_denies_runtime_database_delete_tool() -> None:
    reason = guard.evaluate_pre_tool_use(delete_payload(guard.REPO_ROOT / "data" / "bills.db"))

    assert reason is not None
    assert "runtime" in reason


def test_denies_explicit_database_wipe_sql() -> None:
    reason = guard.evaluate_pre_tool_use(
        command_payload('sqlite3 data/bills.db "DELETE FROM transactions"')
    )

    assert reason is not None
    assert "数据库清库" in reason


def test_allows_outside_repo_write() -> None:
    payload = {
        "cwd": str(guard.REPO_ROOT),
        "tool_name": "Write",
        "tool_input": {
            "file_path": str(Path.home() / ".codex" / "memories" / "MEMORY.md"),
            "content": "allowed",
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_allows_outside_repo_non_root_delete() -> None:
    payload = {
        "cwd": str(guard.REPO_ROOT),
        "tool_name": "powershell",
        "tool_input": {
            "command": rf"Remove-Item {Path.home()}\.codex\memories\old-note.md",
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_allows_git_clean_dry_run() -> None:
    assert guard.evaluate_pre_tool_use(command_payload("git clean -nfdx")) is None
    assert guard.evaluate_pre_tool_use(command_payload("git -C . clean -nfdx")) is None


def test_bridge_output_shape_for_claude_payload() -> None:
    payload = command_payload("git clean -fdx")
    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    decision = guard.build_hook_output(payload, reason)
    assert decision["hookSpecificOutput"]["hookEventName"] == "PreToolUse"
    assert decision["hookSpecificOutput"]["permissionDecision"] == "deny"


def test_script_entrypoint_allows_benign_payload() -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(guard.REPO_ROOT / "scripts" / "hooks" / "pre_tool_repo_guard.py"),
        ],
        cwd=guard.REPO_ROOT,
        input=json.dumps(command_payload("Set-Content C:\\Users\\hcy\\note.txt ok")),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0
    assert result.stdout == ""


def test_payload_helpers_accept_supported_host_shapes() -> None:
    assert guard.load_payload("") is None
    assert guard.load_payload("{") is None
    assert guard.load_payload("[]") is None
    assert guard.load_payload('{"toolName":"powershell"}') == {"toolName": "powershell"}

    assert guard.get_tool_name({"toolName": "PowerShell"}) == "PowerShell"
    assert guard.get_tool_input({"toolArgs": {"command": "Get-Date"}}) == {
        "command": "Get-Date"
    }
    assert guard.get_tool_input({"toolArgs": '{"command":"Get-Date"}'}) == {
        "command": "Get-Date"
    }
    assert guard.get_tool_input({"toolArgs": "[1,2]"}) == {}
    assert guard.get_tool_input({"toolArgs": "not-json"}) == {}

    patch_text = "*** Begin Patch\n*** Add File: x.txt\n+x\n*** End Patch\n"
    assert guard.get_tool_input({"toolArgs": patch_text}) == {"patch": patch_text}


def test_command_helpers_cover_args_quotes_and_bad_tokenization() -> None:
    assert guard.extract_command_text({"command": "python", "args": ["-m", "pytest"]}) == (
        "python -m pytest"
    )
    assert guard.strip_wrapping_quotes('"quoted"') == "quoted"
    assert guard.tokenize_command_text('"unterminated') == ('"unterminated',)


def test_delete_target_extraction_handles_path_flags_and_filter_values() -> None:
    targets = guard.extract_delete_targets(
        r"Remove-Item -LiteralPath data\bills.db -Filter ignored.txt"
    )

    assert targets == (r"data\bills.db",)

    colon_targets = guard.extract_delete_targets(
        "Remove-Item -LiteralPath:data\\bills.db -Path:C:\\"
    )

    assert colon_targets == (r"data\bills.db", "C:\\")


def test_denies_powershell_colon_path_runtime_and_root_delete() -> None:
    runtime_reason = guard.evaluate_pre_tool_use(
        command_payload(r"Remove-Item -LiteralPath:data\bills.db")
    )
    root_reason = guard.evaluate_pre_tool_use(
        command_payload("Remove-Item -LiteralPath:C:\\")
    )

    assert runtime_reason is not None
    assert "runtime" in runtime_reason
    assert root_reason is not None
    assert "整盘" in root_reason


def test_wildcard_runtime_delete_and_system_drive_root_are_denied() -> None:
    assert guard.command_denial_reason(r"Remove-Item data\*") is not None
    assert guard.command_denial_reason(r"Remove-Item $env:SystemDrive\*") is not None


def test_delete_tool_repo_root_and_helper_none_cases() -> None:
    assert guard.is_repo_wipe_target(None) is False
    assert guard.is_runtime_wipe_target(None) is False

    reason = guard.evaluate_pre_tool_use(delete_payload(guard.REPO_ROOT))

    assert reason is not None
    assert "当前仓库" in reason


def test_non_claude_hook_output_shape() -> None:
    decision = guard.build_hook_output({"toolName": "powershell"}, "blocked")

    assert decision == {
        "permissionDecision": "deny",
        "permissionDecisionReason": "blocked",
    }


def test_script_entrypoint_returns_deny_payload() -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(guard.REPO_ROOT / "scripts" / "hooks" / "pre_tool_repo_guard.py"),
        ],
        cwd=guard.REPO_ROOT,
        input=json.dumps(command_payload(r"Remove-Item data\bills.db")),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
