from __future__ import annotations

import subprocess

import pytest

from bill_analyser.core import auth_rust_bridge
from bill_analyser.core.auth_rust_bridge import (
    AuthRustBridgeUnavailable,
    AuthRustValidationError,
    validate_refresh_token_claims,
)


def test_python_bridge_invokes_real_rust_refresh_claim_validation() -> None:
    """Python bridge should call the Rust auth contract for refresh claims."""
    claims = validate_refresh_token_claims({"type": "refresh", "user_id": 42, "username": "alice"})

    assert claims.user_id == 42
    assert claims.username == "alice"


@pytest.mark.parametrize(
    ("payload", "expected_status", "expected_message"),
    [
        ({"type": "access", "user_id": 42, "username": "alice"}, 400, "Not a refresh token"),
        ({"type": "refresh", "user_id": "bad", "username": ""}, 401, "Invalid refresh token"),
    ],
)
def test_python_bridge_maps_rust_refresh_claim_errors(
    payload: dict[str, object],
    expected_status: int,
    expected_message: str,
) -> None:
    """Rust validation errors should keep the existing REST status/message contract."""
    with pytest.raises(AuthRustValidationError) as error_info:
        validate_refresh_token_claims(payload)

    assert error_info.value.status == expected_status
    assert error_info.value.error == "Invalid token"
    assert error_info.value.message == expected_message


def test_python_bridge_maps_process_failures_to_unavailable(monkeypatch: pytest.MonkeyPatch) -> None:
    """Bridge process failures should fail closed without falling back to Python validation."""
    monkeypatch.setattr(auth_rust_bridge, "_resolve_bridge_command", lambda command: ["bridge", command])

    def raise_os_error(*_args, **_kwargs):
        raise OSError("bridge missing")

    monkeypatch.setattr(auth_rust_bridge.subprocess, "run", raise_os_error)
    with pytest.raises(AuthRustBridgeUnavailable):
        auth_rust_bridge._invoke_auth_bridge("validate-refresh-token-claims", {"type": "refresh"})

    monkeypatch.setattr(
        auth_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 1, "", "failed"),
    )
    with pytest.raises(AuthRustBridgeUnavailable):
        auth_rust_bridge._invoke_auth_bridge("validate-refresh-token-claims", {"type": "refresh"})


def test_python_bridge_rejects_invalid_rust_response(monkeypatch: pytest.MonkeyPatch) -> None:
    """Invalid Rust bridge stdout should be treated as infrastructure failure."""
    monkeypatch.setattr(auth_rust_bridge, "_resolve_bridge_command", lambda command: ["bridge", command])

    monkeypatch.setattr(
        auth_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 0, "not-json", ""),
    )
    with pytest.raises(AuthRustBridgeUnavailable):
        auth_rust_bridge._invoke_auth_bridge("validate-refresh-token-claims", {"type": "refresh"})

    monkeypatch.setattr(
        auth_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 0, "[]", ""),
    )
    with pytest.raises(AuthRustBridgeUnavailable):
        auth_rust_bridge._invoke_auth_bridge("validate-refresh-token-claims", {"type": "refresh"})


@pytest.mark.parametrize(
    "response",
    [
        {"success": True, "result": {}},
        {"success": True, "result": {"user_id": True, "username": "alice"}},
        {"success": True, "result": {"user_id": 42, "username": ""}},
        {"success": False, "error": {}},
        {"success": False, "error": {"status": "401", "error": "Invalid token", "message": "Invalid refresh token"}},
        {"result": {"user_id": 42, "username": "alice"}},
    ],
)
def test_python_bridge_rejects_malformed_rust_payloads(
    response: dict[str, object],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Malformed bridge JSON should fail closed instead of leaking as a 500."""
    monkeypatch.setattr(auth_rust_bridge, "_invoke_auth_bridge", lambda _command, _payload: response)

    with pytest.raises(AuthRustBridgeUnavailable):
        validate_refresh_token_claims({"type": "refresh", "user_id": 42, "username": "alice"})


def test_python_bridge_honors_explicit_bridge_executable(monkeypatch: pytest.MonkeyPatch) -> None:
    """Operator-provided bridge executable should be used without shell expansion."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_AUTH_BRIDGE", "custom-auth-bridge")

    assert auth_rust_bridge._resolve_bridge_command("validate-refresh-token-claims") == [
        "custom-auth-bridge",
        "validate-refresh-token-claims",
    ]


def test_python_bridge_requires_prebuilt_bridge_without_env(
    tmp_path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Request handling should not run cargo as an implicit runtime fallback."""
    monkeypatch.delenv("BILL_ANALYSER_RUST_AUTH_BRIDGE", raising=False)
    monkeypatch.setattr(auth_rust_bridge, "_repo_root", lambda: tmp_path)

    with pytest.raises(AuthRustBridgeUnavailable):
        auth_rust_bridge._resolve_bridge_command("validate-refresh-token-claims")
