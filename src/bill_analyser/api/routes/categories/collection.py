"""categories collection route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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


@bp.route("/tree", methods=["GET"])
@log_method
@require_auth
def get_category_tree():
    """获取分类树结构(与/相同,保持兼容性)"""
    return get_categories()


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
