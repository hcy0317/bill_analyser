"""budgets execution route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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

        summary = build_budget_execution_summary(results)

        logger.info("[get_budget_execution] 返回%s条执行详情", len(results))

        return jsonify(
            {
                "success": True,
                "result": {
                    "items": results,
                    "summary": summary,
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
