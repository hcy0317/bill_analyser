"""budgets history route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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
