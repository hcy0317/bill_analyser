"""LLM preview feedback payload helpers."""

from __future__ import annotations

# pylint: disable=line-too-long

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite


class LLMCandidateFeedbackMixin:
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
