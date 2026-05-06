"""Python subprocess bridge for Rust-backed settings bundle taxonomy helpers."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


_BRIDGE_TIMEOUT_SECONDS = 10
_BRIDGE_ENV = "BILL_ANALYSER_RUST_TAXONOMY_BRIDGE"


class SettingsBundleRustBridgeUnavailable(RuntimeError):
    """Raised when the Rust taxonomy bridge cannot be executed safely."""


class SettingsBundleRustBridgeOperationError(RuntimeError):
    """Raised when Rust returns a settings bundle domain failure."""


def normalize_sections(bundle: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
    """Normalize and validate settings bundle sections via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-normalize-sections",
        {"bundle": bundle},
    )
    if not isinstance(result, dict):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid settings sections"
        )
    return {str(key): _coerce_dict_list(value) for key, value in result.items()}


def export_taxonomy_sections(
    *,
    accounts: list[dict[str, Any]],
    categories: list[dict[str, Any]],
    tags: list[dict[str, Any]],
    templates: list[dict[str, Any]],
    scheduled: list[dict[str, Any]],
) -> dict[str, list[dict[str, Any]]]:
    """Build settings bundle taxonomy export sections via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-export-taxonomy-sections",
        {
            "payload": {
                "accounts": accounts,
                "categories": categories,
                "tags": tags,
                "templates": templates,
                "scheduled": scheduled,
            }
        },
    )
    if not isinstance(result, dict):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid taxonomy export sections"
        )
    return {str(key): _coerce_dict_list(value) for key, value in result.items()}


def normalize_account_import(
    item: dict[str, Any],
    ref_map: dict[str, int],
) -> dict[str, Any]:
    """Normalize one account import row and resolve its parent ref via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-normalize-account-import",
        {"payload": {"item": item, "ref_map": ref_map}},
    )
    return _coerce_dict(result, "account import payload")


def normalize_category_import(item: dict[str, Any]) -> dict[str, Any]:
    """Normalize one category import row via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-normalize-category-import",
        {"payload": {"item": item}},
    )
    return _coerce_dict(result, "category import payload")


def normalize_tag_import(item: dict[str, Any]) -> dict[str, Any]:
    """Normalize one tag import row via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-normalize-tag-import",
        {"payload": {"item": item}},
    )
    return _coerce_dict(result, "tag import payload")


def resolve_template_payload(
    item: dict[str, Any],
    *,
    account_ref_map: dict[str, int],
    category_ref_map: dict[str, int],
    tag_ref_map: dict[str, int],
) -> dict[str, Any]:
    """Resolve one settings-bundle template import payload via Rust."""
    result = _invoke_settings_bundle_bridge(
        "settings-resolve-template-payload",
        {
            "payload": {
                "item": item,
                "account_ref_map": account_ref_map,
                "category_ref_map": category_ref_map,
                "tag_ref_map": tag_ref_map,
            }
        },
    )
    resolved = _coerce_dict(result, "template import resolution")
    if not isinstance(resolved.get("payload"), dict):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid template payload"
        )
    if not isinstance(resolved.get("warnings"), list):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid template warnings"
        )
    if not isinstance(resolved.get("unresolved"), bool):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid template unresolved flag"
        )
    return {
        "payload": dict(resolved["payload"]),
        "warnings": [str(item) for item in resolved["warnings"]],
        "unresolved": bool(resolved["unresolved"]),
    }


def _coerce_dict(raw_value: Any, label: str) -> dict[str, Any]:
    if not isinstance(raw_value, dict):
        raise SettingsBundleRustBridgeUnavailable(
            f"Rust taxonomy bridge returned invalid {label}"
        )
    return dict(raw_value)


def _coerce_dict_list(raw_value: Any) -> list[dict[str, Any]]:
    if not isinstance(raw_value, list):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid settings bundle section list"
        )
    if not all(isinstance(item, dict) for item in raw_value):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid settings bundle section item"
        )
    return [dict(item) for item in raw_value]


def _invoke_settings_bundle_bridge(command: str, payload: dict[str, Any]) -> Any:
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
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge is unavailable"
        ) from exc

    if completed.returncode != 0:
        raise SettingsBundleRustBridgeUnavailable("Rust taxonomy bridge failed") from None

    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned invalid JSON"
        ) from exc

    if not isinstance(response, dict):
        raise SettingsBundleRustBridgeUnavailable(
            "Rust taxonomy bridge returned an invalid response"
        ) from None

    if response.get("success") is True:
        return response.get("result")

    if response.get("success") is False and isinstance(response.get("error"), str):
        raise SettingsBundleRustBridgeOperationError(response["error"])

    raise SettingsBundleRustBridgeUnavailable(
        "Rust taxonomy bridge returned an invalid response"
    )


def _resolve_bridge_command(command: str) -> list[str]:
    env_bridge = os.environ.get(_BRIDGE_ENV)
    if env_bridge:
        return [env_bridge, command]

    for candidate in _candidate_bridge_paths(_repo_root()):
        if candidate.exists():
            return [str(candidate), command]

    raise SettingsBundleRustBridgeUnavailable(
        "Rust taxonomy bridge executable is not available"
    )


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _candidate_bridge_paths(repo_root: Path) -> tuple[Path, ...]:
    executable_name = "bill_taxonomy_bridge.exe" if os.name == "nt" else "bill_taxonomy_bridge"
    return (
        repo_root / "target" / "debug" / executable_name,
        repo_root / "target" / "release" / executable_name,
    )
