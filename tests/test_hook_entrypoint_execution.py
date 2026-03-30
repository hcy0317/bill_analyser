from __future__ import annotations

from pathlib import Path
import subprocess
import sys


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