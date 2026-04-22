"""Rule Center API Routes — 规则中心统一视图。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("RulesAPI")

bp = Blueprint("rules", __name__)


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


@bp.route("/overview", methods=["GET"])
@log_method
@require_auth
def get_rules_overview():
    """统一规则聚合：learning rules + category rules + recurring rules。"""
    try:
        db = _get_db()
        user_id = _get_user_id()

        # 1. Import learning rules
        learning_rules = []
        learning_count = 0
        try:
            learning_count = _run_async(db.count_import_learning_rules(user_id=user_id))
            raw_rules = _run_async(db.get_import_learning_rules(user_id=user_id, limit=500))
            for r in raw_rules:
                learning_rules.append({
                    "id": r.get("id"),
                    "matchType": r.get("match_type"),
                    "matchValue": r.get("match_value"),
                    "learnedType": r.get("learned_type"),
                    "learnedCategoryId": r.get("learned_category_id"),
                    "enabled": bool(r.get("enabled")),
                    "appliedCount": r.get("applied_count", 0),
                    "source": "learning",
                })
        except Exception as exc:  # pylint: disable=broad-exception-caught
            logger.warning("Failed to load learning rules: %s", exc)

        # 2. Category rules (canonical source)
        category_rule_count = 0
        try:
            category_rules = _run_async(
                db.get_category_rules(
                    user_id=user_id,
                    enabled_only=True,
                )
            )
            category_rule_count = len(category_rules)
        except Exception as exc:  # pylint: disable=broad-exception-caught
            logger.warning("Failed to load category rules: %s", exc)

        # 3. Recurring rules
        recurring_rules = []
        try:
            templates = _run_async(db.get_all_templates(user_id=user_id, template_type=2))
            for t in templates:
                recurring_rules.append({
                    "id": t.get("id"),
                    "name": t.get("name"),
                    "amount": t.get("amount"),
                    "frequency": t.get("frequency"),
                    "enabled": bool(t.get("enabled", 1)),
                    "nextDate": t.get("next_date"),
                    "source": "recurring",
                })
        except Exception as exc:  # pylint: disable=broad-exception-caught
            logger.warning("Failed to load recurring rules: %s", exc)

        return jsonify({
            "success": True,
            "data": {
                "learningRules": learning_rules,
                "learningRuleCount": learning_count,
                "categoryRuleCount": category_rule_count,
                "recurringRules": recurring_rules,
                "recurringRuleCount": len(recurring_rules),
                "totalRuleCount": learning_count + category_rule_count + len(recurring_rules),
            },
        })
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("get_rules_overview error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500
