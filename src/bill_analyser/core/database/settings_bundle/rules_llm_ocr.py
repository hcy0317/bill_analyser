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


class SettingsBundleRulesLlmOcrMixin(object):
        async def _import_settings_category_rules(
            self,
            conn: Any,
            rules: list[dict[str, Any]],
            user_id: int,
            result: dict[str, Any],
            warnings: list[str],
            category_ref_map: dict[str, int],
        ) -> None:
            section = result["sections"]["categoryRecognitionRules"]
            existing = await self._load_existing_category_rules(conn, user_id)
            for item in rules:
                category_id = self._resolve_settings_category_id(item, category_ref_map)
                rule_expression = _safe_text(_get_any(item, "ruleExpression", "rule_expression"))
                if not category_id or not rule_expression:
                    section["skipped"] += 1
                    warnings.append("Skipped category rule with missing category or ruleExpression")
                    continue
                name = _safe_text(item.get("name"))
                row = existing.get((category_id, rule_expression, name))
                now = utc_now_iso()
                values = {
                    "category_id": category_id,
                    "name": name,
                    "priority": _safe_int(item.get("priority"), 100),
                    "rule_expression": rule_expression,
                    "regex_enabled": 1 if _safe_bool(_get_any(item, "regexEnabled", "regex_enabled")) else 0,
                    "enabled": 1 if _safe_bool(item.get("enabled", True)) else 0,
                }
                if row:
                    await conn.execute(
                        """
                        UPDATE category_rules
                        SET category_id = ?, name = ?, priority = ?, rule_expression = ?,
                            regex_enabled = ?, enabled = ?, updated_at = ?
                        WHERE id = ? AND user_id = ?
                        """,
                        (
                            values["category_id"],
                            values["name"],
                            values["priority"],
                            values["rule_expression"],
                            values["regex_enabled"],
                            values["enabled"],
                            now,
                            row["id"],
                            user_id,
                        ),
                    )
                    section["updated"] += 1
                else:
                    cursor = await conn.execute(
                        """
                        INSERT INTO category_rules (
                            user_id, category_id, name, priority, rule_expression,
                            regex_enabled, enabled, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            user_id,
                            values["category_id"],
                            values["name"],
                            values["priority"],
                            values["rule_expression"],
                            values["regex_enabled"],
                            values["enabled"],
                            now,
                            now,
                        ),
                    )
                    existing[(category_id, rule_expression, name)] = {
                        **values,
                        "id": int(cursor.lastrowid or 0),
                    }
                    section["created"] += 1

        async def _load_existing_category_rules(self, conn: Any, user_id: int) -> dict[tuple[int, str, str], dict[str, Any]]:
            async with conn.execute(
                "SELECT * FROM category_rules WHERE user_id = ?",
                (user_id,),
            ) as cursor:
                rows = [dict(row) for row in await cursor.fetchall()]
            return {
                (
                    int(row.get("category_id") or 0),
                    _safe_text(row.get("rule_expression")),
                    _safe_text(row.get("name")),
                ): row
                for row in rows
            }

        async def _import_settings_llm_configs(
            self,
            conn: Any,
            configs: list[dict[str, Any]],
            user_id: int,
            result: dict[str, Any],
        ) -> None:
            section = result["sections"]["llmConfigs"]
            async with conn.execute("SELECT * FROM llm_configs WHERE user_id = ?", (user_id,)) as cursor:
                existing = {_safe_text(row["name"]): dict(row) for row in await cursor.fetchall()}
            for item in configs:
                name = _safe_text(item.get("name"))
                if not name:
                    section["skipped"] += 1
                    continue
                now = utc_now_iso()
                incoming_secret = _safe_text(_get_any(item, "apiKey", "api_key"))
                advanced_settings = _dump_json_object(
                    _get_any(item, "advancedSettings", "advanced_settings")
                )
                row = existing.get(name)
                if row:
                    api_key = row.get("api_key") or ""
                    if not _is_masked_secret(incoming_secret):
                        api_key = incoming_secret
                    await conn.execute(
                        """
                        UPDATE llm_configs
                        SET provider = ?, model = ?, api_key = ?, base_url = ?,
                            advanced_settings = ?, updated_at = ?
                        WHERE id = ? AND user_id = ?
                        """,
                        (
                            _safe_text(item.get("provider"), "openai"),
                            _safe_text(item.get("model")),
                            api_key,
                            _safe_text(_get_any(item, "baseUrl", "base_url")),
                            advanced_settings,
                            now,
                            row["id"],
                            user_id,
                        ),
                    )
                    section["updated"] += 1
                    existing[name] = {
                        **row,
                        "provider": _safe_text(item.get("provider"), "openai"),
                        "model": _safe_text(item.get("model")),
                        "api_key": api_key,
                        "base_url": _safe_text(_get_any(item, "baseUrl", "base_url")),
                        "advanced_settings": advanced_settings,
                    }
                else:
                    cursor = await conn.execute(
                        """
                        INSERT INTO llm_configs (
                            user_id, name, provider, model, api_key, base_url,
                            advanced_settings, is_active, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)
                        """,
                        (
                            user_id,
                            name,
                            _safe_text(item.get("provider"), "openai"),
                            _safe_text(item.get("model")),
                            "" if _is_masked_secret(incoming_secret) else incoming_secret,
                            _safe_text(_get_any(item, "baseUrl", "base_url")),
                            advanced_settings,
                            now,
                            now,
                        ),
                    )
                    existing[name] = {
                        "id": int(cursor.lastrowid or 0),
                        "user_id": user_id,
                        "name": name,
                        "provider": _safe_text(item.get("provider"), "openai"),
                        "model": _safe_text(item.get("model")),
                        "api_key": "" if _is_masked_secret(incoming_secret) else incoming_secret,
                        "base_url": _safe_text(_get_any(item, "baseUrl", "base_url")),
                        "advanced_settings": advanced_settings,
                        "is_active": 0,
                    }
                    section["created"] += 1

        async def _import_settings_ocr_config(
            self,
            conn: Any,
            configs: list[dict[str, Any]],
            result: dict[str, Any],
        ) -> None:
            section = result["sections"]["ocrConfig"]
            if not configs:
                return
            item = configs[0]
            normalized = normalize_ocr_config({
                "provider": _safe_text(item.get("provider"), "disabled"),
                "lang": _safe_text(item.get("lang"), "chi_sim+eng"),
            })
            now = utc_now_iso()
            async with conn.execute(
                "SELECT value FROM app_settings WHERE key = ?",
                (OCR_CONFIG_SETTING_KEY,),
            ) as cursor:
                existing = await cursor.fetchone()
            await conn.execute(
                """
                INSERT INTO app_settings (
                    key, value, value_type, description, is_encrypted, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    value_type = excluded.value_type,
                    description = excluded.description,
                    is_encrypted = excluded.is_encrypted,
                    updated_at = excluded.updated_at
                """,
                (
                    OCR_CONFIG_SETTING_KEY,
                    json.dumps(normalized, ensure_ascii=False),
                    "json",
                    "Receipt OCR runtime configuration",
                    0,
                    now,
                    now,
                ),
            )
            if existing:
                section["updated"] += 1
            else:
                section["created"] += 1
