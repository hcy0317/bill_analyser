"""matching queries route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/sessions/<session_id>/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_session_candidates(session_id: str):
    """返回某个导入会话的显式 matching 候选列表。"""
    try:
        db, bill_service = get_app_context()
        user_id = _get_request_user_id()

        session = _run_async(db.get_import_session(session_id, user_id=user_id))
        if not session:
            return jsonify({"success": False, "error": "Import session not found"}), 404

        result = _run_async(bill_service.get_matching_session_candidates(session_id, user_id=user_id))
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 matching 会话候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/bills/<int:bill_id>/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_bill_candidates(bill_id: int):
    """返回某条正式账单的历史 matching 候选。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()

        result = _run_async(bill_service.get_matching_bill_candidates(bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_bill_candidates_payload(bill_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单 matching 候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/bills/<int:bill_id>/feedback", methods=["GET"])
@log_method
@require_auth
def get_matching_bill_feedback(bill_id: int):
    """返回某条正式账单的 matching feedback 审查事件流。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()

        result = _run_async(bill_service.get_matching_bill_feedback(bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": {
                    "billId": bill_id,
                    "events": [
                        _serialize_matching_feedback_event(event) for event in list(result.get("events") or [])
                    ],
                },
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单 matching feedback 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/reconciliation-candidates", methods=["GET"])
@log_method
@require_auth
def get_reconciliation_candidates():
    """Read persisted import-to-formal-bill reconciliation candidates."""
    try:
        try:
            query = _parse_reconciliation_candidates_query()
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        db, _ = get_app_context()
        user_id = _get_request_user_id()
        candidates = _run_async(db.list_import_reconciliation_candidates(user_id=user_id, **query))
        return jsonify(
            {
                "success": True,
                "data": {
                    "candidates": [
                        _serialize_reconciliation_candidate(candidate)
                        for candidate in list(candidates or [])
                    ]
                },
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取导入后匹配 reconciliation 候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_candidates():
    """按 sessionId 或 billId 统一读取 matching 候选。"""
    try:
        try:
            session_id, bill_id = _parse_matching_candidates_selector()
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        db, bill_service = get_app_context()
        user_id = _get_request_user_id()

        if session_id is not None:
            session = _run_async(db.get_import_session(session_id, user_id=user_id))
            if not session:
                return jsonify({"success": False, "error": "Import session not found"}), 404

            result = _run_async(bill_service.get_matching_session_candidates(session_id, user_id=user_id))
            return jsonify({"success": True, "data": result})

        normalized_bill_id = int(bill_id or 0)
        result = _run_async(bill_service.get_matching_bill_candidates(normalized_bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_bill_candidates_payload(normalized_bill_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取统一 matching 候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500
