"""matching actions route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/reconcile-history", methods=["POST"])
@log_method
@require_auth
def reconcile_matching_history():
    """按显式 billIds 聚合读取历史正式账单 matching 候选（支持可选 families 过滤）。"""
    try:
        try:
            normalized_bill_ids, families = _parse_reconcile_history_request(request.get_json(silent=True))
        except KeyError as exc:
            return jsonify({"success": False, "error": str(exc.args[0])}), 400
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        reconcile_handler = getattr(bill_service, "reconcile_matching_history", None)
        if not callable(reconcile_handler):
            raise AttributeError("Matching reconcile-history handler not available")

        result = _run_async(reconcile_handler(normalized_bill_ids, user_id=user_id, families=families))
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to reconcile matching history")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_reconcile_history_payload(result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("历史 matching 调和失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_matching_candidate(candidate_id: str):
    """接受一个当前已支持 family 的 matching candidate。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        accept_handler = getattr(bill_service, "_accept_matching_candidate", None)
        if not callable(accept_handler):
            raise AttributeError("Matching accept handler not available")
        result = _run_async(
            accept_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to accept candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("接受 matching candidate 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_matching_candidate(candidate_id: str):
    """拒绝一个当前已支持 family 的 matching candidate。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        reject_handler = getattr(bill_service, "_reject_matching_candidate", None)
        if not callable(reject_handler):
            raise AttributeError("Matching reject handler not available")
        result = _run_async(
            reject_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to reject candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("拒绝 matching candidate 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/clear", methods=["POST"])
@log_method
@require_auth
def clear_matching_candidate(candidate_id: str):
    """清除一个当前已支持 family 的 matching candidate 决策状态。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        clear_handler = getattr(bill_service, "_clear_matching_candidate", None)
        if not callable(clear_handler):
            raise AttributeError("Matching clear handler not available")
        result = _run_async(
            clear_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to clear candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("清除 matching candidate 决策失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500
