"""Shared bill date parsing and normalization helpers."""

from __future__ import annotations

from datetime import date, datetime
from typing import Any

_SUPPORTED_BILL_DATE_FORMATS: tuple[str, ...] = (
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%d",
    "%Y/%m/%d %H:%M:%S",
    "%Y/%m/%d %H:%M",
    "%Y/%m/%d",
    "%Y年%m月%d日 %H:%M:%S",
    "%Y年%m月%d日 %H:%M",
    "%Y年%m月%d日",
)
_NORMALIZED_BILL_DATE_FORMAT = "%Y-%m-%d %H:%M:%S"


def parse_bill_datetime(raw_value: Any) -> datetime | None:
    """Parse supported bill date inputs into a naive datetime."""
    if isinstance(raw_value, datetime):
        return raw_value.replace(microsecond=0, tzinfo=None)
    if isinstance(raw_value, date):
        return datetime.combine(raw_value, datetime.min.time())

    text = str(raw_value or "").strip()
    if not text:
        return None

    try:
        return datetime.fromisoformat(text[:19]).replace(microsecond=0, tzinfo=None)
    except ValueError:
        pass

    for date_format in _SUPPORTED_BILL_DATE_FORMATS:
        try:
            return datetime.strptime(text, date_format)
        except ValueError:
            continue
    return None


def normalize_bill_date_text(raw_value: Any) -> str:
    """Normalize supported bill date formats to YYYY-MM-DD HH:MM:SS text."""
    parsed_datetime = parse_bill_datetime(raw_value)
    if parsed_datetime is None:
        return str(raw_value or "").strip()
    return parsed_datetime.strftime(_NORMALIZED_BILL_DATE_FORMAT)
