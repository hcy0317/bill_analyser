"""Python subprocess bridge for Rust-backed category master-data operations."""

from __future__ import annotations

import json
import os
import sqlite3
import subprocess
from pathlib import Path
from typing import Any


_BRIDGE_TIMEOUT_SECONDS = 10
_BRIDGE_ENV = "BILL_ANALYSER_RUST_TAXONOMY_BRIDGE"


class CategoryRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust taxonomy bridge cannot be executed safely."""


class CategoryRustBridgeOperationError(RuntimeError):
    """Raised when Rust returns a domain/database operation failure."""


def list_categories(db_path: str | Path, user_id: int = 1) -> list[dict[str, Any]]:
    """List categories for one user via the Rust taxonomy bridge."""
    result = _invoke_category_bridge("list-categories", _base_payload(db_path, user_id))
    if isinstance(result, list):
        return [_coerce_category_dict(item) for item in result]
    raise CategoryRustBridgeUnavailable(
        "Rust taxonomy bridge returned an invalid category list"
    )


def get_category(
    db_path: str | Path,
    category_id: int,
    user_id: int = 1,
) -> dict[str, Any] | None:
    """Fetch a single category row through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "category_id": int(category_id)}
    result = _invoke_category_bridge("get-category", payload)
    if result is None:
        return None
    if isinstance(result, dict):
        return _coerce_category_dict(result)
    raise CategoryRustBridgeUnavailable("Rust taxonomy bridge returned an invalid category")


def get_category_by_name(
    db_path: str | Path,
    main_category: str,
    sub_category: str,
    user_id: int = 1,
) -> dict[str, Any] | None:
    """Fetch a category by main/sub category name through Rust."""
    payload = {
        **_base_payload(db_path, user_id),
        "main_category": str(main_category),
        "sub_category": str(sub_category),
    }
    result = _invoke_category_bridge("get-category-by-name", payload)
    if result is None:
        return None
    if isinstance(result, dict):
        return _coerce_category_dict(result)
    raise CategoryRustBridgeUnavailable("Rust taxonomy bridge returned an invalid category")


def create_category(
    db_path: str | Path,
    data: dict[str, Any],
    user_id: int = 1,
) -> int | None:
    """Create a category through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "payload": data}
    result = _invoke_category_bridge("create-category", payload)
    if result is None:
        return None
    if isinstance(result, bool) or not isinstance(result, int):
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid category id"
        )
    return result


def ensure_categories(
    db_path: str | Path,
    categories: list[dict[str, Any]],
    user_id: int = 1,
) -> dict[str, int]:
    """Create missing categories through one Rust taxonomy bridge process."""
    payload = {**_base_payload(db_path, user_id), "payload": categories}
    result = _invoke_category_bridge("ensure-categories", payload)
    if not isinstance(result, dict):
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid category ensure summary"
        )
    try:
        return {
            "created": int(result["created"]),
            "skipped": int(result["skipped"]),
        }
    except (KeyError, TypeError, ValueError) as exc:
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid category ensure summary"
        ) from exc


def update_category(
    db_path: str | Path,
    category_id: int,
    data: dict[str, Any],
    user_id: int = 1,
) -> bool:
    """Update one category through the Rust taxonomy bridge."""
    payload = {
        **_base_payload(db_path, user_id),
        "category_id": int(category_id),
        "payload": data,
    }
    return _expect_bool(_invoke_category_bridge("update-category", payload))


def delete_category(db_path: str | Path, category_id: int, user_id: int = 1) -> bool:
    """Delete one category through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "category_id": int(category_id)}
    return _expect_bool(_invoke_category_bridge("delete-category", payload))


def delete_categories_by_main_category(
    db_path: str | Path,
    main_category: str,
    user_id: int = 1,
) -> bool:
    """Delete all categories in one main-category group through Rust."""
    payload = {
        **_base_payload(db_path, user_id),
        "main_category": str(main_category),
    }
    return _expect_bool(
        _invoke_category_bridge("delete-categories-by-main-category", payload)
    )


def update_main_category_name(
    db_path: str | Path,
    old_name: str,
    new_name: str,
    user_id: int = 1,
) -> bool:
    """Rename one main-category group through Rust."""
    payload = {
        **_base_payload(db_path, user_id),
        "old_name": str(old_name),
        "new_name": str(new_name),
    }
    return _expect_bool(_invoke_category_bridge("update-main-category-name", payload))


def _base_payload(db_path: str | Path, user_id: int) -> dict[str, Any]:
    return {"db_path": str(db_path), "user_id": int(user_id)}


def _expect_bool(result: Any) -> bool:
    if isinstance(result, bool):
        return result
    raise CategoryRustBridgeUnavailable(
        "Rust taxonomy bridge returned an invalid boolean"
    )


def _coerce_category_dict(raw_value: Any) -> dict[str, Any]:
    if not isinstance(raw_value, dict):
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid category row"
        )
    return dict(raw_value)


def _invoke_category_bridge(command: str, payload: dict[str, Any]) -> Any:
    bridge_command = _resolve_bridge_command(command)
    encoded_payload = json.dumps(payload, separators=(",", ":"), ensure_ascii=False)

    try:
        completed = subprocess.run(
            bridge_command,
            input=encoded_payload,
            capture_output=True,
            check=False,
            encoding="utf-8",
            timeout=_BRIDGE_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise CategoryRustBridgeUnavailable("Rust taxonomy bridge is unavailable") from exc

    if completed.returncode != 0:
        raise CategoryRustBridgeUnavailable("Rust taxonomy bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid JSON"
        ) from exc

    if not isinstance(response, dict):
        raise CategoryRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid response"
        ) from None

    if response.get("success") is True:
        return response.get("result")

    if response.get("success") is False and isinstance(response.get("error"), str):
        _raise_operation_error(response["error"])

    raise CategoryRustBridgeUnavailable("Rust taxonomy bridge returned an invalid response")


def _raise_operation_error(message: str) -> None:
    lowered = message.lower()
    if "constraint" in lowered or "unique" in lowered:
        raise sqlite3.IntegrityError(message)
    raise CategoryRustBridgeOperationError(message)


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get(_BRIDGE_ENV)
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise CategoryRustBridgeUnavailable("Rust taxonomy bridge executable is not available")


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = "bill_taxonomy_bridge.exe" if os.name == "nt" else "bill_taxonomy_bridge"
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
