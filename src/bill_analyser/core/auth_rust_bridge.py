"""Python bridge for calling the Rust auth foundation."""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any


_BRIDGE_TIMEOUT_SECONDS = 10
_BRIDGE_COMMAND = "validate-refresh-token-claims"


@dataclass(frozen=True)
class RefreshTokenClaims:
    """Decoded refresh token claims validated by Rust."""

    user_id: int
    username: str


class AuthRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust auth bridge cannot be executed safely."""


class AuthRustValidationError(ValueError):
    """Rust-side validation error preserving the REST error contract."""

    def __init__(self, status: int, error: str, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.error = error
        self.message = message


def validate_refresh_token_claims(payload: dict[str, Any]) -> RefreshTokenClaims:
    """Validate decoded refresh-token claims through the Rust auth bridge."""
    response = _invoke_auth_bridge(_BRIDGE_COMMAND, payload)

    if response.get("success") is True:
        return _parse_success_response(response)

    if response.get("success") is False:
        raise _parse_validation_error(response)

    raise AuthRustBridgeUnavailable("Rust auth bridge returned an invalid response")


def _parse_success_response(response: dict[str, Any]) -> RefreshTokenClaims:
    result = response.get("result")
    if not isinstance(result, dict):
        raise AuthRustBridgeUnavailable(
            "Rust auth bridge returned an invalid success response"
        ) from None

    user_id = result.get("user_id")
    username = result.get("username")
    if (
        isinstance(user_id, bool)
        or not isinstance(user_id, int)
        or not isinstance(username, str)
        or not username
    ):
        raise AuthRustBridgeUnavailable(
            "Rust auth bridge returned an invalid success response"
        ) from None

    return RefreshTokenClaims(user_id=user_id, username=username)


def _parse_validation_error(response: dict[str, Any]) -> AuthRustValidationError:
    error = response.get("error")
    if not isinstance(error, dict):
        raise AuthRustBridgeUnavailable(
            "Rust auth bridge returned an invalid error response"
        ) from None

    status = error.get("status")
    error_name = error.get("error")
    message = error.get("message")
    if (
        isinstance(status, bool)
        or not isinstance(status, int)
        or not isinstance(error_name, str)
        or not isinstance(message, str)
    ):
        raise AuthRustBridgeUnavailable(
            "Rust auth bridge returned an invalid error response"
        ) from None

    return AuthRustValidationError(status=status, error=error_name, message=message)


def _invoke_auth_bridge(command: str, payload: dict[str, Any]) -> dict[str, Any]:
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
        raise AuthRustBridgeUnavailable("Rust auth bridge is unavailable") from exc

    if completed.returncode != 0:
        raise AuthRustBridgeUnavailable("Rust auth bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise AuthRustBridgeUnavailable("Rust auth bridge returned invalid JSON") from exc

    if not isinstance(response, dict):
        raise AuthRustBridgeUnavailable("Rust auth bridge returned an invalid response") from None

    return response


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get("BILL_ANALYSER_RUST_AUTH_BRIDGE")
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise AuthRustBridgeUnavailable("Rust auth bridge executable is not available")


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = "bill_auth_bridge.exe" if os.name == "nt" else "bill_auth_bridge"
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
