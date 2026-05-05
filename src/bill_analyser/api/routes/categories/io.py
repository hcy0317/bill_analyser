"""categories io route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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
        return jsonify({"success": False, "error": "Failed to import categories"}), 500
