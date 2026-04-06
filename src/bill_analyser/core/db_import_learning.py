"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseImportLearningMixin(DatabaseFacadeBase):
    """Import annotation, long-term learning rule, and composite-match helpers."""

    @staticmethod
    def _normalize_import_learning_text(raw_value: Any) -> str:
        if raw_value is None:
            return ""
        text = str(raw_value).strip().lower()
        if not text:
            return ""
        parts = [part.strip() for part in text.split("|") if part.strip()]
        if parts:
            text = " | ".join(parts)
        return " ".join(text.split())

    @classmethod
    def build_composite_match_hash(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> str | None:
        features = cls.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        if not features:
            return None

        key_aliases = {
            "parser_id": "p",
            "counterparty": "c",
            "description": "d",
            "payment_method": "m",
        }
        return "|".join(f"{key_aliases[key]}={value}" for key, value in sorted(features.items()))

    @classmethod
    def build_composite_match_features(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> dict[str, str] | None:
        norm = cls._normalize_import_learning_text
        features = {
            "parser_id": norm(parser_id),
            "counterparty": norm(counterparty),
            "description": norm(description),
            "payment_method": norm(payment_method),
        }
        non_empty = {key: value for key, value in features.items() if value}
        if len(non_empty) < 2:
            return None
        return non_empty

    async def _record_import_learning_rule_log(
        self,
        conn: aiosqlite.Connection,
        *,
        rule_id: int | None,
        user_id: int,
        action: str,
        match_type: str,
        match_value: str,
        normalized_match_value: str,
        session_id: str | None = None,
        preview_id: int | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        await conn.execute(
            """
            INSERT INTO import_learning_rule_logs (
                rule_id, user_id, action, match_type, match_value,
                normalized_match_value, session_id, preview_id,
                payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                rule_id,
                user_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                json.dumps(payload or {}, ensure_ascii=False, sort_keys=True),
                utc_now_iso(),
            ),
        )

    @log_method
    async def save_import_annotation_samples(
        self,
        session_id: str,
        samples: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        if not samples:
            return 0

        conn = await self._get_connection()
        now = utc_now_iso()
        saved_count = 0
        for sample in samples:
            preview_id = sample.get("preview_id") or sample.get("id")
            if not preview_id:
                continue
            await conn.execute(
                """
                INSERT INTO import_annotation_samples (
                    session_id, user_id, preview_id,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id, preview_id) DO UPDATE SET
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    updated_at = excluded.updated_at
                """,
                (
                    session_id,
                    user_id,
                    preview_id,
                    sample.get("preview_type") or sample.get("annotated_type"),
                    sample.get("category_id") or sample.get("annotated_category_id"),
                    sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                    sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                    now,
                    now,
                ),
            )
            saved_count += 1

        await conn.commit()
        return saved_count

    @log_method
    async def get_import_annotation_samples(self, session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM import_annotation_samples
            WHERE session_id = ? AND user_id = ?
            ORDER BY updated_at ASC, id ASC
            """,
            (session_id, user_id),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def promote_import_annotation_samples_to_learning(
        self,
        session_id: str,
        preview_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, int]:
        previews = await self.get_preview_by_session(session_id, user_id=user_id)
        preview_map = {int(preview["id"]): preview for preview in previews if preview.get("id")}
        samples = await self.get_import_annotation_samples(session_id, user_id=user_id)

        selected_preview_ids = {int(pid) for pid in (preview_ids or []) if pid}
        if selected_preview_ids:
            samples = [sample for sample in samples if int(sample.get("preview_id", 0) or 0) in selected_preview_ids]
        if not samples:
            return {"selected_samples": 0, "rules_total": 0, "created": 0, "updated": 0}

        pending_rules: dict[tuple[str, str], dict[str, Any]] = {}
        for sample in samples:
            preview_id = int(sample.get("preview_id", 0) or 0)
            preview = preview_map.get(preview_id)
            if not preview:
                continue

            composite_hash = self.build_composite_match_hash(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            match_features = self.build_composite_match_features(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            if not composite_hash or not match_features:
                continue

            pending_rules[("composite", composite_hash)] = {
                "match_type": "composite",
                "match_value": composite_hash,
                "normalized_match_value": composite_hash,
                "learned_type": sample.get("annotated_type") or preview.get("preview_type"),
                "learned_category_id": sample.get("annotated_category_id"),
                "learned_source_account_id": sample.get("annotated_source_account_id"),
                "learned_destination_account_id": sample.get("annotated_destination_account_id"),
                "source_session_id": session_id,
                "source_preview_id": preview_id,
                "parser_id": preview.get("preview_parser_id", ""),
                "composite_match_hash": composite_hash,
                "match_features_json": json.dumps(match_features, ensure_ascii=False, sort_keys=True),
            }

        if not pending_rules:
            return {"selected_samples": len(samples), "rules_total": 0, "created": 0, "updated": 0}

        conn = await self._get_connection()
        now = utc_now_iso()
        created_count = 0
        updated_count = 0

        for rule_data in pending_rules.values():
            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (user_id, rule_data["match_type"], rule_data["normalized_match_value"]),
            ) as cursor:
                existing = await cursor.fetchone()

            await conn.execute(
                """
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id,
                    learned_source_account_id, learned_destination_account_id,
                    enabled, source_session_id, source_preview_id,
                    parser_id, composite_match_hash, match_features_json,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    match_value = excluded.match_value,
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    enabled = 1,
                    source_session_id = excluded.source_session_id,
                    source_preview_id = excluded.source_preview_id,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                """,
                (
                    user_id,
                    rule_data["match_type"],
                    rule_data["match_value"],
                    rule_data["normalized_match_value"],
                    rule_data["learned_type"],
                    rule_data["learned_category_id"],
                    rule_data["learned_source_account_id"],
                    rule_data["learned_destination_account_id"],
                    rule_data["source_session_id"],
                    rule_data["source_preview_id"],
                    rule_data.get("parser_id"),
                    rule_data.get("composite_match_hash"),
                    rule_data.get("match_features_json"),
                    now,
                    now,
                ),
            )

            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (user_id, rule_data["match_type"], rule_data["normalized_match_value"]),
            ) as cursor:
                saved_row = await cursor.fetchone()

            rule_id = int(saved_row["id"]) if saved_row else None
            action = "updated" if existing else "created"
            if existing:
                updated_count += 1
            else:
                created_count += 1

            await self._record_import_learning_rule_log(
                conn,
                rule_id=rule_id,
                user_id=user_id,
                action=action,
                match_type=rule_data["match_type"],
                match_value=rule_data["match_value"],
                normalized_match_value=rule_data["normalized_match_value"],
                session_id=session_id,
                preview_id=rule_data["source_preview_id"],
                payload=rule_data,
            )

        await conn.commit()
        return {
            "selected_samples": len(samples),
            "rules_total": len(pending_rules),
            "created": created_count,
            "updated": updated_count,
        }

    @log_method
    async def get_import_learning_rules(
        self,
        user_id: int = 1,
        enabled_only: bool = False,
        limit: int | None = 200,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]
        if enabled_only:
            query += " AND enabled = 1"
        query += " ORDER BY updated_at DESC, id DESC"
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def count_import_learning_rules(self, user_id: int = 1, enabled_only: bool = False) -> int:
        conn = await self._get_connection()
        query = "SELECT COUNT(*) AS total_count FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]
        if enabled_only:
            query += " AND enabled = 1"
        async with conn.execute(query, tuple(params)) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)

    @log_method
    async def set_import_learning_rule_enabled(self, rule_id: int, enabled: bool, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        enabled_value = 1 if enabled else 0
        now = utc_now_iso()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return False

        await conn.execute(
            "UPDATE import_learning_rules SET enabled = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (enabled_value, now, rule_id, user_id),
        )
        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="enabled" if enabled else "disabled",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload={"enabled": enabled_value},
        )
        await conn.commit()
        return True

    @log_method
    async def delete_import_learning_rule(self, rule_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return False

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="deleted",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload=dict(existing),
        )
        await conn.execute("DELETE FROM import_learning_rules WHERE id = ? AND user_id = ?", (rule_id, user_id))
        await conn.commit()
        return True

    @log_method
    async def increment_import_learning_rule_usage(self, rule_ids: list[int], user_id: int = 1) -> int:
        unique_ids = [int(rule_id) for rule_id in sorted(set(rule_ids)) if rule_id]
        if not unique_ids:
            return 0
        conn = await self._get_connection()
        now = utc_now_iso()
        placeholders = ",".join(["?" for _ in unique_ids])
        cursor = await conn.execute(
            f"""
            UPDATE import_learning_rules
            SET applied_count = applied_count + 1,
                last_applied_at = ?,
                updated_at = ?
            WHERE user_id = ? AND id IN ({placeholders})
            """,
            (now, now, user_id, *unique_ids),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def batch_update_preview_classification(self, updates: list[dict[str, Any]]) -> int:
        if not updates:
            return 0
        conn = await self._get_connection()
        updated_count = 0

        for update_item in updates:
            try:
                preview_id = update_item.get("id")
                if not preview_id:
                    continue
                await conn.execute(
                    """
                    UPDATE bills_preview SET
                        preview_type = ?,
                        preview_main_category = ?,
                        preview_sub_category = ?,
                        preview_source_account_id = ?,
                        preview_destination_account_id = ?
                    WHERE id = ?
                    """,
                    (
                        update_item.get("preview_type", ""),
                        update_item.get("preview_main_category", ""),
                        update_item.get("preview_sub_category", ""),
                        update_item.get("preview_source_account_id"),
                        update_item.get("preview_destination_account_id"),
                        preview_id,
                    ),
                )
                updated_count += 1
            except Exception as exc:  # pragma: no cover - defensive logging branch
                self.logger.error("[重新分类-更新失败] id=%s, error=%s", update_item.get("id"), exc)

        await conn.commit()
        return updated_count
