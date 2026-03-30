from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.hooks.session_snapshot import write_session_snapshot

PYTHON_RUNTIME_PREFIX = "src/bill_analyser/"
API_ROUTE_PREFIX = "src/bill_analyser/api/routes/"
SERVICES_TS_PATH = "src/web/src/lib/services.ts"
PYTHON_EDIT_TOOL_NAMES = {"edit", "write", "create", "apply_patch", "create_file"}
PATH_KEYS = ("file_path", "path", "filePath")


def load_payload(stdin_text: str) -> dict[str, Any] | None:
    raw_text = stdin_text.strip()
    if not raw_text:
        return None

    try:
        payload = json.loads(raw_text)
    except json.JSONDecodeError:
        return None

    return payload if isinstance(payload, dict) else None


def get_tool_name(payload: dict[str, Any]) -> str:
    raw_name = payload.get("tool_name") or payload.get("toolName") or ""
    return str(raw_name).lower()


def get_tool_input(payload: dict[str, Any]) -> dict[str, Any]:
    if "tool_input" in payload and isinstance(payload["tool_input"], dict):
        return payload["tool_input"]

    raw_args = payload.get("toolArgs")
    if isinstance(raw_args, dict):
        return raw_args
    if isinstance(raw_args, str):
        try:
            decoded = json.loads(raw_args)
        except json.JSONDecodeError:
            return {}
        return decoded if isinstance(decoded, dict) else {}

    return {}


def resolve_candidate_path(tool_input: dict[str, Any], cwd_value: Any) -> Path | None:
    for path_key in PATH_KEYS:
        raw_path = str(tool_input.get(path_key) or "").strip()
        if not raw_path:
            continue

        candidate = Path(raw_path)
        if candidate.is_absolute():
            return candidate.resolve()

        raw_cwd = str(cwd_value or "").strip()
        if raw_cwd:
            return (Path(raw_cwd) / candidate).resolve()
        return (REPO_ROOT / candidate).resolve()

    return None


def to_repo_relative(candidate_path: Path | None) -> str | None:
    if candidate_path is None:
        return None

    try:
        return candidate_path.resolve().relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return None


def build_messages(relative_path: str) -> list[str]:
    messages: list[str] = []

    if relative_path.endswith(".py"):
        messages.append("[hook] Python 文件已编辑：建议运行受影响 pytest，并对改动模块执行 pylint。")

    if relative_path.startswith(PYTHON_RUNTIME_PREFIX):
        messages.append(
            "[hook] 检测到业务运行时代码变更：最终验收前必须跑全量 "
            "`./.venv/Scripts/python.exe -m pytest tests/ -v`。"
        )

    if relative_path.startswith(API_ROUTE_PREFIX) or relative_path == "src/bill_analyser/api/app.py":
        messages.append(
            f"[hook] 检测到 API 路由/契约相关文件变更：请确认 `{SERVICES_TS_PATH}` "
            "和相关 store 是否需要同步更新。"
        )

    return messages


def build_hook_messages(payload: dict[str, Any]) -> list[str]:
    tool_name = get_tool_name(payload)
    if tool_name not in PYTHON_EDIT_TOOL_NAMES:
        return []

    tool_input = get_tool_input(payload)
    candidate_path = resolve_candidate_path(tool_input, payload.get("cwd"))
    relative_path = to_repo_relative(candidate_path)
    if not relative_path:
        return []

    messages = build_messages(relative_path)
    snapshot = write_session_snapshot(trigger="post-tool", recent_files=[relative_path], repo_root=REPO_ROOT)
    if snapshot is not None:
        messages.append(
            f"[hook] 已刷新 `{snapshot.snapshot_display_path}`；如果会话因网络中断，可先读取该快照再继续。"
        )

    return messages


def main() -> int:
    payload = load_payload(sys.stdin.read())
    if payload is None:
        return 0

    messages = build_hook_messages(payload)
    if not messages:
        return 0

    sys.stdout.write("\n".join(messages) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
