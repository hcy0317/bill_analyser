"""Recurring Suggestion API Routes — 周期账单自动发现。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.core.recurring_detection import detect_recurring_patterns
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("RecurringAPI")

bp = Blueprint("recurring", __name__)


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
def list_recurring_suggestions():
    """列出周期建议。"""
    try:
        db = _get_db()
        user_id = _get_user_id()
        status = request.args.get("status") or None
        limit = min(int(request.args.get("limit") or 200), 1000)
        offset = max(int(request.args.get("offset") or 0), 0)

        total = _run_async(
            db.count_recurring_suggestions(user_id=user_id, status=status)
        )
        items = _run_async(
            db.get_recurring_suggestions(
                user_id=user_id, status=status, limit=limit, offset=offset,
            )
        )

        return jsonify(
            {
                "success": True,
                "data": {
                    "total": total,
                    "items": _serialize_suggestions(items),
                    "limit": limit,
                    "offset": offset,
                },
            }
        )
    except Exception as exc:
        logger.error("list_recurring_suggestions error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500


@bp.route("/suggestions/detect", methods=["POST"])
@log_method
@require_auth
def detect_recurring():
    """触发周期模式自动检测。"""
    try:
        db = _get_db()
        user_id = _get_user_id()

        # Fetch recent bills for detection
        bills = _run_async(
            db.get_bills(filters=None, limit=10000, offset=0, user_id=user_id)
        )

        # Get bills already linked to recurring
        linked_ids = _run_async(
            db.get_bills_linked_to_recurring(user_id=user_id)
        )

        # Detect patterns
        patterns = detect_recurring_patterns(
            bills=bills,
            min_occurrences=3,
            existing_recurring_ids=linked_ids,
        )

        # Save suggestions
        result = _run_async(
            db.detect_and_save_recurring_suggestions(
                user_id=user_id, patterns=patterns,
            )
        )

        return jsonify(
            {
                "success": True,
                "data": {
                    "detected": len(patterns),
                    **result,
                },
            }
        )
    except Exception as exc:
        logger.error("detect_recurring error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500


@bp.route("/suggestions/<int:suggestion_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_suggestion(suggestion_id: int):
    """接受建议，创建 recurring rule。"""
    try:
        db = _get_db()
        user_id = _get_user_id()

        result = _run_async(
            db.accept_recurring_suggestion(
                suggestion_id=suggestion_id, user_id=user_id,
            )
        )

        if not result:
            return (
                jsonify(
                    {
                        "success": False,
                        "message": "Suggestion not found or already processed",
                    }
                ),
                404,
            )

        return jsonify({"success": True, "data": result})
    except Exception as exc:
        logger.error("accept_suggestion error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500


@bp.route("/suggestions/<int:suggestion_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_suggestion(suggestion_id: int):
    """拒绝建议。"""
    try:
        db = _get_db()
        user_id = _get_user_id()

        ok = _run_async(
            db.reject_recurring_suggestion(
                suggestion_id=suggestion_id, user_id=user_id,
            )
        )

        if not ok:
            return (
                jsonify(
                    {
                        "success": False,
                        "message": "Suggestion not found or already processed",
                    }
                ),
                404,
            )

        return jsonify({"success": True, "data": {"status": "rejected"}})
    except Exception as exc:
        logger.error("reject_suggestion error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500


def _serialize_suggestions(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Convert DB rows to API response format with camelCase keys."""
    result = []
    for item in items:
        result.append(
            {
                "id": item["id"],
                "patternHash": item.get("pattern_hash"),
                "name": item.get("name"),
                "description": item.get("description"),
                "type": item.get("type"),
                "amount": item.get("amount"),
                "sourceAccountId": item.get("source_account_id"),
                "destinationAccountId": item.get("destination_account_id"),
                "counterparty": item.get("counterparty"),
                "frequency": item.get("frequency"),
                "detectedIntervalDays": item.get("detected_interval_days"),
                "confidenceScore": item.get("confidence_score"),
                "sampleCount": item.get("sample_count"),
                "sampleBillIds": item.get("sample_bill_ids", []),
                "firstOccurrence": item.get("first_occurrence"),
                "lastOccurrence": item.get("last_occurrence"),
                "suggestedNextDate": item.get("suggested_next_date"),
                "status": item.get("status"),
                "createdAt": item.get("created_at"),
                "updatedAt": item.get("updated_at"),
            }
        )
    return result
