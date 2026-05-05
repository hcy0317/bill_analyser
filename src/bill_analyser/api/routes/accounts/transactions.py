"""accounts transactions route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/sync-balances", methods=["POST"])
@log_method
@require_auth
def sync_all_balances():
    """
    同步所有账户余额

    v6.68: 新增批量同步所有账户余额的API端点

    该端点会：
    1. 遍历所有账户
    2. 根据账单数据计算每个账户的实际余额
    3. 将计算结果写入账户的balance字段
    4. 返回同步结果和余额差异报告

    Returns:
        JSON响应:
        {
            'success': True,
            'result': {
                'total_accounts': 20,
                'synced_accounts': 20,
                'discrepancies': [
                    {
                        'account_id': 3,
                        'name': '农业银行',
                        'old_balance': 16439.02,
                        'new_balance': -418.75,
                        'diff': -16857.77
                    }
                ],
                'errors': []
            }
        }
    """
    try:
        user_id = _get_request_user_id()
        logger.info("[同步所有账户余额] 开始 user_id=%s", user_id)

        db = get_app_context()
        result = _run_async(db.sync_all_account_balances(user_id=user_id))

        logger.info(
            "[同步所有账户余额] 完成: 成功=%s/%s, 差异=%s个",
            result["synced_accounts"],
            result["total_accounts"],
            len(result["discrepancies"]),
        )

        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("同步所有账户余额失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:account_id>/transactions/move", methods=["POST"])
@log_method
@require_auth
def move_all_transactions_rest(account_id: int):  # pylint: disable=too-many-return-statements
    """REST - 将账户下所有交易移动到另一个账户。"""
    try:
        data = request.get_json() or {}
        to_account_id = data.get("toAccountId")
        password = data.get("password")

        if not to_account_id:
            return jsonify({"success": False, "error": "toAccountId is required"}), 400

        if not password:
            return jsonify({"success": False, "error": "password is required"}), 400

        try:
            to_account_id = int(to_account_id)
        except ValueError:
            return jsonify({"success": False, "error": "Account IDs must be valid integers"}), 400

        if account_id == to_account_id:
            return jsonify(
                {"success": False, "error": "Source and target accounts must be different"},
            ), 400

        db = get_app_context()
        user_id = _get_request_user_id()

        password_valid = _verify_sensitive_operation_password(db, user_id, password)
        if not password_valid:
            _run_async(
                db.create_audit_log(
                    operation_type="move_transactions",
                    operation_target="account",
                    target_id=account_id,
                    details={"from_account_id": account_id, "to_account_id": to_account_id},
                    status="failed",
                    error_message="Invalid password",
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get("User-Agent"),
                )
            )
            return jsonify({"success": False, "error": "Invalid password"}), 401

        result = _run_async(db.move_all_transactions(account_id, to_account_id, user_id=user_id))

        if not result.get("success"):
            _run_async(
                db.create_audit_log(
                    operation_type="move_transactions",
                    operation_target="account",
                    target_id=account_id,
                    details={"from_account_id": account_id, "to_account_id": to_account_id},
                    status="failed",
                    error_message=result.get("message"),
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get("User-Agent"),
                )
            )
            return jsonify(
                {"success": False, "error": result.get("message", "Failed to move transactions")},
            ), 500

        moved_count = result.get("moved_count", 0)
        _run_async(
            db.create_audit_log(
                operation_type="move_transactions",
                operation_target="account",
                target_id=account_id,
                details={
                    "from_account_id": account_id,
                    "to_account_id": to_account_id,
                    "moved_count": moved_count,
                },
                affected_count=moved_count,
                status="success",
                ip_address=request.remote_addr,
                user_agent=request.headers.get("User-Agent"),
            )
        )

        return jsonify({"success": True, "result": True, "moved_count": moved_count})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("REST移动账户交易失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:account_id>/transactions/clear", methods=["POST"])
@log_method
@require_auth
def clear_all_transactions_by_account_rest(account_id: int):  # pylint: disable=too-many-return-statements
    """REST - 删除指定账户的所有交易。"""
    try:
        data = request.get_json() or {}
        password = data.get("password")

        if not password:
            return jsonify({"success": False, "error": "password is required"}), 400

        db = get_app_context()
        user_id = _get_request_user_id()

        password_valid = _verify_sensitive_operation_password(db, user_id, password)
        if not password_valid:
            _run_async(
                db.create_audit_log(
                    operation_type="delete_transactions",
                    operation_target="account",
                    target_id=account_id,
                    details={"account_id": account_id},
                    status="failed",
                    error_message="Invalid password",
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get("User-Agent"),
                )
            )
            return jsonify({"success": False, "error": "Invalid password"}), 401

        result = _run_async(db.delete_all_transactions_by_account(account_id, user_id=user_id))

        if not result.get("success"):
            _run_async(
                db.create_audit_log(
                    operation_type="delete_transactions",
                    operation_target="account",
                    target_id=account_id,
                    details={"account_id": account_id},
                    status="failed",
                    error_message=result.get("message"),
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get("User-Agent"),
                )
            )
            return jsonify(
                {"success": False, "error": result.get("message", "Failed to delete transactions")},
            ), 500

        deleted_count = result.get("deleted_count", 0)
        _run_async(
            db.create_audit_log(
                operation_type="delete_transactions",
                operation_target="account",
                target_id=account_id,
                details={"account_id": account_id, "deleted_count": deleted_count},
                affected_count=deleted_count,
                status="success",
                ip_address=request.remote_addr,
                user_agent=request.headers.get("User-Agent"),
            )
        )

        return jsonify({"success": True, "result": True, "deleted_count": deleted_count})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("REST删除账户交易失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
