from __future__ import annotations

import json
from pathlib import Path

from scripts.hooks import task_state_reader


def test_read_task_state_returns_none_for_missing_or_invalid_files(tmp_path: Path, monkeypatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    missing = repo_root / ".git" / "ai" / "task-state.json"

    monkeypatch.setattr(task_state_reader, "resolve_task_state_path", lambda _repo_root=repo_root: missing)
    assert task_state_reader.read_task_state(repo_root) is None

    missing.parent.mkdir(parents=True, exist_ok=True)
    missing.write_text("not-json", encoding="utf-8")
    assert task_state_reader.read_task_state(repo_root) is None


def test_read_and_summarize_task_state_returns_human_readable_summary(tmp_path: Path, monkeypatch) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()
    task_state_path = repo_root / ".git" / "ai" / "task-state.json"
    task_state_path.parent.mkdir(parents=True, exist_ok=True)
    task_state_path.write_text(
        json.dumps(
            {
                "title": "整理会话交接",
                "status": "handoff",
                "nextStep": "先确认 diff 拆分，再继续实现。",
                "nextVerification": ["运行 agent_stack_health", "运行相关 pytest"],
            },
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )

    monkeypatch.setattr(task_state_reader, "resolve_task_state_path", lambda _repo_root=repo_root: task_state_path)
    payload = task_state_reader.read_task_state(repo_root)
    summary = task_state_reader.summarize_task_state(payload)

    assert payload is not None
    assert summary[0] == "title=整理会话交接"
    assert any("next_verification=" in line for line in summary)
    assert task_state_reader.TASK_STATE_CONTRACT_PATH == ".git/ai/task-state.json"
