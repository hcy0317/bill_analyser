# pylint: disable=wildcard-import,unused-wildcard-import
"""Categorical statistics route handlers."""

from .support import *  # noqa: F403

@bp.route("/category-statistics", methods=["GET"])
@log_method
@require_auth
def get_categorical_analysis():  # pylint: disable=too-many-locals,too-many-branches,too-many-statements
    """
    分类分析API - 按分类汇总收支统计

    查询参数：
        startTime: 开始时间戳（秒）
        endTime: 结束时间戳（秒）
        tagIds: 标签ID列表（逗号分隔，可选）
        tagFilterType: 标签过滤类型（可选）
        keyword: 关键词搜索（可选）
        useTransactionTimezone: 是否使用交易时区（可选）

    返回：
        JSON 响应格式：
        {
            "success": true,
            "result": {
                "startTime": 1732204800,
                "endTime": 1732291199,
                "items": [
                    {
                        "categoryId": "1",
                        "accountId": "214",
                        "amount": 11100  // 单位：分(cents)
                    }
                ]
            }
        }
    """
    try:
        logger.info("[分类分析] API调用: %s", request.args)
        logger.info(
            "[分类分析] 完整请求: method=%s, path=%s, args=%s",
            request.method,
            request.path,
            dict(request.args),
        )

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_time_raw = request.args.get("startTime") or request.args.get("start_time")
        end_time_raw = request.args.get("endTime") or request.args.get("end_time")
        keyword = request.args.get("keyword", "")
        # tag_ids = request.args.get('tagIds', '')
        # tag_filter_type = request.args.get('tagFilterType', type=int)
        # use_transaction_timezone = request.args.get('useTransactionTimezone', 'false').lower() == 'true'  # noqa: E501 # pylint: disable=line-too-long

        logger.info(
            "[分类分析] 解析参数: start_time=%s, end_time=%s, keyword=%s", start_time_raw, end_time_raw, keyword
        )

        # v6.88: 前端选择“全部”时会传 0/0，这里显式识别为全量查询
        start_time_str = str(start_time_raw).strip() if start_time_raw is not None else ""
        end_time_str = str(end_time_raw).strip() if end_time_raw is not None else ""
        is_all_mode = start_time_str == "0" and end_time_str == "0"

        start_time = start_time_raw
        end_time = end_time_raw

        # 如果缺少时间参数,使用本月作为默认范围
        if not is_all_mode and (start_time is None or end_time is None):
            now = datetime.now()
            # 本月第一天00:00:00
            month_start = datetime(now.year, now.month, 1)
            # 下月第一天00:00:00
            if now.month == 12:
                month_end = datetime(now.year + 1, 1, 1)
            else:
                month_end = datetime(now.year, now.month + 1, 1)

            start_time = int(month_start.timestamp())
            end_time = int(month_end.timestamp()) - 1  # 本月最后一秒

            logger.warning(
                "[分类分析] 缺少时间参数,使用本月作为默认范围: start_time=%d, end_time=%d (%s ~ %s)",
                start_time,
                end_time,
                month_start.strftime("%Y-%m-%d"),
                month_end.strftime("%Y-%m-%d"),
            )

        filters = {}
        if not is_all_mode:
            # 转换为整数
            try:
                start_time = int(start_time)
                end_time = int(end_time)
            except (ValueError, TypeError) as e:
                logger.error("[分类分析] 时间戳格式错误: %s", e)
                return jsonify({"success": False, "error": f"Invalid timestamp format: {e}"}), 400

            if start_time > end_time:
                logger.error("[分类分析] 非法时间范围: start_time=%s, end_time=%s", start_time, end_time)
                return jsonify(
                    {
                        "success": False,
                        "error": "Invalid time range",
                        "message": "startTime must be less than or equal to endTime",
                    }
                ), 400

            # 转换时间戳为日期字符串
            start_date = datetime.fromtimestamp(start_time).strftime("%Y-%m-%d")
            end_date = datetime.fromtimestamp(end_time).strftime("%Y-%m-%d")
            filters = {"start_date": start_date, "end_date": end_date}
            logger.info("[分类分析] 查询时间范围: %s 到 %s", start_date, end_date)
        else:
            start_time = 0
            end_time = 0
            logger.info("[分类分析] 使用全部时间范围查询（不加时间过滤）")

        db = get_app_context()

        if keyword:
            filters["keyword"] = keyword
            logger.info("[分类分析] 关键词筛选: %s", keyword)

        user_id = _get_request_user_id()

        # 查询账单数据
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            bills, _ = loop.run_until_complete(
                db.query_bills(page=1, page_size=100000, filters=filters, user_id=user_id)
            )
            logger.info("[分类分析] 查询到 %d 条账单", len(bills))

            # 记录查询到的账单详情（前3条示例）
            for i, bill in enumerate(bills[:3]):
                logger.debug(
                    "[分类分析] 账单示例%d: ID=%s, 日期=%s, 类型=%s, 金额=%s, 渠道=%s, 分类=%s-%s",
                    i + 1,
                    bill.get("id"),
                    bill.get("date"),
                    bill.get("type"),
                    bill.get("amount"),
                    bill.get("channel"),
                    bill.get("main_category"),
                    bill.get("sub_category"),
                )

            # 获取所有分类和账户映射（用于ID转换）
            categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))

            logger.debug(
                "[分类分析] 分类列表: %d个，前3个: %s",
                len(categories),
                [(c.get("id"), c.get("main_category"), c.get("sub_category")) for c in categories[:3]],
            )
            logger.debug(
                "[分类分析] 账户列表: %d个，前3个: %s",
                len(accounts),
                [(a.get("id"), a.get("name")) for a in accounts[:3]],
            )
        finally:
            loop.close()

        # 构建分类和账户名称到ID的映射
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

        logger.debug(
            "[分类分析] 分类映射: %d 个, 账户映射: %d 个",
            len(category_name_to_id),
            len(account_name_to_id),
        )

        # 按 (分类ID, 账户ID) 汇总金额
        statistics_map = {}

        logger.info("[分类分析] 开始处理 %d 条账单进行统计汇总...", len(bills))

        for idx, bill in enumerate(bills):
            # 获取分类ID
            main_category = bill.get("main_category", "未分类")
            sub_category = bill.get("sub_category", "")
            cat_key = f"{main_category}-{sub_category}" if sub_category else main_category
            category_id = category_name_to_id.get(cat_key, "0")

            # 获取账户ID
            account_name = bill.get("channel", "未知账户")
            account_id = str(bill.get("source_account_id", 0))

            if account_id not in valid_account_ids:
                account_id = account_name_to_id.get(account_name, "0")

            # 计算金额（分为单位）
            amount_yuan = float(bill.get("amount", 0))
            amount_cents = yuan_to_cents(amount_yuan)

            # 按交易类型处理符号
            bill_type = bill.get("type", "")
            if bill_type == "支出":
                amount_cents = -abs(amount_cents)  # 支出为负数
            elif bill_type == "收入":
                amount_cents = abs(amount_cents)  # 收入为正数
            elif bill_type == "转账":
                # 转账根据账户判断方向
                dest_account = bill.get("destination_account", "")
                if dest_account and dest_account != account_name:
                    amount_cents = -abs(amount_cents)  # 转出
                else:
                    amount_cents = abs(amount_cents)  # 转入

            # 汇总到统计字典
            key = (category_id, account_id)
            if key not in statistics_map:
                statistics_map[key] = 0
            statistics_map[key] += amount_cents

            # 记录前3条账单的处理详情
            if idx < 3:
                logger.debug(
                    "[分类分析] 处理账单%d: 类型=%s, 原始金额=%.2f元, 转换为%d分, 分类ID=%s, 账户ID=%s, 累计=%d分",
                    idx + 1,
                    bill_type,
                    amount_yuan,
                    amount_cents,
                    category_id,
                    account_id,
                    statistics_map[key],
                )

        # 转换为前端期望的数组格式
        result_items = [
            {"categoryId": cat_id, "accountId": acc_id, "amount": amount}
            for (cat_id, acc_id), amount in statistics_map.items()
        ]

        logger.info("[分类分析] 生成 %d 条统计记录", len(result_items))

        # 记录统计结果详情（前3条）
        for i, item in enumerate(result_items[:3]):
            logger.debug(
                "[分类分析] 统计结果%d: 分类ID=%s, 账户ID=%s, 金额=%d分 (%.2f元)",
                i + 1,
                item["categoryId"],
                item["accountId"],
                item["amount"],
                item["amount"] / 100.0,
            )

        logger.info(
            "[分类分析] ✅ 返回结果: startTime=%d, endTime=%d, items=%d条", start_time, end_time, len(result_items)
        )

        return jsonify(
            {"success": True, "result": {"startTime": start_time, "endTime": end_time, "items": result_items}}
        )

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[分类分析] 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500



__all__ = [name for name in globals() if not name.startswith("__")]
