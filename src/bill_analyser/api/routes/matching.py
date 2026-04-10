"""Matching API Routes - 导入配对候选只读接口。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("MatchingAPI")

bp = Blueprint("matching", __name__)


def get_app_context() -> tuple[Any, Any]:
    """获取 matching API 所需的应用上下文实例。"""
    return (
        cast("Any", current_app.config.get("DB_INSTANCE")),
        cast("Any", current_app.config.get("BILL_SERVICE_INSTANCE")),
    )


def _get_request_user_id() -> int:
    return int(getattr(request, "user_id", 0) or 0)


def _run_async(coroutine: Any) -> Any:
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


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
