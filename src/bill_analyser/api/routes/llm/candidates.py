"""llm candidates route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/candidates", methods=["GET"])
@log_method
@require_auth
def list_candidates():
    """获取 LLM 候选建议列表"""
    try:
        db = cast("Any", current_app.config.get("DB_INSTANCE"))
        user_id = _get_request_user_id()

        status = request.args.get("status")
        type_ = request.args.get("type")
        limit = request.args.get("limit", 50, type=int)
        offset = request.args.get("offset", 0, type=int)

        candidates = _run_async(
            db.get_llm_candidates(
                user_id=user_id,
                status=status,
                type=type_,
                limit=limit,
                offset=offset,
            )
        )
        total = _run_async(
            db.get_llm_candidates_count(
                user_id=user_id,
                status=status,
                type=type_,
            )
        )

        return jsonify({"success": True, "data": candidates, "total": total})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM 候选列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/candidates/<int:candidate_id>", methods=["GET"])
@log_method
@require_auth
def get_candidate(candidate_id: int):
    """获取单条 LLM 候选建议"""
    try:
        db = cast("Any", current_app.config.get("DB_INSTANCE"))
        user_id = _get_request_user_id()
        candidate = _run_async(db.get_llm_candidate_by_id(candidate_id, user_id=user_id))

        if not candidate:
            return jsonify({"success": False, "error": f"Candidate {candidate_id} not found"}), 404

        return jsonify({"success": True, "data": candidate})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/candidates/<int:candidate_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_candidate(candidate_id: int):
    """接受 LLM 候选建议"""
    try:
        user_id = _get_request_user_id()
        service = _get_llm_service(user_id, require_provider=False)
        result = _run_async(service.accept_candidate(candidate_id, user_id=user_id))

        return jsonify({"success": True, "data": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("接受 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/candidates/<int:candidate_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_candidate(candidate_id: int):
    """拒绝 LLM 候选建议"""
    try:
        user_id = _get_request_user_id()
        service = _get_llm_service(user_id, require_provider=False)
        result = _run_async(service.reject_candidate(candidate_id, user_id=user_id))

        return jsonify({"success": True, "data": {"rejected": result}})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("拒绝 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
