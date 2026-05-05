"""categories mutations route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/<category_id>", methods=["PUT"])
@log_method
@require_auth
def update_category(category_id):  # pylint: disable=too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks
    """更新分类"""
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        db, _, _ = get_app_context()
        user_id = _get_request_user_id()

        def rollback_main_category_rename(current_name: str, previous_name: str) -> None:
            try:
                rollback_success = _run_async(
                    db.update_main_category_name(current_name, previous_name, user_id=user_id)
                )
                if rollback_success is False:
                    logger.error(
                        "回滚主分类改名未命中记录或发生冲突: %s -> %s (user_id=%s)",
                        current_name,
                        previous_name,
                        user_id,
                    )
            except Exception as rollback_exc:  # pylint: disable=broad-exception-caught
                logger.error("回滚主分类改名失败: %s", rollback_exc, exc_info=True)

        if str(category_id).startswith("virtual_"):
            # 更新虚拟分类 -> 实际上是创建主分类记录 + 可能的重命名
            old_name = str(category_id).replace("virtual_", "", 1)
            new_name = data.get("name", old_name)
            renamed_group = False

            # 1. 如果改名了，更新所有子分类
            if new_name != old_name:
                try:
                    real_categories = [
                        category
                        for category in _run_async(db.get_all_categories(user_id=user_id))
                        if int(category.get("id", 0) or 0) > 0
                    ]
                    has_old_group = any(
                        category["main_category"] == old_name for category in real_categories
                    )
                    has_target_group = any(
                        category["main_category"] == new_name for category in real_categories
                    )
                    if has_old_group and has_target_group:
                        return jsonify({"success": False, "error": "Category rename conflict"}), 409
                    rename_success = _run_async(
                        db.update_main_category_name(old_name, new_name, user_id=user_id)
                    )
                except Exception as exc:  # pylint: disable=broad-exception-caught
                    logger.error("虚拟主分类批量改名失败: %s", exc, exc_info=True)
                    return jsonify({"success": False, "error": "Failed to rename category"}), 500
                if has_old_group and not rename_success:
                    return jsonify({"success": False, "error": "Category rename conflict"}), 409
                renamed_group = has_old_group and rename_success

            updates = {
                "description": data.get("comment", ""),
                "priority": data.get("displayOrder", 0),
                "keywords": data.get("keywords", ""),
                "type": data.get("type", 1),
                "hidden": not data.get("visible", True),
                "icon": data.get("icon", ""),
                "color": data.get("color", ""),
            }

            try:
                # 2. 创建主分类记录 (如果不存在)
                existing = _run_async(db.get_category_by_name(new_name, "", user_id=user_id))

                if existing:
                    # 更新现有记录
                    cat_id = existing["id"]
                    update_success = _run_async(
                        db.update_category(cat_id, updates, user_id=user_id)
                    )
                    if not update_success:
                        refreshed_existing = _run_async(
                            db.get_category_by_name(new_name, "", user_id=user_id)
                        )
                        if not refreshed_existing:
                            if renamed_group:
                                rollback_main_category_rename(new_name, old_name)
                            return (
                                jsonify({"success": False, "error": "Failed to save category"}),
                                500,
                            )
                        cat_id = refreshed_existing["id"]
                        update_success = _run_async(
                            db.update_category(cat_id, updates, user_id=user_id)
                        )
                        if not update_success:
                            if renamed_group:
                                rollback_main_category_rename(new_name, old_name)
                            return (
                                jsonify({"success": False, "error": "Failed to save category"}),
                                500,
                            )
                else:
                    # 创建新记录
                    cat_data = updates.copy()
                    cat_data["main_category"] = new_name
                    cat_data["sub_category"] = ""
                    cat_id = _run_async(db.create_category(cat_data, user_id=user_id))
                    if cat_id is None:
                        refreshed_existing = _run_async(
                            db.get_category_by_name(new_name, "", user_id=user_id)
                        )
                        if not refreshed_existing:
                            if renamed_group:
                                rollback_main_category_rename(new_name, old_name)
                            return (
                                jsonify({"success": False, "error": "Failed to save category"}),
                                500,
                            )
                        cat_id = refreshed_existing["id"]
                        update_success = _run_async(
                            db.update_category(cat_id, updates, user_id=user_id)
                        )
                        if not update_success:
                            if renamed_group:
                                rollback_main_category_rename(new_name, old_name)
                            return (
                                jsonify({"success": False, "error": "Failed to save category"}),
                                500,
                            )

                # 获取最新数据返回
                updated_cat = _run_async(db.get_category_by_id(cat_id, user_id=user_id))
                if not updated_cat:
                    if renamed_group:
                        rollback_main_category_rename(new_name, old_name)
                    return (
                        jsonify({"success": False, "error": "Failed to load updated category"}),
                        500,
                    )
            except Exception as exc:  # pylint: disable=broad-exception-caught
                if renamed_group:
                    rollback_main_category_rename(new_name, old_name)
                logger.error("虚拟主分类保存失败: %s", exc, exc_info=True)
                return (
                    jsonify({"success": False, "error": "Failed to save category"}),
                    500,
                )

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
        renamed_main_category = False
        original_main_category = None
        renamed_to_main_category = None
        if "name" in data:
            cat = _run_async(db.get_category_by_id(cat_id_int, user_id=user_id))
            if cat:
                if not cat["sub_category"]:
                    # 修改一级分类名称
                    if data["name"] != cat["main_category"]:
                        real_categories = [
                            category
                            for category in _run_async(db.get_all_categories(user_id=user_id))
                            if int(category.get("id", 0) or 0) > 0
                        ]
                        if any(
                            category["main_category"] == data["name"]
                            for category in real_categories
                        ):
                            return (
                                jsonify({"success": False, "error": "Category rename conflict"}),
                                409,
                            )
                        updates["main_category"] = data["name"]
                        original_main_category = cat["main_category"]
                        renamed_to_main_category = data["name"]
                        try:
                            rename_success = _run_async(
                                db.update_main_category_name(
                                    cat["main_category"],
                                    data["name"],
                                    user_id=user_id,
                                )
                            )
                        except Exception as exc:  # pylint: disable=broad-exception-caught
                            logger.error("主分类批量改名失败: %s", exc, exc_info=True)
                            return (
                                jsonify({"success": False, "error": "Failed to rename category"}),
                                500,
                            )
                        if not rename_success:
                            return (
                                jsonify({"success": False, "error": "Category rename conflict"}),
                                409,
                            )
                        renamed_main_category = True
                else:
                    # 修改二级分类名称
                    updates["sub_category"] = data["name"]

        try:
            success = _run_async(db.update_category(cat_id_int, updates, user_id=user_id))
        except sqlite3.IntegrityError:
            if renamed_main_category and original_main_category and renamed_to_main_category:
                rollback_main_category_rename(renamed_to_main_category, original_main_category)
            return jsonify({"success": False, "error": "Category update conflict"}), 409
        except Exception as exc:  # pylint: disable=broad-exception-caught
            if renamed_main_category and original_main_category and renamed_to_main_category:
                rollback_main_category_rename(renamed_to_main_category, original_main_category)
            logger.error("更新分类保存阶段失败: %s", exc, exc_info=True)
            return jsonify({"success": False, "error": "Failed to update category"}), 500

        if success:
            updated_cat = _run_async(db.get_category_by_id(cat_id_int, user_id=user_id))
            if not updated_cat:
                if renamed_main_category and original_main_category and renamed_to_main_category:
                    rollback_main_category_rename(renamed_to_main_category, original_main_category)
                return jsonify({"success": False, "error": "Failed to load updated category"}), 500

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

        if renamed_main_category and original_main_category and renamed_to_main_category:
            rollback_main_category_rename(renamed_to_main_category, original_main_category)

        return jsonify({"success": False, "error": "Category not found"}), 404

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新分类失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to update category"}), 500


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
        logger.error("移动分类失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to move categories"}), 500


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
            try:
                success = _run_async(
                    db.delete_categories_by_main_category(main_category, user_id=user_id)
                )
            except Exception as exc:  # pylint: disable=broad-exception-caught
                logger.error("删除主分类失败: %s", exc, exc_info=True)
                return jsonify({"success": False, "error": "Failed to delete category"}), 500
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
        logger.error("删除分类失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to delete category"}), 500


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
