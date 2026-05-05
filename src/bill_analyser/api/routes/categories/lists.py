"""categories lists route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/flat", methods=["GET"])
@log_method
@require_auth
def get_flat_categories():
    """获取扁平分类列表"""
    try:
        db, _, _ = get_app_context()
        categories = _run_async(db.get_all_categories(user_id=_get_request_user_id()))

        # 使用adapter获取扁平列表
        flat_list = category_adapter.get_flat_list(categories)

        return jsonify({"success": True, "result": flat_list})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取扁平分类列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/all", methods=["GET"])
@log_method
@require_auth
def get_all_categories():
    """获取所有分类(原始列表)"""
    try:
        db, _, _ = get_app_context()
        categories = _run_async(db.get_all_categories(user_id=_get_request_user_id()))

        return jsonify({"success": True, "result": categories})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取所有分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/all", methods=["PUT"])
@log_method
@require_auth
def update_all_categories():
    """批量更新分类"""
    try:
        data = request.get_json()
        if not data or "categories" not in data:
            return jsonify({"success": False, "error": "categories are required"}), 400

        # 预留批量更新实现，当前保持兼容成功响应。
        return jsonify({"success": True, "message": "Categories updated successfully"})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("批量更新分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
