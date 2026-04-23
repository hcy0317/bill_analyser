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
from bill_analyser.core.llm_learning_service import (
    LLMLearningService,
    LLMImportSessionAnalysisError,
)
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


def _error_response(message: str, code: str, status_code: int):
    """Build a stable LLM API error payload."""
    return jsonify({
        "success": False,
        "error": message,
        "code": code,
        "error_code": code,
    }), status_code


# ------------------------------------------------------------------
# POST /analyze-transactions
# ------------------------------------------------------------------
@bp.route("/analyze-transactions", methods=["POST"])
@log_method
@require_auth
def analyze_transactions():  # pylint: disable=too-many-return-statements
    """使用 LLM 分析未分类交易并生成分类建议"""
    try:
        config = _get_llm_config()
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        user_id = _get_request_user_id()

        bill_ids = data.get("bill_ids")
        limit = data.get("limit", 20)
        session_id = data.get("session_id")
        preview_ids = data.get("preview_ids")
        preview_updates = data.get("preview_updates")

        service = _get_llm_service()
        candidates = _run_async(
            service.analyze_transactions(
                user_id=user_id,
                bill_ids=bill_ids,
                limit=limit,
                session_id=session_id,
                preview_ids=preview_ids,
                preview_updates=preview_updates,
            )
        )

        response_payload = {
            "candidates_created": len(candidates),
            "candidates": candidates,
        }
        if session_id:
            response_payload["session_id"] = session_id
            response_payload["mode"] = "import_session"
        elif bill_ids:
            response_payload["mode"] = "persisted_selection"
        else:
            response_payload["mode"] = "persisted_uncategorized"

        return jsonify({"success": True, "data": response_payload, "total": len(candidates)})
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except ValueError as exc:
        status_code = 404 if str(exc) == "Import session not found" else 400
        code = "IMPORT_SESSION_NOT_FOUND" if status_code == 404 else "INVALID_REQUEST"
        return _error_response(str(exc), code, status_code)
    except RuntimeError as exc:
        logger.warning("LLM 分析交易失败: %s", exc)
        if str(exc).startswith("Rate limit exceeded"):
            return _error_response(str(exc), "LLM_RATE_LIMITED", 429)
        return _error_response(str(exc), "LLM_PROVIDER_UNAVAILABLE", 503)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 分析交易失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


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
            service.induce_rules(
                user_id=user_id,
                category_id=category_id,
                sample_count=sample_count,
            )
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
            db.get_llm_candidates(
                user_id=user_id,
                status=status,
                type=type_,
                limit=limit,
                offset=offset,
            )
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
        user_id = _get_request_user_id()
        candidate = _run_async(db.get_llm_candidate_by_id(candidate_id, user_id=user_id))

        if not candidate:
            return jsonify({"success": False, "error": f"Candidate {candidate_id} not found"}), 404

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

        user_id = _get_request_user_id()
        service = _get_llm_service()
        result = _run_async(service.accept_candidate(candidate_id, user_id=user_id))

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

        user_id = _get_request_user_id()
        service = _get_llm_service()
        result = _run_async(service.reject_candidate(candidate_id, user_id=user_id))

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


# ------------------------------------------------------------------
# Multi-config CRUD endpoints
# ------------------------------------------------------------------

def _get_db() -> Any:
    return cast("Any", current_app.config.get("DB_INSTANCE"))


@bp.route("/configs", methods=["GET"])
@log_method
@require_auth
def list_configs():
    """列出所有保存的 LLM 配置。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        items = _run_async(db.get_llm_configs(user_id=user_id))
        # Mask API keys
        for item in items:
            if item.get("api_key"):
                item["api_key"] = item["api_key"][:4] + "****"
        return jsonify({"success": True, "data": items})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("列出 LLM 配置失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/configs", methods=["POST"])
@log_method
@require_auth
def create_config():
    """创建新的 LLM 配置。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        data = request.get_json(silent=True) or {}

        name = (data.get("name") or "").strip()
        if not name:
            return jsonify({"success": False, "error": "name is required"}), 400

        result = _run_async(db.create_llm_config(
            user_id=user_id,
            name=name,
            provider=data.get("provider", "openai"),
            model=data.get("model", ""),
            api_key=data.get("api_key", ""),
            base_url=data.get("base_url", ""),
            is_active=bool(data.get("is_active", False)),
        ))
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建 LLM 配置失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/configs/<int:config_id>", methods=["PUT"])
@log_method
@require_auth
def update_saved_config(config_id: int):
    """更新保存的 LLM 配置。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        data = request.get_json(silent=True) or {}

        result = _run_async(db.update_llm_config(config_id, user_id=user_id, **data))
        if result is None:
            return jsonify({"success": False, "error": "config_not_found"}), 404
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("更新 LLM 配置失败: id=%s, %s", config_id, exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/configs/<int:config_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_saved_config(config_id: int):
    """删除保存的 LLM 配置。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        success = _run_async(db.delete_llm_config(config_id, user_id=user_id))
        if not success:
            return jsonify({"success": False, "error": "config_not_found"}), 404
        return jsonify({"success": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除 LLM 配置失败: id=%s, %s", config_id, exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/configs/<int:config_id>/activate", methods=["POST"])
@log_method
@require_auth
def activate_saved_config(config_id: int):
    """激活指定的 LLM 配置并同步到运行时。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        success = _run_async(db.activate_llm_config(config_id, user_id=user_id))
        if not success:
            return jsonify({"success": False, "error": "config_not_found"}), 404

        # Sync active config to runtime
        active = _run_async(db.get_active_llm_config(user_id=user_id))
        if active:
            current_app.config["LLM_CONFIG"] = {
                "enabled": True,
                "provider": active["provider"],
                "provider_config": {
                    "api_key": active["api_key"],
                    "base_url": active["base_url"],
                    "model": active["model"],
                },
            }

        return jsonify({"success": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("激活 LLM 配置失败: id=%s, %s", config_id, exc)
        return jsonify({"success": False, "error": str(exc)}), 500
