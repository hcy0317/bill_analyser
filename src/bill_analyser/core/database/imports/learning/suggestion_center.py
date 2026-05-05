"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught,too-many-branches,too-many-statements

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso

_UNSET: Any = object()


class ImportLearningSuggestionCenterMixin:
    """Mine and apply cross-session import-learning suggestions."""

    @staticmethod
    def _empty_learning_suggestion_mining_result() -> dict[str, int]:
        return {
            "total_annotations": 0,
            "mined": 0,
            "created": 0,
            "updated": 0,
            "skipped_conflict": 0,
            "skipped_existing": 0,
        }

    async def _get_existing_composite_rule_hashes(self, user_id: int) -> dict[str, int]:
        existing_rules = await self.get_import_learning_rules(user_id=user_id, enabled_only=False, limit=None)
        existing_rule_hashes: dict[str, int] = {}
        for rule in existing_rules:
            if str(rule.get("match_type") or "") == "composite" and rule.get("normalized_match_value"):
                existing_rule_hashes[str(rule["normalized_match_value"])] = int(rule["id"])
        return existing_rule_hashes

    def _resolve_suggestion_match(
        self,
        annotation: dict[str, Any],
    ) -> tuple[str, dict[str, str] | None]:
        composite_hash = str(annotation.get("composite_match_hash") or "")
        match_features: dict[str, str] | None = None
        try:
            parsed_features = json.loads(annotation.get("match_features_json") or "{}")
            if isinstance(parsed_features, dict):
                match_features = {
                    str(key): self._normalize_import_learning_text(value)
                    for key, value in parsed_features.items()
                    if self._normalize_import_learning_text(value)
                }
        except TypeError, ValueError, json.JSONDecodeError:
            match_features = None
        if not composite_hash:
            composite_hash = (
                self.build_composite_match_hash(
                    parser_id=annotation.get("parser_id", ""),
                    counterparty=annotation.get("counterparty", ""),
                    description=annotation.get("description", ""),
                    payment_method=annotation.get("payment_method", ""),
                )
                or ""
            )
        if not match_features:
            match_features = self.build_composite_match_features(
                parser_id=annotation.get("parser_id", ""),
                counterparty=annotation.get("counterparty", ""),
                description=annotation.get("description", ""),
                payment_method=annotation.get("payment_method", ""),
            )
        return composite_hash, match_features

    @staticmethod
    def _new_learning_suggestion_candidate(
        annotation: dict[str, Any],
        composite_hash: str,
        match_features: dict[str, str],
        signature: tuple[Any, ...],
        existing_rule_id: int | None,
    ) -> dict[str, Any]:
        return {
            "match_type": "composite",
            "match_value": composite_hash,
            "normalized_match_value": composite_hash,
            "composite_match_hash": composite_hash,
            "match_features": match_features,
            "suggested_type": signature[0],
            "suggested_category_id": signature[1],
            "suggested_source_account_id": signature[2],
            "suggested_destination_account_id": signature[3],
            "sample_count": 1,
            "source_session_ids": {str(annotation.get("session_id", ""))},
            "source_preview_ids": {int(annotation.get("preview_id", 0))},
            "signature": signature,
            "existing_rule_id": existing_rule_id,
        }

    def _collect_learning_suggestion_candidates(
        self,
        all_annotations: list[dict[str, Any]],
        existing_rule_hashes: dict[str, int],
    ) -> tuple[dict[str, dict[str, Any]], set[str]]:
        candidates: dict[str, dict[str, Any]] = {}
        conflicted_hashes: set[str] = set()
        for annotation in all_annotations:
            composite_hash, match_features = self._resolve_suggestion_match(annotation)
            if not composite_hash or not match_features or composite_hash in conflicted_hashes:
                continue

            signature = self._build_import_learning_suggestion_signature(annotation, annotation)
            existing_candidate = candidates.get(composite_hash)
            if existing_candidate is None:
                candidates[composite_hash] = self._new_learning_suggestion_candidate(
                    annotation,
                    composite_hash,
                    match_features,
                    signature,
                    existing_rule_hashes.get(composite_hash),
                )
                continue

            if existing_candidate["signature"] != signature:
                conflicted_hashes.add(composite_hash)
                candidates.pop(composite_hash, None)
                continue

            existing_candidate["sample_count"] += 1
            existing_candidate["source_session_ids"].add(str(annotation.get("session_id", "")))
            existing_candidate["source_preview_ids"].add(int(annotation.get("preview_id", 0)))
        return candidates, conflicted_hashes

    @staticmethod
    def _summarize_learning_suggestion(candidate: dict[str, Any]) -> str:
        features_summary_parts = []
        for key, label in [("counterparty", "交易方"), ("description", "描述"), ("payment_method", "支付方式")]:
            value = candidate.get("match_features", {}).get(key, "")
            if value:
                features_summary_parts.append(f"{label}: {value}")
        return " | ".join(features_summary_parts) if features_summary_parts else candidate["match_value"]

    @log_method
    async def mine_learning_suggestions(self, user_id: int = 1) -> dict[str, int]:
        """从 durable corpus 中挖掘学习建议，写入 import_learning_suggestions。

        Returns:
            dict with keys: total_annotations, mined, created, updated, skipped_conflict, skipped_existing
        """
        conn = await self._get_connection()

        async with conn.execute(
            """
                SELECT *
                FROM import_learning_corpus_samples
                WHERE user_id = ?
                ORDER BY session_id, updated_at ASC, id ASC
                """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        all_annotations = [dict(row) for row in rows]
        if not all_annotations:
            return self._empty_learning_suggestion_mining_result()

        existing_rule_hashes = await self._get_existing_composite_rule_hashes(user_id)
        candidates, conflicted_hashes = self._collect_learning_suggestion_candidates(
            all_annotations,
            existing_rule_hashes,
        )

        now = utc_now_iso()
        created_count = 0
        updated_count = 0
        skipped_existing = 0

        for candidate in candidates.values():
            if candidate.get("existing_rule_id"):
                skipped_existing += 1
                continue

            session_ids = sorted(candidate.pop("source_session_ids", set()))
            preview_ids = sorted(candidate.pop("source_preview_ids", set()))
            candidate.pop("signature", None)

            summary = self._summarize_learning_suggestion(candidate)

            await conn.execute(
                """
                    INSERT INTO import_learning_suggestions (
                        user_id, match_type, match_value, normalized_match_value,
                        composite_match_hash, match_features_json,
                        suggested_type, suggested_category_id,
                        suggested_source_account_id, suggested_destination_account_id,
                        sample_count, source_session_ids_json, source_preview_ids_json,
                        status, existing_rule_id, summary,
                        created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?)
                    ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                        suggested_type = excluded.suggested_type,
                        suggested_category_id = excluded.suggested_category_id,
                        suggested_source_account_id = excluded.suggested_source_account_id,
                        suggested_destination_account_id = excluded.suggested_destination_account_id,
                        sample_count = excluded.sample_count,
                        source_session_ids_json = excluded.source_session_ids_json,
                        source_preview_ids_json = excluded.source_preview_ids_json,
                        summary = excluded.summary,
                        updated_at = excluded.updated_at
                    """,
                (
                    user_id,
                    candidate["match_type"],
                    candidate["match_value"],
                    candidate["normalized_match_value"],
                    candidate["composite_match_hash"],
                    json.dumps(candidate.get("match_features") or {}, ensure_ascii=False, sort_keys=True),
                    candidate.get("suggested_type"),
                    candidate.get("suggested_category_id"),
                    candidate.get("suggested_source_account_id"),
                    candidate.get("suggested_destination_account_id"),
                    candidate["sample_count"],
                    json.dumps(session_ids, ensure_ascii=False),
                    json.dumps(preview_ids, ensure_ascii=False),
                    candidate.get("existing_rule_id"),
                    summary,
                    now,
                    now,
                ),
            )

            async with conn.execute("SELECT changes()") as cur:
                row = await cur.fetchone()
            changed = int(row[0]) if row else 0
            if changed > 0:
                async with conn.execute(
                    """
                        SELECT id FROM import_learning_suggestions
                        WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                        LIMIT 1
                        """,
                    (user_id, candidate["match_type"], candidate["normalized_match_value"]),
                ) as cur:
                    saved = await cur.fetchone()
                if saved:
                    async with conn.execute(
                        "SELECT created_at FROM import_learning_suggestions WHERE id = ?",
                        (saved["id"],),
                    ) as cur:
                        check = await cur.fetchone()
                    if check and str(check["created_at"]) == now:
                        created_count += 1
                    else:
                        updated_count += 1

        await conn.commit()
        return {
            "total_annotations": len(all_annotations),
            "mined": len(candidates),
            "created": created_count,
            "updated": updated_count,
            "skipped_conflict": len(conflicted_hashes),
            "skipped_existing": skipped_existing,
        }

    @log_method
    async def get_learning_suggestions(
        self,
        user_id: int = 1,
        status: str | None = None,
        limit: int | None = 200,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_suggestions WHERE user_id = ?"
        params: list[Any] = [user_id]
        if status:
            query += " AND status = ?"
            params.append(status)
        query += " ORDER BY sample_count DESC, updated_at DESC, id DESC"
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])
        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def count_learning_suggestions(self, user_id: int = 1, status: str | None = None) -> int:
        conn = await self._get_connection()
        query = "SELECT COUNT(*) AS total_count FROM import_learning_suggestions WHERE user_id = ?"
        params: list[Any] = [user_id]
        if status:
            query += " AND status = ?"
            params.append(status)
        async with conn.execute(query, tuple(params)) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)

    @log_method
    async def accept_learning_suggestion(self, suggestion_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """接受建议：将建议提升为 import_learning_rules 并标记 accepted。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_suggestions WHERE id = ? AND user_id = ? LIMIT 1",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        if not row:
            return None
        suggestion = dict(row)
        if suggestion["status"] != "pending":
            return {"error": "suggestion_not_pending", "current_status": suggestion["status"]}

        now = utc_now_iso()
        features = {}
        try:
            parsed = json.loads(suggestion.get("match_features_json") or "{}")
            if isinstance(parsed, dict):
                features = parsed
        except json.JSONDecodeError, TypeError:
            pass

        await conn.execute(
            """
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id,
                    learned_source_account_id, learned_destination_account_id,
                    enabled, parser_id, composite_match_hash, match_features_json,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    enabled = 1,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                """,
            (
                user_id,
                suggestion["match_type"],
                suggestion["match_value"],
                suggestion["normalized_match_value"],
                suggestion.get("suggested_type"),
                suggestion.get("suggested_category_id"),
                suggestion.get("suggested_source_account_id"),
                suggestion.get("suggested_destination_account_id"),
                features.get("parser_id", ""),
                suggestion.get("composite_match_hash"),
                suggestion.get("match_features_json"),
                now,
                now,
            ),
        )

        async with conn.execute(
            "SELECT id FROM import_learning_rules WHERE user_id = ? AND match_type = ? AND normalized_match_value = ? LIMIT 1",
            (user_id, suggestion["match_type"], suggestion["normalized_match_value"]),
        ) as cursor:
            rule_row = await cursor.fetchone()
        rule_id = int(rule_row["id"]) if rule_row else None

        await conn.execute(
            "UPDATE import_learning_suggestions SET status = 'accepted', existing_rule_id = ?, updated_at = ? WHERE id = ?",
            (rule_id, now, suggestion_id),
        )

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="created_from_suggestion",
            match_type=suggestion["match_type"],
            match_value=suggestion["match_value"],
            normalized_match_value=suggestion["normalized_match_value"],
            payload={"suggestion_id": suggestion_id},
        )
        await self.record_import_learning_feedback_event(
            "suggestion_accept",
            user_id=user_id,
            rule_id=rule_id,
            suggestion_id=suggestion_id,
            payload={"status": "accepted"},
            conn=conn,
        )

        await conn.commit()
        return {"suggestion_id": suggestion_id, "rule_id": rule_id, "status": "accepted"}

    @log_method
    async def reject_learning_suggestion(self, suggestion_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT id, status FROM import_learning_suggestions WHERE id = ? AND user_id = ? LIMIT 1",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        if not row:
            return False
        if row["status"] != "pending":
            return False

        now = utc_now_iso()
        await conn.execute(
            "UPDATE import_learning_suggestions SET status = 'rejected', updated_at = ? WHERE id = ?",
            (now, suggestion_id),
        )
        await self.record_import_learning_feedback_event(
            "suggestion_reject",
            user_id=user_id,
            suggestion_id=suggestion_id,
            payload={"status": "rejected"},
            conn=conn,
        )
        await conn.commit()
        return True
