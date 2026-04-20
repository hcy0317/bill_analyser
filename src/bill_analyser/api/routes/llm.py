"""LLM 学习服务 API 路由"""

from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.llm_learning_service import LLMLearningService
from bill_analyser.core.llm_provider import ProviderFactory
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("LLM_API")

bp = Blueprint("llm", __name__)


def _get_request_user_id() -> int:
    return get_required_request_int("user_id")


def _get_llm_config() -> dict[str, Any]:
    """Get LLM configuration from app config."""
    return cast("dict[str, Any]", current_app.config.get("LLM_CONFIG", {}))


def _get_llm_service() -> LLMLearningService:
    """Build LLM learning service from current config."""
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    config = _get_llm_config()

    provider_name = config.get("provider", "openai")
    provider_config = config.get("provider_config", {})

    provider = ProviderFactory.create(provider_name, provider_config)
    return LLMLearningService(db=db, provider=provider)


# ------------------------------------------------------------------
# POST /analyze-transactions
# ------------------------------------------------------------------
@bp.route("/analyze-transactions", methods=["POST"])
@log_method
@require_auth
def analyze_transactions():
    """使用 LLM 分析未分类交易并生成分类建议"""
    try:
        config = _get_llm_config()
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        data = request.get_json() or {}
        user_id = _get_request_user_id()

        bill_ids = data.get("bill_ids")
        limit = data.get("limit", 20)

        service = _get_llm_service()
        candidates = _run_async(
            service.analyze_transactions(user_id=user_id, bill_ids=bill_ids, limit=limit)
        )

        return jsonify({"success": True, "data": candidates, "total": len(candidates)})
    except RuntimeError as exc:
        logger.warning("LLM 分析交易失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 429
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 分析交易失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /induce-rules
# ------------------------------------------------------------------
@bp.route("/induce-rules", methods=["POST"])
@log_method
@require_auth
def induce_rules():
    """使用 LLM 从已分类样本中归纳关键词规则"""
    try:
        config = _get_llm_config()
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        data = request.get_json()
        if not data or "category_id" not in data:
            return jsonify({"success": False, "error": "category_id is required"}), 400

        user_id = _get_request_user_id()
        category_id = data["category_id"]
        sample_count = data.get("sample_count", 10)

        service = _get_llm_service()
        candidates = _run_async(
            service.induce_rules(user_id=user_id, category_id=category_id, sample_count=sample_count)
        )

        return jsonify({"success": True, "data": candidates, "total": len(candidates)})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except RuntimeError as exc:
        logger.warning("LLM 归纳规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 429
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 归纳规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# GET /candidates
# ------------------------------------------------------------------
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
            db.get_llm_candidates(user_id=user_id, status=status, type=type_, limit=limit, offset=offset)
        )
        total = _run_async(db.get_llm_candidates_count(user_id=user_id, status=status))

        return jsonify({"success": True, "data": candidates, "total": total})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM 候选列表失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# GET /candidates/<id>
# ------------------------------------------------------------------
@bp.route("/candidates/<int:candidate_id>", methods=["GET"])
@log_method
@require_auth
def get_candidate(candidate_id: int):
    """获取单条 LLM 候选建议"""
    try:
        db = cast("Any", current_app.config.get("DB_INSTANCE"))
        candidate = _run_async(db.get_llm_candidate_by_id(candidate_id))

        if not candidate:
            return jsonify({"success": False, "error": "Candidate not found"}), 404

        return jsonify({"success": True, "data": candidate})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /candidates/<id>/accept
# ------------------------------------------------------------------
@bp.route("/candidates/<int:candidate_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_candidate(candidate_id: int):
    """接受 LLM 候选建议"""
    try:
        config = _get_llm_config()
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        service = _get_llm_service()
        result = _run_async(service.accept_candidate(candidate_id))

        return jsonify({"success": True, "data": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("接受 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /candidates/<id>/reject
# ------------------------------------------------------------------
@bp.route("/candidates/<int:candidate_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_candidate(candidate_id: int):
    """拒绝 LLM 候选建议"""
    try:
        config = _get_llm_config()
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        service = _get_llm_service()
        result = _run_async(service.reject_candidate(candidate_id))

        return jsonify({"success": True, "data": {"rejected": result}})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("拒绝 LLM 候选建议失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# GET /config
# ------------------------------------------------------------------
@bp.route("/config", methods=["GET"])
@log_method
@require_auth
def get_config():
    """获取当前 LLM 配置"""
    try:
        config = _get_llm_config()
        # Return safe subset (no API keys)
        safe_config = {
            "enabled": config.get("enabled", False),
            "provider": config.get("provider", "openai"),
            "model": config.get("provider_config", {}).get("model", ""),
            "available_providers": ProviderFactory.available_providers(),
        }
        return jsonify({"success": True, "data": safe_config})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM 配置失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /config
# ------------------------------------------------------------------
@bp.route("/config", methods=["POST"])
@log_method
@require_auth
def update_config():
    """更新 LLM 配置"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        config = _get_llm_config()

        if "enabled" in data:
            config["enabled"] = bool(data["enabled"])
        if "provider" in data:
            config["provider"] = data["provider"]
        if "provider_config" in data:
            config["provider_config"] = data["provider_config"]

        current_app.config["LLM_CONFIG"] = config

        safe_config = {
            "enabled": config.get("enabled", False),
            "provider": config.get("provider", "openai"),
            "model": config.get("provider_config", {}).get("model", ""),
        }
        return jsonify({"success": True, "data": safe_config})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新 LLM 配置失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
