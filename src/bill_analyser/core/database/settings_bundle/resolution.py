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

from .shared import (
    LOCAL_REF_NAMESPACE,
    OCR_CONFIG_SETTING_KEY,
    SETTINGS_BUNDLE_SCHEMA_VERSION,
    SETTINGS_BUNDLE_SECTION_KEYS,
    _dump_json_list,
    _dump_json_object,
    _external_ref,
    _get_any,
    _is_masked_secret,
    _load_json_list,
    _local_id_ref,
    _safe_bool,
    _safe_float,
    _safe_int,
    _safe_text,
    _section_result_template,
    _split_category_name,
)


class SettingsBundleResolutionMixin(object):
        def _resolve_settings_category_id(
            self,
            item: dict[str, Any],
            category_ref_map: dict[str, int],
        ) -> int | None:
            category_ref = _safe_text(_get_any(item, "categoryRef", "category_ref"))
            if category_ref and category_ref in category_ref_map:
                return category_ref_map[category_ref]
            category_id = _safe_int(_get_any(item, "categoryId", "category_id"))
            local_category_ref = _local_id_ref("category", category_id)
            if local_category_ref in category_ref_map:
                return category_ref_map[local_category_ref]
            category_name = _get_any(item, "categoryName", "category_name")
            main = _safe_text(_get_any(item, "mainCategory", "main_category"))
            sub = _safe_text(_get_any(item, "subCategory", "sub_category"))
            if not main and not sub:
                main, sub = _split_category_name(category_name)
            return self._resolve_settings_category_by_name(category_ref_map, main, sub)

        @staticmethod
        def _resolve_settings_category_by_name(
            category_ref_map: dict[str, int],
            main: str,
            sub: str,
        ) -> int | None:
            category_name = "/".join(part for part in (main, sub) if part)
            needle = f"categoryName:{category_name}"
            return category_ref_map.get(needle)

        def _resolve_settings_account_id(
            self,
            item: dict[str, Any],
            account_ref_map: dict[str, int],
            ref_key: str,
            name_key: str,
        ) -> int:
            account_ref = _safe_text(_get_any(item, ref_key, ref_key.replace("Ref", "_ref")))
            if account_ref and account_ref in account_ref_map:
                return account_ref_map[account_ref]
            id_key = "sourceAccountId" if ref_key.startswith("source") else "destinationAccountId"
            raw_id = _safe_int(_get_any(item, id_key, id_key.replace("Id", "_id")))
            local_account_ref = _local_id_ref("account", raw_id)
            if local_account_ref in account_ref_map:
                return account_ref_map[local_account_ref]
            name = _safe_text(_get_any(item, name_key, name_key.replace("Name", "_name")))
            return account_ref_map.get(f"accountName:{name}", 0)

        @staticmethod
        def _resolve_settings_tag_ids(
            item: dict[str, Any],
            tag_ref_map: dict[str, int],
            warnings: list[str],
        ) -> list[int]:
            tag_ids: list[int] = []
            for tag_ref in item.get("tagRefs") or []:
                normalized_ref = _safe_text(tag_ref)
                if normalized_ref in tag_ref_map:
                    tag_ids.append(tag_ref_map[normalized_ref])
                elif normalized_ref:
                    warnings.append(f"Template tag ref not found: {normalized_ref}")
            for tag_name in item.get("tagNames") or []:
                name_key = f"tagName:{_safe_text(tag_name)}"
                if name_key in tag_ref_map:
                    tag_ids.append(tag_ref_map[name_key])
            for raw_id in item.get("tagIds") or []:
                ref = _local_id_ref("tag", _safe_int(raw_id))
                if ref in tag_ref_map:
                    tag_ids.append(tag_ref_map[ref])
            return sorted(set(tag_ids))
