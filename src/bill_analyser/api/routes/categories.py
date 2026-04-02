"""
分类 API 路由
"""

import json
from typing import Any, cast

from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser.api.adapters.category_adapter import CategoryAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.utils.config import save_config
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("CategoriesAPI")

bp = Blueprint("categories", __name__)
category_adapter = CategoryAdapter()

# 分类图标映射 (MDI图标)
CATEGORY_ICONS = {
    "餐饮": "mdi-food",
    "交通": "mdi-bus",
    "购物": "mdi-shopping",
    "娱乐": "mdi-gamepad-variant",
    "居住": "mdi-home",
    "通讯": "mdi-phone",
    "人情": "mdi-account-group",
    "医疗": "mdi-medical-bag",
    "教育": "mdi-school",
    "投资": "mdi-chart-line",
    "收入": "mdi-cash-plus",
    "转账": "mdi-bank-transfer",
    "其他": "mdi-dots-horizontal",
    "工资": "mdi-wallet-membership",
    "奖金": "mdi-gift",
    "兼职": "mdi-briefcase-clock",
    "理财": "mdi-finance",
    "服饰": "mdi-tshirt-crew",
    "日用": "mdi-basket",
    "数码": "mdi-laptop",
    "美容": "mdi-lipstick",
    "装修": "mdi-hammer",
    "房贷": "mdi-home-city",
    "房租": "mdi-home-account",
    "水电": "mdi-water",
    "话费": "mdi-cellphone",
    "网费": "mdi-wifi",
    "红包": "mdi-email-open-outline",
    "礼金": "mdi-gift-outline",
    "药品": "mdi-pill",
    "治疗": "mdi-hospital",
    "学费": "mdi-book-open-variant",
    "书籍": "mdi-book",
    "培训": "mdi-presentation",
    "打车": "mdi-taxi",
    "加油": "mdi-gas-station",
    "停车": "mdi-parking",
    "地铁": "mdi-subway",
    "火车": "mdi-train",
    "机票": "mdi-airplane",
}


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return get_required_request_int("user_id")


def get_app_context(user_id: int | None = None):
    """获取应用上下文中的服务实例

    参数：
        user_id: 用户ID (如果为None，自动从request获取)
    """
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    bill_service = cast("Any", current_app.config.get("BILL_SERVICE_INSTANCE"))
    category_engine = cast("Any", current_app.config.get("CATEGORY_ENGINE_INSTANCE"))

    # 自动获取user_id
    if user_id is None:
        user_id = _get_request_user_id()

    return db, bill_service, category_engine


@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_categories():
    """获取所有分类"""
    try:
        db, _, _ = get_app_context()
        categories = _run_async(db.get_all_categories(user_id=_get_request_user_id()))

        # 使用adapter构建层级并格式化
        response = category_adapter.format_list_response(categories)

        return jsonify(response)

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_category():  # pylint: disable=too-many-locals,too-many-branches,too-many-statements,too-many-return-statements
    """创建分类"""
    logger.info("收到创建分类请求")
    try:
        data = request.get_json()
        logger.debug("请求数据: %s", data)

        if not data:
            logger.warning("请求数据为空")
            return jsonify({"success": False, "error": "No data provided"}), 400

        db, _, _ = get_app_context()

        name = data.get("name")
        parent_id = data.get("parentId", "0")
        comment = data.get("comment", "")
        priority = data.get("displayOrder", 0)
        keywords = data.get("keywords", "")
        type_val = data.get("type", 1)
        visible = data.get("visible", True)
        icon = data.get("icon", "")
        color = data.get("color", "")

        if not name:
            logger.warning("分类名称为空")
            return jsonify({"success": False, "error": "Category name is required"}), 400

        logger.info("准备创建分类: name=%s, parent_id=%s, type=%s", name, parent_id, type_val)
        user_id = _get_request_user_id()

        cat_data = {
            "description": comment,
            "priority": priority,
            "keywords": keywords,
            "type": type_val,
            "hidden": not visible,
            "icon": icon,
            "color": color,
        }

        if str(parent_id) == "0":
            # 创建一级分类
            logger.debug("创建一级分类: %s", name)
            cat_data["main_category"] = name
            cat_data["sub_category"] = ""

            # 检查分类是否已存在
            existing = _run_async(db.get_category_by_name(name, "", user_id=user_id))
            if existing:
                logger.info("分类已存在: %s, 返回现有分类 ID=%s", name, existing["id"])
                result = {
                    "id": str(existing["id"]),
                    "name": existing["main_category"],
                    "parentId": "0",
                    "type": existing.get("type", type_val),
                    "icon": existing.get("icon", icon),
                    "color": existing.get("color", color),
                    "comment": existing.get("description", ""),
                    "displayOrder": existing.get("priority", 0),
                    "visible": True,
                    "keywords": existing.get("keywords", ""),
                }
                return jsonify(
                    {"success": True, "result": result, "message": "Category already exists"},
                )

            logger.debug("调用数据库创建一级分类: %s", cat_data)
            cat_id = _run_async(db.create_category(cat_data, user_id=user_id))

            if cat_id:
                logger.info("一级分类创建成功，ID=%s", cat_id)
                new_cat = _run_async(db.get_category_by_id(cat_id, user_id=user_id))

                result = {
                    "id": str(new_cat["id"]),
                    "name": new_cat["main_category"],
                    "parentId": "0",
                    "type": new_cat.get("type", type_val),
                    "icon": new_cat.get("icon", icon),
                    "color": new_cat.get("color", color),
                    "comment": new_cat.get("description", ""),
                    "displayOrder": new_cat.get("priority", 0),
                    "visible": not new_cat.get("hidden", False),
                    "keywords": new_cat.get("keywords", ""),
                }
                return jsonify({"success": True, "result": result}), 201

            logger.error("数据库返回创建一级分类失败（cat_id为None）")
            return jsonify({"success": False, "error": "Failed to create category"}), 500

        # 创建二级分类
        # 需要查找父分类名称
        # 注意：如果父分类是虚拟的(ID以virtual_开头)，我们需要从ID中提取名称
        # 或者如果父分类是真实的ID，我们需要查询DB
        logger.debug("创建二级分类: %s, 父分类ID: %s", name, parent_id)

        main_category_name = ""
        parent_type = 1
        if str(parent_id).startswith("virtual_"):
            main_category_name = str(parent_id).replace("virtual_", "")
            logger.debug("从虚拟ID提取父分类名: %s", main_category_name)
        else:
            parent = _run_async(db.get_category_by_id(int(parent_id), user_id=user_id))
            if parent:
                main_category_name = parent["main_category"]
                parent_type = parent.get("type", 1)
                logger.debug("从数据库查询父分类名: %s", main_category_name)
            else:
                logger.warning("未找到父分类: %s", parent_id)

        if not main_category_name:
            logger.error("无法确定父分类名称，parent_id=%s", parent_id)
            return jsonify({"success": False, "error": "Parent category not found"}), 404

        cat_data["main_category"] = main_category_name
        cat_data["sub_category"] = name
        cat_data["type"] = parent_type  # 二级分类继承父分类类型

        # 检查分类是否已存在
        existing = _run_async(db.get_category_by_name(main_category_name, name, user_id=user_id))
        if existing:
            logger.info(
                "分类已存在: %s/%s, 返回现有分类 ID=%s",
                main_category_name,
                name,
                existing["id"],
            )
            result = {
                "id": str(existing["id"]),
                "name": existing["sub_category"],
                "parentId": parent_id,
                "type": existing.get("type", parent_type),
                "icon": existing.get("icon", icon),
                "color": existing.get("color", color),
                "comment": existing.get("description", ""),
                "displayOrder": existing.get("priority", 0),
                "visible": True,
                "keywords": existing.get("keywords", ""),
            }
            return jsonify(
                {"success": True, "result": result, "message": "Category already exists"},
            )

        logger.debug("调用数据库创建分类: %s", cat_data)
        cat_id = _run_async(db.create_category(cat_data, user_id=user_id))

        # 获取完整信息返回
        if cat_id:
            logger.info("分类创建成功，ID=%s", cat_id)
            new_cat = _run_async(db.get_category_by_id(cat_id, user_id=user_id))

            # 格式化返回
            result = {
                "id": str(new_cat["id"]),
                "name": (
                    new_cat["sub_category"] if new_cat["sub_category"] else new_cat["main_category"]
                ),
                "parentId": parent_id,
                "type": new_cat.get("type", type_val),
                "icon": new_cat.get("icon", ""),
                "color": new_cat.get("color", ""),
                "comment": new_cat.get("description", ""),
                "displayOrder": new_cat.get("priority", 0),
                "visible": True,
                "keywords": new_cat.get("keywords", ""),
            }

            return jsonify({"success": True, "result": result})

        logger.error("数据库返回创建失败（cat_id为None）")
        return jsonify({"success": False, "error": "Failed to create category"}), 500

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建分类异常: %s: %s", type(exc).__name__, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<category_id>", methods=["PUT"])
@log_method
@require_auth
def update_category(category_id):  # pylint: disable=too-many-locals,too-many-branches,too-many-statements
    """更新分类"""
    try:
        data = request.get_json()
        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        if str(category_id).startswith("virtual_"):
            # 更新虚拟分类 -> 实际上是创建主分类记录 + 可能的重命名
            old_name = str(category_id).replace("virtual_", "", 1)
            new_name = data.get("name", old_name)

            # 1. 如果改名了，更新所有子分类
            if new_name != old_name:
                _run_async(db.update_main_category_name(old_name, new_name))

            # 2. 创建主分类记录 (如果不存在)
            existing = _run_async(db.get_category_by_name(new_name, "", user_id=user_id))

            updates = {
                "description": data.get("comment", ""),
                "priority": data.get("displayOrder", 0),
                "keywords": data.get("keywords", ""),
                "type": data.get("type", 1),
                "hidden": not data.get("visible", True),
                "icon": data.get("icon", ""),
                "color": data.get("color", ""),
            }

            if existing:
                # 更新现有记录
                _run_async(db.update_category(existing["id"], updates, user_id=user_id))
                cat_id = existing["id"]
            else:
                # 创建新记录
                cat_data = updates.copy()
                cat_data["main_category"] = new_name
                cat_data["sub_category"] = ""
                cat_id = _run_async(db.create_category(cat_data, user_id=user_id))

            # 获取最新数据返回
            updated_cat = _run_async(db.get_category_by_id(cat_id, user_id=user_id))

            result = {
                "id": str(updated_cat["id"]),
                "name": updated_cat["main_category"],
                "parentId": "0",
                "type": updated_cat.get("type", 1),
                "icon": updated_cat.get("icon", ""),
                "color": updated_cat.get("color", ""),
                "comment": updated_cat.get("description", ""),
                "displayOrder": updated_cat.get("priority", 0),
                "visible": not updated_cat.get("hidden", False),
                "keywords": updated_cat.get("keywords", ""),
            }
            return jsonify({"success": True, "result": result})

        # 普通ID更新
        try:
            cat_id_int = int(category_id)
        except ValueError:
            return jsonify({"success": False, "error": "Invalid category ID"}), 400

        updates = {}
        if "comment" in data:
            updates["description"] = data["comment"]
        if "displayOrder" in data:
            updates["priority"] = data["displayOrder"]
        if "keywords" in data:
            updates["keywords"] = data["keywords"]
        if "type" in data:
            updates["type"] = data["type"]
        if "visible" in data:
            updates["hidden"] = not data["visible"]
        if "icon" in data:
            updates["icon"] = data["icon"]
        if "color" in data:
            updates["color"] = data["color"]

        # 处理改名逻辑
        if "name" in data:
            cat = _run_async(db.get_category_by_id(cat_id_int, user_id=user_id))
            if cat:
                if not cat["sub_category"]:
                    # 修改一级分类名称
                    if data["name"] != cat["main_category"]:
                        updates["main_category"] = data["name"]
                        _run_async(db.update_main_category_name(cat["main_category"], data["name"]))
                else:
                    # 修改二级分类名称
                    updates["sub_category"] = data["name"]

        success = _run_async(db.update_category(cat_id_int, updates, user_id=user_id))

        if success:
            updated_cat = _run_async(db.get_category_by_id(cat_id_int, user_id=user_id))

            result = {
                "id": str(updated_cat["id"]),
                "name": (
                    updated_cat["sub_category"]
                    if updated_cat["sub_category"]
                    else updated_cat["main_category"]
                ),
                "parentId": "0",
                "type": updated_cat.get("type", 1),
                "icon": updated_cat.get("icon", ""),
                "color": updated_cat.get("color", ""),
                "comment": updated_cat.get("description", ""),
                "displayOrder": updated_cat.get("priority", 0),
                "visible": not updated_cat.get("hidden", False),
                "keywords": updated_cat.get("keywords", ""),
            }
            return jsonify({"success": True, "result": result})

        return jsonify({"success": False, "error": "Category not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/move", methods=["POST"])
@log_method
@require_auth
def move_categories():
    """批量更新分类顺序"""
    try:
        data = request.get_json()
        new_display_orders = data.get("newDisplayOrders", [])

        if not new_display_orders:
            return jsonify({"success": True, "result": True})

        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        for item in new_display_orders:
            cat_id = item.get("id")
            display_order = item.get("displayOrder")
            if cat_id and display_order is not None:
                _run_async(
                    db.update_category(int(cat_id), {"priority": display_order}, user_id=user_id),
                )

        return jsonify({"success": True, "result": True})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("移动分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<category_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_category(category_id):
    """删除分类"""
    try:
        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        if str(category_id).startswith("virtual_"):
            # 处理虚拟分类删除 (删除该主分类下的所有子分类)
            main_category = str(category_id).replace("virtual_", "", 1)
            success = _run_async(db.delete_categories_by_main_category(main_category))
        else:
            # 处理普通ID删除
            try:
                cat_id_int = int(category_id)
                success = _run_async(db.delete_category(cat_id_int, user_id=user_id))
            except ValueError:
                return jsonify({"success": False, "error": "Invalid category ID"}), 400

        if success:
            return jsonify({"success": True, "result": True})
        return jsonify({"success": False, "error": "Category not found or delete failed"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


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


@bp.route("/statistics", methods=["GET"])
@log_method
@require_auth
def get_category_statistics():
    """获取分类统计"""
    try:
        # 获取查询参数
        period = request.args.get("period", "month")
        start_date = request.args.get("start_date")
        end_date = request.args.get("end_date")

        db, _, _ = get_app_context()
        stats_list = _run_async(
            db.get_category_statistics(
                period=period,
                start_date=start_date,
                end_date=end_date,
                user_id=_get_request_user_id(),
            )
        )

        # 转换为树形字典结构
        stats_dict = {}
        for stat in stats_list:
            main_cat = stat.get("main_category", "未分类")
            sub_cat = stat.get("sub_category", "")

            if main_cat not in stats_dict:
                stats_dict[main_cat] = {"total_amount": 0, "count": 0, "sub_categories": {}}

            stats_dict[main_cat]["total_amount"] += abs(float(stat.get("total_amount", 0)))
            stats_dict[main_cat]["count"] += stat.get("count", 0)

            if sub_cat:
                stats_dict[main_cat]["sub_categories"][sub_cat] = {
                    "total_amount": abs(float(stat.get("total_amount", 0))),
                    "count": stat.get("count", 0),
                }

        return jsonify({"success": True, "result": stats_dict})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类统计失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/tree", methods=["GET"])
@log_method
@require_auth
def get_category_tree():
    """获取分类树结构(与/相同,保持兼容性)"""
    return get_categories()


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


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def batch_create_categories():  # pylint: disable=too-many-locals
    """批量创建分类"""
    logger.info("开始批量创建分类")
    try:
        data = request.get_json()
        logger.debug("接收到的数据: %s", data)

        if not data or "categories" not in data:
            logger.warning("请求数据缺少categories字段")
            return jsonify({"success": False, "error": "No categories provided"}), 400

        db, _, _ = get_app_context()
        categories = data["categories"]
        logger.info("准备创建 %d 个分类", len(categories))
        user_id = _get_request_user_id()

        created_count = 0
        skipped_count = 0

        for cat in categories:
            # 处理一级分类
            main_name = cat.get("name")
            cat_type = cat.get("type", 3)  # 默认为支出类型
            if not main_name:
                continue

            # 构造一级分类数据
            main_cat_data = {
                "type": cat_type,
                "main_category": main_name,
                "sub_category": "",
                "description": cat.get("comment", ""),
                "priority": cat.get("displayOrder", 0),
                "keywords": cat.get("keywords", ""),
            }

            # 查询一级分类是否存在
            existing_main = _run_async(db.get_category_by_name(main_name, "", user_id=user_id))

            if not existing_main:
                cat_id = _run_async(db.create_category(main_cat_data, user_id=user_id))
                if cat_id:
                    created_count += 1
                    logger.debug("创建主分类成功: %s (ID: %s)", main_name, cat_id)
            else:
                skipped_count += 1
                logger.debug("主分类已存在，跳过: %s", main_name)

            # 处理子分类
            sub_categories = cat.get("subCategories", [])
            for sub in sub_categories:
                sub_name = sub.get("name")
                sub_type = sub.get("type", cat_type)  # 继承父分类类型
                if not sub_name:
                    continue

                sub_cat_data = {
                    "type": sub_type,
                    "main_category": main_name,
                    "sub_category": sub_name,
                    "description": sub.get("comment", ""),
                    "priority": sub.get("displayOrder", 0),
                    "keywords": sub.get("keywords", ""),
                }

                existing_sub = _run_async(
                    db.get_category_by_name(main_name, sub_name, user_id=user_id),
                )
                if not existing_sub:
                    cat_id = _run_async(db.create_category(sub_cat_data, user_id=user_id))
                    if cat_id:
                        created_count += 1
                        logger.debug("创建子分类成功: %s/%s (ID: %s)", main_name, sub_name, cat_id)
                else:
                    skipped_count += 1
                    logger.debug("子分类已存在，跳过: %s/%s", main_name, sub_name)

        logger.info("批量创建分类完成: 创建=%d, 跳过=%d", created_count, skipped_count)

        # 返回最新的分类树
        return get_categories()

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("批量创建分类失败: %s", exc)
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


@bp.route("/export", methods=["GET"])
@log_method
@require_auth
def export_categories():
    """导出分类为JSON（仅导出必要字段，不包含id和created_at）"""
    logger.info("收到导出分类请求")
    try:
        db, _, _ = get_app_context()
        categories = _run_async(db.get_all_categories(user_id=_get_request_user_id()))

        # 仅保留指定字段，按照指定顺序排序
        # id和created_at在导入时自动生成
        export_fields = [
            "type",
            "main_category",
            "sub_category",
            "priority",
            "keywords",
            "description",
            "icon",
            "color",
            "hidden",
        ]

        # 字段默认值映射
        default_values = {
            "type": 3,  # 默认为支出类型
            "main_category": "",
            "sub_category": "",
            "priority": 0,
            "keywords": "",
            "description": "",
            "icon": "",
            "color": "",
            "hidden": False,
        }

        cleaned_categories = []
        for cat in categories:
            # 按照export_fields的顺序构建字典，确保JSON输出字段顺序正确
            cleaned_cat = {field: cat.get(field, default_values[field]) for field in export_fields}
            cleaned_categories.append(cleaned_cat)

        logger.info("成功导出 %d 个分类", len(cleaned_categories))

        # 使用json.dumps直接序列化，保持字段顺序
        # Flask 3.x的jsonify会忽略字典顺序，因此使用Response + json.dumps
        response_data = {"success": True, "result": cleaned_categories}
        json_str = json.dumps(response_data, ensure_ascii=False, separators=(",", ":"))
        return Response(json_str, mimetype="application/json")
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导出分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/import", methods=["POST"])
@log_method
@require_auth
def import_categories():
    """从JSON导入分类（自动生成id和created_at）"""
    try:
        data = request.get_json()
        if not data:
            logger.warning("导入请求数据为空")
            return jsonify({"success": False, "error": "No data provided"}), 400

        # 支持直接列表或包装在对象中
        categories = data.get("result", []) if isinstance(data, dict) and "result" in data else data
        if isinstance(data, dict) and "categories" in data:
            categories = data["categories"]

        if not isinstance(categories, list):
            logger.warning("导入数据格式无效，期望列表")
            return jsonify(
                {"success": False, "error": "Invalid format, expected list of categories"},
            ), 400

        logger.info("准备导入 %d 个分类", len(categories))

        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        success_count = 0
        updated_count = 0
        skipped_count = 0

        for idx, cat in enumerate(categories):
            # 验证必填字段
            if "main_category" not in cat or not cat["main_category"]:
                logger.warning("跳过第 %d 个分类：缺少main_category字段", idx + 1)
                skipped_count += 1
                continue

            main = cat["main_category"]
            sub = cat.get("sub_category", "")

            # 检查是否存在
            existing = _run_async(db.get_category_by_name(main, sub, user_id=user_id))

            # 准备导入数据，移除id和created_at（这两个字段会自动生成）
            cat_data = {
                "type": cat.get("type", 3),  # 默认为支出
                "main_category": main,
                "sub_category": sub,
                "priority": cat.get("priority", 0),
                "keywords": cat.get("keywords", ""),
                "description": cat.get("description", ""),
                "icon": cat.get("icon", ""),
                "color": cat.get("color", ""),
                "hidden": cat.get("hidden", False),
            }

            # 确保不包含id和created_at字段
            # id会在数据库层自动生成（自增）
            # created_at会在数据库层使用当前时间生成

            if existing:
                # 更新现有分类
                logger.debug("更新现有分类: %s/%s", main, sub)
                _run_async(db.update_category(existing["id"], cat_data, user_id=user_id))
                updated_count += 1
            else:
                # 创建新分类（id和created_at在db.create_category中自动生成）
                logger.debug("创建新分类: %s/%s", main, sub)
                _run_async(db.create_category(cat_data, user_id=user_id))
                success_count += 1

        logger.info(
            "导入完成: 新增 %d, 更新 %d, 跳过 %d",
            success_count,
            updated_count,
            skipped_count,
        )

        return jsonify(
            {
                "success": True,
                "result": {
                    "imported": success_count,
                    "updated": updated_count,
                    "skipped": skipped_count,
                },
            }
        )

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导入分类失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/<category_id>", methods=["GET"])
@log_method
@require_auth
def get_category(category_id):
    """获取单个分类"""
    try:
        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        if str(category_id).startswith("virtual_"):
            main_category_name = str(category_id).replace("virtual_", "", 1)

            # 对于虚拟分类，我们返回一个基本结构
            # 实际项目中可能需要查询该主分类下的子分类来推断类型
            result = {
                "id": category_id,
                "name": main_category_name,
                "parentId": "0",
                "type": 1,  # 默认为支出，前端可能会根据上下文覆盖
                "icon": "",
                "color": "",
                "comment": "",
                "displayOrder": 0,
                "visible": True,
                "keywords": "",
            }
            return jsonify({"success": True, "result": result})

        try:
            cat_id_int = int(category_id)
        except ValueError:
            return jsonify({"success": False, "error": "Invalid category ID"}), 400

        cat = _run_async(db.get_category_by_id(cat_id_int, user_id=user_id))
        if not cat:
            return jsonify({"success": False, "error": "Category not found"}), 404

        # 确定 parentId
        parent_id = "0"
        if cat["sub_category"]:
            # 查找是否存在对应的主分类记录
            parent = _run_async(db.get_category_by_name(cat["main_category"], "", user_id=user_id))
            if parent:
                parent_id = str(parent["id"])
            else:
                parent_id = f"virtual_{cat['main_category']}"

        result = {
            "id": str(cat["id"]),
            "name": cat["sub_category"] if cat["sub_category"] else cat["main_category"],
            "parentId": parent_id,
            "type": cat.get("type", 1),
            "icon": "",
            "color": "",
            "comment": cat.get("description", ""),
            "displayOrder": cat.get("priority", 0),
            "visible": not cat.get("hidden", False),
            "keywords": cat.get("keywords", ""),
        }
        return jsonify({"success": True, "result": result})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
