from __future__ import annotations

# ruff: noqa: I001

import json
from pathlib import Path
from typing import TYPE_CHECKING

from scripts.hooks import task_state
from scripts.hooks.work_context import GitState


if TYPE_CHECKING:
    import pytest


def test_write_task_state_persists_restart_safe_json(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    persisted_path = repo_root / ".git" / "ai" / "task-state.json"

    monkeypatch.setattr(task_state, "resolve_task_state_path", lambda _repo_root=repo_root: persisted_path)
    monkeypatch.setattr(
        task_state,
        "collect_git_state",
        lambda _repo_root=repo_root: GitState(
            branch="feature/handoff",
            head="abc1234",
            staged_files=(".github/prompts/handoff.prompt.md",),
            unstaged_files=("docs/AI_WORKFLOW.md",),
            untracked_files=(),
        ),
    )

    result = task_state.write_task_state(
        trigger="handoff",
        recent_files=[".github/prompts/handoff.prompt.md"],
        status="handoff",
        title="整理会话交接",
        blocked_by=["等待人工确认"],
        snapshot_display_path=".git/ai/last-session.md",
        repo_root=repo_root,
    )

    assert result is not None
    assert result.task_state_display_path == ".git/ai/task-state.json"
    assert result.status == "handoff"
    assert persisted_path.exists()

    payload = json.loads(persisted_path.read_text(encoding="utf-8"))
    assert payload["title"] == "整理会话交接"
    assert payload["status"] == "handoff"
    assert payload["blockedBy"] == ["等待人工确认"]
    assert payload["snapshotPath"] == ".git/ai/last-session.md"
    assert ".github/prompts/handoff.prompt.md" in payload["recentFiles"]
    assert "ai-customization" in payload["scopes"]


def test_write_task_state_returns_none_when_file_cannot_be_written(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    persisted_path = repo_root / ".git" / "ai" / "task-state.json"

    monkeypatch.setattr(task_state, "resolve_task_state_path", lambda _repo_root=repo_root: persisted_path)
    monkeypatch.setattr(
        task_state,
        "collect_git_state",
        lambda _repo_root=repo_root: GitState(
            branch="main",
            head="1234567",
            staged_files=(),
            unstaged_files=(),
            untracked_files=(),
        ),
    )

    monkeypatch.setattr(task_state, "write_text_atomic", lambda *_args, **_kwargs: (_ for _ in ()).throw(OSError("disk full")))

    result = task_state.write_task_state(
        trigger="handoff",
        recent_files=["AGENTS.md"],
        repo_root=repo_root,
    )

    assert result is None


def test_build_task_title_prefers_handoff_and_start_work_specific_assets() -> None:
    assert (
        task_state.build_task_title(("ai-customization",), (".github/prompts/handoff.prompt.md",))
        == "整理会话交接与恢复状态"
    )
    assert (
        task_state.build_task_title(("ai-customization",), (".github/prompts/start-work.prompt.md",))
        == "从已批准计划进入执行"
    )
    assert task_state.build_task_title(("rust-runtime",), ("crates/bill-analyser-http/src/router.rs",)) == "推进 Rust 运行时代码改动"


def test_build_task_state_payload_preserves_absolute_paths_outside_repo() -> None:
    payload = task_state.build_task_state_payload(
        trigger="manual",
        recent_files=["D:/outside/notes.md"],
        repo_root=task_state.REPO_ROOT,
    )
    recent_files = payload["recentFiles"] if isinstance(payload.get("recentFiles"), list) else []

    assert "D:/outside/notes.md" in recent_files


def test_build_task_state_payload_falls_back_for_unknown_status() -> None:
    payload = task_state.build_task_state_payload(
        trigger="manual",
        recent_files=["AGENTS.md"],
        status="unexpected",
        repo_root=task_state.REPO_ROOT,
    )

    assert payload["status"] == task_state.DEFAULT_STATUS
