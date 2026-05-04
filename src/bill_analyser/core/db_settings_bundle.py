"""Unified JSON settings bundle import/export helpers."""

# pylint: disable=line-too-long,too-many-lines,too-many-locals,too-many-branches,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments

from __future__ import annotations

import json
from typing import Any

from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso
from .db_llm_config import normalize_llm_advanced_settings

SETTINGS_BUNDLE_SCHEMA_VERSION = 1
SETTINGS_BUNDLE_SECTION_KEYS = (
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
    "llmConfigs",
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


class DatabaseSettingsBundleMixin(DatabaseFacadeBase):
    """Export and import user settings as a stable JSON bundle."""

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

        sections = {
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
                self._export_settings_template(item, category_refs, category_names, account_refs, account_names, tag_refs, tag_names)
                for item in templates
            ],
            "scheduledTransactions": [
                self._export_settings_template(item, category_refs, category_names, account_refs, account_names, tag_refs, tag_names)
                for item in scheduled
            ],
            "categoryRecognitionRules": [
                self._export_settings_category_rule(item, category_refs)
                for item in rules
            ],
            "llmConfigs": [
                self._export_settings_llm_config(item)
                for item in llm_configs
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
        values = {
            "name": name,
            "type": _safe_int(item.get("type"), 1),
            "category": _get_any(item, "category", default=None),
            "currency": _safe_text(item.get("currency"), "CNY"),
            "icon": _safe_text(item.get("icon")),
            "color": _safe_text(item.get("color")),
            "balance": _safe_float(item.get("balance")),
            "initial_balance": _safe_float(_get_any(item, "initialBalance", "initial_balance")),
            "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
            "display_order": _safe_int(_get_any(item, "displayOrder", "display_order")),
            "comment": _safe_text(item.get("comment")),
            "aliases": _dump_json_list(item.get("aliases")),
            "parent_id": parent_id,
        }
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
        values = {
            "type": _safe_int(item.get("type"), 3),
            "main_category": main,
            "sub_category": sub,
            "description": _safe_text(item.get("description")),
            "priority": _safe_int(item.get("priority")),
            "keywords": _safe_text(item.get("keywords")),
            "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
            "icon": _safe_text(item.get("icon")),
            "color": _safe_text(item.get("color")),
        }
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
        values = {
            "name": name,
            "color": _safe_text(item.get("color")),
            "icon": _safe_text(item.get("icon")),
            "display_order": _safe_int(_get_any(item, "displayOrder", "display_order")),
            "hidden": 1 if _safe_bool(item.get("hidden")) else 0,
        }
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
