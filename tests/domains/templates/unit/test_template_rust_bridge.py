"""Template Rust bridge unit coverage."""

from __future__ import annotations

# pylint: disable=protected-access,too-few-public-methods

import subprocess
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import template_rust_bridge


class _Completed:
    """Small subprocess.CompletedProcess stand-in."""

    def __init__(self, returncode: int, stdout: str) -> None:
        self.returncode = returncode
        self.stdout = stdout


def test_template_rust_bridge_wrappers_validate_result_shapes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Wrapper functions should validate Rust response payload shapes."""
    responses: dict[str, Any] = {
        "list-templates": [{"id": "1", "templateType": 1}],
        "get-template": {"id": "1", "templateType": 1},
        "list-enabled-recurring-templates": [{"id": 2, "enabled": 1}],
        "create-template": 3,
        "update-template": True,
        "delete-template": True,
        "update-template-display-orders": True,
    }
    monkeypatch.setattr(
        template_rust_bridge,
        "_invoke_template_bridge",
        lambda command, _payload: responses[command],
    )

    assert template_rust_bridge.list_templates("test.db") == [{"id": "1", "templateType": 1}]
    assert template_rust_bridge.get_template("test.db", 1) == {"id": "1", "templateType": 1}
    assert template_rust_bridge.list_enabled_recurring_templates("test.db") == [
        {"id": 2, "enabled": 1}
    ]
    assert template_rust_bridge.create_template("test.db", {"name": "T"}) == 3
    assert template_rust_bridge.update_template("test.db", 1, {"name": "T2"}) is True
    assert template_rust_bridge.delete_template("test.db", 1) is True
    assert template_rust_bridge.update_display_orders("test.db", [(1, 2)], 1) is True

    responses["list-templates"] = {"bad": True}
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge.list_templates("test.db")

    responses["list-templates"] = [{"id": "1"}]
    responses["get-template"] = ["bad"]
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge.get_template("test.db", 1)

    responses["get-template"] = None
    assert template_rust_bridge.get_template("test.db", 1) is None

    responses["list-enabled-recurring-templates"] = {"bad": True}
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge.list_enabled_recurring_templates("test.db")

    responses["create-template"] = True
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge.create_template("test.db", {"name": "T"})

    responses["update-template"] = "bad"
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge.update_template("test.db", 1, {"name": "T2"})


def test_template_rust_bridge_invocation_error_paths(monkeypatch: pytest.MonkeyPatch) -> None:
    """Subprocess and response failures should map to clear bridge exceptions."""
    monkeypatch.setattr(
        template_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def _raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("missing")

    monkeypatch.setattr(template_rust_bridge.subprocess, "run", _raise_os_error)
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge._invoke_template_bridge("list-templates", {})

    def _raise_timeout(*_args: Any, **_kwargs: Any) -> None:
        raise subprocess.TimeoutExpired(cmd="bridge", timeout=1)

    monkeypatch.setattr(template_rust_bridge.subprocess, "run", _raise_timeout)
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge._invoke_template_bridge("list-templates", {})

    for completed in [
        _Completed(1, ""),
        _Completed(0, "not-json"),
        _Completed(0, "[]"),
        _Completed(0, '{"success":false,"error":{"bad":true}}'),
        _Completed(0, '{"success":null}'),
    ]:
        monkeypatch.setattr(
            template_rust_bridge.subprocess,
            "run",
            lambda *_args, completed=completed, **_kwargs: completed,
        )
        with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
            template_rust_bridge._invoke_template_bridge("list-templates", {})

    monkeypatch.setattr(
        template_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: _Completed(0, '{"success":false,"error":"domain error"}'),
    )
    with pytest.raises(template_rust_bridge.TemplateRustBridgeOperationError):
        template_rust_bridge._invoke_template_bridge("list-templates", {})


def test_template_rust_bridge_command_resolution(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Command resolution should prefer env override then a built bridge binary."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", "custom-bridge")
    assert template_rust_bridge._resolve_bridge_command("list-templates") == [
        "custom-bridge",
        "list-templates",
    ]

    monkeypatch.delenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE")
    bridge_path = tmp_path
    monkeypatch.setattr(template_rust_bridge, "_repo_root", lambda: tmp_path)
    monkeypatch.setattr(
        template_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (bridge_path,),
    )
    assert template_rust_bridge._resolve_bridge_command("get-template") == [
        str(bridge_path),
        "get-template",
    ]

    monkeypatch.setattr(
        template_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (tmp_path / "missing.exe",),
    )
    with pytest.raises(template_rust_bridge.TemplateRustBridgeUnavailable):
        template_rust_bridge._resolve_bridge_command("get-template")
