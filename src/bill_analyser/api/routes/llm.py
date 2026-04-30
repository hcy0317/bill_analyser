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
from bill_analyser.core.db_llm_config import normalize_llm_advanced_settings
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("LLM_API")

bp = Blueprint("llm", __name__)


def _get_request_user_id() -> int:
    return get_required_request_int("user_id")


def _copy_runtime_llm_config(config: dict[str, Any]) -> dict[str, Any]:
    """Copy a runtime LLM config without sharing nested mutable state."""
    provider_config = config.get("provider_config") or {}
    if not isinstance(provider_config, dict):
        provider_config = {}
    advanced_settings = normalize_llm_advanced_settings(
        config.get("advanced_settings") or provider_config.get("advanced_settings") or {}
    )
    return {
        "enabled": bool(config.get("enabled", False)),
        "provider": config.get("provider", "openai"),
        "provider_config": dict(provider_config),
        "advanced_settings": advanced_settings,
    }


def _build_runtime_config_from_saved_config(config: dict[str, Any]) -> dict[str, Any]:
    """Build request-local provider settings from a persisted user-scoped config."""
    return {
        "enabled": True,
        "provider": config.get("provider", "openai"),
        "advanced_settings": normalize_llm_advanced_settings(config.get("advanced_settings")),
        "provider_config": {
            "api_key": config.get("api_key", ""),
            "base_url": config.get("base_url", ""),
            "model": config.get("model", ""),
        },
    }


def _get_user_runtime_config_store() -> dict[int, dict[str, Any]]:
    """Return per-user legacy runtime configs kept out of process-global LLM_CONFIG."""
    store = current_app.config.setdefault("LLM_CONFIG_BY_USER", {})
    if not isinstance(store, dict):
        store = {}
        current_app.config["LLM_CONFIG_BY_USER"] = store
    return cast("dict[int, dict[str, Any]]", store)


def _set_user_runtime_config(user_id: int, config: dict[str, Any]) -> None:
    _get_user_runtime_config_store()[user_id] = _copy_runtime_llm_config(config)


def _clear_user_runtime_config(user_id: int) -> None:
    _get_user_runtime_config_store().pop(user_id, None)


def _get_llm_config(user_id: int | None = None) -> dict[str, Any]:
    """Get the effective LLM config without leaking one user's saved config to another."""
    if user_id is not None:
        user_runtime_config = _get_user_runtime_config_store().get(user_id)
        if user_runtime_config:
            return _copy_runtime_llm_config(user_runtime_config)

        db = cast("Any", current_app.config.get("DB_INSTANCE"))
        if db is not None and hasattr(db, "get_active_llm_config"):
            active_config = _run_async(db.get_active_llm_config(user_id=user_id))
            if active_config:
                return _copy_runtime_llm_config(
                    _build_runtime_config_from_saved_config(active_config)
                )

    default_config = cast("dict[str, Any]", current_app.config.get("LLM_CONFIG", {}))
    return _copy_runtime_llm_config(default_config)


def _get_llm_service(user_id: int) -> LLMLearningService:
    """Build LLM learning service from current config."""
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    config = _get_llm_config(user_id)

    provider_name = config.get("provider", "openai")
    provider_config = config.get("provider_config", {})
    advanced_settings = normalize_llm_advanced_settings(
        config.get("advanced_settings") or provider_config.get("advanced_settings") or {}
    )

    provider = ProviderFactory.create(provider_name, provider_config)
    return LLMLearningService(db=db, provider=provider, advanced_settings=advanced_settings)


def _error_response(message: str, code: str, status_code: int):
    """Build a stable LLM API error payload."""
    return jsonify({
        "success": False,
        "error": message,
        "code": code,
        "error_code": code,
    }), status_code


def _safe_llm_config_payload(config: dict[str, Any]) -> dict[str, Any]:
    """Return a saved LLM config without exposing a cleartext API key."""
    safe_config = dict(config)
    api_key = str(safe_config.pop("api_key", "") or "")
    safe_config["has_api_key"] = bool(api_key)
    safe_config["api_key"] = "********" if api_key else ""
    safe_config["advanced_settings"] = normalize_llm_advanced_settings(
        safe_config.get("advanced_settings")
    )
    return safe_config


# ------------------------------------------------------------------
# POST /analyze-transactions
# ------------------------------------------------------------------
@bp.route("/analyze-transactions", methods=["POST"])
@log_method
@require_auth
def analyze_transactions():  # pylint: disable=too-many-return-statements
    """使用 LLM 分析未分类交易并生成分类建议"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        bill_ids = data.get("bill_ids")
        limit = data.get("limit", 20)
        session_id = data.get("session_id")
        preview_ids = data.get("preview_ids")
        preview_updates = data.get("preview_updates")

        service = _get_llm_service(user_id)
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
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        data = request.get_json()
        if not data or "category_id" not in data:
            return jsonify({"success": False, "error": "category_id is required"}), 400

        category_id = data["category_id"]
        sample_count = data.get("sample_count", 10)

        service = _get_llm_service(user_id)
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
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        service = _get_llm_service(user_id)
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
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        service = _get_llm_service(user_id)
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
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        # Return safe subset (no API keys)
        safe_config = {
            "enabled": config.get("enabled", False),
            "provider": config.get("provider", "openai"),
            "model": config.get("provider_config", {}).get("model", ""),
            "advanced_settings": normalize_llm_advanced_settings(config.get("advanced_settings")),
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
        user_id = _get_request_user_id()
        data = request.get_json()
        if not data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        config = _get_llm_config(user_id)

        if "enabled" in data:
            config["enabled"] = bool(data["enabled"])
        if "provider" in data:
            config["provider"] = data["provider"]
        if "provider_config" in data:
            config["provider_config"] = data["provider_config"]
        if "advanced_settings" in data:
            config["advanced_settings"] = normalize_llm_advanced_settings(data["advanced_settings"])

        _set_user_runtime_config(user_id, config)

        safe_config = {
            "enabled": config.get("enabled", False),
            "provider": config.get("provider", "openai"),
            "model": config.get("provider_config", {}).get("model", ""),
            "advanced_settings": normalize_llm_advanced_settings(config.get("advanced_settings")),
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
        safe_items = [_safe_llm_config_payload(item) for item in items]
        return jsonify({"success": True, "data": safe_items})
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
            advanced_settings=data.get("advanced_settings"),
            is_active=bool(data.get("is_active", False)),
        ))
        if result.get("is_active"):
            _clear_user_runtime_config(user_id)
        return jsonify({"success": True, "data": _safe_llm_config_payload(result)})
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
        if data.get("api_key") == "********":
            data = {key: value for key, value in data.items() if key != "api_key"}

        result = _run_async(db.update_llm_config(config_id, user_id=user_id, **data))
        if result is None:
            return jsonify({"success": False, "error": "config_not_found"}), 404
        if result.get("is_active"):
            _clear_user_runtime_config(user_id)
        return jsonify({"success": True, "data": _safe_llm_config_payload(result)})
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
            _clear_user_runtime_config(user_id)

        return jsonify({"success": True})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("激活 LLM 配置失败: id=%s, %s", config_id, exc)
        return jsonify({"success": False, "error": str(exc)}), 500


# ------------------------------------------------------------------
# POST /preview-recommend  (A5 yellow LLM path)
# ------------------------------------------------------------------
@bp.route("/preview-recommend", methods=["POST"])
@log_method
@require_auth
def preview_recommend():
    """为导入预览行生成黄色 LLM 推荐建议。"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True) or {}
        session_id = data.get("session_id")
        if not session_id:
            return _error_response("session_id is required", "INVALID_REQUEST", 400)

        preview_ids = data.get("preview_ids")
        preview_updates = data.get("preview_updates")
        limit = data.get("limit", 20)

        service = _get_llm_service(user_id)
        suggestions = _run_async(
            service.recommend_for_preview(
                user_id=user_id,
                session_id=session_id,
                preview_ids=preview_ids,
                preview_updates=preview_updates,
                limit=limit,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                "suggestions": suggestions,
                "count": len(suggestions),
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except RuntimeError as exc:
        logger.warning("LLM preview recommend 失败: %s", exc)
        if str(exc).startswith("Rate limit exceeded"):
            return _error_response(str(exc), "LLM_RATE_LIMITED", 429)
        return _error_response(str(exc), "LLM_PROVIDER_UNAVAILABLE", 503)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview recommend 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


# ------------------------------------------------------------------
# POST /preview-recommend/accept  (A5 memory write)
# ------------------------------------------------------------------
@bp.route("/preview-recommend/accept", methods=["POST"])
@log_method
@require_auth
def preview_recommend_accept():
    """接受黄色 LLM 推荐并写入 MEMORY。"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True) or {}

        session_id = data.get("session_id")
        preview_id = data.get("preview_id")
        suggestion = data.get("suggestion")

        if not session_id or not preview_id:
            return _error_response(
                "session_id and preview_id are required", "INVALID_REQUEST", 400
            )

        service = _get_llm_service(user_id)
        accept_result = _run_async(
            service.accept_preview_recommendation(
                user_id=user_id,
                session_id=session_id,
                preview_id=int(preview_id),
                suggestion=suggestion,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                **accept_result,
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview accept 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


# ------------------------------------------------------------------
# POST /preview-recommend/reject  (A5 memory write)
# ------------------------------------------------------------------
@bp.route("/preview-recommend/reject", methods=["POST"])
@log_method
@require_auth
def preview_recommend_reject():
    """拒绝黄色 LLM 推荐并写入 MEMORY（含可选用户纠正）。"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True) or {}

        session_id = data.get("session_id")
        preview_id = data.get("preview_id")
        suggestion = data.get("suggestion")
        user_correction = data.get("user_correction")

        if not session_id or not preview_id:
            return _error_response(
                "session_id and preview_id are required", "INVALID_REQUEST", 400
            )

        service = _get_llm_service(user_id)
        reject_result = _run_async(
            service.reject_preview_recommendation(
                user_id=user_id,
                session_id=session_id,
                preview_id=int(preview_id),
                suggestion=suggestion,
                user_correction=user_correction,
            )
        )

        return jsonify({
            "success": True,
            "data": {
                "session_id": session_id,
                **reject_result,
            },
        })
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM preview reject 失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


# ------------------------------------------------------------------
# GET /memory  (A5 memory audit)
# ------------------------------------------------------------------
@bp.route("/memory", methods=["GET"])
@log_method
@require_auth
def list_memory_events():
    """获取 LLM memory 事件列表（审计用）。"""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        session_id = request.args.get("session_id")
        event_type = request.args.get("event_type")
        limit = request.args.get("limit", 100, type=int)
        offset = request.args.get("offset", 0, type=int)

        events = _run_async(
            db.get_llm_memory_events(
                user_id,
                session_id=session_id,
                event_type=event_type,
                limit=limit,
                offset=offset,
            )
        )
        total = _run_async(db.get_llm_memory_events_count(user_id, session_id=session_id))

        return jsonify({"success": True, "data": events, "total": total})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 LLM memory 事件失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
