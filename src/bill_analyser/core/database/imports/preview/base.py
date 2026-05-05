"""Import-preview editing, confirmation, and temporary staging helpers."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-positional-arguments,too-many-locals,broad-exception-caught,assignment-from-no-return,too-many-nested-blocks,too-many-lines

from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from datetime import date

from bill_analyser.import_contracts.parser_tags import resolve_parser_tags, serialize_parser_tags
from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import normalize_bill_date_text
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class ImportPreviewBaseMixin(object):
        _SERVER_PAGED_PREVIEW_SORT_FIELDS: dict[str, str] = {
            "time": "preview_date",
            "type": "preview_type",
            "sourceAmount": "preview_amount",
            "counterparty": "preview_counterparty",
            "paymentMethod": "preview_payment_method",
            "comment": "preview_description",
        }

        _TRANSFER_PREVIEW_SNAPSHOT_FIELDS: tuple[tuple[str, Any], ...] = (
            ("preview_type", ""),
            ("preview_main_category", ""),
            ("preview_sub_category", ""),
            ("preview_recurring_id", None),
            ("preview_recurring_name", ""),
            ("preview_recurring_candidate_count", 0),
            ("preview_recurring_match_score", 0),
            ("preview_recurring_match_reasons", ""),
            ("preview_recurring_matched_date", ""),
        )

        _LEARNING_PREVIEW_SNAPSHOT_FIELDS: tuple[tuple[str, Any], ...] = (
            ("preview_type", ""),
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
        def _preview_state_matches_snapshot(preview: dict[str, Any], expected_state: dict[str, Any] | None) -> bool:
            if not expected_state:
                return True

            current_recurring_id = preview.get("preview_recurring_id")
            normalized_current_recurring_id = None if current_recurring_id in (None, "") else int(current_recurring_id)
            expected_recurring_id = expected_state.get("preview_recurring_id")
            normalized_expected_recurring_id = None if expected_recurring_id in (None, "") else int(expected_recurring_id)
            current_source_account_id = preview.get("preview_source_account_id")
            normalized_current_source_account_id = (
                None if current_source_account_id in (None, "", 0, "0") else int(current_source_account_id)
            )
            expected_source_account_id = expected_state.get("preview_source_account_id")
            normalized_expected_source_account_id = (
                None if expected_source_account_id in (None, "", 0, "0") else int(expected_source_account_id)
            )
            current_destination_account_id = preview.get("preview_destination_account_id")
            normalized_current_destination_account_id = (
                None if current_destination_account_id in (None, "", 0, "0") else int(current_destination_account_id)
            )
            expected_destination_account_id = expected_state.get("preview_destination_account_id")
            normalized_expected_destination_account_id = (
                None if expected_destination_account_id in (None, "", 0, "0") else int(expected_destination_account_id)
            )
            compare_source_account = "preview_source_account_id" in expected_state
            compare_destination_account = "preview_destination_account_id" in expected_state

            return (
                str(preview.get("session_id") or "") == str(expected_state.get("session_id") or "")
                and str(preview.get("preview_type") or "") == str(expected_state.get("preview_type") or "")
                and str(preview.get("preview_main_category") or "") == str(expected_state.get("preview_main_category") or "")
                and str(preview.get("preview_sub_category") or "") == str(expected_state.get("preview_sub_category") or "")
                and normalized_current_recurring_id == normalized_expected_recurring_id
                and (
                    not compare_source_account
                    or normalized_current_source_account_id == normalized_expected_source_account_id
                )
                and (
                    not compare_destination_account
                    or normalized_current_destination_account_id == normalized_expected_destination_account_id
                )
                and str(preview.get("preview_matching_feedback_json") or "")
                == str(expected_state.get("preview_matching_feedback_json") or "")
            )

        def _normalize_preview_row(self, preview: dict[str, Any]) -> dict[str, Any]:
            normalized_preview = dict(preview)
            normalized_preview["preview_parser_tags"] = resolve_parser_tags(
                normalized_preview.get("preview_parser_tags_json"),
                parser_id=normalized_preview.get("preview_parser_id", ""),
                payment_method=normalized_preview.get("preview_payment_method", ""),
            )
            normalized_preview["preview_matching_feedback"] = self._deserialize_preview_matching_feedback(
                normalized_preview.get("preview_matching_feedback_json")
            )
            return normalized_preview

        @classmethod
        def _build_transfer_previous_preview_snapshot(cls, preview: dict[str, Any]) -> dict[str, Any]:
            snapshot: dict[str, Any] = {}
            for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
                value = preview.get(field, default_value)
                if field == "preview_recurring_candidate_count":
                    snapshot[field] = int(value or 0)
                elif field == "preview_recurring_match_score":
                    snapshot[field] = float(value or 0)
                elif field == "preview_recurring_id":
                    snapshot[field] = None if value in (None, "") else value
                else:
                    snapshot[field] = default_value if value is None else value
            return snapshot

        @classmethod
        def _normalize_transfer_previous_preview_snapshot(cls, raw_payload: Any) -> dict[str, Any]:
            if not isinstance(raw_payload, dict):
                return {}

            snapshot: dict[str, Any] = {}
            for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
                value = raw_payload.get(field, default_value)
                if field == "preview_recurring_candidate_count":
                    snapshot[field] = int(value or 0)
                elif field == "preview_recurring_match_score":
                    snapshot[field] = float(value or 0)
                elif field == "preview_recurring_id":
                    snapshot[field] = None if value in (None, "") else value
                else:
                    snapshot[field] = default_value if value is None else value
            return snapshot

        @classmethod
        def _append_transfer_snapshot_restore_updates(
            cls,
            snapshot: dict[str, Any],
            update_parts: list[str],
            params: list[Any],
        ) -> None:
            normalized_snapshot = cls._normalize_transfer_previous_preview_snapshot(snapshot)
            if not normalized_snapshot:
                return

            for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
                update_parts.append(f"{field} = ?")
                params.append(normalized_snapshot.get(field, default_value))

        @classmethod
        def _build_learning_previous_preview_snapshot(cls, preview: dict[str, Any]) -> dict[str, Any]:
            snapshot: dict[str, Any] = {}
            for field, default_value in cls._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                value = preview.get(field, default_value)
                if field in {"preview_source_account_id", "preview_destination_account_id"}:
                    snapshot[field] = None if value in (None, "") else int(value)
                else:
                    snapshot[field] = default_value if value is None else value
            return snapshot

        @classmethod
        def _normalize_learning_previous_preview_snapshot(cls, raw_payload: Any) -> dict[str, Any]:
            if not isinstance(raw_payload, dict):
                return {}

            snapshot: dict[str, Any] = {}
            for field, default_value in cls._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                value = raw_payload.get(field, default_value)
                if field in {"preview_source_account_id", "preview_destination_account_id"}:
                    snapshot[field] = None if value in (None, "") else int(value)
                else:
                    snapshot[field] = default_value if value is None else value
            return snapshot

        @classmethod
        def _append_learning_snapshot_restore_updates(
            cls,
            snapshot: dict[str, Any],
            update_parts: list[str],
            params: list[Any],
        ) -> None:
            normalized_snapshot = cls._normalize_learning_previous_preview_snapshot(snapshot)
            if not normalized_snapshot:
                return

            for field, default_value in cls._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                update_parts.append(f"{field} = ?")
                params.append(normalized_snapshot.get(field, default_value))

        @classmethod
        def _learning_preview_matches_snapshot(cls, preview: dict[str, Any], snapshot: dict[str, Any]) -> bool:
            normalized_snapshot = cls._normalize_learning_previous_preview_snapshot(snapshot)
            if not normalized_snapshot:
                return False

            for field, default_value in cls._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                current_value = preview.get(field, default_value)
                if field in {"preview_source_account_id", "preview_destination_account_id"}:
                    normalized_current_value = None if current_value in (None, "") else int(current_value)
                else:
                    normalized_current_value = default_value if current_value is None else current_value
                if normalized_current_value != normalized_snapshot.get(field, default_value):
                    return False

            return True

        @classmethod
        def _build_learning_accept_preview_updates(
            cls,
            applied_result: dict[str, Any],
            preview: dict[str, Any],
        ) -> dict[str, Any]:
            normalized_updates: dict[str, Any] = {}
            for field, default_value in cls._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                raw_value = applied_result.get(field, preview.get(field, default_value))
                if field in {"preview_source_account_id", "preview_destination_account_id"}:
                    normalized_updates[field] = None if raw_value in (None, "") else int(raw_value)
                else:
                    normalized_updates[field] = default_value if raw_value is None else raw_value
            return normalized_updates
