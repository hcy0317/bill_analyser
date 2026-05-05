"""budgets io route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/export", methods=["GET"])
@log_method
@require_auth
def export_budgets():
    """导出预算"""
    logger.info("[export_budgets] 开始导出预算")

    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        budgets = _run_async(db.export_budgets(user_id=user_id))

        logger.info("[export_budgets] 导出%s条预算", len(budgets))

        # 保持字段顺序
        json_str = json.dumps(
            {"success": True, "result": budgets},
            ensure_ascii=False,
            separators=(",", ":"),
        )

        return Response(json_str, mimetype="application/json")

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导出预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/import", methods=["POST"])
@log_method
@require_auth
def import_budgets():
    """
    导入预算

    Request Body:
        [
            {
                "name": "餐饮预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amount": 1000.00,
                "start_date": "2024-01-01"
            }
        ]
    """
    logger.info("[import_budgets] 开始导入预算")

    try:
        data = request.get_json(silent=True)
        if not data or not isinstance(data, list):
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid data format. Expected array of budgets.",
                }
            ), 400

        for index, item in enumerate(data):
            _validate_import_budget_item(item, index)

        logger.info("[import_budgets] 准备导入%s条预算", len(data))

        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(db.import_budgets(data, user_id=user_id))

        logger.info(
            "[import_budgets] 导入完成: 创建=%s, 更新=%s, 错误=%s",
            result["created"],
            result["updated"],
            result["errors"],
        )

        return jsonify({"success": True, "result": result})

    except ValueError as exc:
        logger.warning("导入预算参数无效: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导入预算失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
