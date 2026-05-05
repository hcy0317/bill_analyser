"""Unified JSON settings bundle import/export helpers."""

# pylint: disable=line-too-long,too-many-lines,too-many-locals,too-many-branches,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments

from __future__ import annotations

import json
from typing import Any

from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.database.llm.config import normalize_llm_advanced_settings
from bill_analyser.core.ai.ocr.service import normalize_ocr_config

SETTINGS_BUNDLE_SCHEMA_VERSION = 1

OCR_CONFIG_SETTING_KEY = "receipt_ocr_config"

SETTINGS_BUNDLE_SECTION_KEYS = (
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
    "llmConfigs",
    "ocrConfig",
)

LOCAL_REF_NAMESPACE = "__local_settings_bundle_id__"

def _empty_section_summary() -> dict[str, int]:
    return {"created": 0, "updated": 0, "skipped": 0}

def _section_result_template() -> dict[str, dict[str, int]]:
    return {section: _empty_section_summary() for section in SETTINGS_BUNDLE_SECTION_KEYS}

def _safe_text(value: Any, default: str = "") -> str:
    if value is None:
        return default
    return str(value).strip()

def _safe_int(value: Any, default: int = 0) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return default

def _safe_float(value: Any, default: float = 0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default

def _safe_bool(value: Any) -> bool:
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return bool(value)

def _get_any(data: dict[str, Any], *keys: str, default: Any = None) -> Any:
    for key in keys:
        if key in data:
            return data[key]
    return default

def _load_json_list(value: Any) -> list[str]:
    if isinstance(value, list):
        return [_safe_text(item) for item in value if _safe_text(item)]
    if not value:
        return []
    try:
        loaded = json.loads(str(value))
    except json.JSONDecodeError:
        return []
    if not isinstance(loaded, list):
        return []
    return [_safe_text(item) for item in loaded if _safe_text(item)]

def _dump_json_list(value: Any) -> str:
    return json.dumps(_load_json_list(value), ensure_ascii=False)

def _dump_json_object(value: Any) -> str:
    if isinstance(value, str):
        try:
            loaded = json.loads(value) if value.strip() else {}
        except json.JSONDecodeError:
            loaded = {}
    elif isinstance(value, dict):
        loaded = value
    else:
        loaded = {}
    if not isinstance(loaded, dict):
        loaded = {}
    return json.dumps(loaded, ensure_ascii=False)

def _external_ref(item: dict[str, Any], prefix: str) -> str:
    explicit = _safe_text(_get_any(item, "externalRef", "external_ref"))
    if explicit:
        return "" if explicit.startswith(f"{LOCAL_REF_NAMESPACE}:") else explicit
    item_id = _safe_text(_get_any(item, "id", "sourceId", "source_id"))
    return f"{prefix}:{item_id}" if item_id else ""

def _local_id_ref(prefix: str, item_id: int) -> str:
    return f"{LOCAL_REF_NAMESPACE}:{prefix}:{item_id}"

def _is_masked_secret(value: Any) -> bool:
    text = _safe_text(value)
    return text in {"", "********", "redacted", "<redacted>"}

def _split_category_name(value: Any) -> tuple[str, str]:
    text = _safe_text(value)
    if not text:
        return "", ""
    if "/" not in text:
        return text, ""
    main, sub = text.split("/", 1)
    return main.strip(), sub.strip()
