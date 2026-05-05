"""llm configs route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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
