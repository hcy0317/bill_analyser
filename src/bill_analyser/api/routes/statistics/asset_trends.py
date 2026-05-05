# pylint: disable=wildcard-import,unused-wildcard-import
"""Asset trend statistics route handlers."""

from .support import *  # noqa: F403

@bp.route("/asset-trends", methods=["GET"])
@log_method
@require_auth
def get_asset_trends():  # pylint: disable=too-many-locals,too-many-branches,too-many-statements
    """
    资产趋势API - 每日账户余额变化统计

    查询参数：
        startTime: 开始时间戳（秒）
        endTime: 结束时间戳（秒）

    返回：
        JSON 响应格式：
        {
            "success": true,
            "result": [
                {
                    "year": 2024,
                    "month": 11,
                    "day": 21,
                    "items": [
                        {
                            "accountId": "214",
                            "accountOpeningBalance": 0,
                            "accountClosingBalance": 11100
                        }
                    ]
                }
            ]
        }
    """
    try:
        # v6.72: API调用日志改为DEBUG级别，仅保留开始和完成的INFO日志
        logger.debug("[资产趋势] API调用: %s", request.args)
        logger.debug(
            "[资产趋势] 完整请求: method=%s, path=%s, args=%s",
            request.method,
            request.path,
            dict(request.args),
        )

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_time_raw = request.args.get("startTime") or request.args.get("start_time")
        end_time_raw = request.args.get("endTime") or request.args.get("end_time")

        # v6.72: 参数解析改为DEBUG级别
        logger.debug("[资产趋势] 解析参数: start_time=%s, end_time=%s", start_time_raw, end_time_raw)

        # v6.88: 前端“全部”会传 0/0，识别为全量模式
        start_time_str = str(start_time_raw).strip() if start_time_raw is not None else ""
        end_time_str = str(end_time_raw).strip() if end_time_raw is not None else ""
        is_all_mode = start_time_str == "0" and end_time_str == "0"

        start_time = start_time_raw
        end_time = end_time_raw

        user_id = _get_request_user_id()

        if is_all_mode:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                db = get_app_context()
                bills_all, _ = loop.run_until_complete(
                    db.query_bills(
                        page=1,
                        page_size=1000000,
                        filters={},
                        user_id=user_id,
                    )
                )
            finally:
                loop.close()

            bill_timestamps = []
            for bill in bills_all:
                bill_date_str = bill.get("date", "")
                if not bill_date_str:
                    continue
                try:
                    bill_date = datetime.fromisoformat(bill_date_str.replace("Z", "+00:00"))
                    bill_timestamps.append(int(bill_date.timestamp()))
                except (ValueError, AttributeError):
                    continue

            if not bill_timestamps:
                logger.info("[资产趋势] 全量模式无账单数据，返回空结果")
                return jsonify({"success": True, "result": []})

            start_time = min(bill_timestamps)
            end_time = max(bill_timestamps)
            logger.info(
                "[资产趋势] 使用全量模式时间范围: %s ~ %s",
                datetime.fromtimestamp(start_time).strftime("%Y-%m-%d"),
                datetime.fromtimestamp(end_time).strftime("%Y-%m-%d"),
            )

        if not is_all_mode and (start_time is None or end_time is None):
            # v6.84: 资产趋势缺少时间参数时，默认使用本月范围（与分类分析接口保持一致）
            now = datetime.now()
            month_start = datetime(now.year, now.month, 1)

            if now.month == 12:
                next_month = datetime(now.year + 1, 1, 1)
            else:
                next_month = datetime(now.year, now.month + 1, 1)

            month_end = next_month - timedelta(seconds=1)

            start_time = int(month_start.timestamp())
            end_time = int(month_end.timestamp())

            logger.warning(
                "[资产趋势] 缺少时间参数,使用本月作为默认范围: start_time=%d, end_time=%d (%s ~ %s)",
                start_time,
                end_time,
                month_start.strftime("%Y-%m-%d"),
                month_end.strftime("%Y-%m-%d"),
            )

        # 转换为整数
        try:
            start_time = int(start_time)
            end_time = int(end_time)
        except (ValueError, TypeError) as exc:
            logger.error("[资产趋势] 时间戳格式错误: %s", exc)
            return jsonify({"success": False, "error": f"Invalid timestamp format: {exc}"}), 400

        if start_time > end_time:
            logger.error("[资产趋势] 非法时间范围: start_time=%s, end_time=%s", start_time, end_time)
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid time range",
                    "message": "startTime must be less than or equal to endTime",
                }
            ), 400

        # 【关键】验证时间范围，防止无限循环（限制最多365天，支持本年查询）
        time_diff_seconds = end_time - start_time
        time_diff_days = time_diff_seconds // 86400
        # v6.72: 时间跨度改为DEBUG级别，除非超限
        logger.debug("[资产趋势] 时间跨度: %d天", time_diff_days)

        if not is_all_mode and time_diff_days > 365:
            logger.warning("[资产趋势] 时间跨度超过365天限制，拒绝请求: %d天", time_diff_days)
            error_msg = "资产趋势查询最多支持365天范围，请缩小时间范围"
            return jsonify(
                {
                    "success": False,
                    "errorMessage": error_msg,
                    "error": error_msg,  # 兼容前端error字段
                    "errorCode": 400,
                }
            ), 400

        # 转换时间戳为日期
        start_date = datetime.fromtimestamp(start_time)
        end_date = datetime.fromtimestamp(end_time)

        # v6.72: 查询开始时用INFO记录一次，后续详情用DEBUG
        logger.info(
            "[资产趋势] 开始查询: %s ~ %s (%d天)",
            start_date.strftime("%Y-%m-%d"),
            end_date.strftime("%Y-%m-%d"),
            time_diff_days,
        )

        db = get_app_context()

        # 优化：一次性查询所有数据，避免N+1查询
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 1. 获取所有账户
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
            # v6.72: 中间步骤日志改为DEBUG级别
            logger.debug("[资产趋势] 查询到 %d 个账户", len(accounts))

            # 2. 获取起始日期前的所有账户余额（期初余额）
            start_date_str = start_date.strftime("%Y-%m-%d")
            initial_balances = loop.run_until_complete(
                db.get_balances_before_date(start_date_str, user_id=user_id)
            )
            logger.debug("[资产趋势] 已计算期初余额")

            # 3. 获取时间范围内的所有账单
            end_date_str = end_date.strftime("%Y-%m-%d")
            bills_in_range, _ = loop.run_until_complete(
                db.query_bills(
                    page=1,
                    page_size=1000000,  # 足够大的数量以获取所有账单
                    filters={"start_date": start_date_str, "end_date": end_date_str},
                    user_id=user_id,
                )
            )
            logger.debug("[资产趋势] 查询到范围内 %d 条账单", len(bills_in_range))

        finally:
            loop.close()

        # 初始化当前余额（账户初始余额 + 历史累计余额）
        current_balances = {}
        for acc in accounts:
            acc_id = acc["id"]
            # 账户创建时的初始余额
            acc_initial = float(acc.get("initial_balance", 0.0))
            # 历史累计余额
            history_balance = initial_balances.get(acc_id, 0.0)
            current_balances[acc_id] = acc_initial + history_balance

        # 按日期分组账单
        bills_by_date = {}
        for bill in bills_in_range:
            # 截取日期部分 YYYY-MM-DD
            date_str = bill.get("date", "")[:10]
            if not date_str:
                continue
            if date_str not in bills_by_date:
                bills_by_date[date_str] = []
            bills_by_date[date_str].append(bill)

        # 按日期遍历，计算每日余额
        result = []
        current_date = start_date
        day_count = 0

        while current_date <= end_date:
            day_count += 1
            date_str = current_date.strftime("%Y-%m-%d")

            # v6.72: 移除逐日计算日志，改为最终汇总日志（见函数末尾）
            # 原有逻辑: if day_count <= 3 or day_count % 10 == 0: logger.info(...)

            # 获取当天的账单
            day_bills = bills_by_date.get(date_str, [])

            # 保存当天的期初余额快照
            opening_balances = current_balances.copy()

            # 根据当天账单更新余额
            for bill in day_bills:
                amount = float(bill.get("amount", 0))
                bill_type = bill.get("type", "")
                source_id = bill.get("source_account_id", 0)
                dest_id = bill.get("destination_account_id", 0)
                dest_amount = float(bill.get("destination_amount", 0))
                if dest_amount == 0:
                    dest_amount = amount

                if bill_type == "收入":
                    if source_id in current_balances:
                        current_balances[source_id] += amount
                elif bill_type == "支出":
                    if source_id in current_balances:
                        current_balances[source_id] -= abs(amount)
                elif bill_type == "转账":
                    if source_id in current_balances:
                        current_balances[source_id] -= abs(amount)
                    if dest_id in current_balances:
                        current_balances[dest_id] += abs(dest_amount)

            # 生成当天的结果项
            day_items = []
            for acc in accounts:
                acc_id = acc["id"]
                opening = opening_balances.get(acc_id, 0.0)
                closing = current_balances.get(acc_id, 0.0)

                day_items.append(
                    {
                        "accountId": str(acc_id),
                        "accountOpeningBalance": yuan_to_cents(opening),
                        "accountClosingBalance": yuan_to_cents(closing),
                    }
                )

            result.append(
                {
                    "year": current_date.year,
                    "month": current_date.month,
                    "day": current_date.day,
                    "items": day_items,
                }
            )

            current_date += timedelta(days=1)

        # v6.72: 合并日志输出，只保留一条汇总日志
        if result:
            total_accounts_per_day = len(result[0]["items"]) if result[0]["items"] else 0
            logger.info(
                "[资产趋势] 完成: %d天, %d个账户, 日期范围 %s ~ %s",
                len(result),
                total_accounts_per_day,
                (
                    result[0]["year"] * 10000
                    + result[0]["month"] * 100
                    + result[0]["day"]
                    if result
                    else 0
                ),
                (
                    result[-1]["year"] * 10000
                    + result[-1]["month"] * 100
                    + result[-1]["day"]
                    if result
                    else 0
                ),
            )

        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[资产趋势] 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
