"""llm preview route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/preview-recommend", methods=["POST"])
@log_method
@require_auth
def preview_recommend():
    """为导入预览行生成黄色 LLM 推荐建议。"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True) or {}
        session_id = data.get("session_id")
        if not session_id:
            return _error_response("session_id is required", "INVALID_REQUEST", 400)

        preview_ids = data.get("preview_ids")
        preview_updates = data.get("preview_updates")
        limit = data.get("limit", 20)

        service = _get_llm_service(user_id)
        suggestions = _run_async(
            service.recommend_for_preview(
                user_id=user_id,
                session_id=session_id,
                preview_ids=preview_ids,
                preview_updates=preview_updates,
                limit=limit,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                "suggestions": suggestions,
                "count": len(suggestions),
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except RuntimeError as exc:
        logger.warning("LLM preview recommend 失败: %s", exc)
        if str(exc).startswith("Rate limit exceeded"):
            return _error_response(str(exc), "LLM_RATE_LIMITED", 429)
        return _error_response(str(exc), "LLM_PROVIDER_UNAVAILABLE", 503)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview recommend 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


@bp.route("/preview-recommend/accept", methods=["POST"])
@log_method
@require_auth
def preview_recommend_accept():
    """接受黄色 LLM 推荐并写入 MEMORY。"""
    try:
        user_id = _get_request_user_id()
        data = request.get_json(silent=True) or {}

        session_id = data.get("session_id")
        preview_id = data.get("preview_id")
        suggestion = data.get("suggestion")

        if not session_id or not preview_id:
            return _error_response(
                "session_id and preview_id are required", "INVALID_REQUEST", 400
            )

        service = _get_llm_service(user_id, require_provider=False)
        accept_result = _run_async(
            service.accept_preview_recommendation(
                user_id=user_id,
                session_id=session_id,
                preview_id=int(preview_id),
                suggestion=suggestion,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                **accept_result,
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview accept 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


@bp.route("/preview-recommend/reject", methods=["POST"])
@log_method
@require_auth
def preview_recommend_reject():
    """拒绝黄色 LLM 推荐并写入 MEMORY（含可选用户纠正）。"""
    try:
        user_id = _get_request_user_id()
        data = request.get_json(silent=True) or {}

        session_id = data.get("session_id")
        preview_id = data.get("preview_id")
        suggestion = data.get("suggestion")
        user_correction = data.get("user_correction")

        if not session_id or not preview_id:
            return _error_response(
                "session_id and preview_id are required", "INVALID_REQUEST", 400
            )

        service = _get_llm_service(user_id, require_provider=False)
        reject_result = _run_async(
            service.reject_preview_recommendation(
                user_id=user_id,
                session_id=session_id,
                preview_id=int(preview_id),
                suggestion=suggestion,
                user_correction=user_correction,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                **reject_result,
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview reject 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


@bp.route("/memory", methods=["GET"])
@log_method
@require_auth
def list_memory_events():
    """获取 LLM memory 事件列表（审计用）。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        session_id = request.args.get("session_id")
        event_type = request.args.get("event_type")
        limit = request.args.get("limit", 100, type=int)
        offset = request.args.get("offset", 0, type=int)

        events = _run_async(
            db.get_llm_memory_events(
                user_id,
                session_id=session_id,
                event_type=event_type,
                limit=limit,
                offset=offset,
            )
        )
        total = _run_async(
            db.get_llm_memory_events_count(
                user_id,
                session_id=session_id,
                event_type=event_type,
            )
        )

        return jsonify({"success": True, "data": events, "total": total})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM memory 事件失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
