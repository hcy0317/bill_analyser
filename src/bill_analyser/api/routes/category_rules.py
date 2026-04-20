"""分类规则 API 路由"""

from flask import Blueprint, current_app, jsonify, request
from typing import cast, Any

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("CategoryRulesAPI")

bp = Blueprint("category_rules", __name__)


def _get_request_user_id() -> int:
    return get_required_request_int("user_id")


def _get_app_services():
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    engine = cast("Any", current_app.config.get("CATEGORY_ENGINE_INSTANCE"))
    return db, engine


def _reload_engine(engine, db, user_id: int):
    """Invalidate cache and reload rules after mutation."""
    engine.invalidate_cache()
    _run_async(engine.load_rules_from_db(db, user_id=user_id))


# ------------------------------------------------------------------
# GET /  – list rules
# ------------------------------------------------------------------
@bp.route("/", methods=["GET"])
@log_method
@require_auth
def list_rules():
    """获取分类规则列表"""
    try:
        db, _ = _get_app_services()
        user_id = _get_request_user_id()

        category_id = request.args.get("category_id", type=int)
        enabled_only = request.args.get("enabled_only", "true").lower() != "false"

        rules = _run_async(
            db.get_category_rules(
                user_id=user_id,
                category_id=category_id,
                enabled_only=enabled_only,
            )
        )
        return jsonify({"success": True, "data": rules, "total": len(rules)})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类规则列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /  – create rule
# ------------------------------------------------------------------
@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_rule():
    """创建分类规则"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        if "category_id" not in data or "rule_expression" not in data:
            return jsonify({"success": False, "error": "category_id and rule_expression are required"}), 400

        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        rule_id = _run_async(db.create_category_rule(data, user_id=user_id))
        if rule_id is None:
            return jsonify({"success": False, "error": "Failed to create rule"}), 400

        _reload_engine(engine, db, user_id)

        rule = _run_async(db.get_category_rule(rule_id, user_id=user_id))
        return jsonify({"success": True, "data": rule}), 201
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# PUT /<id>  – update rule
# ------------------------------------------------------------------
@bp.route("/<int:rule_id>", methods=["PUT"])
@log_method
@require_auth
def update_rule(rule_id: int):
    """更新分类规则"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        ok = _run_async(db.update_category_rule(rule_id, data, user_id=user_id))
        if not ok:
            return jsonify({"success": False, "error": "Rule not found or no change"}), 404

        _reload_engine(engine, db, user_id)

        rule = _run_async(db.get_category_rule(rule_id, user_id=user_id))
        return jsonify({"success": True, "data": rule})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# DELETE /<id>  – delete rule
# ------------------------------------------------------------------
@bp.route("/<int:rule_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_rule(rule_id: int):
    """删除分类规则"""
    try:
        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        ok = _run_async(db.delete_category_rule(rule_id, user_id=user_id))
        if not ok:
            return jsonify({"success": False, "error": "Rule not found"}), 404

        _reload_engine(engine, db, user_id)
        return jsonify({"success": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /reorder  – reorder rules
# ------------------------------------------------------------------
@bp.route("/reorder", methods=["POST"])
@log_method
@require_auth
def reorder_rules():
    """重新排序分类规则"""
    try:
        data = request.get_json()
        if not data or "rule_ids" not in data:
            return jsonify({"success": False, "error": "rule_ids is required"}), 400

        rule_ids = data["rule_ids"]
        if not isinstance(rule_ids, list):
            return jsonify({"success": False, "error": "rule_ids must be a list"}), 400

        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        ok = _run_async(db.reorder_category_rules(rule_ids, user_id=user_id))
        if not ok:
            return jsonify({"success": False, "error": "Reorder failed"}), 500

        _reload_engine(engine, db, user_id)
        return jsonify({"success": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("重排分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /migrate  – migrate old keywords to rules
# ------------------------------------------------------------------
@bp.route("/migrate", methods=["POST"])
@log_method
@require_auth
def migrate_keywords():
    """将旧关键词迁移为分类规则"""
    try:
        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        result = _run_async(db.migrate_keywords_to_rules(user_id=user_id))

        _reload_engine(engine, db, user_id)
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("迁移关键词失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /<id>/test  – test a rule against text
# ------------------------------------------------------------------
@bp.route("/<int:rule_id>/test", methods=["POST"])
@log_method
@require_auth
def test_rule(rule_id: int):
    """测试规则是否匹配给定文本"""
    try:
        data = request.get_json()
        if not data or "text" not in data:
            return jsonify({"success": False, "error": "text is required"}), 400

        db, engine = _get_app_services()
        user_id = _get_request_user_id()

        rule = _run_async(db.get_category_rule(rule_id, user_id=user_id))
        if not rule:
            return jsonify({"success": False, "error": "Rule not found"}), 404

        expr = rule.get("rule_expression", "")
        regex_enabled = bool(rule.get("regex_enabled", False))

        compiled = engine.keyword_matcher.compile_rule_expression(
            expr, regex_enabled=regex_enabled,
        )
        matched = engine.keyword_matcher.match_compiled(data["text"], compiled)

        return jsonify({"success": True, "data": {"matched": matched}})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("测试分类规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
