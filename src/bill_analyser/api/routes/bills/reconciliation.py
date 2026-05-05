# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

def _log_reconciliation_entry() -> None:
    logger.info(
        "[get_reconciliation_statements] 入口参数: account_id=%s, start_time=%s, end_time=%s, "
        "category_ids=%s, type=%s, keyword=%s",
        request.args.get("account_id"),
        request.args.get("start_time"),
        request.args.get("end_time"),
        request.args.get("category_ids"),
        request.args.get("type"),
        request.args.get("keyword"),
    )


def _parse_reconciliation_query():
    account_id = request.args.get("account_id")
    start_time = request.args.get("start_time", type=int)
    end_time = request.args.get("end_time", type=int)
    if account_id is None or start_time is None or end_time is None:
        logger.error("[get_reconciliation_statements] 缺少必需参数")
        return None, jsonify({"success": False, "error": "Missing required parameters: account_id, start_time, end_time"}), 400

    if start_time == 0 and end_time == 0:
        start_date = None
        end_date = None
        logger.info("[get_reconciliation_statements] 查询全部时间范围")
    else:
        start_date = datetime.fromtimestamp(start_time).strftime("%Y-%m-%d")
        end_date = datetime.fromtimestamp(end_time).strftime("%Y-%m-%d")
        logger.info("[get_reconciliation_statements] 时间范围: %s 至 %s", start_date, end_date)

    try:
        account_id_int = int(account_id)
    except (ValueError, TypeError):
        logger.error("[get_reconciliation_statements] 无效的账户ID: %s", account_id)
        return None, jsonify({"success": False, "error": f"Invalid account_id: {account_id}"}), 400

    return {
        "account_id": account_id,
        "account_id_int": account_id_int,
        "start_time": start_time,
        "end_time": end_time,
        "start_date": start_date,
        "end_date": end_date,
        "category_ids": request.args.get("category_ids"),
        "trans_type": request.args.get("type", type=int),
        "keyword": request.args.get("keyword"),
    }, None, None


def _build_reconciliation_filters(params: dict[str, Any]) -> dict[str, Any]:
    filters: dict[str, Any] = {"account_ids": [params["account_id_int"]]}
    if params["start_date"] is not None and params["end_date"] is not None:
        filters["start_date"] = params["start_date"]
        filters["end_date"] = params["end_date"]
    if params["category_ids"]:
        category_id_list = [cid.strip() for cid in params["category_ids"].split(",") if cid.strip()]
        if category_id_list:
            filters["category_ids"] = category_id_list
            logger.info("[get_reconciliation_statements] 分类筛选: %s", category_id_list)
    if params["trans_type"]:
        type_map = {1: "收入", 2: "支出", 3: "转账", 4: "投资"}
        if params["trans_type"] in type_map:
            filters["type"] = type_map[params["trans_type"]]
            logger.info("[get_reconciliation_statements] 类型筛选: %s (%d)", filters["type"], params["trans_type"])
    if params["keyword"]:
        filters["keyword"] = params["keyword"]
        logger.info("[get_reconciliation_statements] 关键词筛选: %s", params["keyword"])
    return filters


def _load_reconciliation_opening_balance(loop, db, account, params: dict[str, Any]) -> float:
    try:
        if params["start_date"] is not None:
            pre_filters = {"end_date": params["start_date"], "account_ids": [params["account_id_int"]]}
            pre_bills, _ = loop.run_until_complete(
                db.query_bills(page=1, page_size=1, filters=pre_filters, user_id=request.user_id)
            )
            if pre_bills and len(pre_bills) > 0:
                opening_balance = float(pre_bills[0].get("account_balance", 0))
                logger.info("[get_reconciliation_statements] 期初余额: %.2f (基于历史账单)", opening_balance)
                return opening_balance
            logger.info("[get_reconciliation_statements] 期初余额: 0.00 (无历史账单)")
            return 0.0

        opening_balance = float(account.get("initial_balance", 0))
        logger.info("[get_reconciliation_statements] 期初余额: %.2f (账户初始余额)", opening_balance)
        return opening_balance
    except Exception as balance_err:
        logger.warning("[get_reconciliation_statements] 获取期初余额失败: %s", balance_err)
        return 0.0


def _load_reconciliation_category_map(loop, db) -> dict[tuple[str, str], dict[str, Any]]:
    db_category_map = loop.run_until_complete(db.get_category_mappings(user_id=request.user_id))
    category_map: dict[tuple[str, str], dict[str, Any]] = {}
    if db_category_map and "id_to_category" in db_category_map:
        for cat in db_category_map["id_to_category"].values():
            category_map[(cat["main_category"], cat["sub_category"])] = cat
    return category_map


def _calculate_reconciliation_summary(
    bills_sorted: list[dict[str, Any]],
    account_id_int: int,
    opening_balance: float,
) -> dict[str, Any]:
    total_inflows = 0.0
    total_outflows = 0.0
    current_balance = opening_balance
    balance_history = {}

    for bill in bills_sorted:
        bill_type = bill.get("type", "")
        amount = float(bill.get("amount", 0))
        source_acc_id = int(bill.get("source_account_id")) if bill.get("source_account_id") else None
        dest_acc_id = int(bill.get("destination_account_id")) if bill.get("destination_account_id") else None
        transaction_opening_balance = current_balance

        if bill_type == "收入":
            total_inflows += amount
            current_balance += amount
        elif bill_type == "支出":
            total_outflows += amount
            current_balance -= amount
        elif bill_type in {"转账", "投资"}:
            if dest_acc_id and dest_acc_id == account_id_int:
                total_inflows += amount
                current_balance += amount
            elif source_acc_id and source_acc_id == account_id_int:
                total_outflows += amount
                current_balance -= amount
            elif bill_type == "转账":
                logger.warning("[get_reconciliation_statements] 转账账单但账户ID不匹配: bill_id=%s", bill.get("id"))
                continue
        else:
            logger.warning("[get_reconciliation_statements] 未知账单类型: %s", bill_type)
            continue

        balance_history[bill["id"]] = {"opening": transaction_opening_balance, "closing": current_balance}
        logger.info(
            "[get_reconciliation_statements] ID=%s, Date=%s, Type=%s, Amount=%s → Opening=%s, Closing=%s",
            bill["id"],
            bill.get("date"),
            bill_type,
            amount,
            transaction_opening_balance,
            current_balance,
        )

    return {
        "opening_balance": opening_balance,
        "closing_balance": current_balance,
        "total_inflows": total_inflows,
        "total_outflows": total_outflows,
        "balance_history": balance_history,
    }


def _build_reconciliation_transactions(loop, adapter, bills_sorted, account_map, category_map, balance_history):
    transactions = []
    for bill in bills_sorted:
        bill_id = bill["id"]
        if bill_id not in balance_history:
            continue
        v1_transaction = loop.run_until_complete(adapter.backend_to_frontend(bill, account_map, category_map))
        v1_transaction["accountOpeningBalance"] = yuan_to_cents(balance_history[bill_id]["opening"])
        v1_transaction["accountClosingBalance"] = yuan_to_cents(balance_history[bill_id]["closing"])
        transactions.append(v1_transaction)
    transactions.sort(key=lambda x: x["time"], reverse=True)
    return transactions


def _build_reconciliation_result(params, account_name, summary, transactions) -> dict[str, Any]:
    net_flow = summary["total_inflows"] - summary["total_outflows"]
    return {
        "accountId": str(params["account_id"]),
        "accountName": account_name,
        "startTime": params["start_time"],
        "endTime": params["end_time"],
        "openingBalance": yuan_to_cents(summary["opening_balance"]),
        "closingBalance": yuan_to_cents(summary["closing_balance"]),
        "totalInflows": yuan_to_cents(summary["total_inflows"]),
        "totalOutflows": yuan_to_cents(summary["total_outflows"]),
        "netFlow": yuan_to_cents(net_flow),
        "transactions": transactions,
        "itemCount": len(transactions),
    }


def _log_reconciliation_bills(label: str, bills: list[dict[str, Any]], limit: int | None = None) -> None:
    logger.info("[get_reconciliation_statements] %s", label)
    for i, bill in enumerate(bills[:limit] if limit else bills, 1):
        logger.info("  #%s: ID=%s, Date=%s, Amount=%s", i, bill["id"], bill.get("date"), bill.get("amount"))


@bp.route("/reconciliation_statements", methods=["GET"])
@log_method
@require_auth
def get_reconciliation_statements():
    """获取账户对账单。"""
    _log_reconciliation_entry()

    try:
        params, error_body, error_status = _parse_reconciliation_query()
        if params is None:
            return error_body, error_status

        db, _, _, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            account = loop.run_until_complete(db.get_account_by_id(params["account_id_int"], user_id=request.user_id))
            if not account:
                logger.warning("[get_reconciliation_statements] 账户不存在: %s", params["account_id_int"])
                return jsonify({"success": False, "error": "Account not found"}), 404

            account_name = account["name"]
            logger.info("[get_reconciliation_statements] 查询账户: %s (ID=%s)", account_name, params["account_id_int"])
            filters = _build_reconciliation_filters(params)
            logger.info("[get_reconciliation_statements] 查询筛选条件: %s", filters)
            bills, total = loop.run_until_complete(
                db.query_bills(page=1, page_size=10000, filters=filters, user_id=request.user_id)
            )
            logger.info("[get_reconciliation_statements] 查询到账单数量: %d (total=%d)", len(bills), total)
            _log_reconciliation_bills("原始账单列表（前5条）:", bills, limit=5)

            bills_sorted = sorted(bills, key=lambda b: b.get("date", ""))
            _log_reconciliation_bills("排序后的账单顺序（按时间正序）:", bills_sorted)
            opening_balance = _load_reconciliation_opening_balance(loop, db, account, params)
            account_map = loop.run_until_complete(db.get_account_mappings())
            category_map = _load_reconciliation_category_map(loop, db)
            summary = _calculate_reconciliation_summary(bills_sorted, params["account_id_int"], opening_balance)
            transactions = _build_reconciliation_transactions(
                loop, adapter, bills_sorted, account_map, category_map, summary["balance_history"]
            )
            result = _build_reconciliation_result(params, account_name, summary, transactions)
            logger.info("[get_reconciliation_statements] 返回成功, 交易数量: %d", len(transactions))
            return jsonify({"success": True, "result": result})

        finally:
            loop.close()

    except Exception as e:
        logger.error("[get_reconciliation_statements] 获取对账单失败: %s", e, exc_info=True)
        return jsonify(
            {"success": False, "error": str(e), "message": "Failed to retrieve reconciliation statements"}
        ), 500

__all__ = [name for name in globals() if not name.startswith("__")]
