"""Recurring template schedule calculation helpers."""

# pylint: disable=line-too-long,protected-access,unused-argument

from __future__ import annotations

from datetime import date, datetime, timedelta
from typing import Any


def _parse_date_value(self, raw_value: Any) -> date | None:
    """解析 YYYY-MM-DD 或 YYYY-MM-DD HH:MM:SS 格式日期。"""
    if not raw_value:
        return None

    text = str(raw_value).strip()
    if not text:
        return None

    try:
        return datetime.fromisoformat(text[:19]).date()
    except ValueError:
        pass

    try:
        return date.fromisoformat(text[:10])
    except ValueError:
        return None


def _parse_schedule_frequency_values(self, raw_value: Any) -> list[int]:
    """解析定时频率值，例如 '1,15'。"""
    if not raw_value:
        return []

    values: list[int] = []
    for item in str(raw_value).split(","):
        item = item.strip()
        if not item:
            continue
        try:
            values.append(int(item))
        except ValueError:
            continue
    return sorted(set(values))


def _weekday_sunday_first(self, target_date: date) -> int:
    """将 Python weekday(Monday=0) 转为 Sunday=0。"""
    return (target_date.weekday() + 1) % 7


def _is_recurring_active_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
    """判断定时模板在指定日期是否生效。"""
    start_date = self._parse_date_value(recurring.get("start_date"))
    end_date = self._parse_date_value(recurring.get("end_date"))
    if start_date and target_date < start_date:
        return False
    if end_date and target_date > end_date:
        return False
    return True


def _is_recurring_due_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
    """判断定时模板是否在某天应发生。"""
    if not self._is_recurring_active_on_date(recurring, target_date):
        return False

    frequency_type = int(recurring.get("scheduled_frequency_type") or 0)
    frequency_values = self._parse_schedule_frequency_values(recurring.get("frequency"))
    start_date = self._parse_date_value(recurring.get("start_date"))
    next_date = self._parse_date_value(recurring.get("next_date"))

    if frequency_type == 1:
        if frequency_values:
            valid_weekdays = frequency_values
        elif start_date:
            valid_weekdays = [self._weekday_sunday_first(start_date)]
        else:
            valid_weekdays = []
        return self._weekday_sunday_first(target_date) in valid_weekdays

    if frequency_type == 2:
        if frequency_values:
            valid_days = frequency_values
        elif start_date:
            valid_days = [start_date.day]
        else:
            valid_days = []
        return target_date.day in valid_days

    if next_date:
        return target_date == next_date
    return bool(start_date and target_date == start_date)


def _find_recurring_occurrence_near_date(
    self,
    recurring: dict[str, Any],
    target_date: date,
    tolerance_days: int,
) -> date | None:
    """在容差窗口内寻找最近的计划发生日期。"""
    nearest_date: date | None = None
    nearest_diff: int | None = None

    for offset in range(-tolerance_days, tolerance_days + 1):
        current_date = target_date + timedelta(days=offset)
        if not self._is_recurring_due_on_date(recurring, current_date):
            continue

        diff = abs(offset)
        if nearest_date is None or (nearest_diff is not None and diff < nearest_diff):
            nearest_date = current_date
            nearest_diff = diff

    return nearest_date


def _get_next_recurring_occurrence_after(
    self,
    recurring: dict[str, Any],
    after_date: date,
    max_search_days: int = 370,
) -> date | None:
    """获取指定日期后的下一次计划发生日期。"""
    for offset in range(1, max_search_days + 1):
        candidate = after_date + timedelta(days=offset)
        if self._is_recurring_due_on_date(recurring, candidate):
            return candidate
    return None


def _get_first_recurring_occurrence(self, recurring: dict[str, Any], max_search_days: int = 370) -> date | None:
    """获取定时模板的首次计划日期（包含 start_date 当天）。"""
    start_date = self._parse_date_value(recurring.get("start_date"))
    if not start_date:
        return self._parse_date_value(recurring.get("next_date"))

    for offset in range(0, max_search_days + 1):
        candidate = start_date + timedelta(days=offset)
        if self._is_recurring_due_on_date(recurring, candidate):
            return candidate

    return self._parse_date_value(recurring.get("next_date"))
