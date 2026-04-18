"""Learning Suggestion Center API Routes — 独立学习建议中心。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("LearningAPI")

bp = Blueprint("learning", __name__)


def _get_db() -> Any:
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _get_user_id() -> int:
    return int(getattr(request, "user_id", 0) or 0)


def _run_async(coroutine: Any) -> Any:
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


@bp.route("/suggestions", methods=["GET"])
@log_method
@require_auth
def list_suggestions():
    """列出学习建议，支持 status 筛选与分页。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        status = request.args.get("status") or None
        limit = min(int(request.args.get("limit") or 200), 1000)
        offset = max(int(request.args.get("offset") or 0), 0)

        total = _run_async(db.count_learning_suggestions(user_id=user_id, status=status))
        items = _run_async(db.get_learning_suggestions(
            user_id=user_id, status=status, limit=limit, offset=offset,
        ))

        return jsonify({
            "success": True,
            "data": {
                "items": items,
                "total": total,
                "limit": limit,
                "offset": offset,
            },
        })
    except Exception as exc:
        logger.error("[学习建议列表] error=%s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/suggestions/generate", methods=["POST"])
@log_method
@require_auth
def generate_suggestions():
    """从全量历史标注中挖掘学习建议。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        stats = _run_async(db.mine_learning_suggestions(user_id=user_id))
        return jsonify({"success": True, "data": stats})
    except Exception as exc:
        logger.error("[学习建议挖掘] error=%s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/suggestions/<int:suggestion_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_suggestion(suggestion_id: int):
    """接受建议并提升为长期学习规则。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        result = _run_async(db.accept_learning_suggestion(suggestion_id, user_id=user_id))
        if result is None:
            return jsonify({"success": False, "error": "suggestion_not_found"}), 404
        if "error" in result:
            return jsonify({"success": False, "error": result["error"]}), 409
        return jsonify({"success": True, "data": result})
    except Exception as exc:
        logger.error("[学习建议接受] id=%s, error=%s", suggestion_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/suggestions/batch-accept", methods=["POST"])
@log_method
@require_auth
def batch_accept_suggestions():
    """批量接受学习建议并提升为长期学习规则。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        data = request.get_json(silent=True) or {}
        suggestion_ids = data.get("suggestionIds")
        if not isinstance(suggestion_ids, list) or len(suggestion_ids) == 0:
            return jsonify({"success": False, "error": "suggestionIds must be a non-empty array"}), 400
        if len(suggestion_ids) > 100:
            return jsonify({"success": False, "error": "batch size must not exceed 100"}), 400

        try:
            validated_ids = list(dict.fromkeys(int(sid) for sid in suggestion_ids))
        except (ValueError, TypeError) as exc:
            return jsonify({"success": False, "error": f"Invalid suggestionIds: {exc}"}), 400

        accepted = []
        failed = []
        for sid_int in validated_ids:
            result = _run_async(db.accept_learning_suggestion(sid_int, user_id=user_id))
            if result is None:
                failed.append({"id": sid_int, "error": "suggestion_not_found"})
            elif "error" in result:
                failed.append({"id": sid_int, "error": result["error"]})
            else:
                accepted.append({"id": sid_int, "ruleId": result.get("rule_id")})

        return jsonify({
            "success": True,
            "data": {
                "accepted": accepted,
                "failed": failed,
                "acceptedCount": len(accepted),
                "failedCount": len(failed),
            },
        })
    except Exception as exc:
        logger.error("[学习建议批量接受] error=%s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/suggestions/<int:suggestion_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_suggestion(suggestion_id: int):
    """拒绝建议。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        success = _run_async(db.reject_learning_suggestion(suggestion_id, user_id=user_id))
        if not success:
            return jsonify({"success": False, "error": "suggestion_not_found_or_not_pending"}), 404
        return jsonify({"success": True})
    except Exception as exc:
        logger.error("[学习建议拒绝] id=%s, error=%s", suggestion_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/rules", methods=["GET"])
@log_method
@require_auth
def list_rules():
    """列出已有学习规则，支持分页与启用状态筛选。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        enabled_only = request.args.get("enabled_only", "").lower() in ("1", "true", "yes")
        limit = min(int(request.args.get("limit") or 200), 1000)
        offset = max(int(request.args.get("offset") or 0), 0)

        total = _run_async(db.count_import_learning_rules(user_id=user_id, enabled_only=enabled_only))
        items = _run_async(db.get_import_learning_rules(
            user_id=user_id, enabled_only=enabled_only, limit=limit, offset=offset,
        ))

        return jsonify({
            "success": True,
            "data": {
                "items": items,
                "total": total,
                "limit": limit,
                "offset": offset,
            },
        })
    except Exception as exc:
        logger.error("[学习规则列表] error=%s", exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/rules/<int:rule_id>/toggle", methods=["PUT"])
@log_method
@require_auth
def toggle_rule(rule_id: int):
    """启用/禁用学习规则。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        data = request.get_json(silent=True) or {}
        enabled = bool(data.get("enabled", True))
        success = _run_async(db.set_import_learning_rule_enabled(rule_id, enabled, user_id=user_id))
        if not success:
            return jsonify({"success": False, "error": "rule_not_found"}), 404
        return jsonify({"success": True, "data": {"ruleId": rule_id, "enabled": enabled}})
    except Exception as exc:
        logger.error("[学习规则切换] id=%s, error=%s", rule_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/rules/<int:rule_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_rule(rule_id: int):
    """删除学习规则。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        success = _run_async(db.delete_import_learning_rule(rule_id, user_id=user_id))
        if not success:
            return jsonify({"success": False, "error": "rule_not_found"}), 404
        return jsonify({"success": True})
    except Exception as exc:
        logger.error("[学习规则删除] id=%s, error=%s", rule_id, exc, exc_info=True)
        return jsonify({"success": False, "error": str(exc)}), 500
