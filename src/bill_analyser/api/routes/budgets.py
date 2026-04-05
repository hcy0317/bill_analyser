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


def _parse_budget_query_filters() -> BudgetRouteFilters:
    """Parse integer and CSV filters from the current request query string."""
    return BudgetRouteFilters(
        budget_id=_get_optional_int(request.args.get("budget_id"), "budget_id"),
        category_id=_get_optional_int(request.args.get("category_id"), "category_id"),
        account_ids=tuple(_parse_csv_int_list(request.args.get("account_ids", ""), "account_ids") or ()),
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
    mape_values = [item["backtest_mape"] for item in results if item.get("backtest_mape") is not None]
    if not mape_values:
        return None
    return round(sum(mape_values) / len(mape_values), 2)


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_budgets():
    """
    获取预算列表

    Query Parameters:
        - period_type: 周期类型（daily/weekly/monthly/yearly）
        - enabled: 是否启用（true/false）
        - category: 分类名称
    """
    logger.info("[get_budgets] 开始获取预算列表")

    try:
        filters = {}
        if request.args.get("period_type"):
            filters["period_type"] = request.args.get("period_type")
        enabled_arg = request.args.get("enabled")
        if enabled_arg:
            filters["enabled"] = enabled_arg.lower() == "true"
        if request.args.get("category"):
            filters["category"] = request.args.get("category")

        logger.info("[get_budgets] 筛选条件: %s", filters)

        db = get_app_context()
        user_id = _get_request_user_id()
        budgets = _run_async(db.get_budgets(filters, user_id=user_id))

        logger.info("[get_budgets] 返回%s条预算", len(budgets))

        return jsonify({"success": True, "result": budgets})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算列表失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["GET"])
@log_method
@require_auth
def get_budget(budget_id: int):
    """获取预算详情"""
    logger.info("[get_budget] 获取预算ID: %s", budget_id)

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))

        if not budget:
            logger.warning("[get_budget] 预算不存在: %s", budget_id)
            return jsonify({"success": False, "error": "Budget not found"}), 404

        logger.info("[get_budget] 成功获取预算: %s", budget["name"])

        return jsonify({"success": True, "result": budget})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算详情失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_budget():  # pylint: disable=too-many-return-statements
    """
    创建预算

    Request Body:
        {
            "name": "餐饮预算",
            "category": "餐饮",
            "sub_category": "午餐",
            "period_type": "monthly",
            "amount": 1000.00,
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "alert_threshold": 80,
            "enabled": true
        }
    """
    logger.info("[create_budget] 开始创建预算")

    try:
        data = request.get_json(silent=True)
        if data is None:
            return jsonify({"success": False, "error": "No data provided"}), 400
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON body. Expected object."}), 400

        logger.info("[create_budget] 请求数据: %s", data)

        # 预算名称已改为备注，允许为空；分类仍为必填
        required_fields = ["period_type", "amount", "start_date"]
        for field in required_fields:
            if field not in data:
                logger.warning("[create_budget] 缺少必填字段: %s", field)
                return jsonify({"success": False, "error": f"Missing required field: {field}"}), 400

        if not data.get("category"):
            logger.warning("[create_budget] 缺少必填字段: category")
            return jsonify({"success": False, "error": "Missing required field: category"}), 400

        _validate_budget_period_args(str(data["period_type"]))
        _validate_budget_date_range(
            str(data.get("start_date") or "") or None,
            str(data.get("end_date") or "") or None,
        )

        # 设置默认值
        data.setdefault("name", "")
        data.setdefault("enabled", True)
        data.setdefault("alert_threshold", 80)
        data["created_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        db = get_app_context()
        user_id = _get_request_user_id()
        budget_id = _run_async(db.create_budget(data, user_id=user_id))

        if budget_id:
            budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))
            logger.info("[create_budget] 创建成功: ID=%s, name=%s", budget_id, data["name"])

            return jsonify({"success": True, "result": budget}), 201

        logger.error("[create_budget] 创建失败")
        return jsonify({"success": False, "error": "Failed to create budget"}), 500

    except ValueError as exc:
        logger.warning("创建预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["PUT"])
@log_method
@require_auth
def update_budget(budget_id: int):  # pylint: disable=too-many-return-statements
    """更新预算"""
    logger.info("[update_budget] 更新预算ID: %s", budget_id)

    try:
        data = request.get_json(silent=True)
        if data is None:
            return jsonify({"success": False, "error": "No data provided"}), 400
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON body. Expected object."}), 400

        logger.info("[update_budget] 更新数据: %s", data)

        db = get_app_context()
        user_id = _get_request_user_id()

        if data.get("period_type") is not None:
            _validate_budget_period_args(str(data["period_type"]))

        existing_budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))
        if not existing_budget:
            logger.warning("[update_budget] 预算不存在: %s", budget_id)
            return jsonify({"success": False, "error": "Budget not found"}), 404

        _validate_budget_date_range(
            str(data.get("start_date", existing_budget.get("start_date")) or "") or None,
            str(data.get("end_date", existing_budget.get("end_date")) or "") or None,
        )

        data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        result = _run_async(db.update_budget(budget_id, data, user_id=user_id))

        if result:
            logger.info("[update_budget] 更新成功: ID=%s", budget_id)
            return jsonify({"success": True, "message": "Budget updated successfully"})

        logger.warning("[update_budget] 预算不存在: %s", budget_id)
        return jsonify({"success": False, "error": "Budget not found"}), 404

    except ValueError as exc:
        logger.warning("更新预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_budget(budget_id: int):
    """删除预算"""
    logger.info("[delete_budget] 删除预算ID: %s", budget_id)

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(db.delete_budget(budget_id, user_id=user_id))

        if result:
            logger.info("[delete_budget] 删除成功: ID=%s", budget_id)
            return jsonify({"success": True, "message": "Budget deleted successfully"})

        logger.warning("[delete_budget] 预算不存在: %s", budget_id)
        return jsonify({"success": False, "error": "Budget not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/execution", methods=["GET"])
@log_method
@require_auth
def get_budget_execution():  # pylint: disable=too-many-locals
    """
    获取预算执行详情

    Query Parameters:
        - budget_type: 预算类型（3=支出, 5=投资）
        - start_date: 开始日期
        - end_date: 结束日期
        - budget_id: 预算ID
        - category_id: 分类ID
        - account_ids: 账户ID（逗号分隔）
        - tag_ids: 标签ID（逗号分隔）
    """
    logger.info("[get_budget_execution] 开始获取预算执行详情")

    try:
        period_scope = _build_budget_period_scope(
            budget_type=_get_optional_int(request.args.get("budget_type"), "budget_type") or 3,
            period_type=request.args.get("period_type"),
            year=_get_optional_int(request.args.get("year"), "year"),
            month=_get_optional_int(request.args.get("month"), "month"),
            quarter=_get_optional_int(request.args.get("quarter"), "quarter"),
            start_date=request.args.get("start_date"),
            end_date=request.args.get("end_date"),
        )
        filters = _parse_budget_query_filters()

        logger.info(
            (
                "[get_budget_execution] 参数: type=%s, period=%s, dates=%s~%s, "
                "category=%s, accounts=%s, tags=%s"
            ),
            period_scope.budget_type,
            period_scope.period_type,
            period_scope.start_date,
            period_scope.end_date,
            filters.category_id,
            list(filters.account_ids) or None,
            list(filters.tag_ids) or None,
        )

        db = get_app_context()
        user_id = _get_request_user_id()
        results = _run_async(
            db.get_budget_execution_details(
                budget_type=period_scope.budget_type,
                period_type=period_scope.period_type,
                start_date=period_scope.start_date,
                end_date=period_scope.end_date,
                budget_id=filters.budget_id,
                category_id=filters.category_id,
                account_ids=_to_optional_int_list(filters.account_ids),
                tag_ids=_to_optional_int_list(filters.tag_ids),
                user_id=user_id,
            )
        )

        # 计算汇总
        total_budget = sum(r["budget_amount"] for r in results)
        total_spent = sum(r["spent_amount"] for r in results)
        overall_execution_rate = (total_spent / total_budget * 100) if total_budget > 0 else 0

        logger.info("[get_budget_execution] 返回%s条执行详情", len(results))

        return jsonify(
            {
                "success": True,
                "result": {
                    "items": results,
                    "summary": {
                        "total_budget": total_budget,
                        "total_spent": total_spent,
                        "total_remaining": total_budget - total_spent,
                        "overall_execution_rate": round(overall_execution_rate, 2),
                        "count": len(results),
                    },
                    "period_start": period_scope.start_date,
                    "period_end": period_scope.end_date,
                },
            }
        )

    except ValueError as exc:
        logger.warning("获取预算执行详情参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算执行详情失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/forecast", methods=["GET"])
@log_method
@require_auth
def get_period_forecast():  # pylint: disable=too-many-locals
    """
    获取周期预计

    Query Parameters:
        - budget_type: 预算类型（3=支出, 5=投资）
        - period_type: 周期类型（daily/weekly/monthly/yearly）
        - start_date: 开始日期
        - end_date: 结束日期
        - forecast_strategy: 预测策略（historical_average/moving_average）
        - months_history: 历史周期数量
    """
    logger.info("[get_period_forecast] 开始获取周期预计")

    try:
        forecast_strategy = request.args.get("forecast_strategy", "historical_average")
        months_history_value = _get_optional_int(
            request.args.get("months_history"),
            "months_history",
        )
        months_history = 6 if months_history_value is None else months_history_value
        period_scope = _build_budget_period_scope(
            budget_type=_get_optional_int(request.args.get("budget_type"), "budget_type") or 3,
            period_type=request.args.get("period_type"),
            year=_get_optional_int(request.args.get("year"), "year"),
            month=_get_optional_int(request.args.get("month"), "month"),
            quarter=_get_optional_int(request.args.get("quarter"), "quarter"),
            start_date=request.args.get("start_date"),
            end_date=request.args.get("end_date"),
            months_history=months_history,
        )

        logger.info(
            (
                "[get_period_forecast] 参数: type=%s, period=%s, dates=%s~%s, "
                "strategy=%s, months_history=%s"
            ),
            period_scope.budget_type,
            period_scope.period_type,
            period_scope.start_date,
            period_scope.end_date,
            forecast_strategy,
            months_history,
        )
        days_elapsed, days_remaining = _calculate_budget_period_progress(
            period_scope.start_date,
            period_scope.end_date,
        )

        db = get_app_context()
        user_id = _get_request_user_id()
        results = _run_async(
            db.get_period_forecast(
                budget_type=period_scope.budget_type,
                period_type=period_scope.period_type,
                start_date=period_scope.start_date,
                end_date=period_scope.end_date,
                forecast_strategy=forecast_strategy,
                history_periods=months_history,
                user_id=user_id,
            )
        )

        # 计算汇总
        total_forecast = sum(r["forecast_amount"] for r in results)
        avg_backtest_mape = _calculate_avg_backtest_mape(results)

        logger.info("[get_period_forecast] 返回%s条预测数据", len(results))

        return jsonify(
            {
                "success": True,
                "result": {
                    "items": results,
                    "period_start": period_scope.start_date,
                    "period_end": period_scope.end_date,
                    "periodStart": period_scope.start_date,
                    "periodEnd": period_scope.end_date,
                    "daysElapsed": days_elapsed,
                    "daysRemaining": days_remaining,
                    "summary": {
                        "total_forecast": round(total_forecast, 2),
                        "count": len(results),
                        "forecast_strategy": forecast_strategy,
                        "history_periods": months_history,
                        "avg_backtest_mape": avg_backtest_mape,
                        "days_elapsed": days_elapsed,
                        "days_remaining": days_remaining,
                    },
                },
            }
        )

    except ValueError as exc:
        logger.warning("获取周期预计参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取周期预计失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/history/snapshot", methods=["POST"])
@log_method
@require_auth
def create_budget_history_snapshot():  # pylint: disable=too-many-locals
    """创建预算执行历史快照。"""
    logger.info("[create_budget_history_snapshot] 开始创建预算执行快照")

    try:
        raw_data = request.get_json(silent=True)
        if isinstance(raw_data, dict):
            data = raw_data
        else:
            raise ValueError("Invalid JSON body. Expected object.")

        period_scope = _build_budget_period_scope(
            budget_type=_get_optional_int(data.get("budget_type"), "budget_type") or 3,
            period_type=data.get("period_type"),
            year=_get_optional_int(data.get("year"), "year"),
            month=_get_optional_int(data.get("month"), "month"),
            quarter=_get_optional_int(data.get("quarter"), "quarter"),
            start_date=data.get("start_date"),
            end_date=data.get("end_date"),
        )
        filters = _parse_budget_json_filters(data)

        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(
            db.create_budget_execution_snapshots(
                budget_type=period_scope.budget_type,
                period_type=period_scope.period_type,
                start_date=period_scope.start_date,
                end_date=period_scope.end_date,
                budget_id=filters.budget_id,
                category_id=filters.category_id,
                account_ids=_to_optional_int_list(filters.account_ids),
                tag_ids=_to_optional_int_list(filters.tag_ids),
                user_id=user_id,
            )
        )

        return jsonify({"success": True, "result": result})

    except ValueError as exc:
        logger.warning("创建预算执行快照参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建预算执行快照失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/history", methods=["GET"])
@log_method
@require_auth
def get_budget_history():  # pylint: disable=too-many-locals
    """获取预算执行历史快照。"""
    logger.info("[get_budget_history] 开始获取预算执行历史")

    try:
        period_scope = _build_budget_period_scope(
            budget_type=_get_optional_int(request.args.get("budget_type"), "budget_type") or 3,
            period_type=request.args.get("period_type"),
            year=_get_optional_int(request.args.get("year"), "year"),
            month=_get_optional_int(request.args.get("month"), "month"),
            quarter=_get_optional_int(request.args.get("quarter"), "quarter"),
            start_date=request.args.get("start_date"),
            end_date=request.args.get("end_date"),
        )
        filters = _parse_budget_query_filters()

        db = get_app_context()
        user_id = _get_request_user_id()
        items = _run_async(
            db.get_budget_execution_history(
                budget_type=period_scope.budget_type,
                period_type=period_scope.period_type,
                start_date=period_scope.start_date,
                end_date=period_scope.end_date,
                budget_id=filters.budget_id,
                category_id=filters.category_id,
                account_ids=_to_optional_int_list(filters.account_ids),
                tag_ids=_to_optional_int_list(filters.tag_ids),
                user_id=user_id,
            )
        )

        return jsonify(
            {
                "success": True,
                "result": {
                    "items": items,
                    "summary": {
                        "count": len(items),
                        "period_start": period_scope.start_date,
                        "period_end": period_scope.end_date,
                    },
                },
            }
        )

    except ValueError as exc:
        logger.warning("获取预算执行历史参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算执行历史失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/export", methods=["GET"])
@log_method
@require_auth
def export_budgets():
    """导出预算"""
    logger.info("[export_budgets] 开始导出预算")

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        budgets = _run_async(db.export_budgets(user_id=user_id))

        logger.info("[export_budgets] 导出%s条预算", len(budgets))

        # 保持字段顺序
        json_str = json.dumps(
            {"success": True, "result": budgets},
            ensure_ascii=False,
            separators=(",", ":"),
        )

        return Response(json_str, mimetype="application/json")

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导出预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/import", methods=["POST"])
@log_method
@require_auth
def import_budgets():
    """
    导入预算

    Request Body:
        [
            {
                "name": "餐饮预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amount": 1000.00,
                "start_date": "2024-01-01"
            }
        ]
    """
    logger.info("[import_budgets] 开始导入预算")

    try:
        data = request.get_json(silent=True)
        if not data or not isinstance(data, list):
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid data format. Expected array of budgets.",
                }
            ), 400

        for index, item in enumerate(data):
            _validate_import_budget_item(item, index)

        logger.info("[import_budgets] 准备导入%s条预算", len(data))

        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(db.import_budgets(data, user_id=user_id))

        logger.info(
            "[import_budgets] 导入完成: 创建=%s, 更新=%s, 错误=%s",
            result["created"],
            result["updated"],
            result["errors"],
        )

        return jsonify({"success": True, "result": result})

    except ValueError as exc:
        logger.warning("导入预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导入预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
