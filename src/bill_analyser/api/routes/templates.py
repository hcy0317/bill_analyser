"""
Templates API Routes - 模板相关API端点
"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("TemplatesAPI")

bp = Blueprint("templates", __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    return cast(Any, current_app.config.get("DB_INSTANCE"))


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return int(getattr(request, "user_id", 0) or 0)


def _run_async(coroutine):
    """在独立事件循环中执行异步数据库调用。"""
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


def _get_template_type(default: int | None = None) -> int | None:
    """从 query 或 body 中提取模板类型。"""
    template_type = request.args.get("templateType")

    if template_type is None and request.is_json:
        body = request.get_json(silent=True) or {}
        template_type = body.get("templateType")

    if template_type in [None, ""]:
        return default

    normalized = str(template_type).strip()
    if not normalized:
        return default

    try:
        return int(normalized)
    except (TypeError, ValueError):
        return default


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_templates():
    """获取模板列表"""
    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        template_type = _get_template_type()

        templates = _run_async(
            db.get_all_templates(
                user_id=user_id,
                template_type=template_type,
            )
        )

        return jsonify({"success": True, "result": templates})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取模板列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:template_id>", methods=["GET"])
@log_method
@require_auth
def get_template(template_id: int):
    """获取模板详情"""
    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        template_type = _get_template_type()

        template = _run_async(
            db.get_template_by_id(
                template_id,
                user_id=user_id,
                template_type=template_type,
            )
        )

        if not template:
            return jsonify({"success": False, "error": "Template not found"}), 404

        return jsonify({"success": True, "result": template})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取模板详情失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_template():
    """创建模板"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        db = get_app_context()
        user_id = _get_request_user_id()

        template_id = _run_async(db.create_template(data, user_id=user_id))
        template = _run_async(
            db.get_template_by_id(
                template_id,
                user_id=user_id,
                template_type=_get_template_type(default=1),
            )
        )

        return jsonify({"success": True, "result": template or {"id": str(template_id)}}), 201

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建模板失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:template_id>", methods=["PUT"])
@log_method
@require_auth
def update_template(template_id: int):
    """更新模板"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        db = get_app_context()
        user_id = _get_request_user_id()
        template_type = _get_template_type(default=1)

        result = _run_async(
            db.update_template(
                template_id,
                data,
                user_id=user_id,
                template_type=template_type,
            )
        )

        template = None
        if result:
            template = _run_async(
                db.get_template_by_id(
                    template_id,
                    user_id=user_id,
                    template_type=template_type,
                )
            )

        if result:
            return jsonify({"success": True, "result": template})
        return jsonify({"success": False, "error": "Template not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新模板失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<int:template_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_template(template_id: int):
    """删除模板"""
    try:
        db = get_app_context()
        user_id = _get_request_user_id()
        template_type = _get_template_type(default=1)

        result = _run_async(
            db.delete_template(
                template_id,
                user_id=user_id,
                template_type=template_type,
            )
        )

        if result:
            return jsonify({"success": True, "result": True})
        return jsonify({"success": False, "error": "Template not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除模板失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/display-orders", methods=["PUT"])
@log_method
@require_auth
def update_template_display_orders():
    """批量更新模板显示顺序。"""
    try:
        data = request.get_json() or {}
        new_orders_raw = data.get("newDisplayOrders") or []
        template_type = _get_template_type(default=1)

        if not new_orders_raw:
            return jsonify({"success": False, "error": "Missing newDisplayOrders"}), 400

        orders = []
        for item in new_orders_raw:
            orders.append((int(item["id"]), int(item["displayOrder"])))

        db = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(
            db.update_template_display_orders(
                orders,
                template_type=template_type,
                user_id=user_id,
            )
        )

        return jsonify({"success": True, "result": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新模板排序失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
