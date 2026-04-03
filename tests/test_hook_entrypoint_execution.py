from __future__ import annotations

import io
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def test_post_tool_validation_hint_can_run_as_a_script() -> None:
    result = subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts" / "hooks" / "post_tool_validation_hint.py")],
        cwd=REPO_ROOT,
        input="{}",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert "ModuleNotFoundError" not in result.stderr


def test_stop_commit_title_hint_can_run_as_a_script() -> None:
    result = subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts" / "hooks" / "stop_commit_title_hint.py")],
        cwd=REPO_ROOT,
        input="",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert "ModuleNotFoundError" not in result.stderr


def test_task_state_can_run_as_a_script() -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "hooks" / "task_state.py"),
            "--trigger",
            "manual",
            "--recent-file",
            "AGENTS.md",
            "--json",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert ".git/ai/task-state.json" in result.stdout
    assert "ImportError" not in result.stderr


def test_copilot_global_hook_bridge_can_run_as_a_script() -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "hooks" / "copilot_global_hook_bridge.py"),
            "pre-tool",
        ],
        cwd=REPO_ROOT,
        input="{}",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert "Traceback" not in result.stderr


def test_copilot_global_hook_bridge_reconfigures_non_utf8_streams_and_preserves_unicode(monkeypatch) -> None:
    from scripts.hooks import copilot_global_hook_bridge as bridge

    observed: list[tuple[str, dict[str, object]]] = []
    bridge.OBSERVATION_KEYS_EMITTED.clear()
    monkeypatch.setattr(
        bridge,
        "record_learning_observation",
        lambda description, *, context=None, once_key=None: observed.append((description, context or {})),
    )

    stdout_buffer = io.BytesIO()
    stderr_buffer = io.BytesIO()
    stdout = io.TextIOWrapper(stdout_buffer, encoding="gbk", errors="strict")
    stderr = io.TextIOWrapper(stderr_buffer, encoding="gbk", errors="strict")

    changed = bridge.prepare_standard_streams_for_unicode(stdout=stdout, stderr=stderr)
    bridge.write_text(stdout, "🔍 全局 hook 编码自愈", "stdout")
    stdout.flush()

    assert set(changed) == {"stdout", "stderr"}
    assert stdout_buffer.getvalue() == "🔍 全局 hook 编码自愈".encode()
    assert len(observed) == 1
    assert "auto-reconfigured stdout, stderr" in observed[0][0]


def test_repo_hook_configs_force_utf8_python_mode() -> None:
    hook_files = [
        REPO_ROOT / ".github" / "hooks" / "repo-guard.json",
        REPO_ROOT / ".github" / "hooks" / "post-tool-validation-hint.json",
        REPO_ROOT / ".github" / "hooks" / "stop-commit-title-hint.json",
    ]

    for hook_file in hook_files:
        config = json.loads(hook_file.read_text(encoding="utf-8"))
        for commands in config.get("hooks", {}).values():
            for command in commands:
                bash = str(command.get("bash") or "")
                powershell = str(command.get("powershell") or "")
                assert "-X utf8" in bash, f"{hook_file.name} bash hook must force UTF-8 mode"
                assert "-X utf8" in powershell, f"{hook_file.name} powershell hook must force UTF-8 mode"


def test_copilot_global_hook_bridge_handles_gbk_stdout_in_subprocess() -> None:
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "gbk"

    code = (
        "from scripts.hooks import copilot_global_hook_bridge as bridge; "
        "import sys; "
        "bridge.prepare_standard_streams_for_unicode(); "
        "bridge.write_text(sys.stdout, '🔍 全局 hook 子进程编码自愈', 'stdout')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert "UnicodeEncodeError" not in result.stderr
    assert "🔍 全局 hook 子进程编码自愈" in result.stdout
