"""Shared preview-selection normalization for import confirmation."""

from __future__ import annotations

from typing import Any

PREVIEW_SELECTION_KEYS = ("selected", "isSelected", "is_selected", "preview_selected")


def coerce_preview_selected(value: Any, default: bool = True) -> bool:
    """Coerce preview selection values from API/DB payloads."""
    if value is None:
        return default
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in {"", "none", "null"}:
            return default
        if normalized in {"0", "false", "no", "off", "n"}:
            return False
        if normalized in {"1", "true", "yes", "on", "y"}:
            return True
    return bool(value)


def preview_update_is_selected(update_item: dict[str, Any], default: bool = True) -> bool:
    """Return the normalized selected flag from any supported preview update key."""
    for key in PREVIEW_SELECTION_KEYS:
        if key in update_item:
            return coerce_preview_selected(update_item.get(key), default=default)
    return default
