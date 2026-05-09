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
    assert any(" - 同步 AI 入口" in message for message in messages)
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


def test_commit_title_uses_dash_fragments_for_multi_surface_gate_changes() -> None:
    title = hook._build_commit_title(
        [
            "scripts/hooks/stop_commit_title_hint.py",
            ".github/copilot-instructions.md",
            "docs/AI_WORKFLOW.md",
            "tests/test_stop_commit_title_hint.py",
        ],
        "tighten commit and PR gate behavior",
    )

    assert title.startswith("chore(hooks): 补充会话结束提交标题提示")
    assert " - 同步 AI 入口" in title
    assert " - 同步文档" in title
    assert " - 补充测试" in title
