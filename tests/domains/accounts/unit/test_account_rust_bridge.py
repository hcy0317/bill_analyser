"""Account Rust bridge unit tests."""

from __future__ import annotations

# pylint: disable=protected-access,too-few-public-methods

import subprocess
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import account_rust_bridge


class _Completed:
    """Small subprocess.CompletedProcess stand-in."""

    def __init__(self, returncode: int, stdout: str) -> None:
        self.returncode = returncode
        self.stdout = stdout


def test_account_rust_bridge_wrappers_validate_result_shapes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """账户 Rust bridge wrapper 应校验 Rust 返回结构。"""
    responses: dict[str, Any] = {
        "list-accounts": [{"id": 1, "name": "账户"}],
        "get-account": {"id": 1, "name": "账户"},
        "get-sub-accounts": [{"id": 2, "parent_id": 1}],
        "create-account": 3,
        "update-account": True,
        "delete-account": True,
        "update-account-display-orders": True,
    }

    monkeypatch.setattr(
        account_rust_bridge,
        "_invoke_account_bridge",
        lambda command, payload: responses[command],
    )

    assert account_rust_bridge.list_accounts("test.db") == [{"id": 1, "name": "账户"}]
    assert account_rust_bridge.get_account("test.db", 1) == {"id": 1, "name": "账户"}
    assert account_rust_bridge.get_sub_accounts("test.db", 1) == [{"id": 2, "parent_id": 1}]
    assert account_rust_bridge.create_account("test.db", {"name": "账户"}) == 3
    assert account_rust_bridge.update_account("test.db", 1, {"name": "账户"}) is True
    assert account_rust_bridge.delete_account("test.db", 1) is True
    assert account_rust_bridge.update_display_orders("test.db", [(1, 2)]) is True

    responses["list-accounts"] = {"bad": True}
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge.list_accounts("test.db")

    responses["list-accounts"] = [{"id": 1}]
    responses["get-account"] = ["bad"]
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge.get_account("test.db", 1)

    responses["get-account"] = None
    assert account_rust_bridge.get_account("test.db", 1) is None

    responses["get-sub-accounts"] = {"bad": True}
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge.get_sub_accounts("test.db", 1)

    responses["create-account"] = True
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge.create_account("test.db", {"name": "账户"})

    responses["update-account"] = "bad"
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge.update_account("test.db", 1, {"name": "账户"})


def test_account_rust_bridge_invocation_error_paths(monkeypatch: pytest.MonkeyPatch) -> None:
    """Rust bridge subprocess 调用失败时应转为明确异常。"""
    monkeypatch.setattr(
        account_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def _raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("missing")

    monkeypatch.setattr(account_rust_bridge.subprocess, "run", _raise_os_error)
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge._invoke_account_bridge("list-accounts", {})

    def _raise_timeout(*_args: Any, **_kwargs: Any) -> None:
        raise subprocess.TimeoutExpired(cmd="bridge", timeout=1)

    monkeypatch.setattr(account_rust_bridge.subprocess, "run", _raise_timeout)
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge._invoke_account_bridge("list-accounts", {})

    for completed in [
        _Completed(1, ""),
        _Completed(0, "not-json"),
        _Completed(0, "[]"),
        _Completed(0, '{"success":false,"error":{"bad":true}}'),
        _Completed(0, '{"success":null}'),
    ]:
        monkeypatch.setattr(
            account_rust_bridge.subprocess,
            "run",
            lambda *_args, completed=completed, **_kwargs: completed,
        )
        with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
            account_rust_bridge._invoke_account_bridge("list-accounts", {})

    monkeypatch.setattr(
        account_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: _Completed(0, '{"success":false,"error":"domain error"}'),
    )
    with pytest.raises(account_rust_bridge.AccountRustBridgeOperationError):
        account_rust_bridge._invoke_account_bridge("list-accounts", {})


def test_account_rust_bridge_command_resolution(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """bridge 命令解析应优先使用环境变量，其次使用已构建二进制。"""
    monkeypatch.setenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", "custom-bridge")
    assert account_rust_bridge._resolve_bridge_command("list-accounts") == [
        "custom-bridge",
        "list-accounts",
    ]

    monkeypatch.delenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE")
    bridge_path = tmp_path
    monkeypatch.setattr(account_rust_bridge, "_repo_root", lambda: tmp_path)
    monkeypatch.setattr(
        account_rust_bridge,
        "_candidate_bridge_paths",
        lambda repo_root: (bridge_path,),
    )
    assert account_rust_bridge._resolve_bridge_command("get-account") == [
        str(bridge_path),
        "get-account",
    ]

    monkeypatch.setattr(
        account_rust_bridge,
        "_candidate_bridge_paths",
        lambda repo_root: (tmp_path / "missing.exe",),
    )
    with pytest.raises(account_rust_bridge.AccountRustBridgeUnavailable):
        account_rust_bridge._resolve_bridge_command("get-account")
