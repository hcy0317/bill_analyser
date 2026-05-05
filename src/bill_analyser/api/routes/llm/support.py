"""LLM 学习服务 API 路由"""

# pylint: disable=unused-import

from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.ai.llm.learning_service import (
    LLMLearningService,
    LLMImportSessionAnalysisError,
)
from bill_analyser.core.ai.llm.provider import ProviderFactory
from bill_analyser.core.database.llm.config import normalize_llm_advanced_settings
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


def _get_llm_service(
    user_id: int,
    *,
    require_provider: bool = True,
) -> LLMLearningService:
    """Build LLM learning service from current config."""
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    provider = None
    advanced_settings: dict[str, Any] = {}

    if require_provider:
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


# ------------------------------------------------------------------
# POST /induce-rules
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /rule-synthesis
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# GET /candidates
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# GET /candidates/<id>
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /candidates/<id>/accept
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /candidates/<id>/reject
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# GET /config
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /config
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# Multi-config CRUD endpoints
# ------------------------------------------------------------------

def _get_db() -> Any:
    return cast("Any", current_app.config.get("DB_INSTANCE"))












# ------------------------------------------------------------------
# POST /preview-recommend  (A5 yellow LLM path)
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /preview-recommend/accept  (A5 memory write)
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# POST /preview-recommend/reject  (A5 memory write)
# ------------------------------------------------------------------


# ------------------------------------------------------------------
# GET /memory  (A5 memory audit)
# ------------------------------------------------------------------

__all__ = [name for name in globals() if not name.startswith("__")]
