from __future__ import annotations

from pathlib import Path

import pytest

from scripts.hooks import session_snapshot
from scripts.hooks.task_state import TaskStateWriteResult
from scripts.hooks import work_context
from scripts.hooks.work_context import GitState


def test_collect_git_state_includes_untracked_files(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    outputs = {
        ("rev-parse", "--abbrev-ref", "HEAD"): "feature/resume\n",
        ("rev-parse", "--short", "HEAD"): "abcdef0\n",
        ("diff", "--cached", "--name-only"): "AGENTS.md\n",
        ("diff", "--name-only"): "scripts/hooks/post_tool_validation_hint.py\n",
        ("ls-files", "--others", "--exclude-standard"): "tests/test_new_file.py\n",
    }

    def fake_run_git(args: tuple[str, ...], _repo_root: Path):
        class Result:
            def __init__(self, stdout: str) -> None:
                self.stdout = stdout
                self.returncode = 0

        return Result(outputs.get(tuple(args), ""))

    monkeypatch.setattr(work_context, "run_git", fake_run_git)

    state = session_snapshot.collect_git_state(tmp_path)

    assert state.branch == "feature/resume"
    assert state.head == "abcdef0"
    assert state.staged_files == ("AGENTS.md",)
    assert state.unstaged_files == ("scripts/hooks/post_tool_validation_hint.py",)
    assert state.untracked_files == ("tests/test_new_file.py",)


def test_resolve_snapshot_path_supports_worktree_git_file(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    (repo_root / ".git").write_text("gitdir: C:/fake/worktree/.git\n", encoding="utf-8")

    def fake_resolve_git_ai_path(relative_path: Path, _repo_root: Path) -> Path:
        assert relative_path.as_posix() == "ai/last-session.md"
        return (repo_root / ".git" / "ai" / "last-session.md").resolve()

    monkeypatch.setattr(session_snapshot, "resolve_git_ai_path", fake_resolve_git_ai_path)

    resolved = session_snapshot.resolve_snapshot_path(repo_root)

    assert resolved == (repo_root / ".git" / "ai" / "last-session.md").resolve()


def test_write_session_snapshot_returns_none_on_filesystem_error(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    snapshot_path = repo_root / ".git" / "ai" / "last-session.md"

    monkeypatch.setattr(session_snapshot, "resolve_snapshot_path", lambda _repo_root=repo_root: snapshot_path)
    monkeypatch.setattr(
        session_snapshot,
        "collect_git_state",
        lambda _repo_root=repo_root: GitState(
            branch="main",
            head="1234567",
            staged_files=(),
            unstaged_files=(),
            untracked_files=(),
        ),
    )
    monkeypatch.setattr(session_snapshot, "write_task_state", lambda **_kwargs: None)

    monkeypatch.setattr(session_snapshot, "write_text_atomic", lambda *_args, **_kwargs: (_ for _ in ()).throw(OSError("disk full")))

    result = session_snapshot.write_session_snapshot(trigger="post-tool", recent_files=["AGENTS.md"], repo_root=repo_root)

    assert result is None


def test_write_session_snapshot_embeds_linked_task_state_metadata(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    snapshot_path = repo_root / ".git" / "ai" / "last-session.md"

    monkeypatch.setattr(session_snapshot, "resolve_snapshot_path", lambda _repo_root=repo_root: snapshot_path)
    monkeypatch.setattr(
        session_snapshot,
        "collect_git_state",
        lambda _repo_root=repo_root: GitState(
            branch="feature/handoff",
            head="abcdef0",
            staged_files=(".github/prompts/handoff.prompt.md",),
            unstaged_files=("docs/AI_WORKFLOW.md",),
            untracked_files=(),
        ),
    )
    monkeypatch.setattr(
        session_snapshot,
        "write_task_state",
        lambda **_kwargs: TaskStateWriteResult(
            task_state_path=repo_root / ".git" / "ai" / "task-state.json",
            task_state_display_path=".git/ai/task-state.json",
            title="整理会话交接",
            status="handoff",
            scopes=("ai-customization",),
            next_step="先完成 handoff，再恢复实现。",
            verification_steps=("运行 agent_stack_health。",),
        ),
    )

    result = session_snapshot.write_session_snapshot(
        trigger="stop-hook",
        recent_files=[".github/prompts/handoff.prompt.md"],
        repo_root=repo_root,
    )

    assert result is not None
    assert result.task_state_display_path == ".git/ai/task-state.json"
    snapshot_text = snapshot_path.read_text(encoding="utf-8")
    assert "Linked Task State Path" in snapshot_text
    assert ".git/ai/task-state.json" in snapshot_text
    assert "整理会话交接" in snapshot_text