"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso

_UNSET: Any = object()


class ImportLearningCorpusMixin:
    """Persist session annotations into durable import-learning corpus rows."""

    async def _get_import_learning_corpus_preview(
        self,
        conn: aiosqlite.Connection,
        *,
        session_id: str,
        preview_id: int,
        user_id: int,
    ) -> dict[str, Any] | None:
        async with conn.execute(
            """
                SELECT id, session_id, user_id, preview_parser_id,
                       preview_counterparty, preview_description,
                       preview_payment_method, preview_type,
                       preview_main_category, preview_sub_category,
                       preview_source_account_id, preview_destination_account_id
                FROM bills_preview
                WHERE id = ? AND session_id = ? AND user_id = ?
                LIMIT 1
                """,
            (preview_id, session_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _upsert_import_learning_corpus_sample(
        self,
        conn: aiosqlite.Connection,
        *,
        session_id: str,
        user_id: int,
        preview_id: int,
        sample: dict[str, Any],
        now: str,
    ) -> bool:
        preview = await self._get_import_learning_corpus_preview(
            conn,
            session_id=session_id,
            preview_id=preview_id,
            user_id=user_id,
        )
        if not preview:
            return False

        parser_id = str(preview.get("preview_parser_id") or "")
        counterparty = str(preview.get("preview_counterparty") or "")
        description = str(preview.get("preview_description") or "")
        payment_method = str(preview.get("preview_payment_method") or "")
        composite_hash = self.build_composite_match_hash(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        match_features = self.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        await conn.execute(
            """
                INSERT INTO import_learning_corpus_samples (
                    user_id, session_id, preview_id,
                    parser_id, counterparty, description, payment_method,
                    composite_match_hash, match_features_json,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    source_snapshot_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, session_id, preview_id) DO UPDATE SET
                    parser_id = excluded.parser_id,
                    counterparty = excluded.counterparty,
                    description = excluded.description,
                    payment_method = excluded.payment_method,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    source_snapshot_json = excluded.source_snapshot_json,
                    updated_at = excluded.updated_at
                """,
            (
                user_id,
                session_id,
                preview_id,
                parser_id,
                counterparty,
                description,
                payment_method,
                composite_hash,
                json.dumps(match_features or {}, ensure_ascii=False, sort_keys=True),
                sample.get("preview_type") or sample.get("annotated_type"),
                sample.get("category_id") or sample.get("annotated_category_id"),
                sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                json.dumps(preview, ensure_ascii=False, sort_keys=True),
                now,
                now,
            ),
        )
        return True

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
            normalized_preview_id = int(preview_id)
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
                    normalized_preview_id,
                    sample.get("preview_type") or sample.get("annotated_type"),
                    sample.get("category_id") or sample.get("annotated_category_id"),
                    sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                    sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                    now,
                    now,
                ),
            )
            await self._upsert_import_learning_corpus_sample(
                conn,
                session_id=session_id,
                user_id=user_id,
                preview_id=normalized_preview_id,
                sample=sample,
                now=now,
            )
            saved_count += 1

        await conn.commit()
        if saved_count > 0:
            await self.refresh_import_learning_model(user_id=user_id)
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
    async def get_import_learning_corpus_samples(
        self,
        user_id: int = 1,
        *,
        limit: int | None = None,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_corpus_samples WHERE user_id = ? ORDER BY updated_at DESC, id DESC"
        params: list[Any] = [user_id]
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])
        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def count_import_learning_corpus_samples(self, user_id: int = 1) -> int:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT COUNT(*) AS total_count FROM import_learning_corpus_samples WHERE user_id = ?",
            (user_id,),
        ) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)
