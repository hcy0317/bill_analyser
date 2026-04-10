"""Helpers for scoring transfer-only matching candidates on persisted bills."""

from __future__ import annotations

from datetime import timedelta
from typing import Any

from ..bill_date_utils import parse_bill_datetime

_MAX_TRANSFER_PAIR_WINDOW = timedelta(days=3)
_AMOUNT_TOLERANCE = 0.01


def _normalized_bill_type(bill: dict[str, Any]) -> str:
    return str(bill.get("type") or "").strip().lower()


def _is_explicit_transfer_type(bill: dict[str, Any]) -> bool:
    return _normalized_bill_type(bill) in {"转账", "transfer"}


def _coerce_float(value: Any) -> float:
    try:
        return float(value or 0.0)
    except (TypeError, ValueError):
        return 0.0


def _coerce_int(value: Any) -> int | None:
    if value in (None, "", 0, "0"):
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _derive_level(score: float) -> str:
    if score >= 0.8:
        return "high"
    if score >= 0.65:
        return "medium"
    if score > 0:
        return "low"
    return ""


def _has_valid_bill_ids(anchor_bill: dict[str, Any], candidate_bill: dict[str, Any]) -> bool:
    anchor_id = _coerce_int(anchor_bill.get("id"))
    candidate_id = _coerce_int(candidate_bill.get("id"))
    return (
        anchor_id is not None
        and candidate_id is not None
        and anchor_id != candidate_id
    )


def _has_opposite_matching_amounts(
    anchor_bill: dict[str, Any],
    candidate_bill: dict[str, Any],
) -> bool:
    anchor_amount = _coerce_float(anchor_bill.get("amount"))
    candidate_amount = _coerce_float(candidate_bill.get("amount"))
    return (
        abs(abs(anchor_amount) - abs(candidate_amount)) <= _AMOUNT_TOLERANCE
        and anchor_amount * candidate_amount < 0
    )


def _resolve_distinct_source_account_ids(
    anchor_bill: dict[str, Any],
    candidate_bill: dict[str, Any],
) -> tuple[int, int] | None:
    anchor_source_account_id = _coerce_int(anchor_bill.get("source_account_id"))
    candidate_source_account_id = _coerce_int(candidate_bill.get("source_account_id"))
    if anchor_source_account_id is None or candidate_source_account_id is None:
        return None
    if anchor_source_account_id == candidate_source_account_id:
        return None
    return anchor_source_account_id, candidate_source_account_id


def _resolve_time_diff_seconds(
    anchor_bill: dict[str, Any],
    candidate_bill: dict[str, Any],
) -> float | None:
    anchor_datetime = parse_bill_datetime(anchor_bill.get("date"))
    candidate_datetime = parse_bill_datetime(candidate_bill.get("date"))
    if anchor_datetime is None or candidate_datetime is None:
        return None

    time_diff_seconds = abs((anchor_datetime - candidate_datetime).total_seconds())
    if time_diff_seconds > _MAX_TRANSFER_PAIR_WINDOW.total_seconds():
        return None
    return time_diff_seconds


def build_transfer_pair_candidate(
    anchor_bill: dict[str, Any],
    candidate_bill: dict[str, Any],
) -> dict[str, Any] | None:
    """Build a read-only transfer candidate payload for a persisted bill pair."""
    if _is_explicit_transfer_type(anchor_bill) or _is_explicit_transfer_type(candidate_bill):
        return None
    if not _has_valid_bill_ids(anchor_bill, candidate_bill):
        return None
    candidate_id = _coerce_int(candidate_bill.get("id")) or 0

    if not _has_opposite_matching_amounts(anchor_bill, candidate_bill):
        return None
    account_ids = _resolve_distinct_source_account_ids(anchor_bill, candidate_bill)
    if account_ids is None:
        return None
    _, candidate_source_account_id = account_ids

    time_diff_seconds = _resolve_time_diff_seconds(anchor_bill, candidate_bill)
    if time_diff_seconds is None:
        return None

    candidate_amount = _coerce_float(candidate_bill.get("amount"))
    max_window_seconds = _MAX_TRANSFER_PAIR_WINDOW.total_seconds()
    time_score = max(0.0, 1.0 - (time_diff_seconds / max_window_seconds))
    score = round(min(0.85 + (0.14 * time_score), 0.99), 2)
    reason_parts = ["opposite_amount", "different_source_account"]
    if time_diff_seconds <= 3600:
        reason_parts.append("time_close")
    else:
        reason_parts.append("date_window")

    return {
        "bill_id": candidate_id,
        "score": score,
        "level": _derive_level(score),
        "reason": "|".join(reason_parts),
        "time_diff_seconds": int(time_diff_seconds),
        "bill": {
            "id": candidate_id,
            "date": str(candidate_bill.get("date") or ""),
            "type": str(candidate_bill.get("type") or ""),
            "amount": candidate_amount,
            "counterparty": str(candidate_bill.get("counterparty") or ""),
            "description": str(candidate_bill.get("description") or ""),
            "payment_method": str(candidate_bill.get("payment_method") or ""),
            "main_category": str(candidate_bill.get("main_category") or ""),
            "sub_category": str(candidate_bill.get("sub_category") or ""),
            "source_account_id": candidate_source_account_id,
            "destination_account_id": (
                _coerce_int(candidate_bill.get("destination_account_id")) or 0
            ),
        },
    }


def build_transfer_pair_candidates(
    anchor_bill: dict[str, Any], candidate_bills: list[dict[str, Any]] | None
) -> list[dict[str, Any]]:
    """Build ordered transfer candidates for a persisted bill."""
    candidates: list[dict[str, Any]] = []
    for candidate_bill in candidate_bills or []:
        candidate = build_transfer_pair_candidate(anchor_bill, candidate_bill)
        if candidate is not None:
            candidates.append(candidate)

    candidates.sort(
        key=lambda candidate: (
            -float(candidate.get("score") or 0.0),
            int(candidate.get("time_diff_seconds") or 0),
            int(candidate.get("bill_id") or 0),
        )
    )
    for candidate in candidates:
        candidate.pop("time_diff_seconds", None)
    return candidates
