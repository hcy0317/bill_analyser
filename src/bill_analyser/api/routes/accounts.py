"""
Accounts API Routes - 账户相关API端点

重构后使用统一账户适配器处理账户数据格式转换。
"""

import asyncio

from flask import Blueprint, jsonify, request

from bill_analyser.api.adapters.account_adapter import AccountAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("AccountsAPI")

bp = Blueprint("accounts", __name__)
account_adapter = AccountAdapter()


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app  # pylint: disable=import-outside-toplevel

    return current_app.config.get("DB_INSTANCE")


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_accounts():
    """获取账户列表(需要认证) - 返回层级结构"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))
        loop.close()

        # 确保返回的是列表
        if accounts is None:
            accounts = []

        logger.info(f"查询到 {len(accounts)} 个账户")

        # 使用adapter构建层级并格式化
        response = account_adapter.format_list_response(accounts, build_hierarchy_flag=True)

        logger.info("返回账户列表: user_id=%s, 总账户=%s", request.user_id, len(accounts))

        return jsonify(response)

    except Exception as e:
        logger.error("获取账户列表失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:account_id>", methods=["GET"])
@log_method
@require_auth
def get_account(account_id: int):
    """获取账户详情"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))

        if account:
            sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
            if sub_accounts:
                account["subAccounts"] = sub_accounts

        loop.close()

        if not account:
            return jsonify({"success": False, "error": "Account not found"}), 404

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        return jsonify({"success": True, "result": formatted_account})

    except Exception as e:
        logger.error("获取账户详情失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_account():
    """创建账户(需要认证)"""
    logger.info("=" * 50)
    logger.info("创建账户请求开始")
    logger.info(f"user_id={request.user_id}")

    try:
        data = request.get_json()
        if not data:
            logger.error("未提供数据")
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.debug(f"请求数据: {data}")

        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account_id = loop.run_until_complete(db.create_account(data, user_id=request.user_id))
        loop.close()

        logger.info(f"账户创建成功: account_id={account_id}")

        # 获取完整账户信息
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))

        # 获取子账户并附加
        sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
        if sub_accounts:
            account["subAccounts"] = sub_accounts

        loop.close()

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        logger.info("=" * 50)

        return jsonify({"success": True, "result": formatted_account}), 201

    except Exception as e:
        logger.error(f"创建账户失败: {e}", exc_info=True)
        logger.info("=" * 50)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:account_id>", methods=["PUT"])
@log_method
@require_auth
def update_account(account_id: int):
    """更新账户 - RESTful API（支持父账户和子账户更新）"""
    try:
        logger.info(f"[账户更新] 开始: account_id={account_id}, user_id={getattr(request, 'user_id', 'unknown')}")

        data = request.get_json()
        if not data:
            logger.warning(f"[账户更新] 未提供数据: account_id={account_id}")
            return jsonify({"success": False, "error": "No data provided"}), 400

        # v6.52: 记录aliases字段用于调试
        logger.info(f"[账户更新] 请求数据字段: {list(data.keys())}")
        if "aliases" in data:
            logger.info(f"[账户更新] aliases原始值(前端): {data['aliases']}, 类型: {type(data['aliases'])}")

        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        # v6.52: 记录转换后的aliases字段
        if "aliases" in data:
            logger.info(f"[账户更新] aliases转换后(后端): {data['aliases']}, 类型: {type(data['aliases'])}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 处理子账户更新
        sub_accounts_data = data.pop("subAccounts", None)
        if sub_accounts_data and isinstance(sub_accounts_data, list):
            logger.info(f"[账户更新] 处理 {len(sub_accounts_data)} 个子账户")

            # 获取现有子账户
            existing_subs = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
            existing_sub_ids = {sub["id"] for sub in existing_subs}
            updated_sub_ids = set()

            # 更新或创建子账户
            for sub_data in sub_accounts_data:
                sub_id = sub_data.get("id")

                # 使用adapter转换子账户数据格式
                sub_data = account_adapter.frontend_to_backend(sub_data)

                if sub_id and sub_id in existing_sub_ids:
                    # 更新现有子账户
                    logger.info(f"[账户更新] 更新子账户: sub_id={sub_id}, name={sub_data.get('name')}")
                    loop.run_until_complete(db.update_account(sub_id, sub_data, user_id=request.user_id))
                    updated_sub_ids.add(sub_id)
                else:
                    # 创建新子账户
                    sub_data["parent_id"] = account_id
                    logger.info(f"[账户更新] 创建新子账户: name={sub_data.get('name')}")
                    new_sub_id = loop.run_until_complete(db.create_account(sub_data, user_id=request.user_id))
                    logger.info(f"[账户更新] 新子账户已创建: sub_id={new_sub_id}")

            # 删除不再存在的子账户
            deleted_sub_ids = existing_sub_ids - updated_sub_ids
            for old_sub_id in deleted_sub_ids:
                logger.info(f"[账户更新] 删除旧子账户: sub_id={old_sub_id}")
                loop.run_until_complete(db.delete_account(old_sub_id, user_id=request.user_id))

        # 更新父账户
        result = loop.run_until_complete(db.update_account(account_id, data, user_id=request.user_id))
        loop.close()

        if result:
            logger.info(f"[账户更新] 成功: account_id={account_id}")
            # 获取更新后的账户详情
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            updated_account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))
            if updated_account:
                sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
                if sub_accounts:
                    updated_account["subAccounts"] = sub_accounts
                formatted_account = account_adapter.backend_to_frontend(updated_account)
            loop.close()

            return jsonify({"success": True, "result": formatted_account if updated_account else {}})

        logger.warning(f"[账户更新] 账户不存在: account_id={account_id}")
        return jsonify({"success": False, "error": "Account not found"}), 404

    except Exception as e:
        logger.error(f"[账户更新] 失败: account_id={account_id}, error={e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:account_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_account(account_id: int):
    """删除账户 - RESTful API（支持级联删除子账户）"""
    try:
        logger.info(f"[账户删除] 开始: account_id={account_id}, user_id={getattr(request, 'user_id', 'unknown')}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 检查是否有子账户
        sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
        if sub_accounts and len(sub_accounts) > 0:
            logger.info("[账户删除] 账户包含 %s 个子账户，将进行级联删除: account_id=%s", len(sub_accounts), account_id)

            # 先删除所有子账户
            for sub_account in sub_accounts:
                sub_id = sub_account["id"]
                logger.info(f"[账户删除] 删除子账户: sub_account_id={sub_id}, name={sub_account.get('name')}")
                loop.run_until_complete(db.delete_account(sub_id, user_id=request.user_id))

            logger.info(f"[账户删除] 已删除 {len(sub_accounts)} 个子账户")

        # 删除父账户
        result = loop.run_until_complete(db.delete_account(account_id, user_id=request.user_id))
        loop.close()

        if result:
            logger.info(f"[账户删除] 成功: account_id={account_id}")
            return jsonify({"success": True, "result": True})

        logger.warning(f"[账户删除] 账户不存在: account_id={account_id}")
        return jsonify({"success": False, "error": "Account not found"}), 404

    except Exception as e:
        logger.error(f"[账户删除] 失败: account_id={account_id}, error={e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/display-orders", methods=["PUT"])
@log_method
@require_auth
def update_account_display_orders():
    """批量更新账户显示顺序 - REST API"""
    try:
        data = request.get_json()
        if not data or "newDisplayOrders" not in data:
            return jsonify({"success": False, "error": "Missing newDisplayOrders parameter"}), 400

        new_orders = data["newDisplayOrders"]
        if not isinstance(new_orders, list):
            return jsonify({"success": False, "error": "newDisplayOrders must be a list"}), 400

        logger.info("[账户排序更新] 开始: user_id=%s, count=%s", request.user_id, len(new_orders))

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        success_count = 0
        try:
            for item in new_orders:
                if "id" not in item or "displayOrder" not in item:
                    return jsonify({"success": False, "error": "Each item must have id and displayOrder"}), 400

                acc_id = int(item["id"])
                order = int(item["displayOrder"])
                update_result = loop.run_until_complete(
                    db.update_account(acc_id, {"display_order": order}, user_id=request.user_id)
                )
                if update_result:
                    success_count += 1
        finally:
            loop.close()

        logger.info("[账户排序更新] 完成: user_id=%s, success=%s/%s", request.user_id, success_count, len(new_orders))
        return jsonify({"success": True, "result": True})
    except Exception as e:
        logger.error("[账户排序更新] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


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
        logger.info(f"[同步所有账户余额] 开始 user_id={request.user_id}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.sync_all_account_balances(user_id=request.user_id))
        loop.close()

        logger.info(
            f"[同步所有账户余额] 完成: 成功={result['synced_accounts']}/{result['total_accounts']}, "
            f"差异={len(result['discrepancies'])}个"
        )

        return jsonify({"success": True, "result": result})

    except Exception as e:
        logger.error(f"同步所有账户余额失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:account_id>/transactions/move", methods=["POST"])
@log_method
@require_auth
def move_all_transactions_rest(account_id: int):
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
            return jsonify({"success": False, "error": "Source and target accounts must be different"}), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            password_valid = loop.run_until_complete(db.verify_operation_password(password))
            if not password_valid:
                loop.run_until_complete(
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

            result = loop.run_until_complete(
                db.move_all_transactions(account_id, to_account_id, user_id=request.user_id)
            )

            if not result.get("success"):
                loop.run_until_complete(
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
                return jsonify({"success": False, "error": result.get("message", "Failed to move transactions")}), 500

            moved_count = result.get("moved_count", 0)
            loop.run_until_complete(
                db.create_audit_log(
                    operation_type="move_transactions",
                    operation_target="account",
                    target_id=account_id,
                    details={"from_account_id": account_id, "to_account_id": to_account_id, "moved_count": moved_count},
                    affected_count=moved_count,
                    status="success",
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get("User-Agent"),
                )
            )
        finally:
            loop.close()

        return jsonify({"success": True, "result": True, "moved_count": moved_count})
    except Exception as e:
        logger.error(f"REST移动账户交易失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:account_id>/transactions/clear", methods=["POST"])
@log_method
@require_auth
def clear_all_transactions_by_account_rest(account_id: int):
    """REST - 删除指定账户的所有交易。"""
    try:
        data = request.get_json() or {}
        password = data.get("password")

        if not password:
            return jsonify({"success": False, "error": "password is required"}), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            password_valid = loop.run_until_complete(db.verify_operation_password(password))
            if not password_valid:
                loop.run_until_complete(
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

            result = loop.run_until_complete(db.delete_all_transactions_by_account(account_id, user_id=request.user_id))

            if not result.get("success"):
                loop.run_until_complete(
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
                return jsonify({"success": False, "error": result.get("message", "Failed to delete transactions")}), 500

            deleted_count = result.get("deleted_count", 0)
            loop.run_until_complete(
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
        finally:
            loop.close()

        return jsonify({"success": True, "result": True, "deleted_count": deleted_count})
    except Exception as e:
        logger.error(f"REST删除账户交易失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500
