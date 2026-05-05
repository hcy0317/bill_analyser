# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_review import *  # noqa: F403


@bp.route("/import/v2/reclassify/<session_id>", methods=["POST"])
@log_method
@require_auth
def reclassify_preview_session(session_id: str):
    """
    v6.55: 重新分类导入会话中的预览账单

    功能：
    1. 刷新分类规则（从数据库重新加载）
    2. 从 bills_preview 表读取所有账单
    3. 根据 dedup_type 使用不同类型的分类规则
    4. 重新执行账户匹配
    5. 更新 bills_preview 表
    6. 返回更新后的预览数据

    Request:
        POST /api/bills/import/v2/reclassify/<session_id>

    Response:
        {
            'success': true,
            'data': {
                'session_id': 'xxx',
                'total': 100,
                'categorized': 80,
                'account_matched': 90,
                'preview': [...]  // 更新后的预览数据
            }
        }
    """
    try:
        logger.info("[v2重新分类] session_id=%s, user_id=%s", session_id, request.user_id)

        data = request.get_json(silent=True) or {}
        preview_updates = data.get("preview_updates") or []

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 1. 执行重新分类
            reclassify_result = loop.run_until_complete(
                bill_service.reclassify_preview_bills(
                    session_id, preview_updates=preview_updates, user_id=request.user_id
                )
            )

            if not reclassify_result.get("success"):
                return jsonify(
                    {
                        "success": False,
                        "error": reclassify_result.get("errors", ["未知错误"])[0]
                        if reclassify_result.get("errors")
                        else "重新分类失败",
                    }
                ), 500

            # 2. 获取更新后的预览数据
            preview_method_params = inspect.signature(bill_service.get_import_preview).parameters
            if "user_id" in preview_method_params:
                preview_data = loop.run_until_complete(
                    bill_service.get_import_preview(session_id, user_id=request.user_id)
                )
            else:
                preview_data = loop.run_until_complete(bill_service.get_import_preview(session_id))

            logger.info(
                "[v2重新分类] 完成 session=%s, total=%s, categorized=%s, account_matched=%s",
                session_id,
                reclassify_result.get("total"),
                reclassify_result.get("categorized"),
                reclassify_result.get("account_matched"),
            )

            return jsonify(
                {
                    "success": True,
                    "data": {
                        "session_id": session_id,
                        "total": reclassify_result.get("total", 0),
                        "categorized": reclassify_result.get("categorized", 0),
                        "account_matched": reclassify_result.get("account_matched", 0),
                        "session_samples_saved": reclassify_result.get("session_samples_saved", 0),
                        "annotation_applied": reclassify_result.get("annotation_applied", 0),
                        "preview": preview_data,
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[v2重新分类] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


def _parse_import_learning_preview_ids(raw_preview_ids):
    if raw_preview_ids is None:
        return None

    if not isinstance(raw_preview_ids, list):
        raise ValueError("previewIds must be an array")

    normalized_preview_ids: list[int] = []
    for raw_preview_id in raw_preview_ids:
        if (
            not isinstance(raw_preview_id, int)
            or isinstance(raw_preview_id, bool)
            or raw_preview_id <= 0
        ):
            raise ValueError("previewIds must contain positive integers")
        normalized_preview_ids.append(raw_preview_id)

    return normalized_preview_ids


@bp.route("/import/v2/learning/<session_id>/suggestions", methods=["GET", "POST"])
@log_method
@require_auth
def list_import_learning_suggestions(session_id: str):
    """返回当前导入会话的 dry-run 长期学习建议。"""
    try:
        logger.info(
            "[长期学习建议] session_id=%s, user_id=%s",
            session_id,
            request.user_id,
        )
        preview_updates = None
        preview_ids = None
        if request.method == "POST":
            data = request.get_json(silent=True) or {}
            preview_updates = data.get("preview_updates")

            try:
                preview_ids = _parse_import_learning_preview_ids(data.get("previewIds"))
            except ValueError as exc:
                return jsonify({"success": False, "error": str(exc)}), 400

        db, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            session = loop.run_until_complete(
                db.get_import_session(session_id, user_id=request.user_id)
            )
            if not session:
                return jsonify({"success": False, "error": "Import session not found"}), 404

            suggestions_result = loop.run_until_complete(
                bill_service.get_import_learning_suggestions(
                    session_id,
                    preview_updates=preview_updates,
                    preview_ids=preview_ids,
                    user_id=request.user_id,
                )
            )
            return jsonify({"success": True, "data": suggestions_result})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[长期学习建议] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/learning/<session_id>/promote", methods=["POST"])
@log_method
@require_auth
def promote_import_learning(session_id: str):
    """将当前导入会话中的人工标注提升为长期学习规则。"""
    try:
        logger.info("[长期学习提升] session_id=%s, user_id=%s", session_id, request.user_id)
        data = request.get_json(silent=True) or {}
        preview_updates = data.get("preview_updates")
        try:
            preview_ids = _parse_import_learning_preview_ids(data.get("previewIds"))
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        db, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            session = loop.run_until_complete(
                db.get_import_session(session_id, user_id=request.user_id)
            )
            if not session:
                return jsonify({"success": False, "error": "Import session not found"}), 404

            promote_result = loop.run_until_complete(
                bill_service.promote_session_annotations_to_learning(
                    session_id,
                    preview_updates=preview_updates,
                    preview_ids=preview_ids,
                    user_id=request.user_id,
                )
            )

            return jsonify({"success": True, "data": promote_result})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[长期学习提升] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules", methods=["GET"])
@log_method
@require_auth
def list_import_learning_rules():
    """获取当前用户的长期导入学习规则列表。"""
    try:
        db, _, _ = get_app_context()
        page = max(int(request.args.get("page", 1) or 1), 1)
        page_size = int(request.args.get("pageSize", request.args.get("limit", 100)) or 100)
        enabled_only = str(request.args.get("enabledOnly", "")).lower() in ("1", "true", "yes")

        if page_size == 0:
            page_size = 100

        page_size = max(min(page_size, 500), -1)
        limit = page_size if page_size > 0 else None
        offset = (page - 1) * page_size if page_size > 0 else 0

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            total_count = loop.run_until_complete(
                db.count_import_learning_rules(user_id=request.user_id, enabled_only=enabled_only)
            )

            rules = loop.run_until_complete(
                db.get_import_learning_rules(
                    user_id=request.user_id, enabled_only=enabled_only, limit=limit, offset=offset
                )
            )

            categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            categories_by_id = {
                int(category["id"]): category for category in categories if category.get("id") is not None
            }
            accounts_by_id = {int(account["id"]): account for account in accounts if account.get("id") is not None}

            result = []
            for rule in rules:
                learned_category_id = rule.get("learned_category_id")
                learned_source_account_id = rule.get("learned_source_account_id")
                learned_destination_account_id = rule.get("learned_destination_account_id")
                match_features = {}
                try:
                    raw_match_features = rule.get("match_features_json")
                    if raw_match_features:
                        match_features = json.loads(raw_match_features)
                except (TypeError, ValueError, json.JSONDecodeError):
                    match_features = {}

                learned_category = categories_by_id.get(int(learned_category_id)) if learned_category_id else None
                source_account = (
                    accounts_by_id.get(int(learned_source_account_id)) if learned_source_account_id else None
                )
                destination_account = (
                    accounts_by_id.get(int(learned_destination_account_id)) if learned_destination_account_id else None
                )

                result.append(
                    {
                        "id": rule.get("id"),
                        "matchType": rule.get("match_type", ""),
                        "matchValue": rule.get("match_value", ""),
                        "matchFeatures": match_features,
                        "learnedType": rule.get("learned_type", ""),
                        "learnedCategoryId": rule.get("learned_category_id") or "",
                        "learnedCategoryName": (
                            f"{learned_category.get('main_category', '')}/{learned_category.get('sub_category', '')}"
                            if learned_category and learned_category.get("sub_category")
                            else (learned_category.get("main_category", "") if learned_category else "")
                        ),
                        "learnedSourceAccountId": rule.get("learned_source_account_id") or "",
                        "learnedSourceAccountName": source_account.get("name", "") if source_account else "",
                        "learnedDestinationAccountId": rule.get("learned_destination_account_id") or "",
                        "learnedDestinationAccountName": destination_account.get("name", "")
                        if destination_account
                        else "",
                        "enabled": bool(rule.get("enabled", 1)),
                        "appliedCount": int(rule.get("applied_count", 0) or 0),
                        "createdAt": rule.get("created_at", ""),
                        "updatedAt": rule.get("updated_at", ""),
                        "lastAppliedAt": rule.get("last_applied_at", ""),
                    }
                )

            total_pages = 1
            if page_size > 0:
                total_pages = max((total_count + page_size - 1) // page_size, 1)

            effective_page = min(page, total_pages) if total_count > 0 else 1

            response = jsonify(
                {
                    "success": True,
                    "result": result,
                    "totalCount": total_count,
                    "page": effective_page,
                    "pageSize": page_size,
                    "totalPages": total_pages,
                }
            )
            response.headers["Cache-Control"] = "no-store, no-cache, must-revalidate, max-age=0"
            response.headers["Pragma"] = "no-cache"
            response.headers["Expires"] = "0"
            return response
        finally:
            loop.close()

    except Exception as e:
        logger.error("[长期学习规则列表] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules/<int:rule_id>", methods=["PUT"])
@log_method
@require_auth
def update_import_learning_rule(rule_id: int):
    """更新单条长期导入学习规则。"""
    try:
        data = request.get_json(silent=True) or {}
        editable_fields = {"matchValue", "learnedType", "learnedCategoryId"}
        if "enabled" not in data and not any(field in data for field in editable_fields):
            return jsonify({"success": False, "error": "enabled is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            if any(field in data for field in editable_fields):
                update_payload: dict[str, Any] = {}
                if "matchValue" in data:
                    update_payload["match_value"] = str(data.get("matchValue") or "").strip()
                if "learnedType" in data:
                    update_payload["learned_type"] = str(data.get("learnedType") or "").strip()
                if "learnedCategoryId" in data:
                    raw_category_id = data.get("learnedCategoryId")
                    update_payload["learned_category_id"] = (
                        None if raw_category_id in (None, "", 0, "0") else int(raw_category_id)
                    )
                if "enabled" in data:
                    update_payload["enabled"] = bool(data.get("enabled"))

                result = loop.run_until_complete(
                    db.update_import_learning_rule(rule_id, user_id=request.user_id, **update_payload)
                )
                if result is None:
                    return jsonify({"success": False, "error": "Rule not found"}), 404
                return jsonify({"success": True, "result": result})

            success = loop.run_until_complete(
                db.set_import_learning_rule_enabled(rule_id, bool(data.get("enabled")), user_id=request.user_id)
            )
            if not success:
                return jsonify({"success": False, "error": "Rule not found"}), 404

            return jsonify({"success": True, "result": True})
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400
        finally:
            loop.close()

    except Exception as e:
        logger.error("[更新长期学习规则] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules/<int:rule_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_import_learning_rule(rule_id: int):
    """删除单条长期导入学习规则。"""
    try:
        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.delete_import_learning_rule(rule_id, user_id=request.user_id))
            if not success:
                return jsonify({"success": False, "error": "Rule not found"}), 404

            return jsonify({"success": True, "result": True})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[删除长期学习规则] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
