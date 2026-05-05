"""Python subprocess bridge for Rust-backed template master-data operations."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


_BRIDGE_TIMEOUT_SECONDS = 10
_BRIDGE_ENV = "BILL_ANALYSER_RUST_TAXONOMY_BRIDGE"


class TemplateRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust taxonomy bridge cannot be executed safely."""


class TemplateRustBridgeOperationError(RuntimeError):
    """Raised when Rust returns a domain/database operation failure."""


def list_templates(
    db_path: str | Path,
    user_id: int = 1,
    template_type: int | None = None,
) -> list[dict[str, Any]]:
    """List transaction templates through the Rust taxonomy bridge."""
    result = _invoke_template_bridge(
        "list-templates",
        _base_payload(db_path, user_id, template_type=template_type),
    )
    if isinstance(result, list):
        return [_coerce_template_dict(item) for item in result]
    raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid template list")


def get_template(
    db_path: str | Path,
    template_id: int,
    user_id: int = 1,
    template_type: int | None = None,
) -> dict[str, Any] | None:
    """Fetch a single template DTO through the Rust taxonomy bridge."""
    payload = {
        **_base_payload(db_path, user_id, template_type=template_type),
        "template_id": int(template_id),
    }
    result = _invoke_template_bridge("get-template", payload)
    if result is None:
        return None
    if isinstance(result, dict):
        return _coerce_template_dict(result)
    raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid template")


def list_enabled_recurring_templates(
    db_path: str | Path,
    user_id: int = 1,
) -> list[dict[str, Any]]:
    """List raw enabled recurring template rows through Rust."""
    result = _invoke_template_bridge(
        "list-enabled-recurring-templates",
        _base_payload(db_path, user_id),
    )
    if isinstance(result, list):
        return [_coerce_template_dict(item) for item in result]
    raise TemplateRustBridgeUnavailable(
        "Rust taxonomy bridge returned an invalid recurring template list"
    )


def create_template(db_path: str | Path, data: dict[str, Any], user_id: int = 1) -> int:
    """Create one transaction template through Rust."""
    payload = {**_base_payload(db_path, user_id), "payload": data}
    result = _invoke_template_bridge("create-template", payload)
    if isinstance(result, bool) or not isinstance(result, int):
        raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid template id")
    return result


def update_template(
    db_path: str | Path,
    template_id: int,
    data: dict[str, Any],
    user_id: int = 1,
    template_type: int | None = None,
) -> bool:
    """Update one transaction template through Rust."""
    payload = {
        **_base_payload(db_path, user_id, template_type=template_type),
        "template_id": int(template_id),
        "payload": data,
    }
    return _expect_bool(_invoke_template_bridge("update-template", payload))


def delete_template(
    db_path: str | Path,
    template_id: int,
    user_id: int = 1,
    template_type: int | None = None,
) -> bool:
    """Delete one transaction template through Rust."""
    payload = {
        **_base_payload(db_path, user_id, template_type=template_type),
        "template_id": int(template_id),
    }
    return _expect_bool(_invoke_template_bridge("delete-template", payload))


def update_display_orders(
    db_path: str | Path,
    orders: list[tuple[int, int]],
    template_type: int,
    user_id: int = 1,
) -> bool:
    """Persist template display-order updates through Rust."""
    bridge_orders = [
        [int(template_id), int(display_order)]
        for template_id, display_order in orders
    ]
    payload = {
        **_base_payload(db_path, user_id),
        "orders": bridge_orders,
        "template_type": int(template_type),
    }
    return _expect_bool(_invoke_template_bridge("update-template-display-orders", payload))


def _base_payload(
    db_path: str | Path,
    user_id: int,
    template_type: int | None = None,
) -> dict[str, Any]:
    payload = {"db_path": str(db_path), "user_id": int(user_id)}
    if template_type is not None:
        payload["template_type"] = int(template_type)
    return payload


def _expect_bool(result: Any) -> bool:
    if isinstance(result, bool):
        return result
    raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid boolean")


def _coerce_template_dict(raw_value: Any) -> dict[str, Any]:
    if not isinstance(raw_value, dict):
        raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid template row")
    return dict(raw_value)


def _invoke_template_bridge(command: str, payload: dict[str, Any]) -> Any:
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
        raise TemplateRustBridgeUnavailable("Rust taxonomy bridge is unavailable") from exc

    if completed.returncode != 0:
        raise TemplateRustBridgeUnavailable("Rust taxonomy bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned invalid JSON") from exc

    if not isinstance(response, dict):
        raise TemplateRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid response"
        ) from None

    if response.get("success") is True:
        return response.get("result")

    if response.get("success") is False and isinstance(response.get("error"), str):
        raise TemplateRustBridgeOperationError(response["error"])

    raise TemplateRustBridgeUnavailable("Rust taxonomy bridge returned an invalid response")


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get(_BRIDGE_ENV)
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise TemplateRustBridgeUnavailable("Rust taxonomy bridge executable is not available")


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = "bill_taxonomy_bridge.exe" if os.name == "nt" else "bill_taxonomy_bridge"
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
