# pylint: disable=wildcard-import,unused-wildcard-import
"""Category trend statistics route handlers."""

from .support import *  # noqa: F403

@bp.route("/category-statistics/trends", methods=["GET"])
@log_method
@require_auth
def get_trend_analysis():  # pylint: disable=too-many-locals,too-many-branches,too-many-statements
    """
    趋势分析API - 按年月分组的分类统计

    查询参数：
        startYearMonth: 开始年月（格式: 202411）
        endYearMonth: 结束年月（格式: 202412）
        tagIds: 标签ID列表（逗号分隔，可选）
        tagFilterType: 标签过滤类型（可选）
        keyword: 关键词搜索（可选）
        useTransactionTimezone: 是否使用交易时区（可选）

    返回：
        JSON 响应格式：
        {
            "success": true,
            "result": [
                {
                    "year": 2024,
                    "month": 11,
                    "items": [
                        {
                            "categoryId": "1",
                            "accountId": "214",
                            "amount": 11100
                        }
                    ]
                }
            ]
        }
    """
    try:
        logger.info("[趋势分析] API调用: %s", request.args)
        logger.info(
            "[趋势分析] 完整请求: method=%s, path=%s, args=%s",
            request.method,
            request.path,
            dict(request.args),
        )

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_year_month = (
            request.args.get("startYearMonth") or request.args.get("start_year_month") or ""
        )
        end_year_month = (
            request.args.get("endYearMonth") or request.args.get("end_year_month") or ""
        )
        keyword = request.args.get("keyword", "")

        logger.info(
            "[趋势分析] 解析参数: start_year_month=%s, end_year_month=%s, keyword=%s",
            start_year_month,
            end_year_month,
            keyword,
        )

        # v6.88: 前端选择“全部”时会传 1970-01 / 197001，这里识别为全量查询
        start_year_month_clean = start_year_month.replace("-", "") if start_year_month else ""
        end_year_month_clean = end_year_month.replace("-", "") if end_year_month else ""
        is_all_mode = (
            start_year_month_clean in ["0", "197001"]
            and end_year_month_clean in ["0", "197001"]
        )

        if not is_all_mode and (not start_year_month or not end_year_month):
            now = datetime.now()
            start_year_month = f"{now.year}01"  # 本年1月
            end_year_month = f"{now.year}12"  # 本年12月

            logger.warning(
                (
                    "[趋势分析] 缺少年月参数,使用本年作为默认范围: "
                    "start_year_month=%s, end_year_month=%s"
                ),
                start_year_month,
                end_year_month,
            )

        if not is_all_mode:
            # 解析年月字符串（支持两种格式: 202411 或 2024-11）
            try:
                # 去除连字符
                start_year_month_clean = start_year_month.replace("-", "")
                end_year_month_clean = end_year_month.replace("-", "")

                logger.info(
                    "[趋势分析] 清理后的年月: start=%s, end=%s",
                    start_year_month_clean,
                    end_year_month_clean,
                )

                start_year = int(start_year_month_clean[:4])
                start_month = int(start_year_month_clean[4:6])
                end_year = int(end_year_month_clean[:4])
                end_month = int(end_year_month_clean[4:6])

                logger.info(
                    "[趋势分析] 解析年月: start=%d-%02d, end=%d-%02d",
                    start_year,
                    start_month,
                    end_year,
                    end_month,
                )

                if (start_year, start_month) > (end_year, end_month):
                    logger.error(
                        "[趋势分析] 非法年月范围: start=%d-%02d, end=%d-%02d",
                        start_year,
                        start_month,
                        end_year,
                        end_month,
                    )
                    return jsonify(
                        {
                            "success": False,
                            "error": "Invalid year-month range",
                            "message": "startYearMonth must be less than or equal to endYearMonth",
                        }
                    ), 400
            except (ValueError, IndexError) as exc:
                logger.error("[趋势分析] 年月格式错误: %s", exc)
                return jsonify(
                    {
                        "success": False,
                        "error": f"Invalid year-month format (expected: 202411 or 2024-11): {exc}",
                    }
                ), 400

            start_date = f"{start_year}-{start_month:02d}-01"
            # 计算结束日期（月末最后一天）
            _, last_day = monthrange(end_year, end_month)
            end_date = f"{end_year}-{end_month:02d}-{last_day}"
            logger.info("[趋势分析] 查询时间范围: %s 到 %s", start_date, end_date)
        else:
            start_year = 0
            start_month = 0
            end_year = 0
            end_month = 0
            start_date = ""
            end_date = ""
            logger.info("[趋势分析] 使用全部时间范围查询（不加年月过滤）")

        db = get_app_context()

        # 构建查询过滤条件
        filters = {}
        if not is_all_mode:
            filters["start_date"] = start_date
            filters["end_date"] = end_date

        if keyword:
            filters["keyword"] = keyword

        user_id = _get_request_user_id()

        # 查询账单数据
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            bills, _ = loop.run_until_complete(
                db.query_bills(page=1, page_size=100000, filters=filters, user_id=user_id)
            )
            logger.info("[趋势分析] 查询到 %d 条账单", len(bills))

            # 记录账单日期分布
            if bills:
                dates = [b.get("date", "")[:10] for b in bills if b.get("date")]
                logger.debug(
                    "[趋势分析] 账单日期范围: %s 到 %s",
                    min(dates) if dates else "N/A",
                    max(dates) if dates else "N/A",
                )
                # 前3条账单示例
                for i, bill in enumerate(bills[:3]):
                    logger.debug(
                        "[趋势分析] 账单示例%d: ID=%s, 日期=%s, 类型=%s, 金额=%s",
                        i + 1,
                        bill.get("id"),
                        bill.get("date"),
                        bill.get("type"),
                        bill.get("amount"),
                    )

            # 获取映射数据
            categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))

            logger.debug("[趋势分析] 映射数据: 分类=%d个, 账户=%d个", len(categories), len(accounts))
        finally:
            loop.close()

        # 构建映射
        category_name_to_id = {}
        for cat in categories:
            main_cat = cat.get("main_category", "")
            sub_cat = cat.get("sub_category", "")
            key = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
            category_name_to_id[key] = str(cat["id"])

        account_name_to_id = {}
        valid_account_ids = set()
        for acc in accounts:
            account_name_to_id[acc["name"]] = str(acc["id"])
            valid_account_ids.add(str(acc["id"]))

        # 按年月分组统计
        # 结构：{(year, month): {(category_id, account_id): amount}}
        monthly_stats = {}

        for bill in bills:
            # 解析账单日期
            bill_date_str = bill.get("date", "")
            if not bill_date_str:
                continue

            try:
                bill_date = datetime.fromisoformat(bill_date_str.replace("Z", "+00:00"))
                year = bill_date.year
                month = bill_date.month
            except (ValueError, AttributeError):
                continue

            # 获取分类和账户ID
            main_category = bill.get("main_category", "未分类")
            sub_category = bill.get("sub_category", "")
            cat_key = f"{main_category}-{sub_category}" if sub_category else main_category
            category_id = category_name_to_id.get(cat_key, "0")

            account_name = bill.get("channel", "未知账户")
            account_id = str(bill.get("source_account_id", 0))

            if account_id not in valid_account_ids:
                account_id = account_name_to_id.get(account_name, "0")

            # 计算金额
            amount_yuan = float(bill.get("amount", 0))
            amount_cents = yuan_to_cents(amount_yuan)

            bill_type = bill.get("type", "")
            if bill_type == "支出":
                amount_cents = -abs(amount_cents)
            elif bill_type == "收入":
                amount_cents = abs(amount_cents)
            elif bill_type == "转账":
                dest_account = bill.get("destination_account", "")
                if dest_account and dest_account != account_name:
                    amount_cents = -abs(amount_cents)
                else:
                    amount_cents = abs(amount_cents)

            # 汇总统计
            month_key = (year, month)
            if month_key not in monthly_stats:
                monthly_stats[month_key] = {}

            stat_key = (category_id, account_id)
            if stat_key not in monthly_stats[month_key]:
                monthly_stats[month_key][stat_key] = 0
            monthly_stats[month_key][stat_key] += amount_cents

        # v6.88: 全量模式下按实际账单时间动态计算起止年月
        if is_all_mode:
            valid_bill_dates = []
            for bill in bills:
                bill_date_str = bill.get("date", "")
                if not bill_date_str:
                    continue
                try:
                    bill_date = datetime.fromisoformat(bill_date_str.replace("Z", "+00:00"))
                    valid_bill_dates.append((bill_date.year, bill_date.month))
                except (ValueError, AttributeError):
                    continue

            if valid_bill_dates:
                sorted_dates = sorted(valid_bill_dates)
                start_year, start_month = sorted_dates[0]
                end_year, end_month = sorted_dates[-1]
                logger.info(
                    "[趋势分析] 全量模式动态范围: %d-%02d 到 %d-%02d",
                    start_year,
                    start_month,
                    end_year,
                    end_month,
                )
            else:
                logger.info("[趋势分析] 全量模式无账单数据，返回空结果")
                return jsonify({"success": True, "result": []})

        # 转换为前端期望的数组格式
        result = []

        logger.info(
            "[趋势分析] 月度统计: 共处理 %d 个月的数据，有数据的月份: %d 个",
            (end_year - start_year) * 12 + (end_month - start_month) + 1,
            len(monthly_stats),
        )
        logger.debug("[趋势分析] 有数据的月份: %s", list(monthly_stats.keys()))

        # 遍历所有年月（包括没有数据的月份）
        current_year, current_month = start_year, start_month
        while (current_year, current_month) <= (end_year, end_month):
            month_key = (current_year, current_month)
            month_data = monthly_stats.get(month_key, {})

            items = [
                {"categoryId": cat_id, "accountId": acc_id, "amount": amount}
                for (cat_id, acc_id), amount in month_data.items()
            ]

            result.append(
                {"year": current_year, "month": current_month, "items": items},
            )

            # 递增月份
            current_month += 1
            if current_month > 12:
                current_month = 1
                current_year += 1

        logger.info("[趋势分析] 生成 %d 个月份的统计数据", len(result))

        # 记录结果详情（前3个月）
        for i, month_data in enumerate(result[:3]):
            logger.debug(
                "[趋势分析] 月份%d: %d年%d月, 统计项=%d个",
                i + 1,
                month_data["year"],
                month_data["month"],
                len(month_data["items"]),
            )
            if month_data["items"]:
                # 计算该月总收入和总支出
                total_income = sum(
                    item["amount"] for item in month_data["items"] if item["amount"] > 0
                )
                total_expense = sum(
                    abs(item["amount"]) for item in month_data["items"] if item["amount"] < 0
                )
                logger.debug(
                    "[趋势分析]   该月: 收入=%.2f元, 支出=%.2f元",
                    total_income / 100.0,
                    total_expense / 100.0,
                )

        logger.info("[趋势分析] ✅ 返回结果: %d个月的数据", len(result))

        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[趋势分析] 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500



__all__ = [name for name in globals() if not name.startswith("__")]
