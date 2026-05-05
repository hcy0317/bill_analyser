"""Budget forecast plus import/export helpers for the split database facade."""

from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.budgets.core import DatabaseBudgetsCoreMixin
from bill_analyser.core.database.shared import BudgetForecastRequest

if TYPE_CHECKING:
    import aiosqlite


@dataclass(frozen=True)
class BudgetForecastBuildContext:
    """Immutable helper bundle used while building forecast response rows."""

    budget_map: dict[str, dict[str, float]]
    category_context: dict[str, Any]
    request: BudgetForecastRequest
    normalized_strategy: str
    normalized_history_periods: int
    target_period_key: str | None
    num_periods: int


class DatabaseBudgetForecastMixin(DatabaseBudgetsCoreMixin):
    """Budget forecast and import/export helpers."""

    @staticmethod
    def _coerce_budget_forecast_request(
        request: BudgetForecastRequest | None = None,
        **kwargs: Any,
    ) -> BudgetForecastRequest:
        """Build a forecast request bundle while preserving legacy keyword call sites."""
        if request is not None:
            return request
        return BudgetForecastRequest(
            budget_type=int(kwargs.get("budget_type", 3) or 3),
            period_type=str(kwargs.get("period_type", "monthly") or "monthly"),
            start_date=kwargs.get("start_date"),
            end_date=kwargs.get("end_date"),
            forecast_strategy=str(
                kwargs.get("forecast_strategy", "historical_average")
                or "historical_average"
            ),
            history_periods=int(kwargs.get("history_periods", 6) or 6),
            user_id=int(kwargs.get("user_id", 1) or 1),
        )

    @staticmethod
    def _build_budget_forecast_group_by(period_type: str) -> str:
        """Return the SQL period-grouping expression for the requested forecast granularity."""
        if period_type == "daily":
            return "date(date)"
        if period_type == "weekly":
            return "strftime('%Y-%W', date)"
        if period_type == "quarterly":
            return (
                "strftime('%Y', date) || '-Q' || "
                "(CAST(((CAST(strftime('%m', date) AS INTEGER) - 1) / 3) AS INTEGER) + 1)"
            )
        if period_type == "monthly":
            return "strftime('%Y-%m', date)"
        return "strftime('%Y', date)"

    async def _fetch_budget_forecast_rows(
        self,
        conn: aiosqlite.Connection,
        request: BudgetForecastRequest,
    ) -> list[dict[str, Any]]:
        """Fetch grouped historical bill totals used for budget forecasting."""
        history_start_date, history_end_date = self._expand_forecast_history_window(
            request.period_type,
            request.start_date,
            request.end_date,
            request.history_periods,
        )
        group_by = self._build_budget_forecast_group_by(request.period_type)
        query = f"""
            SELECT
                {group_by} as period,
                main_category,
                COALESCE(SUM(amount), 0) as total_amount,
                COUNT(*) as transaction_count
            FROM bills
            WHERE type = ? AND user_id = ?
        """
        params: list[Any] = [
            self._get_budget_type_name(request.budget_type),
            request.user_id,
        ]
        if history_start_date:
            query += " AND date >= ?"
            params.append(history_start_date)
        if history_end_date:
            query += " AND date <= ?"
            params.append(self._normalize_budget_query_end_date(history_end_date))
        query += f" GROUP BY {group_by}, main_category ORDER BY period DESC, total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @staticmethod
    def _aggregate_budget_forecast_rows(
        rows: list[dict[str, Any]],
    ) -> tuple[dict[str, dict[str, Any]], int]:
        """Aggregate grouped forecast rows by category and count distinct periods."""
        category_totals: dict[str, dict[str, Any]] = {}
        period_keys: set[str] = set()
        for row in rows:
            period = row["period"]
            category = row["main_category"] or "未分类"
            amount = abs(row["total_amount"])
            period_keys.add(period)
            bucket = category_totals.setdefault(category, {"total": 0.0, "periods": []})
            bucket["total"] += amount
            bucket["periods"].append({"period": period, "amount": amount})
        return category_totals, len(period_keys) if period_keys else 1

    def _build_budget_forecast_budget_map(
        self,
        budget_rows: list[dict[str, Any]],
        category_context: dict[str, Any],
        budget_type: int,
    ) -> dict[str, dict[str, float]]:
        """Build a category-to-budget lookup for the forecast response."""
        budget_map: dict[str, dict[str, float]] = {}
        for row in budget_rows:
            category_name = row["category"] or "未分类"
            resolved_budget_type = self._resolve_budget_category_type(
                category_name,
                row["sub_category"],
                category_context,
                preferred_type=budget_type,
            )
            if int(resolved_budget_type or 0) != int(budget_type):
                continue
            bucket = budget_map.setdefault(category_name, {"primary": 0.0, "sub_total": 0.0})
            amount_value = float(row["amount"] or 0)
            if not row["sub_category"]:
                bucket["primary"] += amount_value
            else:
                bucket["sub_total"] += amount_value
        return budget_map

    async def _fetch_budget_forecast_budget_map(
        self,
        conn: aiosqlite.Connection,
        request: BudgetForecastRequest,
        category_context: dict[str, Any],
    ) -> dict[str, dict[str, float]]:
        """Fetch budget rows and convert them into the forecast budget lookup map."""
        budget_query = """
            SELECT category, sub_category, amount
            FROM budgets
            WHERE period_type = ? AND enabled = 1 AND user_id = ?
        """
        budget_params: list[Any] = [request.period_type, request.user_id]
        if request.start_date:
            budget_query += " AND (end_date IS NULL OR end_date = '' OR end_date >= ?)"
            budget_params.append(request.start_date)
        if request.end_date:
            budget_query += " AND (start_date IS NULL OR start_date = '' OR start_date <= ?)"
            budget_params.append(request.end_date)

        async with conn.execute(budget_query, budget_params) as cursor:
            budget_rows = [dict(row) for row in await cursor.fetchall()]
        return self._build_budget_forecast_budget_map(
            budget_rows,
            category_context,
            request.budget_type,
        )

    @staticmethod
    def _normalize_forecast_strategy(strategy: str) -> str:
        """Normalize the incoming strategy selector to the supported default."""
        return strategy or "historical_average"

    @staticmethod
    def _normalize_forecast_history_periods(history_periods: int) -> int:
        """Clamp the history-period count to at least one period."""
        return max(int(history_periods or 0), 1)

    @staticmethod
    def _calculate_forecast_amount(
        recent_amounts: list[float],
        normalized_strategy: str,
    ) -> tuple[float, float, str]:
        """Calculate the average/forecast amount and explain the chosen strategy."""
        average_amount = sum(recent_amounts) / len(recent_amounts)
        moving_window_size = min(3, len(recent_amounts))
        moving_average_amount = (
            sum(recent_amounts[-moving_window_size:]) / moving_window_size
            if moving_window_size > 0
            else 0
        )
        if normalized_strategy == "moving_average":
            return average_amount, moving_average_amount, f"基于最近{moving_window_size}个周期的移动平均"
        return (
            average_amount,
            average_amount,
            f"基于最近{len(recent_amounts)}个周期的历史均值",
        )

    @classmethod
    def _calculate_forecast_backtest_mape(
        cls,
        recent_amounts: list[float],
        normalized_strategy: str,
    ) -> float | None:
        """Calculate a simple backtest MAPE score for the selected forecast strategy."""
        backtest_errors: list[float] = []
        for index in range(1, len(recent_amounts)):
            actual_amount = recent_amounts[index]
            if actual_amount <= 0:
                continue
            history_slice = recent_amounts[:index]
            if normalized_strategy == "moving_average":
                window_size = min(3, len(history_slice))
                predicted_amount = (
                    sum(history_slice[-window_size:]) / window_size if window_size > 0 else 0
                )
            else:
                predicted_amount = sum(history_slice) / len(history_slice)
            backtest_errors.append(abs(actual_amount - predicted_amount) / actual_amount)
        if not backtest_errors:
            return None
        return round((sum(backtest_errors) / len(backtest_errors)) * 100, 2)

    @staticmethod
    def _resolve_forecast_confidence(backtest_mape: float | None) -> str:
        """Map a backtest MAPE score to the coarse confidence label used by the API."""
        if backtest_mape is None:
            return "low"
        if backtest_mape <= 10:
            return "high"
        if backtest_mape <= 20:
            return "medium"
        return "low"

    @staticmethod
    def _resolve_forecast_trend(recent_amounts: list[float]) -> str:
        """Resolve the simple directional trend label from recent actuals."""
        if len(recent_amounts) < 2:
            return "stable"
        latest_amount = recent_amounts[-1]
        baseline_avg = sum(recent_amounts[:-1]) / len(recent_amounts[:-1])
        if baseline_avg > 0 and latest_amount > baseline_avg * 1.05:
            return "up"
        if baseline_avg > 0 and latest_amount < baseline_avg * 0.95:
            return "down"
        return "stable"

    @staticmethod
    def _resolve_forecast_budget_amount(
        budget_map: dict[str, dict[str, float]],
        category: str,
    ) -> float:
        """Resolve the primary/sub-category budget amount used by one forecast item."""
        if category not in budget_map:
            return 0.0
        budget_info = budget_map[category]
        return (
            budget_info["primary"]
            if budget_info["primary"] > 0
            else budget_info["sub_total"]
        )

    def _build_budget_forecast_item(  # pylint: disable=too-many-locals
        self,
        category: str,
        data: dict[str, Any],
        context: BudgetForecastBuildContext,
    ) -> dict[str, Any] | None:
        """Build the forecast response item for one category."""
        sorted_periods = sorted(data["periods"], key=lambda item: item["period"])
        recent_periods = sorted_periods[-context.normalized_history_periods :]
        if not recent_periods:
            return None

        recent_amounts = [float(item["amount"]) for item in recent_periods]
        average_amount, forecast_amount, strategy_explanation = self._calculate_forecast_amount(
            recent_amounts,
            context.normalized_strategy,
        )
        backtest_mape = self._calculate_forecast_backtest_mape(
            recent_amounts,
            context.normalized_strategy,
        )
        confidence = self._resolve_forecast_confidence(backtest_mape)
        trend = self._resolve_forecast_trend(recent_amounts)
        current_period_amounts = {
            item["period"]: item["amount"] for item in sorted_periods
        }
        current_spent = current_period_amounts.get(context.target_period_key, 0.0)
        budget_amount = self._resolve_forecast_budget_amount(context.budget_map, category)
        category_info = self._resolve_budget_category_info(
            category,
            "",
            context.category_context,
            context.request.budget_type,
        )
        return {
            "category": category,
            "category_info": category_info,
            "total_amount": round(sum(recent_amounts), 2),
            "average_amount": round(average_amount, 2),
            "period_count": context.num_periods,
            "sample_periods": len(recent_amounts),
            "current_spent": round(current_spent, 2),
            "budget_amount": round(budget_amount, 2),
            "forecast_amount": round(forecast_amount, 2),
            "projected_over_budget": forecast_amount > budget_amount > 0,
            "forecast_strategy": context.normalized_strategy,
            "strategy_explanation": strategy_explanation,
            "backtest_mape": backtest_mape,
            "confidence": confidence,
            "trend": trend,
            "periods": recent_periods,
        }

    async def _get_period_forecast_for_request(  # pylint: disable=too-many-locals
        self,
        request: BudgetForecastRequest,
    ) -> list[dict[str, Any]]:
        """Resolve period-forecast rows for the provided immutable request bundle."""
        conn = await self._get_connection()
        rows = await self._fetch_budget_forecast_rows(conn, request)
        normalized_strategy = self._normalize_forecast_strategy(request.forecast_strategy)
        normalized_history_periods = self._normalize_forecast_history_periods(
            request.history_periods
        )
        target_period_key = self._build_forecast_period_key(
            request.period_type,
            request.start_date,
        )
        category_totals, num_periods = self._aggregate_budget_forecast_rows(rows)

        categories = await self.get_all_categories(user_id=request.user_id)
        category_context = self._build_budget_category_context(categories)
        budget_map = await self._fetch_budget_forecast_budget_map(
            conn,
            request,
            category_context,
        )
        build_context = BudgetForecastBuildContext(
            budget_map=budget_map,
            category_context=category_context,
            request=request,
            normalized_strategy=normalized_strategy,
            normalized_history_periods=normalized_history_periods,
            target_period_key=target_period_key,
            num_periods=num_periods,
        )

        results: list[dict[str, Any]] = []
        for category, data in category_totals.items():
            forecast_item = self._build_budget_forecast_item(category, data, build_context)
            if forecast_item is not None:
                results.append(forecast_item)
        results.sort(key=lambda item: item["forecast_amount"], reverse=True)
        return results

    @log_method
    async def get_period_forecast(  # pylint: disable=too-many-arguments,too-many-positional-arguments
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str | None = None,
        end_date: str | None = None,
        forecast_strategy: str = "historical_average",
        history_periods: int = 6,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """Public forecast API using the immutable request bundle."""
        resolved_request = self._coerce_budget_forecast_request(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            forecast_strategy=forecast_strategy,
            history_periods=history_periods,
            user_id=user_id,
        )
        return await self._get_period_forecast_for_request(resolved_request)

    @log_method
    async def import_budgets(
        self,
        budgets_data: list[dict[str, Any]],
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Import or upsert budget rows from a serialized payload list."""
        conn = await self._get_connection()
        now = self._current_budget_timestamp_text()
        created_count = 0
        updated_count = 0
        error_count = 0
        errors: list[str] = []

        for index, data in enumerate(budgets_data, start=1):
            try:
                if not data.get("name") or not data.get("amount"):
                    errors.append(f"第{index}条: 缺少必填字段(name或amount)")
                    error_count += 1
                    continue

                async with conn.execute(
                    "SELECT id FROM budgets WHERE name = ? AND user_id = ?",
                    (data["name"], user_id),
                ) as cursor:
                    existing = await cursor.fetchone()

                if existing:
                    await conn.execute(
                        """
                        UPDATE budgets SET
                            category = ?,
                            sub_category = ?,
                            period_type = ?,
                            amount = ?,
                            start_date = ?,
                            end_date = ?,
                            alert_threshold = ?,
                            enabled = ?,
                            updated_at = ?
                        WHERE id = ? AND user_id = ?
                        """,
                        (
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            existing["id"],
                            user_id,
                        ),
                    )
                    updated_count += 1
                else:
                    await conn.execute(
                        """
                        INSERT INTO budgets (
                            name, category, sub_category, period_type, amount,
                            start_date, end_date, alert_threshold, enabled,
                            created_at, updated_at, user_id
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            data["name"],
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            now,
                            user_id,
                        ),
                    )
                    created_count += 1
            except (sqlite3.Error, TypeError, ValueError, KeyError) as exc:  # pragma: no cover
                errors.append(f"第{index}条: {exc!s}")
                error_count += 1
                self.logger.error("导入预算失败: %s", exc)

        await conn.commit()
        return {
            "created": created_count,
            "updated": updated_count,
            "errors": error_count,
            "error_details": errors,
        }

    @log_method
    async def export_budgets(self, user_id: int = 1) -> list[dict[str, Any]]:
        """Export budget rows with the stable field subset used by the REST API."""
        conn = await self._get_connection()
        export_fields = [
            "name",
            "category",
            "sub_category",
            "period_type",
            "amount",
            "start_date",
            "end_date",
            "alert_threshold",
            "enabled",
        ]
        async with conn.execute(
            "SELECT * FROM budgets WHERE user_id = ? ORDER BY created_at",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        budgets: list[dict[str, Any]] = []
        for row in rows:
            budgets.append({field: row[field] for field in export_fields if field in row.keys()})
        return budgets
