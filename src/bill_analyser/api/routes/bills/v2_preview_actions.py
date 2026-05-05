# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

@bp.route("/import/v2/preview-item/<int:preview_id>/recurring-candidates", methods=["GET"])
@log_method
@require_auth
def get_preview_recurring_candidates(preview_id: int):
    """获取导入预览账单可匹配的定时交易候选。"""
    try:
        tolerance_days = request.args.get("toleranceDays", default=3, type=int)
        tolerance_days = max(0, min(tolerance_days, 31))

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            db.get_recurring_candidates_for_preview(preview_id, user_id=request.user_id, tolerance_days=tolerance_days)
        )
        loop.close()

        if not result.get("preview"):
            return jsonify({"success": False, "error": "Preview bill not found"}), 404

        return jsonify(
            {
                "success": True,
                "result": {
                    "previewId": preview_id,
                    "linkedRecurringId": result.get("linked_recurring_id"),
                    "candidates": result.get("candidates", []),
                },
            }
        )
    except Exception as e:
        logger.error("获取预览定时交易候选失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview-item/<int:preview_id>/recurring-match", methods=["PUT"])
@log_method
@require_auth
def bind_preview_recurring_match(preview_id: int):
    """在导入预览阶段绑定定时交易候选。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        recurring_id = data.get("recurringId")
        if recurring_id in (None, ""):
            return jsonify({"success": False, "error": "Missing recurringId"}), 400
        if isinstance(recurring_id, bool):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        if isinstance(recurring_id, int):
            normalized_recurring_id = recurring_id
        elif isinstance(recurring_id, str):
            normalized_recurring_id_raw = recurring_id.strip()
            if not normalized_recurring_id_raw.isdigit():
                return jsonify({"success": False, "error": "Invalid request"}), 400
            normalized_recurring_id = int(normalized_recurring_id_raw)
        else:
            return jsonify({"success": False, "error": "Invalid request"}), 400

        if normalized_recurring_id <= 0:
            return jsonify({"success": False, "error": "Invalid request"}), 400

        expected_state = data.get("expectedState")
        if not isinstance(expected_state, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        response_mode = data.get("responseMode")

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        recurring_kwargs = {
            "expected_state": expected_state,
            "user_id": request.user_id,
        }
        if "responseMode" in data:
            recurring_kwargs["response_mode"] = response_mode
        try:
            result = loop.run_until_complete(
                bill_service.update_preview_recurring_match(
                    preview_id,
                    normalized_recurring_id,
                    **recurring_kwargs,
                )
            )
        finally:
            loop.close()

        if not result.get("success"):
            return jsonify({"success": False, "error": result.get("error", "Failed to update recurring match")}), int(
                result.get("status_code", 400)
            )

        return jsonify(
            {
                "success": True,
                "data": {
                    "previewId": result.get("preview_id", preview_id),
                    "sessionId": result.get("session_id", ""),
                    "recurringId": result.get("recurring_id"),
                    "previewItem": result.get("preview_item"),
                    "preview": result.get("preview", []),
                },
            }
        )
    except Exception as e:
        logger.error("绑定预览定时交易失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/import/v2/preview-item/<int:preview_id>/recurring-match", methods=["DELETE"])
@log_method
@require_auth
def clear_preview_recurring_match(preview_id: int):
    """在导入预览阶段清除定时交易候选绑定。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        expected_state = data.get("expectedState")
        if not isinstance(expected_state, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        response_mode = data.get("responseMode")

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        recurring_kwargs = {
            "expected_state": expected_state,
            "user_id": request.user_id,
        }
        if "responseMode" in data:
            recurring_kwargs["response_mode"] = response_mode
        try:
            result = loop.run_until_complete(
                bill_service.update_preview_recurring_match(
                    preview_id,
                    None,
                    **recurring_kwargs,
                )
            )
        finally:
            loop.close()

        if not result.get("success"):
            return jsonify({"success": False, "error": result.get("error", "Failed to clear recurring match")}), int(
                result.get("status_code", 400)
            )

        return jsonify(
            {
                "success": True,
                "data": {
                    "previewId": result.get("preview_id", preview_id),
                    "sessionId": result.get("session_id", ""),
                    "recurringId": result.get("recurring_id"),
                    "previewItem": result.get("preview_item"),
                    "preview": result.get("preview", []),
                },
            }
        )
    except Exception as e:
        logger.error("清除预览定时交易绑定失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/import/v2/preview-item/<int:preview_id>/transfer-decision", methods=["POST"])
@log_method
@require_auth
def update_preview_transfer_decision(preview_id: int):
    """更新导入预览中转账建议的接受/拒绝/清除状态。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        decision = str(data.get("decision") or "").strip().lower()
        if decision not in {"accept", "reject", "clear"}:
            return jsonify({"success": False, "error": "Invalid decision"}), 400

        expected_state = data.get("expectedState")
        if not isinstance(expected_state, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        response_mode = data.get("responseMode")

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        transfer_kwargs = {
            "expected_state": expected_state,
            "user_id": request.user_id,
        }
        if "responseMode" in data:
            transfer_kwargs["response_mode"] = response_mode
        try:
            result = loop.run_until_complete(
                bill_service.apply_preview_transfer_decision(
                    preview_id,
                    decision,
                    **transfer_kwargs,
                )
            )
        finally:
            loop.close()

        if not result.get("success"):
            return jsonify({"success": False, "error": result.get("error", "Failed to update decision")}), int(
                result.get("status_code", 400)
            )

        return jsonify(
            {
                "success": True,
                "data": {
                    "previewId": result.get("preview_id", preview_id),
                    "sessionId": result.get("session_id", ""),
                    "decision": result.get("decision", decision),
                    "previewItem": result.get("preview_item"),
                    "preview": result.get("preview", []),
                },
            }
        )
    except Exception as e:
        logger.error("更新预览转账建议决策失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview/<session_id>/update", methods=["PUT"])
@log_method
@require_auth
def update_preview_bill(session_id: str):
    """
    更新预览账单（用户编辑）

    Request:
        JSON:
            - id: 预览账单ID
            - 其他可更新字段...
    """
    try:
        data = request.get_json()
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing bill id"}), 400

        db, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)
        response_mode = data.get("responseMode")

        preview_id = data["id"]

        # 使用session_id验证预览账单归属（可选的安全检查）
        logger.debug("[更新预览] session_id=%s, preview_id=%s", session_id, preview_id)

        updates = {
            "preview_type": data.get("type"),
            "preview_amount": data.get("amount"),
            "preview_destination_amount": data.get("destinationAmount"),
            "preview_main_category": data.get("mainCategory"),
            "preview_sub_category": data.get("subCategory"),
            "preview_source_account_id": data.get("sourceAccountId"),
            "preview_destination_account_id": data.get("destinationAccountId"),
            "preview_counterparty": data.get("counterparty"),
            "preview_payment_method": data.get("paymentMethod"),
            "preview_description": data.get("description"),
            "is_selected": 1 if data.get("isSelected", True) else 0,
        }
        # 过滤None值
        updates = {k: v for k, v in updates.items() if v is not None}

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            preview_row = loop.run_until_complete(db.get_preview_bill_by_id(preview_id, user_id=user_id))
            if not preview_row or str(preview_row.get("session_id") or "") != str(session_id):
                return jsonify({"success": False, "error": "Preview bill not found"}), 404

            success = loop.run_until_complete(db.update_preview_bill(preview_id, updates, user_id))
            if (
                success
                and isinstance(response_mode, str)
                and response_mode.strip().lower() == "preview-item"
            ):
                preview_item = loop.run_until_complete(
                    bill_service.get_import_preview_item(int(preview_id), user_id=user_id)
                )
                return jsonify(
                    {
                        "success": True,
                        "data": {
                            "updated": True,
                            "previewItem": preview_item,
                        },
                    }
                )

            return jsonify({"success": success})

        finally:
            loop.close()

    except Exception as e:
        logger.error("[更新预览] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
