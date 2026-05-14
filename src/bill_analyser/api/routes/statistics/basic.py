"""Basic statistics route handlers."""

from .support import (
    Analyzer,
    _run_async,
    bp,
    get_app_context,
    jsonify,
    logger,
    log_method,
    request,
    require_auth,
)


@bp.route("/overview", methods=["GET"])
@log_method
@require_auth
def get_overview():
    """获取概览统计"""
    try:
        period = request.args.get("period", "month")
        start_date = request.args.get("start_date")
        end_date = request.args.get("end_date")

        db = get_app_context()
        analyzer = Analyzer(db=db)

        # 构建过滤条件
        filters = {}
        if start_date:
            filters["start_date"] = start_date
        if end_date:
            filters["end_date"] = end_date

        data = _run_async(
            analyzer.generate_report(period=period, filters=filters if filters else None),
        )

        summary_data = data.get("summary", {})
        total_income = round(float(summary_data.get("total_income", 0) or 0), 2)
        total_expense = round(abs(float(summary_data.get("total_expense", 0) or 0)), 2)
        net_income = round(total_income - total_expense, 2)

        # 提取summary字段到顶层以符合测试预期
        result = {
            "total_income": total_income,
            "total_expense": total_expense,
            "net_income": net_income,
            "bill_count": data.get("total_records", 0),
            "by_category": data.get("by_category", {}),
            "by_type": data.get("by_type", {}),
            "top_income": data.get("top_income", []),
            "top_expenses": data.get("top_expenses", []),
            "period": data.get("period", ""),
            "start_date": data.get("start_date", ""),
            "end_date": data.get("end_date", ""),
        }

        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取概览统计失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/trends", methods=["GET"])
@log_method
@require_auth
def get_trends():
    """获取趋势数据"""
    try:
        period = request.args.get("period", "month")
        category = request.args.get("category")

        db = get_app_context()
        analyzer = Analyzer(db=db)
        trends = _run_async(analyzer.get_trends(period=period, category=category))

        return jsonify({"success": True, "result": trends})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取趋势数据失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/comparison", methods=["GET"])
@log_method
@require_auth
def get_comparison():
    """获取对比数据"""
    try:
        period = request.args.get("period", "month")
        compare_type = request.args.get("type", "category")  # 可选值：category、month、year

        db = get_app_context()
        analyzer = Analyzer(db=db)
        comparison = _run_async(analyzer.get_comparison(period=period, compare_type=compare_type))

        return jsonify({"success": True, "result": comparison})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取对比数据失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/category", methods=["GET"])
@log_method
@require_auth
def get_category_analysis():
    """获取分类分析"""
    try:
        period = request.args.get("period", "month")
        main_category = request.args.get("main_category")

        db = get_app_context()
        analyzer = Analyzer(db=db)
        analysis = _run_async(analyzer.analyze_category(period=period, main_category=main_category))

        return jsonify({"success": True, "data": analysis})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类分析失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/trend", methods=["GET"])
@log_method
@require_auth
def get_trend():
    """获取趋势数据"""
    try:
        granularity = request.args.get("granularity", "month")  # 可选值：day/week/month
        category = request.args.get("category")

        db = get_app_context()
        analyzer = Analyzer(db=db)
        result = _run_async(analyzer.get_trends(period=granularity, category=category))

        # 提取trends数组,并转换字段名以符合测试预期
        trends_data = result.get("trends", [])
        formatted_trends = []
        for trend in trends_data:
            formatted_trends.append(
                {
                    "date": trend.get("period", ""),
                    "income": trend.get("income", 0),
                    "expense": trend.get("expense", 0),
                    "net": trend.get("net", 0),
                }
            )

        return jsonify({"success": True, "data": formatted_trends})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取趋势数据失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
