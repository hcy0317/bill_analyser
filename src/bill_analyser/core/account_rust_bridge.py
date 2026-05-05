"""Python subprocess bridge for Rust-backed account master-data operations."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


_BRIDGE_TIMEOUT_SECONDS = 10
_BRIDGE_ENV = "BILL_ANALYSER_RUST_TAXONOMY_BRIDGE"


class AccountRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust taxonomy bridge cannot be executed safely."""


class AccountRustBridgeOperationError(RuntimeError):
    """Raised when Rust returns a domain/database operation failure."""


def list_accounts(db_path: str | Path, user_id: int = 1) -> list[dict[str, Any]]:
    """List accounts for one user via the Rust taxonomy bridge."""
    result = _invoke_account_bridge("list-accounts", _base_payload(db_path, user_id))
    if isinstance(result, list):
        return [_coerce_account_dict(item) for item in result]
    raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid account list")


def get_account(
    db_path: str | Path,
    account_id: int,
    user_id: int = 1,
) -> dict[str, Any] | None:
    """Fetch a single account row through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "account_id": int(account_id)}
    result = _invoke_account_bridge("get-account", payload)
    if result is None:
        return None
    if isinstance(result, dict):
        return _coerce_account_dict(result)
    raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid account")


def get_sub_accounts(
    db_path: str | Path,
    parent_id: int,
    user_id: int = 1,
) -> list[dict[str, Any]]:
    """Fetch child accounts for a parent through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "parent_id": int(parent_id)}
    result = _invoke_account_bridge("get-sub-accounts", payload)
    if isinstance(result, list):
        return [_coerce_account_dict(item) for item in result]
    raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid subaccount list")


def create_account(db_path: str | Path, data: dict[str, Any], user_id: int = 1) -> int:
    """Create an account through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "payload": data}
    result = _invoke_account_bridge("create-account", payload)
    if isinstance(result, bool) or not isinstance(result, int):
        raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid account id")
    return result


def update_account(
    db_path: str | Path,
    account_id: int,
    data: dict[str, Any],
    user_id: int = 1,
) -> bool:
    """Update one account through the Rust taxonomy bridge."""
    payload = {
        **_base_payload(db_path, user_id),
        "account_id": int(account_id),
        "payload": data,
    }
    return _expect_bool(_invoke_account_bridge("update-account", payload))


def delete_account(db_path: str | Path, account_id: int, user_id: int = 1) -> bool:
    """Delete one account through the Rust taxonomy bridge."""
    payload = {**_base_payload(db_path, user_id), "account_id": int(account_id)}
    return _expect_bool(_invoke_account_bridge("delete-account", payload))


def update_display_orders(
    db_path: str | Path,
    orders: list[tuple[int, int]],
    user_id: int = 1,
) -> bool:
    """Persist account display-order updates through the Rust taxonomy bridge."""
    bridge_orders = [[int(account_id), int(display_order)] for account_id, display_order in orders]
    payload = {**_base_payload(db_path, user_id), "orders": bridge_orders}
    return _expect_bool(_invoke_account_bridge("update-account-display-orders", payload))


def _base_payload(db_path: str | Path, user_id: int) -> dict[str, Any]:
    return {"db_path": str(db_path), "user_id": int(user_id)}


def _expect_bool(result: Any) -> bool:
    if isinstance(result, bool):
        return result
    raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid boolean")


def _coerce_account_dict(raw_value: Any) -> dict[str, Any]:
    if not isinstance(raw_value, dict):
        raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid account row")
    return dict(raw_value)


def _invoke_account_bridge(command: str, payload: dict[str, Any]) -> Any:
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
        raise AccountRustBridgeUnavailable("Rust taxonomy bridge is unavailable") from exc

    if completed.returncode != 0:
        raise AccountRustBridgeUnavailable("Rust taxonomy bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned invalid JSON") from exc

    if not isinstance(response, dict):
        raise AccountRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid response"
        ) from None

    if response.get("success") is True:
        return response.get("result")

    if response.get("success") is False and isinstance(response.get("error"), str):
        raise AccountRustBridgeOperationError(response["error"])

    raise AccountRustBridgeUnavailable("Rust taxonomy bridge returned an invalid response")


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get(_BRIDGE_ENV)
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise AccountRustBridgeUnavailable("Rust taxonomy bridge executable is not available")


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = "bill_taxonomy_bridge.exe" if os.name == "nt" else "bill_taxonomy_bridge"
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
