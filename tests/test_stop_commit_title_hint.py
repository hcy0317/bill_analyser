from __future__ import annotations

import pytest

from scripts.hooks import stop_commit_title_hint as hook
from scripts.hooks.session_snapshot import resolve_snapshot_path


def test_build_stop_messages_include_commit_title_and_resume_hint() -> None:
    changed_files = [
        "scripts/hooks/stop_commit_title_hint.py",
        ".github/copilot-instructions.md",
    ]

    snapshot_path = resolve_snapshot_path(hook.REPO_ROOT)
    assert snapshot_path is not None
    if snapshot_path.exists():
        snapshot_path.unlink()

    messages = hook.build_stop_messages("unstaged", changed_files, "fix resume flow")

    assert any("中文 Conventional Commit 标题建议" in message for message in messages)
    assert any("session-resume" in message for message in messages)
    assert any("last-session.md" in message for message in messages)
    assert any("task-state.json" in message for message in messages)
    assert any("建议下一步" in message for message in messages)
    assert snapshot_path.exists()
    snapshot_text = snapshot_path.read_text(encoding="utf-8")
    assert "scripts/hooks/stop_commit_title_hint.py" in snapshot_text
    assert ".github/copilot-instructions.md" in snapshot_text
    assert "Linked Task State Path" in snapshot_text


def test_build_stop_messages_returns_empty_without_diff() -> None:
    assert hook.build_stop_messages("", [], "") == []


def test_build_stop_messages_still_emit_commit_hint_when_snapshot_write_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(hook, "write_session_snapshot", lambda **_kwargs: None)

    messages = hook.build_stop_messages("unstaged", ["scripts/hooks/stop_commit_title_hint.py"], "fix")

    assert any("中文 Conventional Commit 标题建议" in message for message in messages)
    assert not any("session-resume" in message for message in messages)
    assert not any("task-state.json" in message for message in messages)