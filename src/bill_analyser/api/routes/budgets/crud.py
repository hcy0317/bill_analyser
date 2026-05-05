"""budgets crud route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_budgets():
    """
    获取预算列表

    Query Parameters:
        - period_type: 周期类型（daily/weekly/monthly/yearly）
        - enabled: 是否启用（true/false）
        - category: 分类名称
    """
    logger.info("[get_budgets] 开始获取预算列表")

    try:
        filters = {}
        if request.args.get("period_type"):
            filters["period_type"] = request.args.get("period_type")
        enabled_arg = request.args.get("enabled")
        if enabled_arg:
            filters["enabled"] = enabled_arg.lower() == "true"
        if request.args.get("category"):
            filters["category"] = request.args.get("category")
        budget_type = _get_optional_int(request.args.get("budget_type"), "budget_type")

        logger.info("[get_budgets] 筛选条件: %s", filters)

        db = get_app_context()
        user_id = _get_request_user_id()
        budgets = _run_async(
            db.get_budgets_for_listing(
                filters,
                budget_type=budget_type,
                user_id=user_id,
            )
        )

        logger.info("[get_budgets] 返回%s条预算", len(budgets))

        return jsonify({"success": True, "result": budgets})

    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算列表失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["GET"])
@log_method
@require_auth
def get_budget(budget_id: int):
    """获取预算详情"""
    logger.info("[get_budget] 获取预算ID: %s", budget_id)

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))

        if not budget:
            logger.warning("[get_budget] 预算不存在: %s", budget_id)
            return jsonify({"success": False, "error": "Budget not found"}), 404

        logger.info("[get_budget] 成功获取预算: %s", budget["name"])

        return jsonify({"success": True, "result": budget})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取预算详情失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_budget():  # pylint: disable=too-many-return-statements
    """
    创建预算

    Request Body:
        {
            "name": "餐饮预算",
            "category": "餐饮",
            "sub_category": "午餐",
            "period_type": "monthly",
            "amount": 1000.00,
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "alert_threshold": 80,
            "enabled": true
        }
    """
    logger.info("[create_budget] 开始创建预算")

    try:
        data = request.get_json(silent=True)
        if data is None:
            return jsonify({"success": False, "error": "No data provided"}), 400
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON body. Expected object."}), 400

        logger.info("[create_budget] 请求数据: %s", data)

        # 预算名称已改为备注，允许为空；分类仍为必填
        required_fields = ["period_type", "amount", "start_date"]
        for field in required_fields:
            if field not in data:
                logger.warning("[create_budget] 缺少必填字段: %s", field)
                return jsonify({"success": False, "error": f"Missing required field: {field}"}), 400

        if not data.get("category"):
            logger.warning("[create_budget] 缺少必填字段: category")
            return jsonify({"success": False, "error": "Missing required field: category"}), 400

        _validate_budget_period_args(str(data["period_type"]))
        _validate_budget_date_range(
            str(data.get("start_date") or "") or None,
            str(data.get("end_date") or "") or None,
        )

        # 设置默认值
        data.setdefault("name", "")
        data.setdefault("enabled", True)
        data.setdefault("alert_threshold", 80)
        data["created_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        db = get_app_context()
        user_id = _get_request_user_id()
        budget_id = _run_async(db.create_budget(data, user_id=user_id))

        if budget_id:
            budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))
            logger.info("[create_budget] 创建成功: ID=%s, name=%s", budget_id, data["name"])

            return jsonify({"success": True, "result": budget}), 201

        logger.error("[create_budget] 创建失败")
        return jsonify({"success": False, "error": "Failed to create budget"}), 500

    except ValueError as exc:
        logger.warning("创建预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["PUT"])
@log_method
@require_auth
def update_budget(budget_id: int):  # pylint: disable=too-many-return-statements
    """更新预算"""
    logger.info("[update_budget] 更新预算ID: %s", budget_id)

    try:
        data = request.get_json(silent=True)
        if data is None:
            return jsonify({"success": False, "error": "No data provided"}), 400
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON body. Expected object."}), 400

        logger.info("[update_budget] 更新数据: %s", data)

        db = get_app_context()
        user_id = _get_request_user_id()

        if data.get("period_type") is not None:
            _validate_budget_period_args(str(data["period_type"]))

        existing_budget = _run_async(db.get_budget_by_id(budget_id, user_id=user_id))
        if not existing_budget:
            logger.warning("[update_budget] 预算不存在: %s", budget_id)
            return jsonify({"success": False, "error": "Budget not found"}), 404

        _validate_budget_date_range(
            str(data.get("start_date", existing_budget.get("start_date")) or "") or None,
            str(data.get("end_date", existing_budget.get("end_date")) or "") or None,
        )

        data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        result = _run_async(db.update_budget(budget_id, data, user_id=user_id))

        if result:
            logger.info("[update_budget] 更新成功: ID=%s", budget_id)
            return jsonify({"success": True, "message": "Budget updated successfully"})

        logger.warning("[update_budget] 预算不存在: %s", budget_id)
        return jsonify({"success": False, "error": "Budget not found"}), 404

    except ValueError as exc:
        logger.warning("更新预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:budget_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_budget(budget_id: int):
    """删除预算"""
    logger.info("[delete_budget] 删除预算ID: %s", budget_id)

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(db.delete_budget(budget_id, user_id=user_id))

        if result:
            logger.info("[delete_budget] 删除成功: ID=%s", budget_id)
            return jsonify({"success": True, "message": "Budget deleted successfully"})

        logger.warning("[delete_budget] 预算不存在: %s", budget_id)
        return jsonify({"success": False, "error": "Budget not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
