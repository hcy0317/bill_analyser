"""accounts crud route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_accounts():
    """获取账户列表(需要认证) - 返回层级结构"""
    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        accounts = _run_async(db.get_all_accounts(user_id=user_id))

        # 确保返回的是列表
        if accounts is None:
            accounts = []

        logger.info("查询到 %s 个账户", len(accounts))

        # 使用adapter构建层级并格式化
        response = account_adapter.format_list_response(accounts, build_hierarchy_flag=True)

        logger.info("返回账户列表: user_id=%s, 总账户=%s", user_id, len(accounts))

        return jsonify(response)

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取账户列表失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:account_id>", methods=["GET"])
@log_method
@require_auth
def get_account(account_id: int):
    """获取账户详情"""
    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        account = _run_async(db.get_account_by_id(account_id, user_id=user_id))

        if account:
            sub_accounts = _run_async(db.get_sub_accounts(account_id, user_id=user_id))
            if sub_accounts:
                account["subAccounts"] = sub_accounts

        if not account:
            return jsonify({"success": False, "error": "Account not found"}), 404

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        return jsonify({"success": True, "result": formatted_account})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取账户详情失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_account():
    """创建账户(需要认证)"""
    logger.info("=" * 50)
    logger.info("创建账户请求开始")
    user_id = _get_request_user_id()
    logger.info("user_id=%s", user_id)

    try:
        data = request.get_json()
        if not data:
            logger.error("未提供数据")
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.debug("请求数据: %s", data)

        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        db = get_app_context()
        account_id = _run_async(db.create_account(data, user_id=user_id))

        logger.info("账户创建成功: account_id=%s", account_id)

        # 获取完整账户信息
        account = _run_async(db.get_account_by_id(account_id, user_id=user_id))

        # 获取子账户并附加
        sub_accounts = _run_async(db.get_sub_accounts(account_id, user_id=user_id))
        if sub_accounts:
            account["subAccounts"] = sub_accounts

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        logger.info("=" * 50)

        return jsonify({"success": True, "result": formatted_account}), 201

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建账户失败: %s", exc, exc_info=True)
        logger.info("=" * 50)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:account_id>", methods=["PUT"])
@log_method
@require_auth
def update_account(account_id: int):  # pylint: disable=too-many-locals,too-many-statements
    """更新账户 - REST API（支持父账户和子账户更新）"""
    try:
        user_id = _get_request_user_id()
        logger.info("[账户更新] 开始: account_id=%s, user_id=%s", account_id, user_id)

        data = request.get_json()
        if not data:
            logger.warning("[账户更新] 未提供数据: account_id=%s", account_id)
            return jsonify({"success": False, "error": "No data provided"}), 400

        # v6.52: 记录aliases字段用于调试
        logger.info("[账户更新] 请求数据字段: %s", list(data.keys()))
        if "aliases" in data:
            logger.info(
                "[账户更新] aliases原始值(前端): %s, 类型: %s",
                data["aliases"],
                type(data["aliases"]),
            )

        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        # v6.52: 记录转换后的aliases字段
        if "aliases" in data:
            logger.info(
                "[账户更新] aliases转换后(后端): %s, 类型: %s",
                data["aliases"],
                type(data["aliases"]),
            )

        db = get_app_context()

        # 处理子账户更新
        sub_accounts_data = data.pop("subAccounts", None)
        if sub_accounts_data and isinstance(sub_accounts_data, list):
            logger.info("[账户更新] 处理 %s 个子账户", len(sub_accounts_data))

            # 获取现有子账户
            existing_subs = _run_async(db.get_sub_accounts(account_id, user_id=user_id))
            existing_sub_ids = {sub["id"] for sub in existing_subs}
            updated_sub_ids = set()

            # 更新或创建子账户
            for sub_data in sub_accounts_data:
                sub_id = sub_data.get("id")

                # 使用adapter转换子账户数据格式
                sub_data = account_adapter.frontend_to_backend(sub_data)

                if sub_id and sub_id in existing_sub_ids:
                    # 更新现有子账户
                    logger.info("[账户更新] 更新子账户: sub_id=%s, name=%s", sub_id, sub_data.get("name"))
                    _run_async(db.update_account(sub_id, sub_data, user_id=user_id))
                    updated_sub_ids.add(sub_id)
                else:
                    # 创建新子账户
                    sub_data["parent_id"] = account_id
                    logger.info("[账户更新] 创建新子账户: name=%s", sub_data.get("name"))
                    new_sub_id = _run_async(db.create_account(sub_data, user_id=user_id))
                    logger.info("[账户更新] 新子账户已创建: sub_id=%s", new_sub_id)

            # 删除不再存在的子账户
            deleted_sub_ids = existing_sub_ids - updated_sub_ids
            for old_sub_id in deleted_sub_ids:
                logger.info("[账户更新] 删除旧子账户: sub_id=%s", old_sub_id)
                _run_async(db.delete_account(old_sub_id, user_id=user_id))

        # 更新父账户
        result = _run_async(db.update_account(account_id, data, user_id=user_id))

        if result:
            logger.info("[账户更新] 成功: account_id=%s", account_id)
            # 获取更新后的账户详情
            formatted_account: dict[str, Any] = {}
            updated_account = _run_async(db.get_account_by_id(account_id, user_id=user_id))
            if updated_account:
                sub_accounts = _run_async(db.get_sub_accounts(account_id, user_id=user_id))
                if sub_accounts:
                    updated_account["subAccounts"] = sub_accounts
                formatted_account = account_adapter.backend_to_frontend(updated_account)

            return jsonify(
                {"success": True, "result": formatted_account if updated_account else {}},
            )

        logger.warning("[账户更新] 账户不存在: account_id=%s", account_id)
        return jsonify({"success": False, "error": "Account not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[账户更新] 失败: account_id=%s, error=%s", account_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:account_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_account(account_id: int):
    """删除账户 - REST API（支持级联删除子账户）"""
    try:
        user_id = _get_request_user_id()
        logger.info("[账户删除] 开始: account_id=%s, user_id=%s", account_id, user_id)

        db = get_app_context()

        # 检查是否有子账户
        sub_accounts = _run_async(db.get_sub_accounts(account_id, user_id=user_id))
        if sub_accounts and len(sub_accounts) > 0:
            logger.info(
                "[账户删除] 账户包含 %s 个子账户，将进行级联删除: account_id=%s",
                len(sub_accounts),
                account_id,
            )

            # 先删除所有子账户
            for sub_account in sub_accounts:
                sub_id = sub_account["id"]
                logger.info(
                    "[账户删除] 删除子账户: sub_account_id=%s, name=%s",
                    sub_id,
                    sub_account.get("name"),
                )
                _run_async(db.delete_account(sub_id, user_id=user_id))

            logger.info("[账户删除] 已删除 %s 个子账户", len(sub_accounts))

        # 删除父账户
        result = _run_async(db.delete_account(account_id, user_id=user_id))

        if result:
            logger.info("[账户删除] 成功: account_id=%s", account_id)
            return jsonify({"success": True, "result": True})

        logger.warning("[账户删除] 账户不存在: account_id=%s", account_id)
        return jsonify({"success": False, "error": "Account not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[账户删除] 失败: account_id=%s, error=%s", account_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


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

        user_id = _get_request_user_id()
        logger.info("[账户排序更新] 开始: user_id=%s, count=%s", user_id, len(new_orders))

        db = get_app_context()

        success_count = 0
        for item in new_orders:
            if "id" not in item or "displayOrder" not in item:
                return jsonify(
                    {"success": False, "error": "Each item must have id and displayOrder"},
                ), 400

            acc_id = int(item["id"])
            order = int(item["displayOrder"])
            update_result = _run_async(
                db.update_account(acc_id, {"display_order": order}, user_id=user_id),
            )
            if update_result:
                success_count += 1

        logger.info(
            "[账户排序更新] 完成: user_id=%s, success=%s/%s",
            user_id,
            success_count,
            len(new_orders),
        )
        return jsonify({"success": True, "result": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("[账户排序更新] 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
