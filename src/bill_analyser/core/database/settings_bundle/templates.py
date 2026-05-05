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


class SettingsBundleTemplatesMixin(object):
        async def _import_settings_templates(
            self,
            conn: Any,
            templates: list[dict[str, Any]],
            user_id: int,
            *,
            template_type: int,
            result: dict[str, Any],
            warnings: list[str],
            account_ref_map: dict[str, int],
            category_ref_map: dict[str, int],
            tag_ref_map: dict[str, int],
        ) -> None:
            section_key = "scheduledTransactions" if template_type == 2 else "transactionTemplates"
            section = result["sections"][section_key]
            existing = await self._load_existing_templates(conn, user_id, template_type)
            for item in templates:
                await self._upsert_settings_template(
                    conn,
                    item,
                    user_id,
                    template_type,
                    section,
                    existing,
                    warnings,
                    account_ref_map,
                    category_ref_map,
                    tag_ref_map,
                )

        async def _load_existing_templates(self, conn: Any, user_id: int, template_type: int) -> dict[str, Any]:
            table_name = "recurring_bills" if template_type == 2 else "bill_templates"
            async with conn.execute(
                f"SELECT * FROM {table_name} WHERE user_id = ?",
                (user_id,),
            ) as cursor:
                rows = [dict(row) for row in await cursor.fetchall()]
            return {"by_name": {_safe_text(row.get("name")): row for row in rows}}

        async def _upsert_settings_template(
            self,
            conn: Any,
            item: dict[str, Any],
            user_id: int,
            template_type: int,
            section: dict[str, int],
            existing: dict[str, Any],
            warnings: list[str],
            account_ref_map: dict[str, int],
            category_ref_map: dict[str, int],
            tag_ref_map: dict[str, int],
        ) -> None:
            name = _safe_text(item.get("name"))
            if not name:
                section["skipped"] += 1
                warnings.append("Skipped template without name")
                return
            table_name = "recurring_bills" if template_type == 2 else "bill_templates"
            payload = self._settings_template_payload(
                item, account_ref_map, category_ref_map, tag_ref_map, warnings
            )
            if self._settings_template_has_unresolved_references(item, payload, warnings):
                section["skipped"] += 1
                return
            update_payload = self._settings_template_update_payload(payload, template_type)
            row = existing["by_name"].get(name)
            now = utc_now_iso()
            if row:
                await conn.execute(
                    f"UPDATE {table_name} SET {', '.join(f'{key} = ?' for key in update_payload)}, updated_at = ? "
                    "WHERE id = ? AND user_id = ?",
                    (*update_payload.values(), now, row["id"], user_id),
                )
                section["updated"] += 1
                return

            if template_type == 2:
                cursor = await conn.execute(
                    """
                    INSERT INTO recurring_bills (
                        user_id, template_id, name, description, type, category,
                        amount, account, counterparty, destination_amount, hide_amount,
                        tag, comment, frequency, scheduled_frequency_type, start_date,
                        end_date, next_date, hidden, display_order, utc_offset,
                        enabled, auto_create, created_at, updated_at
                    ) VALUES (?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        user_id,
                        name,
                        payload["description"],
                        payload["type"],
                        payload["category"],
                        payload["amount"],
                        payload["account"],
                        payload["counterparty"],
                        payload["destination_amount"],
                        payload["hide_amount"],
                        payload["tag"],
                        payload["comment"],
                        payload["frequency"],
                        payload["scheduled_frequency_type"],
                        payload["start_date"],
                        payload["end_date"],
                        payload["next_date"],
                        payload["hidden"],
                        payload["display_order"],
                        payload["utc_offset"],
                        payload["enabled"],
                        payload["auto_create"],
                        now,
                        now,
                    ),
                )
            else:
                cursor = await conn.execute(
                    """
                    INSERT INTO bill_templates (
                        user_id, name, description, type, category, amount, account,
                        counterparty, destination_amount, hide_amount, tag, comment,
                        is_favorite, display_order, hidden, utc_offset, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?)
                    """,
                    (
                        user_id,
                        name,
                        payload["description"],
                        payload["type"],
                        payload["category"],
                        payload["amount"],
                        payload["account"],
                        payload["counterparty"],
                        payload["destination_amount"],
                        payload["hide_amount"],
                        payload["tag"],
                        payload["comment"],
                        payload["display_order"],
                        payload["hidden"],
                        payload["utc_offset"],
                        now,
                        now,
                    ),
                )
            template_id = int(cursor.lastrowid or 0)
            existing["by_name"][name] = {"id": template_id, "name": name}
            section["created"] += 1

        @staticmethod
        def _settings_template_update_payload(
            payload: dict[str, Any],
            template_type: int,
        ) -> dict[str, Any]:
            common_keys = {
                "description",
                "type",
                "category",
                "amount",
                "account",
                "counterparty",
                "destination_amount",
                "hide_amount",
                "tag",
                "comment",
                "display_order",
                "hidden",
                "utc_offset",
            }
            if template_type == 1:
                return {key: payload[key] for key in common_keys}
            recurring_keys = {
                "frequency",
                "scheduled_frequency_type",
                "start_date",
                "end_date",
                "next_date",
                "enabled",
                "auto_create",
            }
            return {key: payload[key] for key in (*common_keys, *recurring_keys)}

        def _settings_template_payload(
            self,
            item: dict[str, Any],
            account_ref_map: dict[str, int],
            category_ref_map: dict[str, int],
            tag_ref_map: dict[str, int],
            warnings: list[str],
        ) -> dict[str, Any]:
            category_id = self._resolve_settings_category_id(item, category_ref_map)
            source_account_id = self._resolve_settings_account_id(
                item, account_ref_map, "sourceAccountRef", "sourceAccountName"
            )
            destination_account_id = self._resolve_settings_account_id(
                item, account_ref_map, "destinationAccountRef", "destinationAccountName"
            )
            tag_ids = self._resolve_settings_tag_ids(item, tag_ref_map, warnings)
            scheduled_start = _safe_text(_get_any(item, "scheduledStartDate", "startDate"))
            return {
                "description": _safe_text(item.get("description")),
                "type": _safe_int(item.get("type"), 3),
                "category": str(category_id or ""),
                "amount": _safe_float(_get_any(item, "sourceAmount", "amount")),
                "account": str(source_account_id or 0),
                "counterparty": str(destination_account_id or 0),
                "destination_amount": _safe_float(_get_any(item, "destinationAmount", "destination_amount")),
                "hide_amount": 1 if _safe_bool(_get_any(item, "hideAmount", "hide_amount")) else 0,
                "tag": ",".join(str(tag_id) for tag_id in tag_ids),
                "comment": _safe_text(item.get("comment")),
                "display_order": _safe_int(_get_any(item, "displayOrder", "display_order")),
                "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
                "utc_offset": _safe_int(_get_any(item, "utcOffset", "utc_offset")),
                "frequency": _safe_text(_get_any(item, "scheduledFrequency", "frequency")),
                "scheduled_frequency_type": _safe_int(
                    _get_any(item, "scheduledFrequencyType", "scheduled_frequency_type")
                ),
                "start_date": scheduled_start,
                "end_date": _safe_text(_get_any(item, "scheduledEndDate", "endDate")),
                "next_date": _safe_text(_get_any(item, "nextDate", default=scheduled_start)),
                "enabled": 1 if _safe_bool(_get_any(item, "enabled", default=True)) else 0,
                "auto_create": 1 if _safe_bool(_get_any(item, "autoCreate", "auto_create")) else 0,
            }

        @staticmethod
        def _has_template_ref_value(item: dict[str, Any], *keys: str) -> bool:
            return any(_safe_text(_get_any(item, key)) for key in keys)

        @classmethod
        def _settings_template_has_unresolved_references(
            cls,
            item: dict[str, Any],
            payload: dict[str, Any],
            warnings: list[str],
        ) -> bool:
            name = _safe_text(item.get("name")) or "<unnamed>"
            has_category_ref = cls._has_template_ref_value(
                item,
                "categoryRef",
                "category_ref",
                "categoryName",
                "category_name",
                "mainCategory",
                "main_category",
                "subCategory",
                "sub_category",
                "categoryId",
                "category_id",
            )
            if has_category_ref and not _safe_text(payload.get("category")):
                warnings.append(f"Skipped template {name} with unresolved category reference")
                return True

            has_source_ref = cls._has_template_ref_value(
                item,
                "sourceAccountRef",
                "source_account_ref",
                "sourceAccountName",
                "source_account_name",
                "sourceAccountId",
                "source_account_id",
            )
            if has_source_ref and _safe_int(payload.get("account")) == 0:
                warnings.append(f"Skipped template {name} with unresolved source account reference")
                return True

            has_destination_ref = cls._has_template_ref_value(
                item,
                "destinationAccountRef",
                "destination_account_ref",
                "destinationAccountName",
                "destination_account_name",
                "destinationAccountId",
                "destination_account_id",
            )
            if has_destination_ref and _safe_int(payload.get("counterparty")) == 0:
                warnings.append(f"Skipped template {name} with unresolved destination account reference")
                return True

            return False
