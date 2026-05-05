"""LLM memory event helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-arguments,too-many-positional-arguments,too-many-locals

import sqlite3
from typing import Any

import aiosqlite

from bill_analyser.utils.logger import log_method


class LLMCandidateMemoryEventsMixin:
    @log_method
    async def create_llm_memory_event(
        self,
        user_id: int,
        *,
        conn: aiosqlite.Connection | None = None,
        session_id: str | None = None,
        preview_id: int | None = None,
        event_type: str = "recommendation",
        decision: str | None = None,
        prompt_text: str | None = None,
        llm_response_raw: str | None = None,
        llm_provider: str | None = None,
        llm_model: str | None = None,
        suggested_main_category: str | None = None,
        suggested_sub_category: str | None = None,
        suggested_source_account: str | None = None,
        suggested_destination_account: str | None = None,
        confidence: float = 0.0,
        user_correction_category: str | None = None,
        user_correction_account: str | None = None,
        snapshot_before: str | None = None,
        snapshot_after: str | None = None,
        metadata: str | None = None,
    ) -> int:
        """Append an event to the LLM memory ledger (append-only)."""
        target_conn = conn or await self._get_connection()
        try:
            cursor = await target_conn.execute(
                (
                    "INSERT INTO llm_memory_events "
                    "(user_id, session_id, preview_id, event_type, decision, "
                    "prompt_text, llm_response_raw, llm_provider, llm_model, "
                    "suggested_main_category, suggested_sub_category, "
                    "suggested_source_account, suggested_destination_account, "
                    "confidence, user_correction_category, user_correction_account, "
                    "snapshot_before, snapshot_after, metadata) "
                    "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                ),
                (
                    user_id,
                    session_id,
                    preview_id,
                    event_type,
                    decision,
                    prompt_text,
                    llm_response_raw,
                    llm_provider,
                    llm_model,
                    suggested_main_category,
                    suggested_sub_category,
                    suggested_source_account,
                    suggested_destination_account,
                    confidence,
                    user_correction_category,
                    user_correction_account,
                    snapshot_before,
                    snapshot_after,
                    metadata,
                ),
            )
            if conn is None:
                await target_conn.commit()
            event_id = cursor.lastrowid
            self.logger.info(
                "创建 LLM memory event: ID=%s (user_id=%s, type=%s, decision=%s)",
                event_id, user_id, event_type, decision,
            )
            return event_id  # type: ignore[return-value]
        except sqlite3.Error as exc:
            self.logger.error("创建 LLM memory event 失败: %s", exc, exc_info=True)
            raise

    @log_method
    async def get_llm_memory_events(
        self,
        user_id: int,
        *,
        session_id: str | None = None,
        event_type: str | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """Query LLM memory events for auditing or future prompt context."""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        query = "SELECT * FROM llm_memory_events WHERE user_id = ?"
        params: list[Any] = [user_id]

        if session_id is not None:
            query += " AND session_id = ?"
            params.append(session_id)
        if event_type is not None:
            query += " AND event_type = ?"
            params.append(event_type)

        query += " ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
        params.extend([limit, offset])

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_llm_memory_events_count(
        self,
        user_id: int,
        *,
        session_id: str | None = None,
        event_type: str | None = None,
    ) -> int:
        """Count LLM memory events."""
        conn = await self._get_connection()
        query = "SELECT COUNT(*) FROM llm_memory_events WHERE user_id = ?"
        params: list[Any] = [user_id]
        if session_id is not None:
            query += " AND session_id = ?"
            params.append(session_id)
        if event_type is not None:
            query += " AND event_type = ?"
            params.append(event_type)
        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
            return row[0] if row else 0
