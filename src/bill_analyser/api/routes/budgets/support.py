"""
Budgets API Routes - 预算管理API端点

提供预算CRUD操作、执行状态查询、周期预测等功能
"""

import asyncio
import calendar
import json
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from typing import Any, cast

from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.core.budgets.execution_summary import build_budget_execution_summary
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("BudgetsAPI")
VALID_BUDGET_PERIOD_TYPES = {"daily", "weekly", "monthly", "quarterly", "yearly"}

# RESTful API 蓝图
bp = Blueprint("budgets", __name__)


@dataclass(frozen=True)
class BudgetPeriodScope:
    """Immutable resolved period scope used by budget routes."""

    budget_type: int
    period_type: str
    start_date: str
    end_date: str
    year: int | None = None
    month: int | None = None
    quarter: int | None = None


@dataclass(frozen=True)
class BudgetRouteFilters:
    """Immutable route-level filters shared by execution/history/snapshot endpoints."""

    budget_id: int | None = None
    category_id: int | None = None
    account_ids: tuple[int, ...] = ()
    tag_ids: tuple[int, ...] = ()


def get_app_context():
    """获取应用上下文中的服务实例"""
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _run_async(coroutine):
    """在独立事件循环中执行异步数据库调用。"""
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return int(getattr(request, "user_id", 0) or 0)


def _get_optional_int(value: Any, field_name: str = "value") -> int | None:
    """安全解析可选整数参数。"""
    if value in [None, ""]:
        return None
    try:
        return int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid integer for {field_name}: {value}") from exc


def _parse_csv_int_list(raw_value: str, field_name: str) -> list[int] | None:
    """解析逗号分隔的整数列表参数。"""
    if not raw_value:
        return None

    values: list[int] = []
    for item in raw_value.split(","):
        cleaned_item = item.strip()
        if not cleaned_item:
            continue
        try:
            values.append(int(cleaned_item))
        except ValueError as exc:
            raise ValueError(f"Invalid integer in {field_name}: {cleaned_item}") from exc

    return values or None


def _parse_json_int_list(values: Any, field_name: str) -> list[int]:
    """解析 JSON 数组中的整数列表参数。"""
    if values in [None, []]:
        return []
    if not isinstance(values, list):
        raise ValueError(f"Invalid array for {field_name}: expected JSON array")

    parsed_values: list[int] = []
    for item in values:
        cleaned_item = str(item).strip()
        if not cleaned_item:
            continue
        try:
            parsed_values.append(int(cleaned_item))
        except ValueError as exc:
            raise ValueError(f"Invalid integer in {field_name}: {cleaned_item}") from exc

    return parsed_values


def _validate_budget_date_range(start_date: str | None, end_date: str | None) -> None:
    """校验显式日期区间顺序。"""
    if not start_date or not end_date:
        return

    start_value = date.fromisoformat(start_date[:10])
    end_value = date.fromisoformat(end_date[:10])
    if start_value > end_value:
        raise ValueError("Invalid date range: start_date must be <= end_date")


def _validate_import_budget_item(item: Any, index: int) -> None:
    """校验导入预算单项的结构与核心字段。"""
    if not isinstance(item, dict):
        raise ValueError(f"Invalid budget item at index {index}")

    required_fields = ["period_type", "amount", "start_date"]
    for field in required_fields:
        if field not in item:
            raise ValueError(f"Missing required field at index {index}: {field}")

    if not item.get("category"):
        raise ValueError(f"Missing required field at index {index}: category")

    _validate_budget_period_args(str(item["period_type"]))
    _validate_budget_date_range(
        str(item.get("start_date") or "") or None,
        str(item.get("end_date") or "") or None,
    )


def _resolve_budget_period_range(  # pylint: disable=too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-return-statements
    period_type,
    year=None,
    month=None,
    quarter=None,
    start_date=None,
    end_date=None,
):
    """解析预算执行/快照使用的日期范围。"""
    if start_date and end_date:
        _validate_budget_date_range(start_date, end_date)
        return start_date, end_date

    today = date.today()
    resolved_year = year or today.year
    resolved_period_type = period_type or "monthly"

    if resolved_period_type == "yearly":
        return f"{resolved_year}-01-01", f"{resolved_year}-12-31"

    if resolved_period_type == "daily":
        return today.strftime("%Y-%m-%d"), today.strftime("%Y-%m-%d")

    if resolved_period_type == "weekly":
        week_start = today - timedelta(days=today.weekday())
        week_end = week_start + timedelta(days=6)
        return week_start.strftime("%Y-%m-%d"), week_end.strftime("%Y-%m-%d")

    if resolved_period_type == "quarterly":
        resolved_quarter = quarter or ((today.month - 1) // 3 + 1)
        start_month = (resolved_quarter - 1) * 3 + 1
        end_month = start_month + 2
        end_day = calendar.monthrange(resolved_year, end_month)[1]
        return (
            f"{resolved_year}-{str(start_month).zfill(2)}-01",
            f"{resolved_year}-{str(end_month).zfill(2)}-{str(end_day).zfill(2)}",
        )

    resolved_month = month or today.month
    end_day = calendar.monthrange(resolved_year, resolved_month)[1]
    return (
        f"{resolved_year}-{str(resolved_month).zfill(2)}-01",
        f"{resolved_year}-{str(resolved_month).zfill(2)}-{str(end_day).zfill(2)}",
    )


def _validate_budget_period_args(
    period_type: str,
    *,
    month: int | None = None,
    quarter: int | None = None,
    months_history: int | None = None,
) -> None:
    """校验预算周期和相关数值参数。"""
    if period_type not in VALID_BUDGET_PERIOD_TYPES:
        raise ValueError(f"Invalid period_type: {period_type}")
    if month is not None and not 1 <= month <= 12:
        raise ValueError(f"Invalid month: {month}")
    if quarter is not None and not 1 <= quarter <= 4:
        raise ValueError(f"Invalid quarter: {quarter}")
    if months_history is not None and months_history < 1:
        raise ValueError(f"Invalid months_history: {months_history}")


# pylint: disable=too-many-arguments
def _build_budget_period_scope(
    *,
    budget_type: int,
    period_type: str | None,
    year: int | None = None,
    month: int | None = None,
    quarter: int | None = None,
    start_date: str | None = None,
    end_date: str | None = None,
    months_history: int | None = None,
) -> BudgetPeriodScope:
    """Parse and validate a request period scope, returning resolved start/end dates."""
    normalized_period_type = "monthly" if period_type is None else period_type
    _validate_budget_period_args(
        normalized_period_type,
        month=month,
        quarter=quarter,
        months_history=months_history,
    )
    resolved_start_date, resolved_end_date = _resolve_budget_period_range(
        normalized_period_type,
        year=year,
        month=month,
        quarter=quarter,
        start_date=start_date,
        end_date=end_date,
    )
    return BudgetPeriodScope(
        budget_type=budget_type,
        period_type=normalized_period_type,
        start_date=resolved_start_date,
        end_date=resolved_end_date,
        year=year,
        month=month,
        quarter=quarter,
    )
# pylint: enable=too-many-arguments


def _parse_budget_query_filters() -> BudgetRouteFilters:
    """Parse integer and CSV filters from the current request query string."""
    return BudgetRouteFilters(
        budget_id=_get_optional_int(request.args.get("budget_id"), "budget_id"),
        category_id=_get_optional_int(request.args.get("category_id"), "category_id"),
        account_ids=tuple(
            _parse_csv_int_list(request.args.get("account_ids", ""), "account_ids")
            or ()
        ),
        tag_ids=tuple(_parse_csv_int_list(request.args.get("tag_ids", ""), "tag_ids") or ()),
    )


def _parse_budget_json_filters(data: dict[str, Any]) -> BudgetRouteFilters:
    """Parse integer and JSON-array filters from a request body payload."""
    return BudgetRouteFilters(
        budget_id=_get_optional_int(data.get("budget_id"), "budget_id"),
        category_id=_get_optional_int(data.get("category_id"), "category_id"),
        account_ids=tuple(_parse_json_int_list(data.get("account_ids", []), "account_ids")),
        tag_ids=tuple(_parse_json_int_list(data.get("tag_ids", []), "tag_ids")),
    )


def _to_optional_int_list(values: tuple[int, ...]) -> list[int] | None:
    """Convert immutable tuple filters to DB-friendly optional lists."""
    return list(values) or None


def _calculate_budget_period_progress(period_start: str, period_end: str) -> tuple[int, int]:
    """Calculate elapsed/remaining days for the resolved budget period."""
    today = date.today()
    start_dt = date.fromisoformat(period_start[:10])
    end_dt = date.fromisoformat(period_end[:10])
    total_days = max((end_dt - start_dt).days + 1, 1)
    if today < start_dt:
        return 0, total_days
    if today > end_dt:
        return total_days, 0
    return (today - start_dt).days + 1, max((end_dt - today).days, 0)


def _calculate_avg_backtest_mape(results: list[dict[str, Any]]) -> float | None:
    """Average only non-null forecast backtest MAPE values."""
    mape_values = [
        item["backtest_mape"]
        for item in results
        if item.get("backtest_mape") is not None
    ]
    if not mape_values:
        return None
    return round(sum(mape_values) / len(mape_values), 2)

__all__ = [name for name in globals() if not name.startswith("__")]
