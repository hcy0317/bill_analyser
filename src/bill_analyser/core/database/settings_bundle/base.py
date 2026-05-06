"""Unified JSON settings bundle import/export helpers."""

# pylint: disable=line-too-long,too-many-lines,too-many-locals,too-many-branches,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments
# pylint: disable=bad-indentation,missing-class-docstring,useless-object-inheritance
# pylint: disable=unused-import,duplicate-code

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


class SettingsBundleBaseMixin(object):
        async def export_user_settings_bundle(self, user_id: int = 1) -> dict[str, Any]:
            """Export settings domains as a JSON-serializable bundle."""
            conn = await self._get_connection()
            accounts = await self.get_all_accounts(user_id=user_id)
            categories = await self.get_all_categories(user_id=user_id)
            tags = await self.get_all_tags(user_id=user_id)
            templates = await self._export_settings_templates(conn, user_id=user_id, template_type=1)
            scheduled = await self._export_settings_templates(conn, user_id=user_id, template_type=2)
            rules = await self.get_category_rules(user_id=user_id, enabled_only=False)
            llm_configs = await self.get_llm_configs(user_id=user_id)
            ocr_config = await self.get_app_setting(OCR_CONFIG_SETTING_KEY)

            account_refs = {int(item["id"]): f"account:{item['id']}" for item in accounts}
            account_names = {int(item["id"]): _safe_text(item.get("name")) for item in accounts}
            category_refs = {int(item["id"]): f"category:{item['id']}" for item in categories}
            category_names = {
                int(item["id"]): self._settings_category_name(item)
                for item in categories
                if int(item.get("id") or 0) > 0
            }
            tag_refs = {int(item["id"]): f"tag:{item['id']}" for item in tags}
            tag_names = {int(item["id"]): _safe_text(item.get("name")) for item in tags}

            taxonomy_sections = self._export_settings_taxonomy_sections(
                accounts=accounts,
                categories=categories,
                tags=tags,
                templates=templates,
                scheduled=scheduled,
                account_refs=account_refs,
                account_names=account_names,
                category_refs=category_refs,
                category_names=category_names,
                tag_refs=tag_refs,
                tag_names=tag_names,
            )
            sections = {
                **taxonomy_sections,
                "categoryRecognitionRules": [
                    self._export_settings_category_rule(item, category_refs)
                    for item in rules
                ],
                "llmConfigs": [
                    self._export_settings_llm_config(item)
                    for item in llm_configs
                ],
                "ocrConfig": [
                    self._export_settings_ocr_config(ocr_config)
                ],
            }

            return {
                "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
                "exportedAt": utc_now_iso(),
                "secretsPolicy": {"llmApiKeys": "redacted"},
                "sections": sections,
                "counts": {key: len(value) for key, value in sections.items()},
            }

        async def preview_import_user_settings_bundle(
            self,
            bundle: dict[str, Any],
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Preview settings bundle import without persisting changes."""
            return await self._import_user_settings_bundle(bundle, user_id=user_id, dry_run=True)

        async def import_user_settings_bundle(
            self,
            bundle: dict[str, Any],
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Import settings bundle with non-destructive upsert semantics."""
            return await self._import_user_settings_bundle(bundle, user_id=user_id, dry_run=False)

        async def _import_user_settings_bundle(
            self,
            bundle: dict[str, Any],
            *,
            user_id: int,
            dry_run: bool,
        ) -> dict[str, Any]:
            sections = self._normalize_settings_bundle_sections(bundle)
            result = {
                "dryRun": dry_run,
                "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
                "sections": _section_result_template(),
                "warnings": [],
            }
            warnings: list[str] = result["warnings"]
            conn = await self._get_connection()

            try:
                account_ref_map = await self._import_settings_accounts(
                    conn, sections["accounts"], user_id, result, warnings
                )
                category_ref_map = await self._import_settings_categories(
                    conn, sections["transactionCategories"], user_id, result, warnings
                )
                tag_ref_map = await self._import_settings_tags(
                    conn, sections["transactionTags"], user_id, result, warnings
                )
                await self._import_settings_templates(
                    conn,
                    sections["transactionTemplates"],
                    user_id,
                    template_type=1,
                    result=result,
                    warnings=warnings,
                    account_ref_map=account_ref_map,
                    category_ref_map=category_ref_map,
                    tag_ref_map=tag_ref_map,
                )
                await self._import_settings_templates(
                    conn,
                    sections["scheduledTransactions"],
                    user_id,
                    template_type=2,
                    result=result,
                    warnings=warnings,
                    account_ref_map=account_ref_map,
                    category_ref_map=category_ref_map,
                    tag_ref_map=tag_ref_map,
                )
                await self._import_settings_category_rules(
                    conn,
                    sections["categoryRecognitionRules"],
                    user_id,
                    result,
                    warnings,
                    category_ref_map,
                )
                await self._import_settings_llm_configs(
                    conn, sections["llmConfigs"], user_id, result
                )
                await self._import_settings_ocr_config(
                    conn, sections["ocrConfig"], result
                )

                if dry_run:
                    await conn.rollback()
                else:
                    await conn.commit()
                    self._clear_cache("account_mappings")
                    self._clear_cache("category_mappings")
                return result
            except Exception:
                await conn.rollback()
                raise

        @staticmethod
        def _normalize_settings_bundle_sections(bundle: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
            try:
                return settings_bundle_rust_bridge.normalize_sections(bundle)
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeOperationError as exc:
                raise ValueError(str(exc)) from exc
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable:
                pass
            if not isinstance(bundle, dict):
                raise ValueError("Settings bundle must be a JSON object")
            schema_version = bundle.get("schemaVersion")
            if (
                isinstance(schema_version, bool)
                or not isinstance(schema_version, int)
                or schema_version != SETTINGS_BUNDLE_SCHEMA_VERSION
            ):
                raise ValueError(
                    f"Unsupported settings bundle schemaVersion: {schema_version}"
                )
            sections = bundle.get("sections") if isinstance(bundle.get("sections"), dict) else bundle
            normalized: dict[str, list[dict[str, Any]]] = {}
            for key in SETTINGS_BUNDLE_SECTION_KEYS:
                raw_items = sections.get(key, []) if isinstance(sections, dict) else []
                if raw_items is None:
                    raw_items = []
                if not isinstance(raw_items, list):
                    raise ValueError(f"Settings bundle section {key} must be a list")
                normalized[key] = [item for item in raw_items if isinstance(item, dict)]
            return normalized
