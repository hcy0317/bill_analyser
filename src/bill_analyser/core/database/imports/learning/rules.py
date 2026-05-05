"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught,too-many-statements

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso

_UNSET: Any = object()


class ImportLearningRulesMixin:
    """Read, update, delete, and apply exact/manual import-learning rules."""

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

        selection_requested = preview_ids is not None
        selected_preview_ids = {int(pid) for pid in (preview_ids or []) if pid}
        if selection_requested and not selected_preview_ids:
            return {"selected_samples": 0, "rules_total": 0, "created": 0, "updated": 0}
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
    async def get_import_learning_rule_by_id(
        self,
        rule_id: int,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

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
    async def update_import_learning_rule(
        self,
        rule_id: int,
        user_id: int = 1,
        *,
        match_value: str | None = None,
        learned_type: str | None = None,
        learned_category_id: Any = _UNSET,
        enabled: bool | None = None,
    ) -> dict[str, Any] | None:
        """Update editable fields of a learning rule. Returns updated row or None if not found."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return None

        now = utc_now_iso()
        updates: list[str] = ["updated_at = ?"]
        params: list[Any] = [now]
        updated_match_value = str(existing["match_value"] or "")
        updated_normalized_match_value = str(existing["normalized_match_value"] or "")
        updated_match_features: dict[str, str] | None = None
        updated_composite_hash = str(existing["composite_match_hash"] or "")

        if match_value is not None:
            existing_match_type = str(existing["match_type"] or "")
            if existing_match_type == "composite":
                updated_match_features = self.parse_composite_match_value(match_value)
                if not updated_match_features:
                    raise ValueError("composite matchValue must contain at least two keyed features")
                updated_composite_hash = (
                    self.build_composite_match_hash(
                        parser_id=updated_match_features.get("parser_id", ""),
                        counterparty=updated_match_features.get("counterparty", ""),
                        description=updated_match_features.get("description", ""),
                        payment_method=updated_match_features.get("payment_method", ""),
                    )
                    or ""
                )
                updated_match_value = updated_composite_hash
                updated_normalized_match_value = updated_composite_hash
                updates.append("match_value = ?")
                params.append(updated_match_value)
                updates.append("normalized_match_value = ?")
                params.append(updated_normalized_match_value)
                updates.append("parser_id = ?")
                params.append(updated_match_features.get("parser_id", ""))
                updates.append("composite_match_hash = ?")
                params.append(updated_composite_hash)
                updates.append("match_features_json = ?")
                params.append(json.dumps(updated_match_features, ensure_ascii=False, sort_keys=True))
            else:
                updated_match_value = match_value
                updated_normalized_match_value = self._normalize_import_learning_text(match_value)
                if not updated_normalized_match_value:
                    raise ValueError("matchValue cannot be empty")
                updates.append("match_value = ?")
                params.append(updated_match_value)
                updates.append("normalized_match_value = ?")
                params.append(updated_normalized_match_value)
                updates.append("composite_match_hash = ?")
                params.append(None)
                updates.append("match_features_json = ?")
                params.append(None)
                updates.append("parser_id = ?")
                params.append(None)

        if learned_type is not None:
            updates.append("learned_type = ?")
            params.append(learned_type)

        if learned_category_id is not _UNSET:
            updates.append("learned_category_id = ?")
            params.append(learned_category_id)

        if enabled is not None:
            updates.append("enabled = ?")
            params.append(1 if enabled else 0)

        params.extend([rule_id, user_id])
        await conn.execute(
            f"UPDATE import_learning_rules SET {', '.join(updates)} WHERE id = ? AND user_id = ?",
            tuple(params),
        )
        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="updated",
            match_type=existing["match_type"],
            match_value=updated_match_value,
            normalized_match_value=updated_normalized_match_value,
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload={
                "match_value": match_value,
                "normalized_match_value": updated_normalized_match_value,
                "composite_match_hash": updated_composite_hash,
                "match_features": updated_match_features,
                "learned_type": learned_type,
                "learned_category_id": None if learned_category_id is _UNSET else learned_category_id,
                "enabled": enabled,
            },
        )
        await conn.commit()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

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
