"""categories rules route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/rules", methods=["GET"])
@log_method
@require_auth
def get_category_rules():
    """获取分类规则"""
    try:
        _, _, category_engine = get_app_context()

        return jsonify({"success": True, "result": category_engine.rules})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/rules", methods=["PUT"])
@log_method
@require_auth
def update_category_rules():
    """更新分类规则"""
    try:
        data = request.get_json()
        if not data or "rules" not in data:
            return jsonify({"success": False, "error": "rules are required"}), 400

        # 保存规则到配置文件
        rules = data["rules"]
        save_config("categories.json", rules)

        # 重新加载规则
        _, _, category_engine = get_app_context()
        _run_async(category_engine.load_rules())

        return jsonify({"success": True, "message": "Category rules updated successfully"})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/update-all", methods=["POST"])
@log_method
@require_auth
def recategorize_all_bills():
    """重新分类所有账单"""
    try:
        data = request.get_json() or {}
        force = data.get("force", False)

        db, _, category_engine = get_app_context()
        user_id = _get_request_user_id()

        # 获取所有账单
        all_bills, total = _run_async(
            db.query_bills(page=1, page_size=100000, filters={}, user_id=user_id),
        )

        updated_count = 0
        for bill in all_bills:
            # 如果force=True或账单未分类,则重新分类
            if force or not bill.get("main_category"):
                main_cat, sub_cat = category_engine.match_category(bill)
                if main_cat:
                    _run_async(
                        db.update_bill(
                            bill["id"],
                            {"main_category": main_cat, "sub_category": sub_cat},
                            user_id=user_id,
                        )
                    )
                    updated_count += 1

        return jsonify({"success": True, "result": {"total": total, "updated": updated_count}})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("重新分类所有账单失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
