"""Python bridge for Rust category-rule expression helpers."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any

from bill_analyser.core.category_engine.compiled_rule import CompiledRule
from bill_analyser.core.category_engine.expression import RuleExpressionNode

_BRIDGE_TIMEOUT_SECONDS = 10


class CategoryRuleRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust category-rule bridge cannot be executed safely."""


def compile_rule_expression(expr: str, regex_enabled: bool = False) -> CompiledRule:
    """Compile one category-rule expression through Rust."""
    response = _invoke_category_rule_bridge(
        "compile-rule-expression",
        {"expr": expr, "regex_enabled": bool(regex_enabled)},
    )
    if response.get("success") is not True:
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an unsuccessful response"
        )
    result = response.get("result")
    if not isinstance(result, dict):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an invalid compiled rule"
        )
    return _compiled_rule_from_payload(result)


def _compiled_rule_from_payload(payload: dict[str, Any]) -> CompiledRule:
    or_blocks = _coerce_string_matrix(payload.get("or_blocks"), "or_blocks")
    not_patterns = _coerce_string_list(payload.get("not_patterns"), "not_patterns")
    and_patterns = _coerce_string_list(payload.get("and_patterns"), "and_patterns")
    is_empty = payload.get("is_empty")
    if not isinstance(is_empty, bool):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an invalid empty flag"
        )

    expression_ast_payload = payload.get("expression_ast")
    expression_ast = (
        None
        if expression_ast_payload is None
        else _expression_node_from_payload(expression_ast_payload)
    )
    return CompiledRule(
        or_blocks=or_blocks,
        not_patterns=not_patterns,
        and_patterns=and_patterns,
        is_empty=is_empty,
        expression_ast=expression_ast,
    )


def _expression_node_from_payload(payload: Any) -> RuleExpressionNode:
    if not isinstance(payload, dict):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an invalid expression node"
        )
    kind = payload.get("kind")
    operator = payload.get("operator", "")
    if not isinstance(kind, str) or not isinstance(operator, str):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an invalid expression node"
        )
    patterns = tuple(_coerce_string_list(payload.get("patterns"), "patterns"))
    children_payload = payload.get("children")
    if not isinstance(children_payload, list):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned invalid expression children"
        )
    children = tuple(_expression_node_from_payload(item) for item in children_payload)
    return RuleExpressionNode(
        kind=kind,
        operator=operator,
        patterns=patterns,
        children=children,
    )


def _coerce_string_matrix(raw_value: Any, label: str) -> list[list[str]]:
    if not isinstance(raw_value, list):
        raise CategoryRuleRustBridgeUnavailable(
            f"Rust category-rule bridge returned invalid {label}"
        )
    return [_coerce_string_list(item, label) for item in raw_value]


def _coerce_string_list(raw_value: Any, label: str) -> list[str]:
    if not isinstance(raw_value, list) or not all(
        isinstance(item, str) for item in raw_value
    ):
        raise CategoryRuleRustBridgeUnavailable(
            f"Rust category-rule bridge returned invalid {label}"
        )
    return list(raw_value)


def _invoke_category_rule_bridge(command: str, payload: dict[str, Any]) -> dict[str, Any]:
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
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge is unavailable"
        ) from exc

    if completed.returncode != 0:
        raise CategoryRuleRustBridgeUnavailable("Rust category-rule bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned invalid JSON"
        ) from exc

    if not isinstance(response, dict):
        raise CategoryRuleRustBridgeUnavailable(
            "Rust category-rule bridge returned an invalid response"
        )
    return response


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get("BILL_ANALYSER_RUST_CATEGORY_RULE_BRIDGE")
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise CategoryRuleRustBridgeUnavailable(
        "Rust category-rule bridge executable is not available"
    )


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = (
        "bill_category_rule_bridge.exe"
        if os.name == "nt"
        else "bill_category_rule_bridge"
    )
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
