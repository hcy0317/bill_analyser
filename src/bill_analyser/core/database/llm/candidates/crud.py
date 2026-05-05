"""LLM candidate CRUD helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-arguments,too-many-positional-arguments,too-many-locals,redefined-builtin,unused-import

import json
import sqlite3
from typing import Any

import aiosqlite

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso


class LLMCandidateCrudMixin:
    @log_method
    async def get_llm_candidates(
        self,
        user_id: int,
        status: str | None = None,
        type: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """获取 LLM 候选建议列表。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        query = "SELECT * FROM llm_candidates WHERE user_id = ?"
        params: list[Any] = [user_id]

        if status is not None:
            query += " AND status = ?"
            params.append(status)

        if type is not None:
            query += " AND type = ?"
            params.append(type)

        query += " ORDER BY created_at DESC LIMIT ? OFFSET ?"
        params.extend([limit, offset])

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_llm_candidate_by_id(
        self,
        candidate_id: int,
        user_id: int,
    ) -> dict[str, Any] | None:
        """获取单条 LLM 候选建议。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        async with conn.execute(
            "SELECT * FROM llm_candidates WHERE id = ? AND user_id = ?",
            (candidate_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_llm_candidate(
        self,
        user_id: int,
        type: str,
        source_bill_ids: list[int] | str | None,
        suggested_main_category: str | None,
        suggested_sub_category: str | None,
        suggested_rule_expression: str | None,
        confidence: float,
        llm_provider: str,
        llm_model: str,
        llm_response_raw: str | None,
    ) -> int:
        """创建 LLM 候选建议。"""
        conn = await self._get_connection()

        # Normalise source_bill_ids to JSON string
        if isinstance(source_bill_ids, list):
            source_bill_ids_str = json.dumps(source_bill_ids)
        else:
            source_bill_ids_str = source_bill_ids or ""

        try:
            cursor = await conn.execute(
                (
                    "INSERT INTO llm_candidates "
                    "(user_id, type, source_bill_ids, suggested_main_category, "
                    "suggested_sub_category, suggested_rule_expression, confidence, "
                    "llm_provider, llm_model, llm_response_raw, status) "
                    "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')"
                ),
                (
                    user_id,
                    type,
                    source_bill_ids_str,
                    suggested_main_category,
                    suggested_sub_category,
                    suggested_rule_expression,
                    confidence,
                    llm_provider,
                    llm_model,
                    llm_response_raw,
                ),
            )
            await conn.commit()
            candidate_id = cursor.lastrowid
            self.logger.info("创建 LLM 候选建议: ID=%s (user_id=%s)", candidate_id, user_id)
            return candidate_id  # type: ignore[return-value]
        except sqlite3.Error as exc:
            self.logger.error("创建 LLM 候选建议失败: %s", exc, exc_info=True)
            raise

    @log_method
    async def update_llm_candidate_status(
        self,
        candidate_id: int,
        status: str,
        user_id: int,
        reviewed_at: str | None = None,
    ) -> bool:
        """更新 LLM 候选建议状态。"""
        conn = await self._get_connection()
        if reviewed_at is None:
            reviewed_at = utc_now_iso()

        try:
            cursor = await conn.execute(
                (
                    "UPDATE llm_candidates SET status = ?, reviewed_at = ? "
                    "WHERE id = ? AND user_id = ?"
                ),
                (status, reviewed_at, candidate_id, user_id),
            )
            await conn.commit()
            if cursor.rowcount == 0:
                self.logger.warning(
                    "LLM 候选建议 ID %s 不存在或不属于 user_id=%s",
                    candidate_id,
                    user_id,
                )
                return False
            self.logger.info(
                "已更新 LLM 候选建议状态: ID=%s, status=%s, user_id=%s",
                candidate_id,
                status,
                user_id,
            )
            return True
        except sqlite3.Error as exc:
            self.logger.error("更新 LLM 候选建议状态失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def delete_llm_candidate(
        self,
        candidate_id: int,
        user_id: int,
    ) -> bool:
        """删除 LLM 候选建议。"""
        conn = await self._get_connection()
        try:
            cursor = await conn.execute(
                "DELETE FROM llm_candidates WHERE id = ? AND user_id = ?",
                (candidate_id, user_id),
            )
            await conn.commit()
            deleted = cursor.rowcount > 0
            self.logger.info(
                "删除 LLM 候选建议: ID=%s, deleted=%s, user_id=%s",
                candidate_id,
                deleted,
                user_id,
            )
            return deleted
        except sqlite3.Error as exc:
            self.logger.error("删除 LLM 候选建议失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def get_llm_candidates_count(
        self,
        user_id: int,
        status: str | None = None,
        type: str | None = None,
    ) -> int:
        """获取 LLM 候选建议总数。"""
        conn = await self._get_connection()

        query = "SELECT COUNT(*) FROM llm_candidates WHERE user_id = ?"
        params: list[Any] = [user_id]

        if status is not None:
            query += " AND status = ?"
            params.append(status)

        if type is not None:
            query += " AND type = ?"
            params.append(type)

        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
            return row[0] if row else 0
