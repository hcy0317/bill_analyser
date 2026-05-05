"""Shared budget category, date, and grouping helpers."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,line-too-long,too-many-return-statements,too-many-locals
from datetime import date, timedelta
from typing import Any

from bill_analyser.utils.constants import TransactionType
from bill_analyser.core.database.shared import BudgetGroupKey
from bill_analyser.core.database.time import utc_now

_LEGACY_EXPENSE_CATEGORY_TYPE = 1
_VALID_BUDGET_CATEGORY_TYPES = {
    int(TransactionType.INCOME),
    int(TransactionType.EXPENSE),
    int(TransactionType.TRANSFER),
    int(TransactionType.INVESTMENT),
}
BudgetPeriodGroupKey = tuple[str, str, str, str, int]


class BudgetCoreSharedMixin:
    def _build_budget_category_context(self, categories: list[dict[str, Any]]) -> dict[str, Any]:
        primary_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        sub_by_key: dict[tuple[int, str, str], dict[str, Any]] = {}
        fallback_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        types_by_name: dict[str, set[int]] = {}

        for category in categories:
            main_category = str(category.get("main_category") or "").strip()
            if not main_category:
                continue

            category_type = self._normalize_budget_category_type(category.get("type"))
            if category_type is None:
                continue

            category_entry = dict(category)
            category_entry["type"] = category_type
            normalized_sub_category = self._normalize_budget_sub_category(category.get("sub_category"))
            types_by_name.setdefault(main_category, set()).add(category_type)

            if not normalized_sub_category:
                primary_by_key[(category_type, main_category)] = category_entry
                continue

            fallback_key = (category_type, main_category)
            sub_by_key[(category_type, main_category, normalized_sub_category)] = category_entry
            existing_fallback = fallback_by_key.get(fallback_key)
            if existing_fallback is None or (
                not existing_fallback.get("icon") and category_entry.get("icon")
            ):
                fallback_by_key[fallback_key] = category_entry

        return {
            "primary_by_key": primary_by_key,
            "sub_by_key": sub_by_key,
            "fallback_by_key": fallback_by_key,
            "types_by_name": {name: tuple(sorted(values)) for name, values in types_by_name.items()},
        }

    def _resolve_budget_category_type(
        self,
        category_name: str | None,
        sub_category: Any,
        category_context: dict[str, Any],
        preferred_type: int | None = None,
    ) -> int | None:
        normalized_category_name = str(category_name or "").strip()
        if not normalized_category_name:
            return None

        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        candidate_types = list(category_context["types_by_name"].get(normalized_category_name, ()))
        if not candidate_types:
            return preferred_type if preferred_type is not None else None

        if not normalized_sub_category and len(candidate_types) > 1:
            primary_by_key = category_context["primary_by_key"]
            if preferred_type is not None and (preferred_type, normalized_category_name) in primary_by_key:
                return preferred_type
            for candidate_type in candidate_types:
                if (candidate_type, normalized_category_name) in primary_by_key:
                    return candidate_type
            return None

        if preferred_type is not None and preferred_type in candidate_types:
            if self._matches_budget_category_type(
                normalized_category_name,
                normalized_sub_category,
                category_context,
                preferred_type,
            ):
                return preferred_type

        for candidate_type in candidate_types:
            if self._matches_budget_category_type(
                normalized_category_name,
                normalized_sub_category,
                category_context,
                candidate_type,
            ):
                return candidate_type
        return None

    @staticmethod
    def _normalize_budget_category_type(category_type: Any) -> int | None:
        try:
            raw_type = int(category_type)
        except (TypeError, ValueError):
            return None

        if raw_type == _LEGACY_EXPENSE_CATEGORY_TYPE:
            return int(TransactionType.EXPENSE)
        if raw_type in _VALID_BUDGET_CATEGORY_TYPES:
            return raw_type
        return None

    @staticmethod
    def _matches_budget_category_type(
        normalized_category_name: str,
        normalized_sub_category: str,
        category_context: dict[str, Any],
        candidate_type: int,
    ) -> bool:
        primary_by_key = category_context["primary_by_key"]
        sub_by_key = category_context["sub_by_key"]
        fallback_by_key = category_context["fallback_by_key"]
        if normalized_sub_category:
            return (candidate_type, normalized_category_name, normalized_sub_category) in sub_by_key
        return (
            (candidate_type, normalized_category_name) in primary_by_key
            or (candidate_type, normalized_category_name) in fallback_by_key
        )

    def _resolve_budget_category_info(
        self,
        category_name: str | None,
        sub_category: Any,
        category_context: dict[str, Any],
        budget_type: int,
    ) -> dict[str, Any] | None:
        normalized_category_name = str(category_name or "").strip()
        if not normalized_category_name:
            return None

        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        primary_by_key = category_context["primary_by_key"]
        sub_by_key = category_context["sub_by_key"]
        fallback_by_key = category_context["fallback_by_key"]

        if normalized_sub_category:
            category_info = sub_by_key.get((budget_type, normalized_category_name, normalized_sub_category))
            return dict(category_info) if category_info else None

        primary_category = primary_by_key.get((budget_type, normalized_category_name))
        fallback_category = fallback_by_key.get((budget_type, normalized_category_name))
        if primary_category is None:
            return dict(fallback_category) if fallback_category else None
        if primary_category.get("icon") or fallback_category is None:
            return dict(primary_category)

        merged_category = dict(primary_category)
        merged_category["icon"] = fallback_category.get("icon", "")
        if not merged_category.get("color") and fallback_category.get("color"):
            merged_category["color"] = fallback_category["color"]
        return merged_category

    @staticmethod
    def _normalize_budget_sub_category(sub_category: Any) -> str:
        if sub_category is None:
            return ""
        return str(sub_category).strip()

    @staticmethod
    def _build_budget_group_key(
        category: Any,
        period_type: Any,
        start_date: Any,
        user_id: int,
    ) -> BudgetGroupKey | None:
        raw_category = "" if category is None else str(category)
        raw_period_type = "" if period_type is None else str(period_type)
        raw_start_date = "" if start_date is None else str(start_date)
        if not (raw_category.strip() and raw_period_type.strip() and raw_start_date.strip()):
            return None
        return (raw_category, raw_period_type, raw_start_date, int(user_id))

    @classmethod
    def _build_budget_period_group_key(
        cls,
        category: Any,
        sub_category: Any,
        period_type: Any,
        start_date: Any,
        user_id: int,
    ) -> BudgetPeriodGroupKey | None:
        raw_category = "" if category is None else str(category)
        raw_period_type = "" if period_type is None else str(period_type)
        raw_start_date = "" if start_date is None else str(start_date)
        if not (raw_category.strip() and raw_period_type.strip() and raw_start_date.strip()):
            return None
        return (
            raw_category,
            cls._normalize_budget_sub_category(sub_category),
            raw_period_type,
            raw_start_date,
            int(user_id),
        )

    def _normalize_budget_update_data(
        self,
        existing_budget: dict[str, Any],
        data: dict[str, Any],
    ) -> dict[str, Any]:
        return {
            **data,
            "sub_category": self._normalize_budget_sub_category(
                data.get("sub_category", existing_budget.get("sub_category"))
            ),
        }

    @staticmethod
    def _build_budget_update_assignments(data: dict[str, Any]) -> tuple[list[str], list[Any]]:
        assignments: list[str] = []
        values: list[Any] = []
        for key in [
            "name",
            "category",
            "sub_category",
            "period_type",
            "amount",
            "start_date",
            "end_date",
            "alert_threshold",
            "enabled",
        ]:
            if key in data:
                assignments.append(f"{key} = ?")
                values.append(data[key])
        if "updated_at" in data:
            assignments.append("updated_at = ?")
            values.append(data["updated_at"])
        return assignments, values

    def _collect_budget_sync_group_keys(
        self,
        user_id: int,
        *budgets: dict[str, Any] | None,
    ) -> set[BudgetGroupKey]:
        group_keys: set[BudgetGroupKey] = set()
        for budget in budgets:
            if not budget:
                continue
            group_key = self._build_budget_group_key(
                budget.get("category"),
                budget.get("period_type"),
                budget.get("start_date"),
                user_id,
            )
            if group_key is not None:
                group_keys.add(group_key)
        return group_keys

    @staticmethod
    def _normalize_budget_query_end_date(end_date: str | None) -> str | None:
        if not end_date:
            return None
        normalized_end_date = str(end_date).strip()
        if len(normalized_end_date) <= 10:
            return f"{normalized_end_date} 23:59:59"
        return normalized_end_date

    @classmethod
    def _expand_forecast_history_window(
        cls,
        period_type: str,
        start_date: str | None,
        end_date: str | None,
        history_periods: int,
    ) -> tuple[str | None, str | None]:
        if history_periods <= 1:
            return start_date, end_date

        parsed_start_date = cls._parse_budget_history_date(start_date)
        parsed_end_date = cls._parse_budget_history_date(end_date)
        if not parsed_start_date or not parsed_end_date:
            return start_date, end_date

        periods_back = history_periods - 1
        if period_type == "daily":
            history_start_date = parsed_start_date - timedelta(days=periods_back)
        elif period_type == "weekly":
            history_start_date = parsed_start_date - timedelta(weeks=periods_back)
        elif period_type == "quarterly":
            history_start_date = cls._add_months(parsed_start_date, -(periods_back * 3))
        elif period_type == "yearly":
            history_start_date = date(
                parsed_start_date.year - periods_back,
                parsed_start_date.month,
                parsed_start_date.day,
            )
        else:
            history_start_date = cls._add_months(parsed_start_date, -periods_back)

        return history_start_date.strftime("%Y-%m-%d"), parsed_end_date.strftime("%Y-%m-%d")

    @classmethod
    def _build_forecast_period_key(cls, period_type: str, start_date: str | None) -> str | None:
        parsed_start_date = cls._parse_budget_history_date(start_date)
        if not parsed_start_date:
            return None
        if period_type == "daily":
            return parsed_start_date.strftime("%Y-%m-%d")
        if period_type == "weekly":
            return parsed_start_date.strftime("%Y-%W")
        if period_type == "quarterly":
            return f"{parsed_start_date.year}-Q{((parsed_start_date.month - 1) // 3) + 1}"
        if period_type == "yearly":
            return parsed_start_date.strftime("%Y")
        return parsed_start_date.strftime("%Y-%m")

    @classmethod
    def _resolve_parent_budget_period(
        cls,
        parent_period_type: str,
        child_start_date: str | None,
    ) -> tuple[str, str] | None:
        parsed_start_date = cls._parse_budget_history_date(child_start_date)
        if not parsed_start_date:
            return None

        if parent_period_type == "quarterly":
            quarter_start_month = ((parsed_start_date.month - 1) // 3) * 3 + 1
            parent_start = date(parsed_start_date.year, quarter_start_month, 1)
            parent_end = cls._add_months(parent_start, 3) - timedelta(days=1)
            return parent_start.strftime("%Y-%m-%d"), parent_end.strftime("%Y-%m-%d")

        if parent_period_type == "yearly":
            parent_start = date(parsed_start_date.year, 1, 1)
            parent_end = date(parsed_start_date.year, 12, 31)
            return parent_start.strftime("%Y-%m-%d"), parent_end.strftime("%Y-%m-%d")

        return None

    @staticmethod
    def _current_budget_timestamp_text() -> str:
        return utc_now().replace(tzinfo=None).strftime("%Y-%m-%d %H:%M:%S")

    @staticmethod
    def _get_budget_type_name(budget_type: int) -> str:
        return "支出" if budget_type == 3 else "投资"

    @staticmethod
    def _parse_budget_history_date(date_text: str | None) -> date | None:
        if not date_text:
            return None
        try:
            return date.fromisoformat(str(date_text)[:10])
        except (TypeError, ValueError):
            return None

    @staticmethod
    def _add_months(source_date: date, months: int) -> date:
        total_month = (source_date.year * 12 + source_date.month - 1) + months
        year = total_month // 12
        month = total_month % 12 + 1
        return date(year, month, 1)
