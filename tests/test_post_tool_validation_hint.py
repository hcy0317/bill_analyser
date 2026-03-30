from __future__ import annotations

import pytest

from scripts.hooks import post_tool_validation_hint as hook
from scripts.hooks.session_snapshot import resolve_snapshot_path


def test_build_hook_messages_refreshes_session_snapshot_for_repo_edit() -> None:
    payload = {
        "cwd": str(hook.REPO_ROOT),
        "tool_name": "Edit",
        "tool_input": {
            "file_path": str(hook.REPO_ROOT / "scripts" / "hooks" / "post_tool_validation_hint.py"),
            "old_string": "before",
            "new_string": "after",
        },
    }

    snapshot_path = resolve_snapshot_path(hook.REPO_ROOT)
    assert snapshot_path is not None
    if snapshot_path.exists():
        snapshot_path.unlink()

    messages = hook.build_hook_messages(payload)

    assert any("last-session.md" in message for message in messages)
    assert snapshot_path.exists()
    snapshot_text = snapshot_path.read_text(encoding="utf-8")
    assert "scripts/hooks/post_tool_validation_hint.py" in snapshot_text
    assert "Suggested next verification" in snapshot_text


def test_build_hook_messages_ignores_non_edit_tools() -> None:
    payload = {
        "cwd": str(hook.REPO_ROOT),
        "tool_name": "Read",
        "tool_input": {
            "file_path": str(hook.REPO_ROOT / "AGENTS.md"),
        },
    }

    assert hook.build_hook_messages(payload) == []


def test_build_hook_messages_handles_snapshot_write_failures_gracefully(monkeypatch: pytest.MonkeyPatch) -> None:
    payload = {
        "cwd": str(hook.REPO_ROOT),
        "tool_name": "Edit",
        "tool_input": {
            "file_path": str(hook.REPO_ROOT / "scripts" / "hooks" / "post_tool_validation_hint.py"),
            "old_string": "before",
            "new_string": "after",
        },
    }

    monkeypatch.setattr(hook, "write_session_snapshot", lambda **_kwargs: None)

    messages = hook.build_hook_messages(payload)

    assert not any("last-session.md" in message for message in messages)
    assert any("Python 文件已编辑" in message for message in messages)