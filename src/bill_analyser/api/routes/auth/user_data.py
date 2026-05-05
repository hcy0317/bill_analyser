"""Authenticated user data export and clearing routes."""

from __future__ import annotations

from .support import *  # noqa: F403


def _parse_comma_separated_ints(raw_value: str) -> list[int]:
    """解析逗号分隔的整数列表。"""
    if not raw_value:
        return []

    result: list[int] = []
    for item in raw_value.split(","):
        item = item.strip()
        if not item:
            continue
        try:
            result.append(int(item))
        except ValueError:
            logger.warning("忽略非法整数参数: %s", item)
    return result


def _parse_export_datetime(raw_value: str) -> str | None:
    """将前端 Unix ms 时间戳转换为数据库日期字符串。"""
    if not raw_value or raw_value == "0":
        return None
    try:
        return datetime.fromtimestamp(int(raw_value) / 1000).strftime("%Y-%m-%d %H:%M:%S")
    except (TypeError, ValueError, OSError):
        logger.warning("非法时间戳参数: %s", raw_value)
        return None


def _build_export_filters(categories: list[dict[str, Any]]) -> dict[str, Any]:
    """构建导出接口使用的账单筛选条件。"""
    filters: dict[str, Any] = {}

    start_date = _parse_export_datetime(request.args.get("min_time", "0"))
    end_date = _parse_export_datetime(request.args.get("max_time", "0"))
    if start_date:
        filters["start_date"] = start_date
    if end_date:
        filters["end_date"] = end_date

    type_value = request.args.get("type", "0")
    if type_value and type_value != "0":
        try:
            filters["type"] = FRONTEND_TO_BACKEND_TYPE.get(int(type_value), "")
        except ValueError:
            filters["type"] = type_value
        if not filters["type"]:
            filters.pop("type", None)

    keyword = (request.args.get("keyword") or "").strip()
    if keyword:
        filters["keyword"] = keyword

    amount_filter = (request.args.get("amount_filter") or "").strip()
    if amount_filter:
        filters["amount_filter"] = amount_filter

    account_ids = _parse_comma_separated_ints(request.args.get("account_ids", ""))
    if account_ids:
        filters["account_ids"] = account_ids

    tag_ids = _parse_comma_separated_ints(request.args.get("tag_ids", ""))
    if tag_ids:
        filters["tag_ids"] = tag_ids

    category_ids = _parse_comma_separated_ints(request.args.get("category_ids", ""))
    if category_ids:
        category_map = {int(cat["id"]): cat for cat in categories if cat.get("id") is not None}
        selected_categories = []
        for category_id in category_ids:
            category = category_map.get(category_id)
            if category:
                selected_categories.append(
                    {"main": category.get("main_category", ""), "sub": category.get("sub_category", "")}
                )
        if selected_categories:
            filters["categories"] = selected_categories

    return filters


def _render_bills_export(
    bills: list[dict[str, Any]],
    accounts: list[dict[str, Any]],
    tags_map: dict[int, list[dict[str, Any]]],
    delimiter: str,
) -> str:
    """渲染账单导出文本。"""
    output = io.StringIO()
    writer = csv.writer(output, delimiter=delimiter, lineterminator="\n")
    writer.writerow(
        [
            "id",
            "date",
            "type",
            "amount",
            "main_category",
            "sub_category",
            "source_account",
            "destination_account",
            "counterparty",
            "payment_method",
            "description",
            "tags",
            "comment",
            "created_at",
            "updated_at",
        ]
    )

    account_map = {int(account["id"]): account.get("name", "") for account in accounts}
    for bill in bills:
        bill_id = bill.get("id")
        normalized_bill_id = int(bill_id) if bill_id is not None else -1
        tag_names = "|".join(tag.get("name", "") for tag in tags_map.get(normalized_bill_id, []))
        writer.writerow(
            [
                bill_id or "",
                bill.get("date", ""),
                bill.get("type", ""),
                bill.get("amount", 0),
                bill.get("main_category", ""),
                bill.get("sub_category", ""),
                account_map.get(int(bill.get("source_account_id") or 0), ""),
                account_map.get(int(bill.get("destination_account_id") or 0), ""),
                bill.get("counterparty", ""),
                bill.get("payment_method", ""),
                bill.get("description", ""),
                tag_names,
                bill.get("comment", ""),
                bill.get("created_at", ""),
                bill.get("updated_at", ""),
            ]
        )

    return output.getvalue()


@bp.route("/data/statistics", methods=["GET"])
@log_method
@require_auth
def get_user_data_statistics():
    """获取用户数据统计（需要认证）。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        statistics = loop.run_until_complete(db.get_user_data_statistics(user_id=user_id))

        loop.close()

        logger.info(
            "返回用户数据统计: user_id=%s, bills=%s, accounts=%s, categories=%s, tags=%s, templates=%s",
            user_id,
            statistics["billCount"],
            statistics["accountCount"],
            statistics["categoryCount"],
            statistics["tagCount"],
            statistics["templateCount"],
        )

        return jsonify({"success": True, "result": statistics})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取用户数据统计失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/export.<file_type>", methods=["GET"])
@log_method
@require_auth
def export_user_data(file_type: str):
    """导出当前用户账单数据为 CSV/TSV。"""
    loop = None
    try:
        normalized_file_type = file_type.lower()
        if normalized_file_type not in ["csv", "tsv"]:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Unsupported export file type"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
        filters = _build_export_filters(categories)
        bills = loop.run_until_complete(db.get_bills(filters=filters, user_id=user_id))
        accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        bill_ids = [int(bill["id"]) for bill in bills if bill.get("id") is not None]
        tags_map = loop.run_until_complete(db.get_tags_for_bills(bill_ids, user_id=user_id)) if bill_ids else {}
        loop.close()

        delimiter = "," if normalized_file_type == "csv" else "\t"
        mimetype = "text/csv" if normalized_file_type == "csv" else "text/tab-separated-values"
        content = "\ufeff" + _render_bills_export(bills, accounts, tags_map, delimiter)
        filename = f"bill_analyser_export_{datetime.now().strftime('%Y%m%d_%H%M%S')}.{normalized_file_type}"

        response = Response(content, mimetype=mimetype)
        response.headers["Content-Disposition"] = f"attachment; filename={filename}"
        return response

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("导出用户数据失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/clear/transactions", methods=["POST"])
@log_method
@require_auth
def clear_user_transactions():
    """清空当前用户全部交易数据。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        auth_ok, auth_mode = _resolve_sensitive_operation_auth(
            db=db,
            user=user,
            config=config,
            payload=data,
            loop=loop,
        )
        if not auth_ok:
            loop.close()
            if auth_mode == "missing_credentials":
                return jsonify(
                    {"success": False, "error": "Invalid request", "message": "Current password or stepUpToken is required"}
                ), 400
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        result = loop.run_until_complete(db.clear_user_transactions(user_id))
        loop.run_until_complete(
            db.create_audit_log(
                operation_type="clear_transactions",
                operation_target="user_data",
                target_id=user_id,
                details={"deleted_count": result.get("deleted_count", 0), "auth_mode": auth_mode},
                affected_count=result.get("deleted_count", 0),
                status="success" if result.get("success") else "failed",
                error_message=None if result.get("success") else result.get("message"),
                ip_address=get_client_ip(),
                user_agent=request.headers.get("User-Agent", ""),
            )
        )
        loop.close()

        if not result.get("success"):
            return jsonify(
                {
                    "success": False,
                    "error": "Internal Server Error",
                    "message": result.get("message", "Failed to clear transactions"),
                }
            ), 500

        return jsonify({"success": True, "result": True, "deletedCount": result.get("deleted_count", 0)})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("清空用户交易失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/clear/all", methods=["POST"])
@log_method
@require_auth
def clear_all_user_data():
    """清空当前用户全部业务数据。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        auth_ok, auth_mode = _resolve_sensitive_operation_auth(
            db=db,
            user=user,
            config=config,
            payload=data,
            loop=loop,
        )
        if not auth_ok:
            loop.close()
            if auth_mode == "missing_credentials":
                return jsonify(
                    {"success": False, "error": "Invalid request", "message": "Current password or stepUpToken is required"}
                ), 400
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        result = loop.run_until_complete(db.clear_user_data(user_id))
        loop.run_until_complete(
            db.create_audit_log(
                operation_type="clear_all_user_data",
                operation_target="user_data",
                target_id=user_id,
                details={**result.get("counts", {}), "auth_mode": auth_mode},
                status="success" if result.get("success") else "failed",
                error_message=None if result.get("success") else result.get("message"),
                ip_address=get_client_ip(),
                user_agent=request.headers.get("User-Agent", ""),
            )
        )
        loop.close()

        if not result.get("success"):
            return jsonify(
                {
                    "success": False,
                    "error": "Internal Server Error",
                    "message": result.get("message", "Failed to clear user data"),
                }
            ), 500

        return jsonify({"success": True, "result": True, "counts": result.get("counts", {})})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("清空用户全部业务数据失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
