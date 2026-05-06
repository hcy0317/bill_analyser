"""Unified JSON settings bundle import/export helpers."""

# pylint: disable=line-too-long,too-many-lines,too-many-locals,too-many-branches,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments
# pylint: disable=bad-indentation,missing-class-docstring,useless-object-inheritance
# pylint: disable=too-few-public-methods,unused-import,duplicate-code

from __future__ import annotations

import json
from typing import Any

from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.database.llm.config import normalize_llm_advanced_settings
from bill_analyser.core.ai.ocr.service import normalize_ocr_config
from bill_analyser.core import settings_bundle_rust_bridge

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


class SettingsBundleExportersMixin(object):
        def _export_settings_taxonomy_sections(
            self,
            *,
            accounts: list[dict[str, Any]],
            categories: list[dict[str, Any]],
            tags: list[dict[str, Any]],
            templates: list[dict[str, Any]],
            scheduled: list[dict[str, Any]],
            account_refs: dict[int, str],
            account_names: dict[int, str],
            category_refs: dict[int, str],
            category_names: dict[int, str],
            tag_refs: dict[int, str],
            tag_names: dict[int, str],
        ) -> dict[str, list[dict[str, Any]]]:
            try:
                return settings_bundle_rust_bridge.export_taxonomy_sections(
                    accounts=accounts,
                    categories=categories,
                    tags=tags,
                    templates=templates,
                    scheduled=scheduled,
                )
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable:
                return {
                    "accounts": [
                        self._export_settings_account(item, account_refs, account_names)
                        for item in accounts
                    ],
                    "transactionCategories": [
                        self._export_settings_category(item, category_refs)
                        for item in categories
                    ],
                    "transactionTags": [
                        self._export_settings_tag(item, tag_refs)
                        for item in tags
                    ],
                    "transactionTemplates": [
                        self._export_settings_template(
                            item,
                            category_refs,
                            category_names,
                            account_refs,
                            account_names,
                            tag_refs,
                            tag_names,
                        )
                        for item in templates
                    ],
                    "scheduledTransactions": [
                        self._export_settings_template(
                            item,
                            category_refs,
                            category_names,
                            account_refs,
                            account_names,
                            tag_refs,
                            tag_names,
                        )
                        for item in scheduled
                    ],
                }

        async def _export_settings_templates(
            self,
            conn: Any,
            *,
            user_id: int,
            template_type: int,
        ) -> list[dict[str, Any]]:
            table_name = "recurring_bills" if template_type == 2 else "bill_templates"
            async with conn.execute(
                f"SELECT * FROM {table_name} WHERE user_id = ? ORDER BY COALESCE(display_order, 0), name",
                (user_id,),
            ) as cursor:
                rows = await cursor.fetchall()
            exported = []
            for row in rows:
                raw = dict(row)
                item = self._serialize_template_row(raw, template_type=template_type)
                item["description"] = raw.get("description") or ""
                if template_type == 2:
                    item["enabled"] = bool(raw.get("enabled"))
                    item["autoCreate"] = bool(raw.get("auto_create"))
                    item["nextDate"] = raw.get("next_date")
                exported.append(item)
            return exported

        @staticmethod
        def _settings_category_name(category: dict[str, Any]) -> str:
            main = _safe_text(category.get("main_category"))
            sub = _safe_text(category.get("sub_category"))
            return "/".join(part for part in (main, sub) if part)

        @staticmethod
        def _export_settings_account(
            account: dict[str, Any],
            account_refs: dict[int, str],
            account_names: dict[int, str],
        ) -> dict[str, Any]:
            account_id = int(account["id"])
            parent_id = int(account.get("parent_id") or 0)
            return {
                "externalRef": account_refs[account_id],
                "name": account.get("name") or "",
                "type": account.get("type", 1),
                "category": account.get("category"),
                "currency": account.get("currency") or "CNY",
                "icon": account.get("icon") or "",
                "color": account.get("color") or "",
                "balance": float(account.get("balance") or 0),
                "initialBalance": float(account.get("initial_balance") or 0),
                "hidden": bool(account.get("hidden")),
                "displayOrder": int(account.get("display_order") or 0),
                "comment": account.get("comment") or "",
                "aliases": _load_json_list(account.get("aliases")),
                "parentRef": account_refs.get(parent_id, ""),
                "parentName": account_names.get(parent_id, ""),
            }

        @staticmethod
        def _export_settings_category(
            category: dict[str, Any],
            category_refs: dict[int, str],
        ) -> dict[str, Any]:
            category_id = int(category["id"])
            return {
                "externalRef": category_refs[category_id],
                "type": category.get("type", 1),
                "mainCategory": category.get("main_category") or "",
                "subCategory": category.get("sub_category") or "",
                "priority": int(category.get("priority") or 0),
                "keywords": category.get("keywords") or "",
                "description": category.get("description") or "",
                "icon": category.get("icon") or "",
                "color": category.get("color") or "",
                "hidden": bool(category.get("hidden")),
            }

        @staticmethod
        def _export_settings_tag(tag: dict[str, Any], tag_refs: dict[int, str]) -> dict[str, Any]:
            tag_id = int(tag["id"])
            return {
                "externalRef": tag_refs[tag_id],
                "name": tag.get("name") or "",
                "color": tag.get("color") or "",
                "icon": tag.get("icon") or "",
                "displayOrder": int(tag.get("display_order") or 0),
                "hidden": bool(tag.get("hidden")),
            }

        def _export_settings_template(
            self,
            template: dict[str, Any],
            category_refs: dict[int, str],
            category_names: dict[int, str],
            account_refs: dict[int, str],
            account_names: dict[int, str],
            tag_refs: dict[int, str],
            tag_names: dict[int, str],
        ) -> dict[str, Any]:
            source_account_id = _safe_int(template.get("sourceAccountId"))
            destination_account_id = _safe_int(template.get("destinationAccountId"))
            category_id = _safe_int(template.get("categoryId"))
            tag_ids = [_safe_int(tag_id) for tag_id in template.get("tagIds", [])]
            exported = {key: value for key, value in template.items() if key not in {"id", "tagIds"}}
            exported.update(
                {
                    "externalRef": f"template:{template.get('templateType')}:{template.get('id')}",
                    "categoryRef": category_refs.get(category_id, ""),
                    "categoryName": category_names.get(category_id, ""),
                    "sourceAccountRef": account_refs.get(source_account_id, ""),
                    "sourceAccountName": account_names.get(source_account_id, ""),
                    "destinationAccountRef": account_refs.get(destination_account_id, ""),
                    "destinationAccountName": account_names.get(destination_account_id, ""),
                    "tagRefs": [tag_refs[tag_id] for tag_id in tag_ids if tag_id in tag_refs],
                    "tagNames": [tag_names[tag_id] for tag_id in tag_ids if tag_id in tag_names],
                }
            )
            return exported

        @staticmethod
        def _export_settings_category_rule(
            rule: dict[str, Any],
            category_refs: dict[int, str],
        ) -> dict[str, Any]:
            category_id = int(rule["category_id"])
            return {
                "externalRef": f"categoryRule:{rule.get('id')}",
                "categoryRef": category_refs.get(category_id, ""),
                "mainCategory": rule.get("main_category") or "",
                "subCategory": rule.get("sub_category") or "",
                "name": rule.get("name") or "",
                "priority": int(rule.get("priority") or 100),
                "ruleExpression": rule.get("rule_expression") or "",
                "regexEnabled": bool(rule.get("regex_enabled")),
                "enabled": bool(rule.get("enabled", True)),
            }

        @staticmethod
        def _export_settings_llm_config(config: dict[str, Any]) -> dict[str, Any]:
            return {
                "externalRef": f"llmConfig:{config.get('id')}",
                "name": config.get("name") or "",
                "provider": config.get("provider") or "openai",
                "model": config.get("model") or "",
                "apiKey": "",
                "hasApiKey": bool(config.get("api_key")),
                "baseUrl": config.get("base_url") or "",
                "advancedSettings": normalize_llm_advanced_settings(config.get("advanced_settings")),
                "activeInSource": bool(config.get("is_active")),
            }

        @staticmethod
        def _export_settings_ocr_config(raw_value: str | None) -> dict[str, Any]:
            try:
                loaded = json.loads(raw_value) if raw_value else {}
            except json.JSONDecodeError:
                loaded = {}
            normalized = normalize_ocr_config(loaded)
            return {
                "externalRef": "ocrConfig:receipt-recognition",
                "provider": normalized.get("provider") or "disabled",
                "lang": normalized.get("lang") or "chi_sim+eng",
            }
