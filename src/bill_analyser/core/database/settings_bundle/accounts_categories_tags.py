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


class SettingsBundleAccountsCategoriesTagsMixin(object):
        async def _import_settings_accounts(
            self,
            conn: Any,
            accounts: list[dict[str, Any]],
            user_id: int,
            result: dict[str, Any],
            warnings: list[str],
        ) -> dict[str, int]:
            section = result["sections"]["accounts"]
            existing = await self._load_existing_accounts(conn, user_id)
            ref_map = {_local_id_ref("account", account_id): account_id for account_id in existing["by_id"]}
            ref_map.update(
                {
                    f"accountName:{_safe_text(row.get('name'))}": int(row["id"])
                    for row in existing["by_id"].values()
                    if _safe_text(row.get("name"))
                }
            )
            pending = list(accounts)
            for _ in range(len(pending) + 1):
                next_pending = []
                progressed = False
                for item in pending:
                    parent_ref = _safe_text(_get_any(item, "parentRef", "parent_ref"))
                    if parent_ref and parent_ref not in ref_map:
                        next_pending.append(item)
                        continue
                    account_id = await self._upsert_settings_account(
                        conn, item, user_id, section, existing, ref_map, warnings
                    )
                    if account_id:
                        ref = _external_ref(item, "account")
                        if ref:
                            ref_map[ref] = account_id
                        ref_map[f"accountName:{_safe_text(item.get('name'))}"] = account_id
                        progressed = True
                pending = next_pending
                if not pending or not progressed:
                    break

            for item in pending:
                warnings.append(f"Account parent not found; imported as root: {_safe_text(item.get('name'))}")
                account_id = await self._upsert_settings_account(
                    conn, {**item, "parentRef": ""}, user_id, section, existing, ref_map, warnings
                )
                if account_id:
                    ref = _external_ref(item, "account")
                    if ref:
                        ref_map[ref] = account_id
                    ref_map[f"accountName:{_safe_text(item.get('name'))}"] = account_id
            return ref_map

        async def _load_existing_accounts(self, conn: Any, user_id: int) -> dict[str, Any]:
            async with conn.execute(
                "SELECT * FROM accounts WHERE user_id = ?",
                (user_id,),
            ) as cursor:
                rows = [dict(row) for row in await cursor.fetchall()]
            return {
                "by_id": {int(row["id"]): row for row in rows},
                "by_key": {(_safe_text(row.get("name")), int(row.get("parent_id") or 0)): row for row in rows},
            }

        async def _upsert_settings_account(
            self,
            conn: Any,
            item: dict[str, Any],
            user_id: int,
            section: dict[str, int],
            existing: dict[str, Any],
            ref_map: dict[str, int],
            warnings: list[str],
        ) -> int | None:
            name = _safe_text(item.get("name"))
            if not name:
                section["skipped"] += 1
                warnings.append("Skipped account without name")
                return None
            parent_id = _safe_int(ref_map.get(_safe_text(_get_any(item, "parentRef", "parent_ref"))))
            row = existing["by_key"].get((name, parent_id))
            now = utc_now_iso()
            values = self._settings_account_import_values(item, ref_map)
            if row:
                columns = [key for key in values if key != "name"]
                await conn.execute(
                    f"UPDATE accounts SET {', '.join(f'{key} = ?' for key in columns)}, updated_at = ? "
                    "WHERE id = ? AND user_id = ?",
                    (*[values[key] for key in columns], now, row["id"], user_id),
                )
                section["updated"] += 1
                return int(row["id"])

            cursor = await conn.execute(
                """
                INSERT INTO accounts (
                    user_id, name, type, category, currency, icon, color, balance,
                    initial_balance, hidden, display_order, comment, aliases,
                    parent_id, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    values["name"],
                    values["type"],
                    values["category"],
                    values["currency"],
                    values["icon"],
                    values["color"],
                    values["balance"],
                    values["initial_balance"],
                    values["hidden"],
                    values["display_order"],
                    values["comment"],
                    values["aliases"],
                    values["parent_id"],
                    now,
                    now,
                ),
            )
            account_id = int(cursor.lastrowid or 0)
            new_row = {**values, "id": account_id}
            existing["by_id"][account_id] = new_row
            existing["by_key"][(name, parent_id)] = new_row
            section["created"] += 1
            return account_id

        async def _import_settings_categories(
            self,
            conn: Any,
            categories: list[dict[str, Any]],
            user_id: int,
            result: dict[str, Any],
            warnings: list[str],
        ) -> dict[str, int]:
            section = result["sections"]["transactionCategories"]
            existing = await self._load_existing_categories(conn, user_id)
            ref_map = {_local_id_ref("category", category_id): category_id for category_id in existing["by_id"]}
            ref_map.update(
                {
                    f"categoryName:{self._settings_category_name(row)}": int(row["id"])
                    for row in existing["by_id"].values()
                    if self._settings_category_name(row)
                }
            )
            for item in categories:
                category_id = await self._upsert_settings_category(conn, item, user_id, section, existing, warnings)
                if category_id:
                    ref = _external_ref(item, "category")
                    if ref:
                        ref_map[ref] = category_id
                    main = _safe_text(_get_any(item, "mainCategory", "main_category"))
                    sub = _safe_text(_get_any(item, "subCategory", "sub_category"))
                    category_name = "/".join(part for part in (main, sub) if part)
                    if category_name:
                        ref_map[f"categoryName:{category_name}"] = category_id
            return ref_map

        async def _load_existing_categories(self, conn: Any, user_id: int) -> dict[str, Any]:
            async with conn.execute(
                "SELECT * FROM categories WHERE user_id = ?",
                (user_id,),
            ) as cursor:
                rows = [dict(row) for row in await cursor.fetchall()]
            return {
                "by_id": {int(row["id"]): row for row in rows},
                "by_key": {
                    (_safe_text(row.get("main_category")), _safe_text(row.get("sub_category"))): row
                    for row in rows
                },
            }

        async def _upsert_settings_category(
            self,
            conn: Any,
            item: dict[str, Any],
            user_id: int,
            section: dict[str, int],
            existing: dict[str, Any],
            warnings: list[str],
        ) -> int | None:
            main = _safe_text(_get_any(item, "mainCategory", "main_category"))
            sub = _safe_text(_get_any(item, "subCategory", "sub_category"))
            if not main:
                section["skipped"] += 1
                warnings.append("Skipped category without mainCategory")
                return None
            row = existing["by_key"].get((main, sub))
            now = utc_now_iso()
            values = self._settings_category_import_values(item)
            if row:
                update_columns = [key for key in values if key not in {"main_category", "sub_category"}]
                await conn.execute(
                    f"UPDATE categories SET {', '.join(f'{key} = ?' for key in update_columns)} "
                    "WHERE id = ? AND user_id = ?",
                    (*[values[key] for key in update_columns], row["id"], user_id),
                )
                section["updated"] += 1
                return int(row["id"])
            cursor = await conn.execute(
                """
                INSERT INTO categories (
                    user_id, type, main_category, sub_category, description,
                    priority, keywords, hidden, icon, color, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    values["type"],
                    values["main_category"],
                    values["sub_category"],
                    values["description"],
                    values["priority"],
                    values["keywords"],
                    values["hidden"],
                    values["icon"],
                    values["color"],
                    now,
                ),
            )
            category_id = int(cursor.lastrowid or 0)
            existing["by_id"][category_id] = {**values, "id": category_id}
            existing["by_key"][(main, sub)] = existing["by_id"][category_id]
            section["created"] += 1
            return category_id

        async def _import_settings_tags(
            self,
            conn: Any,
            tags: list[dict[str, Any]],
            user_id: int,
            result: dict[str, Any],
            warnings: list[str],
        ) -> dict[str, int]:
            section = result["sections"]["transactionTags"]
            existing = await self._load_existing_tags(conn, user_id)
            ref_map = {_local_id_ref("tag", tag_id): tag_id for tag_id in existing["by_id"]}
            ref_map.update(
                {
                    f"tagName:{_safe_text(row.get('name'))}": int(row["id"])
                    for row in existing["by_id"].values()
                    if _safe_text(row.get("name"))
                }
            )
            for item in tags:
                tag_id = await self._upsert_settings_tag(conn, item, user_id, section, existing, warnings)
                if tag_id:
                    ref = _external_ref(item, "tag")
                    if ref:
                        ref_map[ref] = tag_id
                    ref_map[f"tagName:{_safe_text(item.get('name'))}"] = tag_id
            return ref_map

        async def _load_existing_tags(self, conn: Any, user_id: int) -> dict[str, Any]:
            async with conn.execute("SELECT * FROM tags WHERE user_id = ?", (user_id,)) as cursor:
                rows = [dict(row) for row in await cursor.fetchall()]
            return {
                "by_id": {int(row["id"]): row for row in rows},
                "by_name": {_safe_text(row.get("name")): row for row in rows},
            }

        async def _upsert_settings_tag(
            self,
            conn: Any,
            item: dict[str, Any],
            user_id: int,
            section: dict[str, int],
            existing: dict[str, Any],
            warnings: list[str],
        ) -> int | None:
            name = _safe_text(item.get("name"))
            if not name:
                section["skipped"] += 1
                warnings.append("Skipped tag without name")
                return None
            row = existing["by_name"].get(name)
            now = utc_now_iso()
            values = self._settings_tag_import_values(item)
            if row:
                await conn.execute(
                    """
                    UPDATE tags
                    SET color = ?, icon = ?, display_order = ?, hidden = ?, updated_at = ?
                    WHERE id = ? AND user_id = ?
                    """,
                    (
                        values["color"],
                        values["icon"],
                        values["display_order"],
                        values["hidden"],
                        now,
                        row["id"],
                        user_id,
                    ),
                )
                section["updated"] += 1
                return int(row["id"])
            cursor = await conn.execute(
                """
                INSERT INTO tags (
                    user_id, name, color, icon, display_order,
                    hidden, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    values["name"],
                    values["color"],
                    values["icon"],
                    values["display_order"],
                    values["hidden"],
                    now,
                    now,
                ),
            )
            tag_id = int(cursor.lastrowid or 0)
            existing["by_id"][tag_id] = {**values, "id": tag_id}
            existing["by_name"][name] = existing["by_id"][tag_id]
            section["created"] += 1
            return tag_id

        @staticmethod
        def _settings_account_import_values(
            item: dict[str, Any],
            ref_map: dict[str, int],
        ) -> dict[str, Any]:
            try:
                return settings_bundle_rust_bridge.normalize_account_import(item, ref_map)
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable:
                return {
                    "name": _safe_text(item.get("name")),
                    "type": _safe_int(item.get("type"), 1),
                    "category": _get_any(item, "category", default=None),
                    "currency": _safe_text(item.get("currency"), "CNY"),
                    "icon": _safe_text(item.get("icon")),
                    "color": _safe_text(item.get("color")),
                    "balance": _safe_float(item.get("balance")),
                    "initial_balance": _safe_float(
                        _get_any(item, "initialBalance", "initial_balance")
                    ),
                    "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
                    "display_order": _safe_int(
                        _get_any(item, "displayOrder", "display_order")
                    ),
                    "comment": _safe_text(item.get("comment")),
                    "aliases": _dump_json_list(item.get("aliases")),
                    "parent_id": _safe_int(
                        ref_map.get(_safe_text(_get_any(item, "parentRef", "parent_ref")))
                    ),
                }

        @staticmethod
        def _settings_category_import_values(item: dict[str, Any]) -> dict[str, Any]:
            try:
                return settings_bundle_rust_bridge.normalize_category_import(item)
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable:
                return {
                    "type": _safe_int(item.get("type"), 3),
                    "main_category": _safe_text(
                        _get_any(item, "mainCategory", "main_category")
                    ),
                    "sub_category": _safe_text(
                        _get_any(item, "subCategory", "sub_category")
                    ),
                    "description": _safe_text(item.get("description")),
                    "priority": _safe_int(item.get("priority")),
                    "keywords": _safe_text(item.get("keywords")),
                    "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
                    "icon": _safe_text(item.get("icon")),
                    "color": _safe_text(item.get("color")),
                }

        @staticmethod
        def _settings_tag_import_values(item: dict[str, Any]) -> dict[str, Any]:
            try:
                return settings_bundle_rust_bridge.normalize_tag_import(item)
            except settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable:
                return {
                    "name": _safe_text(item.get("name")),
                    "color": _safe_text(item.get("color")),
                    "icon": _safe_text(item.get("icon")),
                    "display_order": _safe_int(
                        _get_any(item, "displayOrder", "display_order")
                    ),
                    "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
                }
