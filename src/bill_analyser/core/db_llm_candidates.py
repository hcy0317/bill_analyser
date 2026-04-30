"""LLM candidates persistence helpers for the split database facade."""
# pylint: disable=too-many-arguments,too-many-positional-arguments,too-many-locals,redefined-builtin

from __future__ import annotations

import json
import sqlite3
from typing import Any

import aiosqlite

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseLLMCandidatesMixin(DatabaseFacadeBase):
    """LLM candidate CRUD helpers for classification and rule induction suggestions."""

    _LLM_PREVIEW_SNAPSHOT_FIELDS: tuple[tuple[str, Any], ...] = (
        ("preview_main_category", ""),
        ("preview_sub_category", ""),
        ("preview_source_account_id", None),
        ("preview_destination_account_id", None),
    )

    @staticmethod
    def _deserialize_preview_matching_feedback(raw_payload: Any) -> dict[str, Any]:
        if isinstance(raw_payload, dict):
            return dict(raw_payload)
        if raw_payload in (None, ""):
            return {}
        try:
            payload = json.loads(str(raw_payload))
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}
        return dict(payload) if isinstance(payload, dict) else {}

    @staticmethod
    def _serialize_preview_matching_feedback(payload: dict[str, Any]) -> str:
        if not payload:
            return ""
        return json.dumps(payload, ensure_ascii=False, sort_keys=True)

    @classmethod
    def _clear_matching_feedback_key(cls, raw_payload: Any, key: str) -> str:
        feedback_payload = cls._deserialize_preview_matching_feedback(raw_payload)
        feedback_payload.pop(str(key), None)
        return cls._serialize_preview_matching_feedback(feedback_payload)

    @classmethod
    def _clear_transfer_matching_feedback(cls, raw_payload: Any) -> str:
        return cls._clear_matching_feedback_key(raw_payload, "transfer")

    @staticmethod
    def _normalize_optional_account_id(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized_value = int(raw_value)
        except (TypeError, ValueError):
            return None
        return normalized_value if normalized_value > 0 else None

    @classmethod
    def _build_llm_previous_preview_snapshot(cls, preview: dict[str, Any]) -> dict[str, Any]:
        snapshot: dict[str, Any] = {}
        for field, default_value in cls._LLM_PREVIEW_SNAPSHOT_FIELDS:
            value = preview.get(field, default_value)
            if field in {"preview_source_account_id", "preview_destination_account_id"}:
                snapshot[field] = cls._normalize_optional_account_id(value)
            else:
                snapshot[field] = default_value if value is None else str(value)
        return snapshot

    @classmethod
    def _normalize_llm_previous_preview_snapshot(cls, raw_payload: Any) -> dict[str, Any]:
        if not isinstance(raw_payload, dict):
            return {}
        snapshot: dict[str, Any] = {}
        for field, default_value in cls._LLM_PREVIEW_SNAPSHOT_FIELDS:
            value = raw_payload.get(field, default_value)
            if field in {"preview_source_account_id", "preview_destination_account_id"}:
                snapshot[field] = cls._normalize_optional_account_id(value)
            else:
                snapshot[field] = default_value if value is None else str(value)
        return snapshot

    @classmethod
    def _append_llm_snapshot_restore_updates(
        cls,
        snapshot: dict[str, Any],
        update_parts: list[str],
        params: list[Any],
    ) -> None:
        normalized_snapshot = cls._normalize_llm_previous_preview_snapshot(snapshot)
        for field, default_value in cls._LLM_PREVIEW_SNAPSHOT_FIELDS:
            update_parts.append(f"{field} = ?")
            value = normalized_snapshot.get(field, default_value)
            if field in {"preview_source_account_id", "preview_destination_account_id"}:
                params.append(cls._normalize_optional_account_id(value))
            else:
                params.append(default_value if value is None else value)

    @classmethod
    def _llm_preview_matches_snapshot(cls, preview: dict[str, Any], snapshot: dict[str, Any]) -> bool:
        normalized_snapshot = cls._normalize_llm_previous_preview_snapshot(snapshot)
        return (
            str(preview.get("preview_main_category") or "") == str(normalized_snapshot.get("preview_main_category") or "")
            and str(preview.get("preview_sub_category") or "") == str(normalized_snapshot.get("preview_sub_category") or "")
            and cls._normalize_optional_account_id(preview.get("preview_source_account_id"))
            == cls._normalize_optional_account_id(normalized_snapshot.get("preview_source_account_id"))
            and cls._normalize_optional_account_id(preview.get("preview_destination_account_id"))
            == cls._normalize_optional_account_id(normalized_snapshot.get("preview_destination_account_id"))
        )

    async def _resolve_account_id_by_name(
        self,
        conn: aiosqlite.Connection,
        *,
        user_id: int,
        account_name: str | None,
    ) -> int | None:
        normalized_account_name = str(account_name or "").strip()
        if not normalized_account_name:
            return None

        async with conn.execute(
            (
                "SELECT id FROM accounts "
                "WHERE user_id = ? AND TRIM(name) = ? "
                "ORDER BY hidden ASC, display_order ASC, id ASC LIMIT 1"
            ),
            (user_id, normalized_account_name),
        ) as cursor:
            row = await cursor.fetchone()
        return self._normalize_optional_account_id(row[0] if row else None)

    def _build_llm_feedback_payload(
        self,
        suggestion: dict[str, Any],
        *,
        review_status: str,
        suppressed: bool,
        previous_preview: dict[str, Any],
        applied_preview: dict[str, Any],
    ) -> dict[str, Any]:
        return {
            "review_status": review_status,
            "suppressed": suppressed,
            "previous_preview": previous_preview,
            "applied_preview": applied_preview,
            "suggested_main_category": str(suggestion.get("suggested_main_category") or ""),
            "suggested_sub_category": str(suggestion.get("suggested_sub_category") or ""),
            "suggested_source_account": str(suggestion.get("suggested_source_account") or ""),
            "suggested_destination_account": str(suggestion.get("suggested_destination_account") or ""),
            "confidence": float(suggestion.get("confidence", 0.0) or 0.0),
            "reason": str(suggestion.get("reason") or ""),
        }

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
    ) -> int:
        """获取 LLM 候选建议总数。"""
        conn = await self._get_connection()

        query = "SELECT COUNT(*) FROM llm_candidates WHERE user_id = ?"
        params: list[Any] = [user_id]

        if status is not None:
            query += " AND status = ?"
            params.append(status)

        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
            return row[0] if row else 0

    # ------------------------------------------------------------------
    # LLM Memory Events
    # ------------------------------------------------------------------

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
    async def apply_preview_llm_recommendation(
        self,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        suggestion: dict[str, Any],
        prompt_text: str | None,
        llm_provider: str | None,
        llm_model: str | None,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT * FROM bills_preview WHERE id = ? AND user_id = ? AND session_id = ?",
                (preview_id, user_id, session_id),
            ) as cursor:
                row = await cursor.fetchone()

            if not row:
                await conn.rollback()
                return None

            preview = dict(row)
            previous_preview_snapshot = self._build_llm_previous_preview_snapshot(preview)
            feedback_payload = self._deserialize_preview_matching_feedback(
                preview.get("preview_matching_feedback_json")
            )

            current_main_category = str(preview.get("preview_main_category") or "").strip()
            current_sub_category = str(preview.get("preview_sub_category") or "").strip()
            current_source_account_id = self._normalize_optional_account_id(
                preview.get("preview_source_account_id")
            )
            current_destination_account_id = self._normalize_optional_account_id(
                preview.get("preview_destination_account_id")
            )

            suggested_main_category = str(suggestion.get("suggested_main_category") or "").strip()
            suggested_sub_category = str(suggestion.get("suggested_sub_category") or "").strip()
            suggested_source_account = str(suggestion.get("suggested_source_account") or "").strip()
            suggested_destination_account = str(suggestion.get("suggested_destination_account") or "").strip()

            resolved_source_account_id = await self._resolve_account_id_by_name(
                conn,
                user_id=user_id,
                account_name=suggested_source_account,
            )
            resolved_destination_account_id = await self._resolve_account_id_by_name(
                conn,
                user_id=user_id,
                account_name=suggested_destination_account,
            )

            next_main_category = current_main_category
            next_sub_category = current_sub_category
            if not current_main_category and not current_sub_category and suggested_main_category:
                next_main_category = suggested_main_category
                next_sub_category = suggested_sub_category

            next_source_account_id = current_source_account_id
            if current_source_account_id is None and resolved_source_account_id is not None:
                next_source_account_id = resolved_source_account_id

            next_destination_account_id = current_destination_account_id
            if current_destination_account_id is None and resolved_destination_account_id is not None:
                next_destination_account_id = resolved_destination_account_id

            applied_preview_snapshot = {
                "preview_main_category": next_main_category,
                "preview_sub_category": next_sub_category,
                "preview_source_account_id": next_source_account_id,
                "preview_destination_account_id": next_destination_account_id,
            }

            update_parts: list[str] = []
            params: list[Any] = []
            applied_fields: list[str] = []

            if next_main_category != current_main_category:
                update_parts.append("preview_main_category = ?")
                params.append(next_main_category)
                applied_fields.append("preview_main_category")
            if next_sub_category != current_sub_category:
                update_parts.append("preview_sub_category = ?")
                params.append(next_sub_category)
                applied_fields.append("preview_sub_category")
            if next_source_account_id != current_source_account_id:
                update_parts.append("preview_source_account_id = ?")
                params.append(next_source_account_id)
                applied_fields.append("preview_source_account_id")
            if next_destination_account_id != current_destination_account_id:
                update_parts.append("preview_destination_account_id = ?")
                params.append(next_destination_account_id)
                applied_fields.append("preview_destination_account_id")

            next_feedback_json = preview.get("preview_matching_feedback_json")
            if applied_fields:
                next_feedback_json = self._clear_transfer_matching_feedback(next_feedback_json)

            feedback_payload = self._deserialize_preview_matching_feedback(next_feedback_json)
            feedback_payload["llm"] = self._build_llm_feedback_payload(
                suggestion,
                review_status="pending",
                suppressed=False,
                previous_preview=previous_preview_snapshot,
                applied_preview=applied_preview_snapshot,
            )
            update_parts.append("preview_matching_feedback_json = ?")
            params.append(self._serialize_preview_matching_feedback(feedback_payload))
            params.extend([preview_id, user_id, session_id])

            cursor = await conn.execute(
                (
                    f"UPDATE bills_preview SET {', '.join(update_parts)} "
                    "WHERE id = ? AND user_id = ? AND session_id = ?"
                ),
                tuple(params),
            )
            if int(cursor.rowcount or 0) < 1:
                await conn.rollback()
                return None

            event_id = await self.create_llm_memory_event(
                user_id=user_id,
                conn=conn,
                session_id=session_id,
                preview_id=preview_id,
                event_type="recommendation",
                decision=None,
                prompt_text=prompt_text,
                llm_response_raw=json.dumps(suggestion, ensure_ascii=False),
                llm_provider=llm_provider,
                llm_model=llm_model,
                suggested_main_category=suggested_main_category or None,
                suggested_sub_category=suggested_sub_category or None,
                suggested_source_account=suggested_source_account or None,
                suggested_destination_account=suggested_destination_account or None,
                confidence=float(suggestion.get("confidence", 0.0) or 0.0),
                snapshot_before=json.dumps(previous_preview_snapshot, ensure_ascii=False),
                snapshot_after=json.dumps(applied_preview_snapshot, ensure_ascii=False),
                metadata=json.dumps(
                    {
                        "reason": str(suggestion.get("reason") or ""),
                        "counterparty": str(preview.get("preview_counterparty") or ""),
                        "payment_method": str(preview.get("preview_payment_method") or ""),
                        "description": str(preview.get("preview_description") or ""),
                        "applied_fields": applied_fields,
                        "resolved_source_account_id": resolved_source_account_id,
                        "resolved_destination_account_id": resolved_destination_account_id,
                    },
                    ensure_ascii=False,
                ),
            )

            await conn.commit()
        except Exception:
            await conn.rollback()
            raise

        updated_preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        return {
            "event_id": event_id,
            "applied_fields": applied_fields,
            "preview": updated_preview,
            "llm": feedback_payload.get("llm"),
        }

    @log_method
    async def review_preview_llm_recommendation(
        self,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        decision: str,
        suggestion: dict[str, Any] | None = None,
        user_correction: dict[str, Any] | None = None,
    ) -> dict[str, Any] | None:
        normalized_decision = str(decision or "").strip().lower()
        if normalized_decision not in {"accept", "reject"}:
            return None

        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT * FROM bills_preview WHERE id = ? AND user_id = ? AND session_id = ?",
                (preview_id, user_id, session_id),
            ) as cursor:
                row = await cursor.fetchone()

            if not row:
                await conn.rollback()
                return None

            preview = dict(row)
            feedback_payload = self._deserialize_preview_matching_feedback(
                preview.get("preview_matching_feedback_json")
            )
            llm_feedback = feedback_payload.get("llm") if isinstance(feedback_payload, dict) else {}
            if not isinstance(llm_feedback, dict):
                llm_feedback = {}

            previous_preview_snapshot = self._normalize_llm_previous_preview_snapshot(
                llm_feedback.get("previous_preview")
            )
            applied_preview_snapshot = self._normalize_llm_previous_preview_snapshot(
                llm_feedback.get("applied_preview")
            )
            should_restore_previous_preview = (
                normalized_decision == "reject"
                and bool(previous_preview_snapshot)
                and (
                    not applied_preview_snapshot
                    or self._llm_preview_matches_snapshot(preview, applied_preview_snapshot)
                )
            )

            resolved_suggestion = dict(suggestion or {})
            if not resolved_suggestion:
                resolved_suggestion = {
                    "suggested_main_category": llm_feedback.get("suggested_main_category"),
                    "suggested_sub_category": llm_feedback.get("suggested_sub_category"),
                    "suggested_source_account": llm_feedback.get("suggested_source_account"),
                    "suggested_destination_account": llm_feedback.get("suggested_destination_account"),
                    "confidence": llm_feedback.get("confidence"),
                    "reason": llm_feedback.get("reason"),
                }

            update_parts: list[str] = []
            params: list[Any] = []
            if should_restore_previous_preview:
                self._append_llm_snapshot_restore_updates(
                    previous_preview_snapshot,
                    update_parts,
                    params,
                )

            current_preview_snapshot = self._build_llm_previous_preview_snapshot(preview)
            next_llm_feedback = self._build_llm_feedback_payload(
                resolved_suggestion,
                review_status="accepted" if normalized_decision == "accept" else "rejected",
                suppressed=normalized_decision == "reject",
                previous_preview=previous_preview_snapshot or current_preview_snapshot,
                applied_preview=applied_preview_snapshot or current_preview_snapshot,
            )
            feedback_payload["llm"] = next_llm_feedback
            update_parts.append("preview_matching_feedback_json = ?")
            params.append(self._serialize_preview_matching_feedback(feedback_payload))
            params.extend([preview_id, user_id, session_id])

            cursor = await conn.execute(
                (
                    f"UPDATE bills_preview SET {', '.join(update_parts)} "
                    "WHERE id = ? AND user_id = ? AND session_id = ?"
                ),
                tuple(params),
            )
            if int(cursor.rowcount or 0) < 1:
                await conn.rollback()
                return None

            refreshed_snapshot = (
                previous_preview_snapshot if should_restore_previous_preview else applied_preview_snapshot or current_preview_snapshot
            )
            event_id = await self.create_llm_memory_event(
                user_id=user_id,
                conn=conn,
                session_id=session_id,
                preview_id=preview_id,
                event_type="feedback",
                decision=normalized_decision,
                llm_response_raw=json.dumps(resolved_suggestion, ensure_ascii=False),
                llm_provider=None,
                llm_model=None,
                suggested_main_category=str(resolved_suggestion.get("suggested_main_category") or "") or None,
                suggested_sub_category=str(resolved_suggestion.get("suggested_sub_category") or "") or None,
                suggested_source_account=str(resolved_suggestion.get("suggested_source_account") or "") or None,
                suggested_destination_account=str(resolved_suggestion.get("suggested_destination_account") or "") or None,
                confidence=float(resolved_suggestion.get("confidence", 0.0) or 0.0),
                user_correction_category=(
                    str(user_correction.get("category") or "") or None
                    if isinstance(user_correction, dict)
                    else None
                ),
                user_correction_account=(
                    str(user_correction.get("account") or "") or None
                    if isinstance(user_correction, dict)
                    else None
                ),
                snapshot_before=json.dumps(current_preview_snapshot, ensure_ascii=False),
                snapshot_after=json.dumps(refreshed_snapshot, ensure_ascii=False),
                metadata=json.dumps(
                    {
                        "reason": str(resolved_suggestion.get("reason") or ""),
                        "rollback": bool(should_restore_previous_preview),
                    },
                    ensure_ascii=False,
                ),
            )

            await conn.commit()
        except Exception:
            await conn.rollback()
            raise

        updated_preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        return {
            "event_id": event_id,
            "restored": should_restore_previous_preview,
            "preview": updated_preview,
            "llm": feedback_payload.get("llm"),
        }

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
