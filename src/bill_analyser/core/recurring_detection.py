"""Recurring pattern detection from historical transactions."""

from __future__ import annotations

import hashlib
from collections import defaultdict
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from statistics import mean, stdev
from typing import Any

from ..utils.logger import get_logger

logger = get_logger("RecurringDetection")

# Known frequency patterns (label, nominal_days, tolerance_ratio)
FREQUENCY_PATTERNS = [
    ("weekly", 7, 0.3),
    ("biweekly", 14, 0.2),
    ("monthly", 30, 0.25),
    ("bimonthly", 60, 0.2),
    ("quarterly", 90, 0.15),
    ("semiannual", 180, 0.15),
    ("annual", 365, 0.1),
]

MIN_OCCURRENCES = 3  # Minimum transactions to consider a pattern
MAX_GAP_RATIO = 2.0  # Max allowed gap relative to detected interval
MAX_INTERVAL_VARIATION = 0.5  # Coefficient of variation above this is irregular
MIN_PATTERN_CONFIDENCE = 0.6


@dataclass
class RecurringPattern:
    """A detected recurring transaction pattern."""

    pattern_hash: str
    name: str
    description: str
    type: str
    amount: float
    source_account_id: int | None
    destination_account_id: str | None
    counterparty: str
    frequency: str
    detected_interval_days: float
    confidence_score: float
    sample_count: int
    sample_bill_ids: list[int]
    first_occurrence: str
    last_occurrence: str
    suggested_next_date: str


def _parse_date(val: Any) -> date | None:
    """Parse a date string to date object."""
    if isinstance(val, date) and not isinstance(val, datetime):
        return val
    if isinstance(val, datetime):
        return val.date()
    if not val:
        return None
    try:
        s = str(val).strip()[:10]
        return datetime.strptime(s, "%Y-%m-%d").date()
    except (ValueError, TypeError):
        return None


def _compute_pattern_hash(
    type_: str, amount_cents: int, counterparty: str, account_id: int | None
) -> str:
    """Compute a stable hash for grouping similar transactions."""
    key = f"{type_}|{amount_cents}|{counterparty.strip().lower()}|{account_id or 0}"
    return hashlib.sha256(key.encode()).hexdigest()[:16]


def _detect_frequency(intervals: list[float]) -> tuple[str, float, float]:
    """Detect the best matching frequency pattern.

    Returns (frequency_label, avg_interval_days, confidence).
    """
    if not intervals:
        return ("unknown", 0, 0)

    avg_interval = mean(intervals)
    interval_stdev = stdev(intervals) if len(intervals) > 1 else 0
    interval_variation = interval_stdev / avg_interval if avg_interval > 0 else 0
    if interval_variation > MAX_INTERVAL_VARIATION:
        return ("irregular", avg_interval, 0)

    best_match = None
    best_score = 0.0

    for label, nominal_days, tolerance in FREQUENCY_PATTERNS:
        deviation = abs(avg_interval - nominal_days) / nominal_days
        if deviation <= tolerance:
            closeness = 1.0 - deviation
            consistency = (
                1.0 - min(interval_stdev / nominal_days, 1.0) if nominal_days > 0 else 0
            )
            score = closeness * 0.6 + consistency * 0.4
            if score > best_score:
                best_score = score
                best_match = (label, avg_interval, score)

    if best_match:
        return best_match

    # Fallback: use average interval as custom frequency
    if avg_interval > 0 and interval_stdev / avg_interval < 0.4:
        consistency = 1.0 - min(interval_stdev / avg_interval, 1.0)
        return (f"every_{int(round(avg_interval))}_days", avg_interval, consistency * 0.5)

    return ("irregular", avg_interval, 0)


def _estimate_next_date(last_date: date, frequency: str, avg_interval: float) -> date:
    """Estimate the next occurrence date."""
    freq_days = {
        "weekly": 7,
        "biweekly": 14,
        "monthly": 30,
        "bimonthly": 60,
        "quarterly": 90,
        "semiannual": 180,
        "annual": 365,
    }
    days = freq_days.get(
        frequency, int(round(avg_interval)) if avg_interval > 0 else 30
    )
    return last_date + timedelta(days=days)


def detect_recurring_patterns(
    bills: list[dict[str, Any]],
    min_occurrences: int = MIN_OCCURRENCES,
    existing_recurring_ids: set[int] | None = None,
) -> list[RecurringPattern]:
    """Detect recurring patterns from a list of historical bills.

    Args:
        bills: List of bill dicts with at least: id, date, type, amount,
               counterparty, source_account_id
        min_occurrences: Minimum number of matching transactions to form a pattern
        existing_recurring_ids: Set of bill IDs already linked to recurring rules
                                (to exclude)

    Returns:
        List of RecurringPattern sorted by confidence descending.
    """
    if existing_recurring_ids is None:
        existing_recurring_ids = set()

    # Group bills by pattern (type + amount + counterparty + account)
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)

    for bill in bills:
        bill_id = bill.get("id")
        if bill_id in existing_recurring_ids:
            continue

        bill_date = _parse_date(bill.get("date"))
        if not bill_date:
            continue

        type_ = str(bill.get("type") or "").strip()
        amount = float(bill.get("amount") or 0)
        amount_cents = round(abs(amount) * 100)
        counterparty = str(bill.get("counterparty") or "").strip()
        account_id = bill.get("source_account_id")

        if not type_ or amount_cents == 0:
            continue

        # Skip if counterparty is empty — harder to form reliable pattern
        if not counterparty:
            continue

        pattern_hash = _compute_pattern_hash(type_, amount_cents, counterparty, account_id)
        groups[pattern_hash].append(
            {
                **bill,
                "_date": bill_date,
                "_amount_cents": amount_cents,
                "_pattern_hash": pattern_hash,
            }
        )

    patterns: list[RecurringPattern] = []

    for pattern_hash, group_bills in groups.items():
        if len(group_bills) < min_occurrences:
            continue

        # Sort by date
        group_bills.sort(key=lambda b: b["_date"])

        # Compute intervals between consecutive occurrences
        dates = [b["_date"] for b in group_bills]
        intervals = [
            (dates[i + 1] - dates[i]).days for i in range(len(dates) - 1)
        ]

        if not intervals:
            continue

        avg_raw = mean(intervals)
        if avg_raw <= 0:
            continue

        raw_stdev = stdev(intervals) if len(intervals) > 1 else 0
        raw_variation = raw_stdev / avg_raw if avg_raw > 0 else 0
        if raw_variation > MAX_INTERVAL_VARIATION:
            continue

        # Filter out extreme gaps (likely not part of the pattern)
        valid_intervals = [
            iv for iv in intervals if iv <= avg_raw * MAX_GAP_RATIO and iv > 0
        ]
        if len(valid_intervals) < min_occurrences - 1:
            continue

        frequency, avg_interval, freq_confidence = _detect_frequency(valid_intervals)
        if frequency in ("unknown", "irregular") or freq_confidence < 0.3:
            continue

        # Compute overall confidence
        sample = group_bills[0]
        recency_days = (date.today() - dates[-1]).days
        recency_score = max(0, 1.0 - recency_days / 180)
        count_score = min(len(group_bills) / 12, 1.0)
        confidence = freq_confidence * 0.5 + recency_score * 0.3 + count_score * 0.2
        confidence = round(min(confidence, 1.0), 3)

        if confidence < MIN_PATTERN_CONFIDENCE:
            continue

        next_date = _estimate_next_date(dates[-1], frequency, avg_interval)

        counterparty = str(sample.get("counterparty") or "").strip()
        description_parts = [str(sample.get("description") or "")]
        if sample.get("main_category"):
            description_parts.append(str(sample["main_category"]))

        patterns.append(
            RecurringPattern(
                pattern_hash=pattern_hash,
                name=counterparty or str(sample.get("description") or "Unknown"),
                description=" / ".join(p for p in description_parts if p),
                type=str(sample.get("type") or ""),
                amount=abs(float(sample.get("amount") or 0)),
                source_account_id=sample.get("source_account_id"),
                destination_account_id=str(
                    sample.get("destination_account_id") or ""
                ),
                counterparty=counterparty,
                frequency=frequency,
                detected_interval_days=round(avg_interval, 1),
                confidence_score=confidence,
                sample_count=len(group_bills),
                sample_bill_ids=[b["id"] for b in group_bills if b.get("id")],
                first_occurrence=dates[0].isoformat(),
                last_occurrence=dates[-1].isoformat(),
                suggested_next_date=next_date.isoformat(),
            )
        )

    patterns.sort(key=lambda p: -p.confidence_score)
    return patterns
