"""Import-config template persistence and header-matching helpers."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso

if TYPE_CHECKING:
    import aiosqlite


class DatabaseImportConfigsMixin(DatabaseFacadeBase):
    """Import config template helpers and header-matching heuristics."""

    @staticmethod
    def _normalize_import_config_header(value: Any) -> str:
        """Normalize one header cell into the canonical matching form."""
        if value is None:
            return ""
        return " ".join(str(value).strip().lower().split())

    def _normalize_import_config_headers(self, headers: list[Any] | None) -> list[str]:
        """Normalize a header list while dropping empty header values."""
        if not headers:
            return []
        normalized_headers: list[str] = []
        for header in headers:
            normalized = self._normalize_import_config_header(header)
            if normalized:
                normalized_headers.append(normalized)
        return normalized_headers

    def _build_import_config_header_signature(self, headers: list[Any] | None) -> str:
        """Build a stable header signature used by exact-template matching."""
        normalized_headers = self._normalize_import_config_headers(headers)
        if not normalized_headers:
            return ""
        return "||".join(normalized_headers)

    @staticmethod
    def _parse_json_object(raw_value: Any, default: dict[str, Any] | None = None) -> dict[str, Any]:
        """Parse a JSON object field while falling back to a safe default mapping."""
        if raw_value in (None, ""):
            return default.copy() if default else {}
        if isinstance(raw_value, dict):
            return dict(raw_value)
        try:
            parsed = json.loads(raw_value)
            if isinstance(parsed, dict):
                return parsed
        except (TypeError, ValueError, json.JSONDecodeError):
            pass
        return default.copy() if default else {}

    def _serialize_import_config_custom_rules(
        self,
        custom_rules: Any,
        sample_headers: list[Any] | None = None,
    ) -> str:
        """Serialize custom rules and attach normalized header metadata."""
        custom_rules_data = self._parse_json_object(custom_rules)
        normalized_headers = self._normalize_import_config_headers(sample_headers)
        if normalized_headers:
            custom_rules_data["sample_headers"] = normalized_headers
            custom_rules_data["header_signature"] = self._build_import_config_header_signature(
                normalized_headers
            )
            custom_rules_data["header_count"] = len(normalized_headers)
        return json.dumps(custom_rules_data, ensure_ascii=False)

    def _deserialize_import_config_row(self, row: aiosqlite.Row) -> dict[str, Any]:
        """Hydrate one import-config database row into the API-facing payload shape."""
        result = dict(row)
        result["field_mappings"] = self._parse_json_object(result.get("field_mappings"))
        result["custom_rules"] = self._parse_json_object(result.get("custom_rules"))
        result["sample_headers"] = result["custom_rules"].get("sample_headers", [])
        result["header_signature"] = result["custom_rules"].get("header_signature", "")
        result["header_count"] = int(result["custom_rules"].get("header_count", 0) or 0)
        result["has_header"] = bool(result.get("has_header", 1))
        result["is_default"] = bool(result.get("is_default", 0))
        result["description_summary"] = self._build_import_config_description_summary(result)
        result["default_recommendation"] = False
        return result

    @staticmethod
    def _build_import_config_description_summary(config: dict[str, Any]) -> str:
        """Build a concise description used by import-config list responses."""
        field_mappings = config.get("field_mappings") or {}
        sample_headers = config.get("sample_headers") or []
        display_labels = {
            "date": "时间",
            "type": "类型",
            "amount": "金额",
            "description": "描述",
            "account": "账户",
            "category": "分类",
            "counterparty": "交易对方",
            "paymentMethod": "支付方式",
            "payment_method": "支付方式",
        }
        display_order = {
            "date": 1,
            "type": 2,
            "amount": 3,
            "description": 4,
            "account": 5,
            "category": 6,
            "counterparty": 7,
            "paymentMethod": 8,
            "payment_method": 8,
        }
        summary_parts: list[str] = []

        if isinstance(field_mappings, dict) and field_mappings:
            mapped_entries = []
            sorted_items = sorted(
                field_mappings.items(),
                key=lambda item: (display_order.get(str(item[0]), 99), str(item[0])),
            )
            for field_name, header_name in sorted_items:
                header_text = str(header_name or "").strip()
                if not header_text:
                    continue
                mapped_entries.append(
                    f"{display_labels.get(str(field_name), str(field_name))}->{header_text}"
                )
                if len(mapped_entries) >= 4:
                    break
            if mapped_entries:
                summary_parts.append(f"映射: {' / '.join(mapped_entries)}")

        normalized_headers = []
        for header in sample_headers[:4]:
            header_text = str(header or "").strip()
            if header_text:
                normalized_headers.append(header_text)
        if normalized_headers:
            summary_parts.append(f"表头: {' / '.join(normalized_headers)}")
        return " | ".join(summary_parts)

    @staticmethod
    def _mark_import_config_default_recommendation(
        configs: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        """Mark the most recently useful config when no explicit default exists."""
        if not configs:
            return configs
        if any(bool(config.get("is_default")) for config in configs):
            return configs

        def _sort_key(config: dict[str, Any]) -> tuple[Any, ...]:
            use_count = int(config.get("use_count", 0) or 0)
            last_used_at = str(config.get("last_used_at", "") or "")
            updated_at = str(config.get("updated_at", "") or "")
            created_at = str(config.get("created_at", "") or "")
            return (use_count, last_used_at, updated_at, created_at)

        recommended = max(configs, key=_sort_key)
        recommended["default_recommendation"] = True
        return configs

    @staticmethod
    def _validate_import_config_payload(
        name: str,
        file_format: str,
        field_mappings: Any,
    ) -> None:
        """Validate the minimum payload required to save one import config."""
        if not name:
            raise ValueError("name is required")
        if not file_format:
            raise ValueError("file_format is required")
        if not isinstance(field_mappings, dict) or not field_mappings:
            raise ValueError("field_mappings is required")

    def _build_import_config_write_payload(
        self,
        data: dict[str, Any],
    ) -> dict[str, Any]:
        """Normalize and serialize one import-config payload before persistence."""
        name = str(data.get("name", "")).strip()
        file_format = str(data.get("file_format", "")).strip().lower()
        field_mappings = data.get("field_mappings") or {}
        self._validate_import_config_payload(name, file_format, field_mappings)

        sample_headers = data.get("sample_headers") or data.get("headers") or []
        return {
            "id": int(data.get("id", 0) or 0),
            "name": name,
            "file_format": file_format,
            "description": str(data.get("description", "") or "").strip(),
            "field_mappings_json": json.dumps(field_mappings, ensure_ascii=False),
            "date_format": str(data.get("date_format", "") or "").strip(),
            "encoding": str(data.get("encoding", "utf-8") or "utf-8").strip(),
            "delimiter": data.get("delimiter"),
            "skip_rows": int(data.get("skip_rows", 0) or 0),
            "has_header": 1 if bool(data.get("has_header", True)) else 0,
            "custom_rules_json": self._serialize_import_config_custom_rules(
                data.get("custom_rules"),
                sample_headers,
            ),
            "is_default": 1 if bool(data.get("is_default", False)) else 0,
        }

    async def _increment_import_config_usage(
        self,
        conn: aiosqlite.Connection,
        config_id: int,
        user_id: int,
        now: str,
    ) -> None:
        """Update usage counters and timestamps for one matched import config."""
        await conn.execute(
            """
            UPDATE import_configs
            SET use_count = COALESCE(use_count, 0) + 1,
                last_used_at = ?,
                updated_at = ?
            WHERE id = ? AND user_id = ?
            """,
            (now, now, config_id, user_id),
        )

    @log_method
    async def save_import_config(self, data: dict[str, Any], user_id: int = 1) -> int:
        """Create or update an import-config template for one user."""
        conn = await self._get_connection()
        now = utc_now_iso()
        payload = self._build_import_config_write_payload(data)
        try:
            if payload["is_default"]:
                await conn.execute(
                    (
                        "UPDATE import_configs SET is_default = 0, updated_at = ? "
                        "WHERE user_id = ? AND file_format = ?"
                    ),
                    (now, user_id, payload["file_format"]),
                )

            if payload["id"]:
                cursor = await conn.execute(
                    """
                    UPDATE import_configs
                    SET name = ?, file_format = ?, description = ?, field_mappings = ?,
                        date_format = ?, encoding = ?, delimiter = ?, skip_rows = ?,
                        has_header = ?, custom_rules = ?, is_default = ?, updated_at = ?
                    WHERE id = ? AND user_id = ?
                    """,
                    (
                        payload["name"],
                        payload["file_format"],
                        payload["description"],
                        payload["field_mappings_json"],
                        payload["date_format"],
                        payload["encoding"],
                        payload["delimiter"],
                        payload["skip_rows"],
                        payload["has_header"],
                        payload["custom_rules_json"],
                        payload["is_default"],
                        now,
                        payload["id"],
                        user_id,
                    ),
                )
                if cursor.rowcount == 0:
                    raise ValueError("import config not found")
                saved_id = payload["id"]
            else:
                cursor = await conn.execute(
                    """
                    INSERT INTO import_configs (
                        user_id, name, file_format, description, field_mappings,
                        date_format, encoding, delimiter, skip_rows, has_header,
                        custom_rules, is_default, use_count, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        user_id,
                        payload["name"],
                        payload["file_format"],
                        payload["description"],
                        payload["field_mappings_json"],
                        payload["date_format"],
                        payload["encoding"],
                        payload["delimiter"],
                        payload["skip_rows"],
                        payload["has_header"],
                        payload["custom_rules_json"],
                        payload["is_default"],
                        0,
                        now,
                        now,
                    ),
                )
                saved_id = int(cursor.lastrowid or 0)

            await conn.commit()
            return saved_id
        except Exception:  # pylint: disable=broad-exception-caught
            await conn.rollback()
            raise

    @log_method
    async def get_import_configs(
        self,
        user_id: int = 1,
        file_format: str | None = None,
        limit: int = 100,
    ) -> list[dict[str, Any]]:
        """List saved import-config templates for one user and optional file format."""
        conn = await self._get_connection()
        params: list[Any] = [user_id]
        query = "SELECT * FROM import_configs WHERE user_id = ?"
        if file_format:
            query += " AND file_format = ?"
            params.append(str(file_format).strip().lower())
        query += " ORDER BY is_default DESC, updated_at DESC LIMIT ?"
        params.append(int(limit))

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        configs = [self._deserialize_import_config_row(row) for row in rows]
        return self._mark_import_config_default_recommendation(configs)

    @log_method
    async def find_matching_import_config(  # pylint: disable=too-many-locals
        self,
        file_format: str,
        headers: list[Any],
        user_id: int = 1,
        min_score: float = 0.6,
    ) -> dict[str, Any] | None:
        """Find the best matching import template for the incoming headers."""
        conn = await self._get_connection()
        normalized_headers = self._normalize_import_config_headers(headers)
        if not normalized_headers:
            return None

        configs = await self.get_import_configs(
            user_id=user_id,
            file_format=file_format,
            limit=200,
        )
        if not configs:
            return None

        incoming_signature = self._build_import_config_header_signature(normalized_headers)
        incoming_set = set(normalized_headers)
        best_match = None
        best_score = 0.0
        best_reason = ""
        default_match = None

        for config in configs:
            if config.get("is_default") and default_match is None:
                default_match = config

            stored_headers = self._normalize_import_config_headers(config.get("sample_headers"))
            stored_signature = config.get("header_signature", "")
            if stored_signature and stored_signature == incoming_signature:
                best_match = config
                best_score = 1.0
                best_reason = "exact_header_signature"
                break
            if not stored_headers:
                continue

            stored_set = set(stored_headers)
            overlap_count = len(incoming_set & stored_set)
            if overlap_count == 0:
                continue
            overlap_score = overlap_count / max(len(incoming_set), len(stored_set))
            if normalized_headers[:1] == stored_headers[:1]:
                overlap_score += 0.05
            if overlap_score > best_score:
                best_match = config
                best_score = overlap_score
                best_reason = "header_overlap"

        now = utc_now_iso()
        if not best_match or best_score < min_score:
            if not default_match:
                return None
            matched = dict(default_match)
            matched["match_score"] = 0.0
            matched["match_reason"] = "default_template_fallback"
            matched["matched_header_count"] = 0
            await self._increment_import_config_usage(conn, int(default_match["id"]), user_id, now)
            await conn.commit()
            return matched

        matched = dict(best_match)
        matched["match_score"] = round(min(best_score, 1.0), 4)
        matched["match_reason"] = best_reason
        matched_headers = self._normalize_import_config_headers(best_match.get("sample_headers"))
        matched["matched_header_count"] = len(incoming_set & set(matched_headers))
        await self._increment_import_config_usage(conn, int(best_match["id"]), user_id, now)
        await conn.commit()
        return matched

    @log_method
    async def delete_import_config(
        self,
        config_id: int,
        user_id: int = 1,
    ) -> bool:
        """Delete one saved import-config template for the target user."""
        conn = await self._get_connection()
        cursor = await conn.execute(
            "DELETE FROM import_configs WHERE id = ? AND user_id = ?",
            (config_id, user_id),
        )
        await conn.commit()
        return cursor.rowcount > 0
