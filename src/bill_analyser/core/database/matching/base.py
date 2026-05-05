"""Historical bill matching persistence helpers."""

from __future__ import annotations

import json
import sqlite3
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import parse_bill_datetime
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.investment.matching import score_investment_candidate
from bill_analyser.core.investment.settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from bill_analyser.core.matching import build_transfer_pair_candidate, build_transfer_pair_candidates
from bill_analyser.core.matching.candidate_ids import build_learning_rule_revision, normalize_learning_rule_revision


class MatchingBaseMixin(object):
        _TRANSFER_PAIR_TYPE = "transfer"

        _INVESTMENT_PAIR_TYPE = "investment"

        _TRANSFER_PAIR_LOOKBACK_DAYS = 3

        _INVESTMENT_PAIR_LOOKBACK_DAYS = 3

        _MANUAL_PAIR_SOURCE = "manual"

        _UNSET = object()

        @staticmethod
        def _build_bill_snapshot_from_row(
            row: dict[str, Any],
            prefix: str,
        ) -> dict[str, Any]:
            return {
                "id": int(row.get(f"{prefix}_id") or 0),
                "date": str(row.get(f"{prefix}_date") or ""),
                "type": str(row.get(f"{prefix}_type") or ""),
                "amount": float(row.get(f"{prefix}_amount") or 0.0),
                "counterparty": str(row.get(f"{prefix}_counterparty") or ""),
                "description": str(row.get(f"{prefix}_description") or ""),
                "payment_method": str(row.get(f"{prefix}_payment_method") or ""),
                "main_category": str(row.get(f"{prefix}_main_category") or ""),
                "sub_category": str(row.get(f"{prefix}_sub_category") or ""),
                "source_account_id": int(row.get(f"{prefix}_source_account_id") or 0),
                "destination_account_id": int(row.get(f"{prefix}_destination_account_id") or 0),
            }

        @staticmethod
        def _normalize_transfer_pair_bill_ids(
            bill_id: int,
            candidate_bill_id: int,
        ) -> tuple[int, int]:
            normalized_bill_id = int(bill_id)
            normalized_candidate_bill_id = int(candidate_bill_id)
            if normalized_bill_id == normalized_candidate_bill_id:
                raise ValueError("billId and candidateBillId must be different")
            return (
                min(normalized_bill_id, normalized_candidate_bill_id),
                max(normalized_bill_id, normalized_candidate_bill_id),
            )

        @staticmethod
        def _has_opposite_matching_amounts(
            left_bill: dict[str, Any],
            right_bill: dict[str, Any],
        ) -> bool:
            left_amount = float(left_bill.get("amount") or 0.0)
            right_amount = float(right_bill.get("amount") or 0.0)
            return abs(abs(left_amount) - abs(right_amount)) <= 0.01 and left_amount * right_amount < 0

        @staticmethod
        def _has_distinct_valid_source_account_ids(
            left_bill: dict[str, Any],
            right_bill: dict[str, Any],
        ) -> bool:
            left_source_account_id = int(left_bill.get("source_account_id") or 0)
            right_source_account_id = int(right_bill.get("source_account_id") or 0)
            return (
                left_source_account_id > 0
                and right_source_account_id > 0
                and left_source_account_id != right_source_account_id
            )

        @staticmethod
        def _is_explicit_transfer_type(raw_type: Any) -> bool:
            return str(raw_type or "").strip().lower() in {"转账", "transfer"}

        @classmethod
        def _is_investment_like_bill(
            cls,
            bill: dict[str, Any],
            keyword_config: dict[str, list[str]],
        ) -> bool:
            return score_investment_candidate(
                bill,
                allow_existing_investment=True,
                keyword_config=keyword_config,
            ) is not None

        @classmethod
        def _resolve_pair_time_diff_seconds(
            cls,
            left_bill: dict[str, Any],
            right_bill: dict[str, Any],
            *,
            lookback_days: int,
        ) -> float | None:
            left_datetime = parse_bill_datetime(left_bill.get("date"))
            right_datetime = parse_bill_datetime(right_bill.get("date"))
            if left_datetime is None or right_datetime is None:
                return None

            time_diff_seconds = abs((left_datetime - right_datetime).total_seconds())
            max_window_seconds = float(max(lookback_days, 0) * 24 * 60 * 60)
            if max_window_seconds <= 0 or time_diff_seconds > max_window_seconds:
                return None
            return time_diff_seconds

        @classmethod
        def _build_linked_pair_payload(
            cls,
            existing_pair: dict[str, Any],
            *,
            bill_id: int,
        ) -> dict[str, Any]:
            other_bill_id = (
                int(existing_pair["right_bill_id"])
                if int(existing_pair["left_bill_id"]) == int(bill_id)
                else int(existing_pair["left_bill_id"])
            )
            return {
                "id": int(existing_pair["id"]),
                "pair_type": str(existing_pair.get("pair_type") or cls._TRANSFER_PAIR_TYPE),
                "source": str(existing_pair.get("source") or cls._MANUAL_PAIR_SOURCE),
                "left_bill_id": int(existing_pair["left_bill_id"]),
                "right_bill_id": int(existing_pair["right_bill_id"]),
                "other_bill_id": other_bill_id,
            }

        @staticmethod
        def _build_bill_pair_feedback_payload(
            *,
            kind: str,
            bill_id: int,
            candidate_bill_id: int,
            pair: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
            payload: dict[str, Any] = {
                "scope": "bill",
                "kind": str(kind or "").strip(),
                "bill_id": int(bill_id),
                "candidate_bill_id": int(candidate_bill_id),
            }
            if isinstance(pair, dict):
                payload["pair"] = {
                    "id": int(pair.get("id") or 0),
                    "pair_type": str(pair.get("pair_type") or ""),
                    "source": str(pair.get("source") or ""),
                    "left_bill_id": int(pair.get("left_bill_id") or 0),
                    "right_bill_id": int(pair.get("right_bill_id") or 0),
                }
            return payload

        @staticmethod
        def _deserialize_bill_pair_feedback_payload(raw_payload_json: Any) -> dict[str, Any]:
            if isinstance(raw_payload_json, dict):
                return dict(raw_payload_json)
            if raw_payload_json in (None, ""):
                return {}
            try:
                payload = json.loads(str(raw_payload_json))
            except (TypeError, ValueError, json.JSONDecodeError):
                return {}
            return dict(payload) if isinstance(payload, dict) else {}

        @staticmethod
        def _is_bill_related_feedback_payload(payload: dict[str, Any], bill_id: int) -> bool:
            normalized_bill_id = int(bill_id)
            related_bill_ids: set[int] = set()

            for raw_bill_id in (payload.get("bill_id"), payload.get("candidate_bill_id")):
                if raw_bill_id in (None, ""):
                    continue
                try:
                    related_bill_ids.add(int(raw_bill_id))
                except (TypeError, ValueError):
                    continue

            pair_payload = payload.get("pair")
            if isinstance(pair_payload, dict):
                for raw_bill_id in (pair_payload.get("left_bill_id"), pair_payload.get("right_bill_id")):
                    if raw_bill_id in (None, ""):
                        continue
                    try:
                        related_bill_ids.add(int(raw_bill_id))
                    except (TypeError, ValueError):
                        continue

            return normalized_bill_id in related_bill_ids
