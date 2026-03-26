"""
Tags API Routes - 标签相关API端点
"""

import asyncio

from flask import Blueprint, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("TagsAPI")

bp = Blueprint("tags", __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app  # pylint: disable=import-outside-toplevel

    return current_app.config.get("DB_INSTANCE")


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_tags():
    """获取标签列表"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tags = loop.run_until_complete(db.get_all_tags(user_id=request.user_id))
        loop.close()

        return jsonify({"success": True, "result": tags})

    except Exception as e:
        logger.error("获取标签列表失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:tag_id>", methods=["GET"])
@log_method
@require_auth
def get_tag(tag_id: int):
    """获取标签详情"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tag = loop.run_until_complete(db.get_tag_by_id(tag_id, user_id=request.user_id))
        loop.close()

        if not tag:
            return jsonify({"success": False, "error": "Tag not found"}), 404

        return jsonify({"success": True, "result": tag})

    except Exception as e:
        logger.error("获取标签详情失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_tag():
    """创建标签"""
    try:
        data = request.get_json()
        if not data or "name" not in data:
            return jsonify({"success": False, "error": "name is required"}), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tag_id = loop.run_until_complete(db.create_tag(data, user_id=request.user_id))
        tag = loop.run_until_complete(db.get_tag_by_id(tag_id, user_id=request.user_id))
        loop.close()

        return jsonify({"success": True, "result": tag or {"id": tag_id}}), 201

    except Exception as e:
        logger.error("创建标签失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:tag_id>", methods=["PUT"])
@log_method
@require_auth
def update_tag(tag_id: int):
    """更新标签"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.update_tag(tag_id, data, user_id=request.user_id))
        tag = None
        if result:
            tag = loop.run_until_complete(db.get_tag_by_id(tag_id, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({"success": True, "result": tag or {"id": tag_id}})
        return jsonify({"success": False, "error": "Tag not found"}), 404

    except Exception as e:
        logger.error("更新标签失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:tag_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_tag(tag_id: int):
    """删除标签"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.delete_tag(tag_id, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({"success": True, "result": True})
        return jsonify({"success": False, "error": "Tag not found"}), 404

    except Exception as e:
        logger.error(f"删除标签失败: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def create_tags_batch():  # pylint: disable=too-many-return-statements
    """批量创建标签 - REST API"""
    try:
        data = request.get_json()
        tags_data = data.get("tags") if data else None
        skip_exists = bool(data.get("skipExists", False)) if data else False

        if not isinstance(tags_data, list) or not tags_data:
            return jsonify({"success": False, "error": "tags is required and must be a non-empty array"}), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        created_tags = []
        try:
            existing_tags = loop.run_until_complete(db.get_all_tags(user_id=request.user_id))
            existing_by_name = {str(tag.get("name", "")).strip().lower(): tag for tag in existing_tags}

            for item in tags_data:
                if not isinstance(item, dict) or not str(item.get("name", "")).strip():
                    return jsonify({"success": False, "error": "Each tag item must contain a non-empty name"}), 400

                normalized_name = str(item["name"]).strip().lower()
                if normalized_name in existing_by_name:
                    if skip_exists:
                        created_tags.append(existing_by_name[normalized_name])
                        continue
                    return jsonify({"success": False, "error": f"Tag already exists: {item['name']}"}), 409

                tag_id = loop.run_until_complete(db.create_tag(item, user_id=request.user_id))
                created_tag = loop.run_until_complete(db.get_tag_by_id(tag_id, user_id=request.user_id))
                if created_tag:
                    created_tags.append(created_tag)
                    existing_by_name[normalized_name] = created_tag
        finally:
            loop.close()

        return jsonify({"success": True, "result": created_tags}), 201
    except Exception as e:
        logger.error("批量创建标签失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/display-orders", methods=["PUT"])
@log_method
@require_auth
def update_tag_display_orders_rest():  # pylint: disable=too-many-return-statements
    """批量更新标签显示顺序 - REST API"""
    try:
        data = request.get_json()
        if not data or "newDisplayOrders" not in data:
            return jsonify({"success": False, "error": "newDisplayOrders is required"}), 400

        new_orders = data["newDisplayOrders"]
        if not isinstance(new_orders, list):
            return jsonify({"success": False, "error": "newDisplayOrders must be an array"}), 400

        orders = []
        for item in new_orders:
            if not isinstance(item, dict) or "id" not in item or "displayOrder" not in item:
                return jsonify({"success": False, "error": "Each item must have id and displayOrder"}), 400

            try:
                tag_id = int(item["id"])
                display_order = int(item["displayOrder"])
                orders.append((tag_id, display_order))
            except (ValueError, TypeError) as e:
                return jsonify({"success": False, "error": f"Invalid id or displayOrder: {e}"}), 400

        logger.info("[标签排序更新] 开始: user_id=%s, count=%s", request.user_id, len(orders))

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_tag_display_orders(orders, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            logger.info("[标签排序更新] 完成: user_id=%s, count=%s", request.user_id, len(orders))
            return jsonify({"success": True, "result": True})
        return jsonify({"success": False, "error": "Failed to update display orders"}), 500
    except Exception as e:
        logger.error("更新标签显示顺序失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500
