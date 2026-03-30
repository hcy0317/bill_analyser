from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from scripts.hooks.task_state import REPO_ROOT, resolve_task_state_path


TASK_STATE_CONTRACT_PATH = ".git/ai/task-state.json"


def read_task_state(repo_root: Path = REPO_ROOT) -> dict[str, Any] | None:
    task_state_path = resolve_task_state_path(repo_root)
    if task_state_path is None or not task_state_path.exists():
        return None

    try:
        raw_payload = json.loads(task_state_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None

    return raw_payload if isinstance(raw_payload, dict) else None


def summarize_task_state(task_state: dict[str, Any] | None) -> tuple[str, ...]:
    if not task_state:
        return (f"Task state unavailable: {TASK_STATE_CONTRACT_PATH}",)

    title = str(task_state.get("title") or "<unknown>")
    status = str(task_state.get("status") or "<unknown>")
    next_step = str(task_state.get("nextStep") or "<unknown>")
    verification = task_state.get("nextVerification")
    verification_steps = verification if isinstance(verification, list) else []

    lines = [
        f"title={title}",
        f"status={status}",
        f"next_step={next_step}",
    ]
    if verification_steps:
        lines.append("next_verification=" + " | ".join(str(item) for item in verification_steps))
    return tuple(lines)
