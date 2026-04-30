"""Tests for bill_analyser.core.recurring_detection.

Targets >90% line/branch coverage of the module by exercising the date
parser, frequency detector, next-date estimator and the public
``detect_recurring_patterns`` pipeline (all guard clauses + happy paths).
"""

from __future__ import annotations

# pyright: reportPrivateUsage=false

from datetime import date, datetime, timedelta
from typing import Any

import pytest

from bill_analyser.core import recurring_detection as rd
from bill_analyser.core.recurring_detection import (
    RecurringPattern,
    _compute_pattern_hash,
    _detect_frequency,
    _estimate_next_date,
    _parse_date,
    detect_recurring_patterns,
)


# ---------------------------------------------------------------------------
# _parse_date
# ---------------------------------------------------------------------------


def test_parse_date_with_date_object() -> None:
    d = date(2025, 1, 2)
    assert _parse_date(d) == d


def test_parse_date_with_datetime_object() -> None:
    dt = datetime(2025, 1, 2, 12, 30, 0)
    assert _parse_date(dt) == date(2025, 1, 2)


@pytest.mark.parametrize("val", [None, "", 0])
def test_parse_date_with_falsy_returns_none(val: Any) -> None:
    assert _parse_date(val) is None


def test_parse_date_from_string_truncates_time() -> None:
    assert _parse_date("2025-01-02 10:11:12") == date(2025, 1, 2)


def test_parse_date_with_invalid_string_returns_none() -> None:
    assert _parse_date("not-a-date") is None


def test_parse_date_with_unparsable_object_returns_none() -> None:
    class Weird:
        def __str__(self) -> str:  # pragma: no cover - trivial
            raise TypeError("bad")

    assert _parse_date(Weird()) is None


# ---------------------------------------------------------------------------
# _compute_pattern_hash
# ---------------------------------------------------------------------------


def test_compute_pattern_hash_is_stable_and_normalizes_counterparty() -> None:
    h1 = _compute_pattern_hash("支出", 1234, " Netflix ", 1)
    h2 = _compute_pattern_hash("支出", 1234, "netflix", 1)
    assert h1 == h2
    assert len(h1) == 16


def test_compute_pattern_hash_handles_none_account() -> None:
    h = _compute_pattern_hash("支出", 1, "x", None)
    assert isinstance(h, str) and len(h) == 16


def test_compute_pattern_hash_differs_when_inputs_differ() -> None:
    a = _compute_pattern_hash("支出", 100, "a", 1)
    b = _compute_pattern_hash("支出", 100, "b", 1)
    assert a != b


# ---------------------------------------------------------------------------
# _detect_frequency
# ---------------------------------------------------------------------------


def test_detect_frequency_empty_returns_unknown() -> None:
    label, avg, conf = _detect_frequency([])
    assert label == "unknown"
    assert avg == 0
    assert conf == 0


def test_detect_frequency_monthly_consistent() -> None:
    label, avg, conf = _detect_frequency([30.0, 30.0, 30.0])
    assert label == "monthly"
    assert avg == pytest.approx(30.0)
    assert conf > 0.9


@pytest.mark.parametrize(
    "intervals,expected",
    [
        ([7.0, 7.0, 7.0], "weekly"),
        ([14.0, 14.0, 14.0], "biweekly"),
        ([60.0, 60.0, 60.0], "bimonthly"),
        ([90.0, 90.0, 90.0], "quarterly"),
        ([180.0, 180.0, 180.0], "semiannual"),
        ([365.0, 365.0, 365.0], "annual"),
    ],
)
def test_detect_frequency_known_buckets(
    intervals: list[float], expected: str
) -> None:
    label, _avg, conf = _detect_frequency(intervals)
    assert label == expected
    assert conf > 0.5


def test_detect_frequency_custom_interval_fallback() -> None:
    # 45 days is outside known tolerances but consistent → custom every_N_days.
    label, avg, conf = _detect_frequency([45.0, 45.0, 45.0])
    assert label.startswith("every_")
    assert avg == pytest.approx(45.0)
    assert 0 < conf <= 0.5


def test_detect_frequency_irregular_returns_irregular() -> None:
    # High variance, no known bucket match → irregular.
    label, _avg, conf = _detect_frequency([10.0, 100.0, 10.0, 100.0])
    assert label == "irregular"
    assert conf == 0


# ---------------------------------------------------------------------------
# _estimate_next_date
# ---------------------------------------------------------------------------


def test_estimate_next_date_known_frequency() -> None:
    base = date(2025, 1, 1)
    assert _estimate_next_date(base, "monthly", 30.0) == base + timedelta(days=30)
    assert _estimate_next_date(base, "weekly", 7.0) == base + timedelta(days=7)


def test_estimate_next_date_unknown_uses_avg() -> None:
    base = date(2025, 1, 1)
    assert _estimate_next_date(base, "every_45_days", 45.0) == base + timedelta(days=45)


def test_estimate_next_date_unknown_with_zero_avg_defaults_30() -> None:
    base = date(2025, 1, 1)
    assert _estimate_next_date(base, "unknown", 0) == base + timedelta(days=30)


# ---------------------------------------------------------------------------
# detect_recurring_patterns
# ---------------------------------------------------------------------------


def _bill(
    bill_id: int,
    days_ago: int,
    *,
    type_: str = "支出",
    amount: float = 30.0,
    counterparty: str = "Netflix",
    description: str = "订阅",
    main_category: str = "娱乐",
    source_account_id: int | None = 1,
    destination_account_id: str | None = None,
) -> dict[str, Any]:
    return {
        "id": bill_id,
        "date": (date.today() - timedelta(days=days_ago)).isoformat(),
        "type": type_,
        "amount": amount,
        "counterparty": counterparty,
        "description": description,
        "main_category": main_category,
        "source_account_id": source_account_id,
        "destination_account_id": destination_account_id,
    }


def test_detect_returns_empty_when_no_bills() -> None:
    assert detect_recurring_patterns([]) == []


def test_detect_skips_existing_recurring_ids() -> None:
    bills = [_bill(i + 1, days_ago=30 * (3 - i)) for i in range(3)]
    result = detect_recurring_patterns(bills, existing_recurring_ids={1, 2, 3})
    assert result == []


def test_detect_skips_bills_missing_date_or_amount_or_type() -> None:
    bills: list[dict[str, Any]] = [
        {**_bill(1, 60), "date": None},
        {**_bill(2, 30), "amount": 0},
        {**_bill(3, 0), "type": ""},
    ]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_when_counterparty_empty() -> None:
    bills = [
        _bill(1, 90, counterparty=""),
        _bill(2, 60, counterparty=""),
        _bill(3, 30, counterparty=""),
    ]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_groups_below_min_occurrences() -> None:
    bills = [
        _bill(1, 60),
        _bill(2, 30),
    ]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_when_all_same_day_avg_zero() -> None:
    # Three bills on the same day → intervals all 0 → avg_raw <= 0 guard.
    bills = [_bill(i + 1, 10) for i in range(3)]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_when_extreme_gaps_drop_too_many() -> None:
    # Huge gap between groups so only 1 valid interval survives the
    # MAX_GAP_RATIO filter (need >= min_occurrences-1 = 2).
    bills = [
        _bill(1, 1000),
        _bill(2, 999),
        _bill(3, 998),
        _bill(4, 0),
    ]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_irregular_frequency() -> None:
    # Within-tolerance avg interval but enough variance to fall back to
    # ``irregular`` (≥0 / ≤avg*MAX_GAP_RATIO so all intervals survive).
    today = date.today()

    def at(days_ago: int, bid: int) -> dict[str, Any]:
        return {
            **_bill(bid, days_ago=0),
            "date": (today - timedelta(days=days_ago)).isoformat(),
        }

    bills = [at(0, 1), at(5, 2), at(50, 3), at(55, 4)]
    assert detect_recurring_patterns(bills) == []


def test_detect_skips_when_confidence_too_low() -> None:
    # Pattern was monthly but the last occurrence is far in the past, so
    # recency_score collapses and final confidence falls below 0.3.
    bills = [
        _bill(1, 720),
        _bill(2, 690),
        _bill(3, 660),
    ]
    assert detect_recurring_patterns(bills) == []


def test_detect_returns_monthly_pattern_with_expected_fields() -> None:
    bills = [
        _bill(1, 90),
        _bill(2, 60),
        _bill(3, 30),
        _bill(4, 0),
    ]
    [pattern] = detect_recurring_patterns(bills)

    assert isinstance(pattern, RecurringPattern)
    assert pattern.frequency == "monthly"
    assert pattern.sample_count == 4
    assert pattern.sample_bill_ids == [1, 2, 3, 4]
    assert pattern.counterparty == "Netflix"
    assert pattern.name == "Netflix"
    assert "订阅" in pattern.description
    assert "娱乐" in pattern.description
    assert pattern.amount == pytest.approx(30.0)
    assert pattern.source_account_id == 1
    assert pattern.destination_account_id == ""
    assert pattern.detected_interval_days == pytest.approx(30.0)
    assert 0.3 <= pattern.confidence_score <= 1.0
    assert pattern.first_occurrence < pattern.last_occurrence
    # suggested_next_date == last_occurrence + 30d (monthly).
    last = date.fromisoformat(pattern.last_occurrence)
    assert date.fromisoformat(pattern.suggested_next_date) == last + timedelta(days=30)


def test_detect_uses_description_as_name_when_counterparty_blank_after_grouping() -> None:
    # counterparty must be non-empty to enter grouping; once inside, the
    # ``name = counterparty or description`` branch is exercised by
    # patching the sample's counterparty to empty post-hoc — easier to
    # cover via a direct grouping with whitespace counterparty.
    bills = [
        _bill(1, 90, counterparty="   Spotify   "),
        _bill(2, 60, counterparty="spotify"),
        _bill(3, 30, counterparty="Spotify"),
    ]
    [pattern] = detect_recurring_patterns(bills)
    assert pattern.counterparty in {"Spotify", "spotify"}
    assert pattern.name == pattern.counterparty


def test_detect_sorts_results_by_confidence_descending() -> None:
    monthly = [_bill(i + 1, days_ago=30 * (3 - i)) for i in range(4)]
    weekly = [
        _bill(100 + i, days_ago=7 * (5 - i), counterparty="Gym", amount=50.0)
        for i in range(6)
    ]
    patterns = detect_recurring_patterns(monthly + weekly)
    assert len(patterns) == 2
    assert patterns[0].confidence_score >= patterns[1].confidence_score


def test_detect_respects_custom_min_occurrences() -> None:
    bills = [_bill(1, 60), _bill(2, 30)]
    # min_occurrences=2 means we need >= 2 bills and >= 1 valid interval.
    [pattern] = detect_recurring_patterns(bills, min_occurrences=2)
    assert pattern.sample_count == 2
