# pylint: disable=wildcard-import,unused-wildcard-import
"""Basic statistics route handlers."""

from .support import *  # noqa: F403

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


@bp.route("/category-pie", methods=["GET"])
@log_method
@require_auth
def get_category_pie():
    """获取分类饼图数据"""
    try:
        bill_type = request.args.get("type", "支出")  # 支出/收入
        start_date = request.args.get("start_date")
        end_date = request.args.get("end_date")

        db = get_app_context()

        # 构建查询条件
        filters = {"type": bill_type}
        if start_date:
            filters["start_date"] = start_date
        if end_date:
            filters["end_date"] = end_date

        bills, _ = _run_async(
            db.query_bills(
                page=1,
                page_size=100000,
                filters=filters,
                user_id=_get_request_user_id(),
            ),
        )

        # 按分类汇总
        category_totals = {}
        for bill in bills:
            category = bill.get("main_category", "未分类")
            amount = abs(float(bill.get("amount", 0)))
            category_totals[category] = category_totals.get(category, 0) + amount

        # 转换为饼图数据格式
        pie_data = [
            {"name": category, "value": amount}
            for category, amount in category_totals.items()
        ]

        # 按金额降序排序
        pie_data.sort(key=lambda x: x["value"], reverse=True)

        return jsonify({"success": True, "data": pie_data})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类饼图数据失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/top-merchants", methods=["GET"])
@log_method
@require_auth
def get_top_merchants():
    """获取TOP商家"""
    try:
        limit = int(request.args.get("limit", 10))
        start_date = request.args.get("start_date")
        end_date = request.args.get("end_date")

        db = get_app_context()

        # 构建查询条件
        filters = {}
        if start_date:
            filters["start_date"] = start_date
        if end_date:
            filters["end_date"] = end_date

        bills, _ = _run_async(
            db.query_bills(
                page=1,
                page_size=100000,
                filters=filters,
                user_id=_get_request_user_id(),
            ),
        )

        # 按商家汇总
        merchant_stats = {}
        for bill in bills:
            merchant = bill.get("counterparty", "未知商家")
            amount = abs(float(bill.get("amount", 0)))

            if merchant not in merchant_stats:
                merchant_stats[merchant] = {"amount": 0, "count": 0}

            merchant_stats[merchant]["amount"] += amount
            merchant_stats[merchant]["count"] += 1

        # 转换为列表并排序
        top_merchants = [
            {"name": merchant, "amount": stats["amount"], "count": stats["count"]}
            for merchant, stats in merchant_stats.items()
        ]

        # 按金额降序排序并限制数量
        top_merchants.sort(key=lambda x: x["amount"], reverse=True)
        top_merchants = top_merchants[:limit]

        return jsonify({"success": True, "data": top_merchants})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取TOP商家失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/amounts", methods=["GET"])
@log_method
@require_auth
def get_transaction_amounts():  # pylint: disable=too-many-locals
    """
    获取多个时间段的交易金额统计

    查询参数：
        query: 时间段查询字符串，格式: "period1_start_end|period2_start_end"
        use_transaction_timezone: 是否使用交易时区 (可选)

    返回：
        JSON响应，包含各时间段的收入/支出/净额统计
    """
    try:
        logger.info("Amounts API called with args: %s", request.args)
        query_str = request.args.get("query", "")
        if not query_str:
            # 尝试兼容旧版 API 参数 'periods'
            query_str = request.args.get("periods", "")

        # use_transaction_timezone = request.args.get('use_transaction_timezone', 'false').lower() == 'true'  # noqa: E501 # pylint: disable=line-too-long

        if not query_str:
            return jsonify({"success": False, "error": "Missing query parameter"}), 400

        db = get_app_context()
        results = {}
        user_id = _get_request_user_id()

        # 解析查询字符串: "today_1763481600_1763567999|thisWeek_1763308800_1763913599"
        for period_query in query_str.split("|"):
            parts = period_query.split("_")
            if len(parts) != 3:
                continue

            period_name, start_timestamp, end_timestamp = parts

            # 转换时间戳为日期字符串
            start_date = datetime.fromtimestamp(int(start_timestamp)).strftime("%Y-%m-%d")
            end_date = datetime.fromtimestamp(int(end_timestamp)).strftime("%Y-%m-%d")

            # 查询该时间段的账单
            bills, _ = _run_async(
                db.query_bills(
                    page=1,
                    page_size=100000,
                    filters={"start_date": start_date, "end_date": end_date},
                    user_id=user_id,
                )
            )

            # 计算统计数据（单位：元）
            total_income = sum(abs(float(bill.get("amount", 0))) for bill in bills if bill.get("type") == "收入")
            total_expense = sum(abs(float(bill.get("amount", 0))) for bill in bills if bill.get("type") == "支出")

            # 转换为前端期望的单位（分）
            # 前端使用整数分作为金额单位，避免浮点数精度问题
            income_cents = yuan_to_cents(total_income)
            expense_cents = yuan_to_cents(total_expense)

            # 构造前端期望的数据格式
            # 前端期望结构：{ startTime, endTime, amounts: [{ currency, incomeAmount, expenseAmount }] }
            results[period_name] = {
                "startTime": int(start_timestamp),
                "endTime": int(end_timestamp),
                "amounts": [
                    {
                        "currency": "CNY",  # 默认人民币，后续可扩展多币种
                        "incomeAmount": income_cents,  # 单位：分（cents）
                        "expenseAmount": expense_cents,  # 单位：分（cents）
                    }
                ],
            }

        return jsonify({"success": True, "result": results})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取交易金额统计失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500



__all__ = [name for name in globals() if not name.startswith("__")]
