"""Core budget helpers and CRUD operations for the split database facade."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-positional-arguments

from __future__ import annotations

from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from ..utils.logger import log_method
from .db_shared import BudgetGroupKey, DatabaseFacadeBase
from .db_time import utc_now


class DatabaseBudgetsCoreMixin(DatabaseFacadeBase):
    """Budget CRUD helpers plus shared category/grouping logic."""

    @log_method
    async def get_budgets(self, filters: dict[str, Any] | None = None, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM budgets WHERE user_id = ?"
        params: list[Any] = [user_id]

        if filters:
            if "period_type" in filters:
                query += " AND period_type = ?"
                params.append(filters["period_type"])
            if "enabled" in filters:
                query += " AND enabled = ?"
                params.append(1 if filters["enabled"] else 0)
            if "category" in filters:
                query += " AND category = ?"
                params.append(filters["category"])

        query += " ORDER BY created_at DESC"
        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_budget_by_id(self, budget_id: int, user_id: int = 1) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    def _build_budget_category_context(self, categories: list[dict[str, Any]]) -> dict[str, Any]:
        primary_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        sub_by_key: dict[tuple[int, str, str], dict[str, Any]] = {}
        fallback_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        types_by_name: dict[str, set[int]] = {}

        for category in categories:
            main_category = str(category.get("main_category") or "").strip()
            if not main_category:
                continue

            category_type = int(category.get("type") or 0)
            if category_type <= 0:
                continue

            normalized_sub_category = self._normalize_budget_sub_category(category.get("sub_category"))
            types_by_name.setdefault(main_category, set()).add(category_type)

            if not normalized_sub_category:
                primary_by_key[(category_type, main_category)] = category
                continue

            fallback_key = (category_type, main_category)
            sub_by_key[(category_type, main_category, normalized_sub_category)] = category
            existing_fallback = fallback_by_key.get(fallback_key)
            if existing_fallback is None or (
                not existing_fallback.get("icon") and category.get("icon")
            ):
                fallback_by_key[fallback_key] = category

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

    async def _insert_budget_record(
        self,
        conn: aiosqlite.Connection,
        data: dict[str, Any],
        user_id: int,
    ) -> int:
        cursor = await conn.execute(
            """
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("name", ""),
                data.get("category"),
                self._normalize_budget_sub_category(data.get("sub_category")),
                data["period_type"],
                data["amount"],
                data["start_date"],
                data.get("end_date"),
                data.get("alert_threshold", 80),
                data.get("enabled", 1),
                data["created_at"],
                data["updated_at"],
                user_id,
            ),
        )
        return int(cursor.lastrowid or 0)

    async def _get_primary_category_budget_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey,
    ) -> dict[str, Any] | None:
        category, period_type, start_date, user_id = group_key
        async with conn.execute(
            """
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def _get_sub_category_budgets_total_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey,
    ) -> float:
        category, period_type, start_date, user_id = group_key
        async with conn.execute(
            """
            SELECT COALESCE(SUM(amount), 0) as total
            FROM budgets
            WHERE category = ?
              AND sub_category IS NOT NULL
              AND sub_category != ''
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return float(row["total"] if row else 0)

    async def _synchronize_primary_budget_for_group(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey | None,
        reference_data: dict[str, Any] | None = None,
    ) -> None:
        if group_key is None:
            return

        category, period_type, start_date, user_id = group_key
        sub_total = await self._get_sub_category_budgets_total_with_conn(conn, group_key)
        primary_budget = await self._get_primary_category_budget_with_conn(conn, group_key)
        if sub_total <= 0:
            return

        now = self._current_budget_timestamp_text()
        if not primary_budget:
            reference = reference_data or {}
            await self._insert_budget_record(
                conn,
                {
                    "name": reference.get("name", ""),
                    "category": category,
                    "sub_category": "",
                    "period_type": period_type,
                    "amount": sub_total,
                    "start_date": start_date,
                    "end_date": reference.get("end_date"),
                    "alert_threshold": reference.get("alert_threshold", 80),
                    "enabled": reference.get("enabled", 1),
                    "created_at": reference.get("created_at", now),
                    "updated_at": now,
                },
                user_id,
            )
            return

        current_amount = float(primary_budget.get("amount") or 0)
        if current_amount >= sub_total:
            return

        await conn.execute(
            "UPDATE budgets SET amount = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (sub_total, now, primary_budget["id"], user_id),
        )

    @log_method
    async def create_budget(self, data: dict[str, Any], user_id: int = 1) -> int:
        conn = await self._get_connection()
        normalized_data = {
            **data,
            "name": data.get("name", ""),
            "sub_category": self._normalize_budget_sub_category(data.get("sub_category")),
        }
        budget_id = await self._insert_budget_record(conn, normalized_data, user_id)
        await self._synchronize_primary_budget_for_group(
            conn,
            self._build_budget_group_key(
                normalized_data.get("category"),
                normalized_data.get("period_type"),
                normalized_data.get("start_date"),
                user_id,
            ),
            reference_data=normalized_data,
        )
        await conn.commit()
        return budget_id

    @log_method
    async def get_primary_category_budget(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return None
        return await self._get_primary_category_budget_with_conn(conn, group_key)

    @log_method
    async def get_budget_by_category(
        self,
        category: str,
        sub_category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return None
        normalized_category, normalized_period_type, normalized_start_date, normalized_user_id = group_key

        if normalized_sub_category:
            async with conn.execute(
                """
                SELECT * FROM budgets
                WHERE category = ?
                  AND sub_category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
                """,
                (
                    normalized_category,
                    normalized_sub_category,
                    normalized_period_type,
                    normalized_start_date,
                    normalized_user_id,
                ),
            ) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None

        async with conn.execute(
            """
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (normalized_category, normalized_period_type, normalized_start_date, normalized_user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_sub_category_budgets_total(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> float:
        conn = await self._get_connection()
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return 0.0
        return await self._get_sub_category_budgets_total_with_conn(conn, group_key)

    @log_method
    async def update_budget(self, budget_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        conn = await self._get_connection()
        existing_budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not existing_budget:
            return False

        normalized_data = self._normalize_budget_update_data(existing_budget, data)
        assignments, values = self._build_budget_update_assignments(normalized_data)
        if not assignments:
            return False

        values.extend([budget_id, user_id])
        cursor = await conn.execute(
            f"UPDATE budgets SET {', '.join(assignments)} WHERE id = ? AND user_id = ?",
            values,
        )
        if cursor.rowcount > 0:
            updated_budget = {**existing_budget, **normalized_data}
            group_keys = self._collect_budget_sync_group_keys(user_id, existing_budget, updated_budget)
            for group_key in group_keys:
                await self._synchronize_primary_budget_for_group(
                    conn,
                    group_key,
                    reference_data=updated_budget or normalized_data,
                )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def delete_budget(self, budget_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not budget:
            return False

        category = budget.get("category")
        period_type = budget.get("period_type")
        start_date = budget.get("start_date")
        is_primary_budget = not self._normalize_budget_sub_category(budget.get("sub_category"))

        if is_primary_budget:
            cursor = await conn.execute(
                """
                DELETE FROM budgets
                WHERE category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
                """,
                (category, period_type, start_date, user_id),
            )
        else:
            cursor = await conn.execute("DELETE FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id))
            await self._synchronize_primary_budget_for_group(
                conn,
                self._build_budget_group_key(category, period_type, start_date, user_id),
                reference_data=budget,
            )
        await conn.commit()
        return cursor.rowcount > 0

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
