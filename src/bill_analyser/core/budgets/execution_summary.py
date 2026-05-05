"""Shared helpers for presenting budget execution results without double counting."""

from __future__ import annotations

from math import isclose
from typing import Any


def build_budget_execution_group_key(item: dict[str, Any]) -> tuple[Any, ...]:
    """Build the grouping key used to collapse synchronized primary budgets."""
    return (
        str(item.get("category") or "").strip(),
        str(item.get("period_type") or "").strip(),
        str(item.get("start_date") or "").strip(),
    )


def _group_budget_execution_items(
    items: list[dict[str, Any]],
) -> dict[tuple[Any, ...], dict[str, Any]]:
    """Group execution rows into primary-vs-sub-budget buckets."""
    grouped: dict[tuple[Any, ...], dict[str, Any]] = {}
    for item in items:
        group_key = build_budget_execution_group_key(item)
        bucket = grouped.setdefault(
            group_key,
            {"primaries": [], "secondary": [], "items": []},
        )
        bucket["items"].append(item)
        if str(item.get("sub_category") or "").strip():
            bucket["secondary"].append(item)
        else:
            bucket["primaries"].append(item)
    return grouped


def _sum_budget_metric(items: list[dict[str, Any]], field_name: str) -> float:
    """Sum one numeric field across a list of execution rows."""
    return sum(float(item.get(field_name) or 0.0) for item in items)


def _primary_budget_is_shadow(
    primary_item: dict[str, Any],
    secondary_items: list[dict[str, Any]],
) -> bool:
    """Return whether the primary row is just a synchronized roll-up of its leaf budgets."""
    if not secondary_items:
        return False

    return isclose(
        float(primary_item.get("budget_amount") or 0.0),
        _sum_budget_metric(secondary_items, "budget_amount"),
        abs_tol=0.01,
    ) and isclose(
        float(primary_item.get("spent_amount") or 0.0),
        _sum_budget_metric(secondary_items, "spent_amount"),
        abs_tol=0.01,
    )


def select_budget_detail_items(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Choose the rows that should be displayed in human-facing detail views."""
    selected_items: list[dict[str, Any]] = []
    for bucket in _group_budget_execution_items(items).values():
        primaries = list(bucket["primaries"])
        secondary_items = list(bucket["secondary"])
        if len(primaries) > 1:
            selected_items.extend(bucket["items"])
            continue

        primary_item = primaries[0] if primaries else None
        if primary_item and secondary_items:
            if _primary_budget_is_shadow(primary_item, secondary_items):
                selected_items.extend(secondary_items)
            else:
                selected_items.append(primary_item)
            continue

        if primary_item:
            selected_items.append(primary_item)
            continue

        selected_items.extend(secondary_items)
    return selected_items


def select_budget_summary_items(items: list[dict[str, Any]]) -> list[dict[str, Any]]:

    """Choose the authoritative rows used for summary totals."""
    selected_items: list[dict[str, Any]] = []
    for bucket in _group_budget_execution_items(items).values():
        primaries = list(bucket["primaries"])
        secondary_items = list(bucket["secondary"])
        if len(primaries) > 1:
            selected_items.extend(bucket["items"])
            continue

        if len(primaries) == 1:
            selected_items.append(primaries[0])
            continue
        selected_items.extend(secondary_items)
    return selected_items



def build_budget_execution_summary(items: list[dict[str, Any]]) -> dict[str, float | int]:
    """Build a deduplicated execution summary shared by REST and CLI surfaces."""
    selected_items = select_budget_summary_items(items)
    total_budget = sum(float(item.get("budget_amount") or 0.0) for item in selected_items)
    total_spent = sum(float(item.get("spent_amount") or 0.0) for item in selected_items)
    overall_execution_rate = (total_spent / total_budget * 100) if total_budget > 0 else 0.0
    return {
        "total_budget": total_budget,
        "total_spent": total_spent,
        "total_remaining": total_budget - total_spent,
        "overall_execution_rate": round(overall_execution_rate, 2),
        "count": len(selected_items),
    }
