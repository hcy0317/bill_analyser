from __future__ import annotations

from pathlib import Path

import pytest

from scripts.hooks import session_snapshot


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

    monkeypatch.setattr(session_snapshot, "_run_git", fake_run_git)

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

    def fake_run_git(args: tuple[str, ...], _repo_root: Path):
        class Result:
            def __init__(self) -> None:
                self.stdout = str(repo_root / ".git" / "ai" / "last-session.md")
                self.returncode = 0

        assert tuple(args) == ("rev-parse", "--git-path", "ai/last-session.md")
        return Result()

    monkeypatch.setattr(session_snapshot, "_run_git", fake_run_git)

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
        lambda _repo_root=repo_root: session_snapshot.GitState(
            branch="main",
            head="1234567",
            staged_files=(),
            unstaged_files=(),
            untracked_files=(),
        ),
    )

    def fail_write_text(self: Path, _text: str, encoding: str = "utf-8") -> None:
        raise OSError("disk full")

    monkeypatch.setattr(Path, "write_text", fail_write_text)

    result = session_snapshot.write_session_snapshot(trigger="post-tool", recent_files=["AGENTS.md"], repo_root=repo_root)

    assert result is None