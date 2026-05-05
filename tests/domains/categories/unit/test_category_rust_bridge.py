"""Category Rust bridge unit coverage."""

from __future__ import annotations

# pylint: disable=protected-access,too-few-public-methods

import sqlite3
import subprocess
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import category_rust_bridge


class _Completed:
    """Small subprocess.CompletedProcess stand-in."""

    def __init__(self, returncode: int, stdout: str) -> None:
        self.returncode = returncode
        self.stdout = stdout


def test_category_rust_bridge_wrappers_validate_result_shapes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Wrapper functions should reject malformed Rust response payloads."""
    responses: dict[str, Any] = {
        "list-categories": [{"id": 1, "main_category": "Food", "sub_category": ""}],
        "get-category": {"id": 1, "main_category": "Food", "sub_category": ""},
        "get-category-by-name": {"id": 1, "main_category": "Food", "sub_category": ""},
        "create-category": 3,
        "ensure-categories": {"created": 2, "skipped": 1},
        "update-category": True,
        "delete-category": True,
        "delete-categories-by-main-category": True,
        "update-main-category-name": True,
    }

    monkeypatch.setattr(
        category_rust_bridge,
        "_invoke_category_bridge",
        lambda command, _payload: responses[command],
    )

    assert category_rust_bridge.list_categories("test.db") == [
        {"id": 1, "main_category": "Food", "sub_category": ""}
    ]
    assert category_rust_bridge.get_category("test.db", 1) == {
        "id": 1,
        "main_category": "Food",
        "sub_category": "",
    }
    assert category_rust_bridge.get_category_by_name("test.db", "Food", "") == {
        "id": 1,
        "main_category": "Food",
        "sub_category": "",
    }
    assert category_rust_bridge.create_category("test.db", {"main_category": "Food"}) == 3
    assert category_rust_bridge.ensure_categories(
        "test.db",
        [{"main_category": "Food"}],
    ) == {"created": 2, "skipped": 1}
    assert category_rust_bridge.update_category("test.db", 1, {"priority": 2}) is True
    assert category_rust_bridge.delete_category("test.db", 1) is True
    assert category_rust_bridge.delete_categories_by_main_category("test.db", "Food") is True
    assert category_rust_bridge.update_main_category_name("test.db", "Food", "Dining") is True

    responses["list-categories"] = {"bad": True}
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.list_categories("test.db")

    responses["list-categories"] = [{"id": 1}]
    responses["get-category"] = ["bad"]
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.get_category("test.db", 1)

    responses["get-category"] = None
    assert category_rust_bridge.get_category("test.db", 1) is None

    responses["get-category-by-name"] = ["bad"]
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.get_category_by_name("test.db", "Food", "")

    responses["get-category-by-name"] = None
    assert category_rust_bridge.get_category_by_name("test.db", "Food", "") is None

    responses["create-category"] = True
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.create_category("test.db", {"main_category": "Food"})

    responses["create-category"] = None
    assert category_rust_bridge.create_category("test.db", {"main_category": "Food"}) is None

    responses["ensure-categories"] = ["bad"]
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.ensure_categories("test.db", [{"main_category": "Food"}])

    responses["ensure-categories"] = {"created": "bad", "skipped": 1}
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.ensure_categories("test.db", [{"main_category": "Food"}])

    responses["update-category"] = "bad"
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge.update_category("test.db", 1, {"priority": 2})


def test_category_rust_bridge_invocation_error_paths(monkeypatch: pytest.MonkeyPatch) -> None:
    """Subprocess and response failures should map to clear bridge exceptions."""
    monkeypatch.setattr(
        category_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def _raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("missing")

    monkeypatch.setattr(category_rust_bridge.subprocess, "run", _raise_os_error)
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge._invoke_category_bridge("list-categories", {})

    def _raise_timeout(*_args: Any, **_kwargs: Any) -> None:
        raise subprocess.TimeoutExpired(cmd="bridge", timeout=1)

    monkeypatch.setattr(category_rust_bridge.subprocess, "run", _raise_timeout)
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge._invoke_category_bridge("list-categories", {})

    for completed in [
        _Completed(1, ""),
        _Completed(0, "not-json"),
        _Completed(0, "[]"),
        _Completed(0, '{"success":false,"error":{"bad":true}}'),
        _Completed(0, '{"success":null}'),
    ]:
        monkeypatch.setattr(
            category_rust_bridge.subprocess,
            "run",
            lambda *_args, completed=completed, **_kwargs: completed,
        )
        with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
            category_rust_bridge._invoke_category_bridge("list-categories", {})

    monkeypatch.setattr(
        category_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: _Completed(0, '{"success":false,"error":"domain error"}'),
    )
    with pytest.raises(category_rust_bridge.CategoryRustBridgeOperationError):
        category_rust_bridge._invoke_category_bridge("list-categories", {})

    monkeypatch.setattr(
        category_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: _Completed(
            0,
            '{"success":false,"error":"UNIQUE constraint failed: categories.user_id"}',
        ),
    )
    with pytest.raises(sqlite3.IntegrityError):
        category_rust_bridge._invoke_category_bridge("update-category", {})


def test_category_rust_bridge_command_resolution(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Command resolution should prefer env override then a built bridge binary."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", "custom-bridge")
    assert category_rust_bridge._resolve_bridge_command("list-categories") == [
        "custom-bridge",
        "list-categories",
    ]

    monkeypatch.delenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE")
    bridge_path = tmp_path
    monkeypatch.setattr(category_rust_bridge, "_repo_root", lambda: tmp_path)
    monkeypatch.setattr(
        category_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (bridge_path,),
    )
    assert category_rust_bridge._resolve_bridge_command("get-category") == [
        str(bridge_path),
        "get-category",
    ]

    monkeypatch.setattr(
        category_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (tmp_path / "missing.exe",),
    )
    with pytest.raises(category_rust_bridge.CategoryRustBridgeUnavailable):
        category_rust_bridge._resolve_bridge_command("get-category")
