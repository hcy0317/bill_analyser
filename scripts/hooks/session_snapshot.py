from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from scripts.hooks.file_io import write_text_atomic
from scripts.hooks.task_state import write_task_state
from scripts.hooks.work_context import (
    REPO_ROOT,
    build_verification_steps,
    choose_next_step,
    collect_git_state,
    detect_scopes,
    format_display_path,
    normalize_paths,
    resolve_git_ai_path,
)


SNAPSHOT_RELATIVE_PATH = Path(".git") / "ai" / "last-session.md"
GIT_INTERNAL_SNAPSHOT_PATH = Path("ai") / "last-session.md"
TASK_STATE_CONTRACT_PATH = ".git/ai/task-state.json"


@dataclass(frozen=True)
class SnapshotWriteResult:
    snapshot_path: Path
    snapshot_display_path: str
    task_state_display_path: str | None
    task_title: str | None
    task_status: str | None
    scopes: tuple[str, ...]
    next_step: str
    verification_steps: tuple[str, ...]


def resolve_snapshot_path(repo_root: Path = REPO_ROOT) -> Path | None:
    return resolve_git_ai_path(GIT_INTERNAL_SNAPSHOT_PATH, repo_root)


def format_snapshot_display_path(snapshot_path: Path, repo_root: Path = REPO_ROOT) -> str:
    return format_display_path(snapshot_path, repo_root)


def build_snapshot_markdown(
    *,
    trigger: str,
    git_state,
    recent_files: Sequence[str],
    snapshot_display_path: str,
    task_state_display_path: str | None = None,
    task_title: str | None = None,
    task_status: str | None = None,
) -> str:
    normalized_recent_files = normalize_paths(recent_files)
    all_paths = normalize_paths(
        (*git_state.staged_files, *git_state.unstaged_files, *git_state.untracked_files, *normalized_recent_files)
    )
    scopes = detect_scopes(all_paths)
    verification_steps = build_verification_steps(all_paths)
    next_step = choose_next_step(all_paths)
    timestamp = datetime.now(tz=timezone.utc).astimezone().isoformat(timespec="seconds")

    def _render_list(items: Sequence[str], empty_text: str) -> str:
        if not items:
            return f"- {empty_text}"
        return "\n".join(f"- `{item}`" for item in items)

    scope_block = _render_list(scopes, "未识别到明确焦点域")
    recent_block = _render_list(normalized_recent_files, "本次 hook 未提供新的编辑路径")
    staged_block = _render_list(git_state.staged_files, "暂无 staged diff")
    unstaged_block = _render_list(git_state.unstaged_files, "暂无 unstaged diff")
    untracked_block = _render_list(git_state.untracked_files, "暂无 untracked 文件")
    verification_block = "\n".join(f"{index}. {step}" for index, step in enumerate(verification_steps, start=1))

    return "\n".join(
        (
            "# Last Session Snapshot",
            "",
            f"- Updated: {timestamp}",
            f"- Trigger: `{trigger}`",
            f"- Branch: `{git_state.branch}`",
            f"- HEAD: `{git_state.head}`",
            f"- Logical Snapshot Path: `{SNAPSHOT_RELATIVE_PATH.as_posix()}`",
            f"- Resolved Snapshot Path: `{snapshot_display_path}`",
            f"- Linked Task State Path: `{task_state_display_path or TASK_STATE_CONTRACT_PATH}`",
            "",
            "## Active task state",
            f"- Title: `{task_title or '未推断出明确任务标题'}`",
            f"- Status: `{task_status or 'unknown'}`",
            "",
            "## Recent edited files",
            recent_block,
            "",
            "## Current diff state",
            "### Staged",
            staged_block,
            "",
            "### Unstaged",
            unstaged_block,
            "",
            "### Untracked",
            untracked_block,
            "",
            "## Detected focus areas",
            scope_block,
            "",
            "## Suggested next verification",
            verification_block,
            "",
            "## Suggested next step",
            f"- {next_step}",
            "",
            "## Resume checklist",
            "1. Read `AGENTS.md`.",
            "2. Read `docs/PROJECT_OVERVIEW.md`.",
            (
                "3. Read `.git/ai/last-session.md` if it exists; under worktree or special git layouts, "
                "resolve the real path with `git rev-parse --git-path ai/last-session.md`."
            ),
            "4. Inspect `git status` and `git diff`.",
            "5. Continue the pending task from the files and verification hints above.",
            "",
            "## Safety",
            "- This snapshot stores metadata only; never put secrets, environment variables, or full patch contents here.",
            "",
        )
    )


def write_session_snapshot(
    *,
    trigger: str,
    recent_files: Sequence[str],
    repo_root: Path = REPO_ROOT,
) -> SnapshotWriteResult | None:
    snapshot_path = resolve_snapshot_path(repo_root)
    if snapshot_path is None:
        return None

    git_state = collect_git_state(repo_root)
    all_paths = normalize_paths((*git_state.staged_files, *git_state.unstaged_files, *git_state.untracked_files, *recent_files))
    snapshot_display_path = format_snapshot_display_path(snapshot_path, repo_root)
    task_state = write_task_state(
        trigger=trigger,
        recent_files=recent_files,
        snapshot_display_path=snapshot_display_path,
        repo_root=repo_root,
    )
    snapshot_text = build_snapshot_markdown(
        trigger=trigger,
        git_state=git_state,
        recent_files=recent_files,
        snapshot_display_path=snapshot_display_path,
        task_state_display_path=task_state.task_state_display_path if task_state else None,
        task_title=task_state.title if task_state else None,
        task_status=task_state.status if task_state else None,
    )
    try:
        write_text_atomic(snapshot_path, snapshot_text, encoding="utf-8")
    except OSError:
        return None

    return SnapshotWriteResult(
        snapshot_path=snapshot_path,
        snapshot_display_path=snapshot_display_path,
        task_state_display_path=task_state.task_state_display_path if task_state else None,
        task_title=task_state.title if task_state else None,
        task_status=task_state.status if task_state else None,
        scopes=detect_scopes(all_paths),
        next_step=choose_next_step(all_paths),
        verification_steps=build_verification_steps(all_paths),
    )