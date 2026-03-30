from __future__ import annotations

# ruff: noqa: I001

import argparse
from dataclasses import dataclass
from datetime import UTC, datetime
import json
from pathlib import Path
import sys
from typing import TYPE_CHECKING


if TYPE_CHECKING:
    from collections.abc import Sequence

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

TASK_STATE_RELATIVE_PATH = Path(".git") / "ai" / "task-state.json"
GIT_INTERNAL_TASK_STATE_PATH = Path("ai") / "task-state.json"
DEFAULT_STATUS = "in_progress"
KNOWN_STATUSES = {"pending", "in_progress", "blocked", "handoff", "completed"}


@dataclass(frozen=True)
class TaskStateWriteResult:
    task_state_path: Path
    task_state_display_path: str
    title: str
    status: str
    scopes: tuple[str, ...]
    next_step: str
    verification_steps: tuple[str, ...]


def _as_text_tuple(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()

    return tuple(str(item) for item in value)


def resolve_task_state_path(repo_root: Path = REPO_ROOT) -> Path | None:
    return resolve_git_ai_path(GIT_INTERNAL_TASK_STATE_PATH, repo_root)


def _normalize_status(status: str | None) -> str:
    normalized = str(status or DEFAULT_STATUS).strip().lower().replace("-", "_")
    return normalized if normalized in KNOWN_STATUSES else DEFAULT_STATUS


def build_task_title(scopes: Sequence[str], recent_files: Sequence[str], explicit_title: str | None = None) -> str:
    title = str(explicit_title or "").strip()
    if title:
        return title

    scope_set = set(scopes)
    normalized_recent_files = normalize_paths(recent_files)

    if any(
        path.endswith("handoff.prompt.md") or path.endswith("session-handoff/SKILL.md")
        for path in normalized_recent_files
    ):
        return "整理会话交接与恢复状态"
    if any(
        path.endswith("start-work.prompt.md") or path.endswith("approved-plan-execution/SKILL.md")
        for path in normalized_recent_files
    ):
        return "从已批准计划进入执行"
    if {"backend-runtime", "frontend"} <= scope_set:
        return "推进跨端实现并对齐契约"
    if "ai-customization" in scope_set:
        return "完善 AI 工作流与定制资产"
    if "backend-runtime" in scope_set:
        return "推进后端运行时代码改动"
    if "frontend" in scope_set:
        return "推进前端交互改动"
    if "tests" in scope_set:
        return "补齐回归验证"
    if "docs" in scope_set:
        return "更新工作流与说明文档"

    return "继续当前仓库任务"


def build_task_state_payload(
    *,
    trigger: str,
    recent_files: Sequence[str],
    status: str = DEFAULT_STATUS,
    title: str | None = None,
    blocked_by: Sequence[str] = (),
    plan_reference: str | None = None,
    notes: Sequence[str] = (),
    next_step: str | None = None,
    verification_steps: Sequence[str] | None = None,
    snapshot_display_path: str | None = None,
    repo_root: Path = REPO_ROOT,
) -> dict[str, object]:
    git_state = collect_git_state(repo_root)
    normalized_recent_files = normalize_paths(recent_files)
    all_paths = normalize_paths(
        (
            *git_state.staged_files,
            *git_state.unstaged_files,
            *git_state.untracked_files,
            *normalized_recent_files,
        )
    )
    scopes = detect_scopes(all_paths)
    resolved_verification_steps = tuple(verification_steps or build_verification_steps(all_paths))
    resolved_next_step = next_step or choose_next_step(all_paths)
    resolved_status = _normalize_status(status)

    return {
        "updatedAt": datetime.now(tz=UTC).astimezone().isoformat(timespec="seconds"),
        "trigger": trigger,
        "title": build_task_title(scopes, normalized_recent_files, title),
        "status": resolved_status,
        "blockedBy": [item for item in blocked_by if str(item).strip()],
        "planReference": plan_reference or "",
        "notes": [item for item in notes if str(item).strip()],
        "branch": git_state.branch,
        "head": git_state.head,
        "logicalTaskStatePath": TASK_STATE_RELATIVE_PATH.as_posix(),
        "snapshotPath": snapshot_display_path or "",
        "recentFiles": list(normalized_recent_files),
        "stagedFiles": list(git_state.staged_files),
        "unstagedFiles": list(git_state.unstaged_files),
        "untrackedFiles": list(git_state.untracked_files),
        "scopes": list(scopes),
        "nextVerification": list(resolved_verification_steps),
        "nextStep": resolved_next_step,
    }


def write_task_state(
    *,
    trigger: str,
    recent_files: Sequence[str],
    status: str = DEFAULT_STATUS,
    title: str | None = None,
    blocked_by: Sequence[str] = (),
    plan_reference: str | None = None,
    notes: Sequence[str] = (),
    next_step: str | None = None,
    verification_steps: Sequence[str] | None = None,
    snapshot_display_path: str | None = None,
    repo_root: Path = REPO_ROOT,
) -> TaskStateWriteResult | None:
    task_state_path = resolve_task_state_path(repo_root)
    if task_state_path is None:
        return None

    payload = build_task_state_payload(
        trigger=trigger,
        recent_files=recent_files,
        status=status,
        title=title,
        blocked_by=blocked_by,
        plan_reference=plan_reference,
        notes=notes,
        next_step=next_step,
        verification_steps=verification_steps,
        snapshot_display_path=snapshot_display_path,
        repo_root=repo_root,
    )

    try:
        task_state_path.parent.mkdir(parents=True, exist_ok=True)
        task_state_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    except OSError:
        return None

    return TaskStateWriteResult(
        task_state_path=task_state_path,
        task_state_display_path=format_display_path(task_state_path, repo_root),
        title=str(payload["title"]),
        status=str(payload["status"]),
        scopes=_as_text_tuple(payload.get("scopes")),
        next_step=str(payload["nextStep"]),
        verification_steps=_as_text_tuple(payload.get("nextVerification")),
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Write lightweight AI task state into .git/ai/task-state.json")
    parser.add_argument(
        "--trigger",
        default="manual",
        help="Metadata trigger describing why the task state is being refreshed",
    )
    parser.add_argument(
        "--status",
        default=DEFAULT_STATUS,
        help="Task status: pending/in_progress/blocked/handoff/completed",
    )
    parser.add_argument("--title", default="", help="Optional explicit task title")
    parser.add_argument(
        "--recent-file",
        action="append",
        dest="recent_files",
        default=[],
        help="Recent file path to include",
    )
    parser.add_argument("--blocked-by", action="append", dest="blocked_by", default=[], help="Blocking dependency note")
    parser.add_argument("--plan-reference", default="", help="Optional plan/checklist reference")
    parser.add_argument("--note", action="append", dest="notes", default=[], help="Additional note to persist")
    parser.add_argument("--next-step", default="", help="Optional explicit next-step override")
    parser.add_argument(
        "--verification-step",
        action="append",
        dest="verification_steps",
        default=[],
        help="Optional explicit verification step override",
    )
    parser.add_argument("--snapshot-path", default="", help="Optional display path to the latest session snapshot")
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT, help="Repository root")
    parser.add_argument("--json", action="store_true", help="Print JSON result instead of plain text")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    result = write_task_state(
        trigger=args.trigger,
        recent_files=args.recent_files,
        status=args.status,
        title=args.title or None,
        blocked_by=args.blocked_by,
        plan_reference=args.plan_reference or None,
        notes=args.notes,
        next_step=args.next_step or None,
        verification_steps=args.verification_steps or None,
        snapshot_display_path=args.snapshot_path or None,
        repo_root=args.repo_root.resolve(),
    )
    if result is None:
        return 1

    if args.json:
        sys.stdout.write(
            json.dumps(
                {
                    "path": result.task_state_display_path,
                    "title": result.title,
                    "status": result.status,
                    "scopes": result.scopes,
                    "nextStep": result.next_step,
                    "verificationSteps": result.verification_steps,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        sys.stdout.write(
            "\n".join(
                (
                    f"Task state updated: {result.task_state_display_path}",
                    f"title={result.title}",
                    f"status={result.status}",
                    f"next_step={result.next_step}",
                )
            )
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
