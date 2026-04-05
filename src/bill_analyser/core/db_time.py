"""Shared UTC timestamp helpers for the split database modules."""

from __future__ import annotations

from datetime import UTC, datetime


def utc_now() -> datetime:
    """Return the current timezone-aware UTC datetime."""
    return datetime.now(UTC)


def utc_now_iso() -> str:
    """Return the repository-standard ISO timestamp text without a timezone suffix."""
    return utc_now().replace(tzinfo=None).isoformat()
