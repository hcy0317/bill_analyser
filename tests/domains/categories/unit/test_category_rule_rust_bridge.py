from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import category_rule_rust_bridge


def test_category_rule_rust_bridge_invokes_real_rust_compiler() -> None:
    """The Python wrapper should map the Rust compiled-rule DTO into Python nodes."""
    compiled = category_rule_rust_bridge.compile_rule_expression(
        "(OR={早餐}/OR={早饭})+AND={咖啡}×NOT={退款}|OR={午餐}",
        regex_enabled=False,
    )

    assert compiled.is_empty is False
    assert compiled.or_blocks == [["早餐"], ["早饭"], ["午餐"]]
    assert compiled.and_patterns == ["咖啡"]
    assert compiled.not_patterns == ["退款"]
    assert compiled.expression_ast is not None
    assert compiled.expression_ast.kind == "any"


def test_category_rule_rust_bridge_validates_result_shapes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Malformed Rust payloads should fail closed instead of entering matching."""
    monkeypatch.setattr(
        category_rule_rust_bridge,
        "_invoke_category_rule_bridge",
        lambda _command, _payload: {
            "success": True,
            "result": {
                "or_blocks": [["早餐"]],
                "not_patterns": ["退款"],
                "and_patterns": ["咖啡"],
                "is_empty": False,
                "expression_ast": {
                    "kind": "clause",
                    "operator": "OR",
                    "patterns": ["早餐"],
                    "children": [],
                },
            },
        },
    )
    compiled = category_rule_rust_bridge.compile_rule_expression("OR={早餐}")
    assert compiled.or_blocks == [["早餐"]]
    assert compiled.expression_ast is not None
    assert compiled.expression_ast.patterns == ("早餐",)

    malformed_payloads = [
        {"success": False, "error": "bad"},
        {"success": True, "result": []},
        {
            "success": True,
            "result": {
                "or_blocks": ["早餐"],
                "not_patterns": [],
                "and_patterns": [],
                "is_empty": False,
                "expression_ast": None,
            },
        },
        {
            "success": True,
            "result": {
                "or_blocks": [],
                "not_patterns": [],
                "and_patterns": [],
                "is_empty": "no",
                "expression_ast": None,
            },
        },
        {
            "success": True,
            "result": {
                "or_blocks": [],
                "not_patterns": [],
                "and_patterns": [],
                "is_empty": False,
                "expression_ast": {
                    "kind": "clause",
                    "operator": "OR",
                    "patterns": [],
                    "children": "bad",
                },
            },
        },
    ]
    for payload in malformed_payloads:
        monkeypatch.setattr(
            category_rule_rust_bridge,
            "_invoke_category_rule_bridge",
            lambda _command, _payload, payload=payload: payload,
        )
        with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
            category_rule_rust_bridge.compile_rule_expression("OR={早餐}")


def test_category_rule_rust_bridge_process_error_paths(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Subprocess failures should map to the category-rule bridge unavailable error."""
    monkeypatch.setattr(
        category_rule_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def _raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("missing")

    monkeypatch.setattr(category_rule_rust_bridge.subprocess, "run", _raise_os_error)
    with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
        category_rule_rust_bridge._invoke_category_rule_bridge(
            "compile-rule-expression",
            {"expr": "OR={早餐}"},
        )

    monkeypatch.setattr(
        category_rule_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 1, "", ""),
    )
    with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
        category_rule_rust_bridge._invoke_category_rule_bridge(
            "compile-rule-expression",
            {"expr": "OR={早餐}"},
        )

    monkeypatch.setattr(
        category_rule_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            ["bridge"],
            0,
            "not-json",
            "",
        ),
    )
    with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
        category_rule_rust_bridge._invoke_category_rule_bridge(
            "compile-rule-expression",
            {"expr": "OR={早餐}"},
        )

    monkeypatch.setattr(
        category_rule_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 0, "[]", ""),
    )
    with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
        category_rule_rust_bridge._invoke_category_rule_bridge(
            "compile-rule-expression",
            {"expr": "OR={早餐}"},
        )


def test_category_rule_rust_bridge_command_resolution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Command resolution should prefer env override then a built bridge binary."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_CATEGORY_RULE_BRIDGE", "custom-rule-bridge")
    assert category_rule_rust_bridge._resolve_bridge_command("compile-rule-expression") == [
        "custom-rule-bridge",
        "compile-rule-expression",
    ]

    monkeypatch.delenv("BILL_ANALYSER_RUST_CATEGORY_RULE_BRIDGE")
    bridge_path = tmp_path
    monkeypatch.setattr(category_rule_rust_bridge, "_repo_root", lambda: tmp_path)
    monkeypatch.setattr(
        category_rule_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (bridge_path,),
    )
    assert category_rule_rust_bridge._resolve_bridge_command(
        "compile-rule-expression"
    ) == [str(bridge_path), "compile-rule-expression"]

    monkeypatch.setattr(
        category_rule_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (tmp_path / "missing.exe",),
    )
    with pytest.raises(category_rule_rust_bridge.CategoryRuleRustBridgeUnavailable):
        category_rule_rust_bridge._resolve_bridge_command("compile-rule-expression")
