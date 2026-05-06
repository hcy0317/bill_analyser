# pylint: disable=wildcard-import,unused-wildcard-import,undefined-variable
from .support import *  # noqa: F403

@bp.route("/category/quick-add-keyword", methods=["POST"])
@log_method
@require_auth
def quick_add_category_keyword():
    """
    快速为分类添加关键词

    用户在手动分类时可以将交易的某个关键词添加到分类规则中

    Request:
        {
            'main_category': '餐饮',
            'sub_category': '外卖',  # 可选
            'keyword': '美团'
        }

    Response:
        {
            'success': true,
            'message': 'Keyword added successfully'
        }
    """
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "Request body is required"}), 400

        main_category = data.get("main_category")
        sub_category = data.get("sub_category")
        keyword = data.get("keyword")

        if not main_category or not keyword:
            return jsonify({"success": False, "error": "main_category and keyword are required"}), 400

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        success = loop.run_until_complete(
            bill_service.add_category_keyword(main_category, sub_category, keyword, user_id=user_id)
        )
        loop.close()

        if success:
            return jsonify({"success": True, "message": "Keyword added successfully"})
        else:
            return jsonify({"success": False, "error": "Failed to add keyword"}), 400

    except Exception as e:
        logger.error("添加关键词失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/category/refresh", methods=["POST"])
@log_method
@require_auth
def refresh_bill_categories():
    """
    刷新账单分类

    使用最新的分类规则重新匹配账单

    Request:
        {
            'bill_ids': [1, 2, 3]  # 可选，不提供则刷新所有未分类账单
        }

    Response:
        {
            'success': true,
            'result': {
                'total': 50,
                'categorized': 45,
                'still_uncategorized': 5
            }
        }
    """
    try:
        data = request.get_json() or {}
        bill_ids = data.get("bill_ids")

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(bill_service.refresh_category_for_bills(bill_ids, user_id=user_id))
        loop.close()

        return jsonify({"success": result.get("success", False), "result": result})

    except Exception as e:
        logger.error("刷新分类失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch/update", methods=["PUT"])
@log_method
@require_auth
def batch_update_bills():
    """批量更新账单"""
    try:
        allowed_update_fields = {
            "date",
            "type",
            "amount",
            "counterparty",
            "description",
            "payment_method",
            "main_category",
            "sub_category",
            "source_account_id",
            "destination_account_id",
            "destination_amount",
        }
        data = request.get_json()
        if not data or "ids" not in data or "updates" not in data:
            return jsonify({"success": False, "error": "ids and updates are required"}), 400

        db, _, _ = get_app_context()

        ids = data["ids"]
        updates = data["updates"]
        if not isinstance(updates, dict):
            return jsonify({"success": False, "error": "updates must be an object"}), 400

        invalid_update_fields = sorted(set(updates) - allowed_update_fields)
        if invalid_update_fields:
            return jsonify(
                {
                    "success": False,
                    "error": f"unsupported update fields: {', '.join(invalid_update_fields)}",
                }
            ), 400

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.batch_update_bills(ids, updates, user_id=request.user_id))
        loop.close()

        if isinstance(result, dict):
            response_result = {
                "updated_count": int(result.get("success_count", 0)),
                "failed_count": int(result.get("failed_count", 0)),
                "failed_ids": result.get("failed_ids", []),
            }
        else:
            response_result = {"updated_count": int(result)}

        return jsonify({"success": True, "result": response_result})

    except Exception as e:
        logger.error("批量更新账单失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch/delete", methods=["DELETE"])
@log_method
@require_auth
def batch_delete_bills():
    """批量删除账单"""
    try:
        data = request.get_json()
        if not data or "ids" not in data:
            return jsonify({"success": False, "error": "ids are required"}), 400

        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **收集受影响的账户ID**
        affected_accounts = set()
        # 注意：如果批量删除数量很大，这里可能会慢。但通常批量删除是分页的。
        # 为了准确同步余额，我们需要知道哪些账户被影响了。
        for bill_id in data["ids"]:
            try:
                bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
                if bill:
                    if bill.get("source_account_id"):
                        affected_accounts.add(int(bill["source_account_id"]))
                    if bill.get("destination_account_id"):
                        affected_accounts.add(int(bill["destination_account_id"]))
            except Exception as e:
                logger.warning("获取账单信息失败 (ID: %s): %s", bill_id, e)

        result = loop.run_until_complete(db.batch_delete_bills(data["ids"], user_id=request.user_id))

        # **同步余额**
        if result > 0:
            logger.info("批量删除成功，开始同步 %s 个账户的余额", len(affected_accounts))
            for account_id in affected_accounts:
                try:
                    loop.run_until_complete(db.sync_account_balance(account_id))
                except Exception as e:
                    logger.error("同步账户余额失败 (ID: %s): %s", account_id, e)

        loop.close()

        return jsonify({"success": True, "result": {"deleted_count": result}})

    except Exception as e:
        logger.error("批量删除账单失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
