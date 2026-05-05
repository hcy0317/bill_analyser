"""Application cloud-setting profile routes."""

from __future__ import annotations

from .support import *  # noqa: F403


def _build_application_cloud_setting_info(setting: dict) -> dict:
    """构建统一的应用云同步设置响应。"""
    return {
        "settingKey": setting.get("setting_key", ""),
        "settingValue": setting.get("setting_value", ""),
    }


def _load_application_cloud_settings(db, user_id: int, loop) -> list[dict]:
    """读取用户应用云同步设置。"""
    settings = loop.run_until_complete(db.get_user_application_cloud_settings(user_id))
    return [_build_application_cloud_setting_info(setting) for setting in settings]


def _validate_application_cloud_setting(setting: dict) -> str:
    """校验单条应用云同步设置，返回错误信息，合法时返回空字符串。"""
    setting_key = str((setting or {}).get("settingKey", "") or "").strip()
    setting_value = (setting or {}).get("settingValue", "")

    if not setting_key:
        return "settingKey is required"

    setting_type = SUPPORTED_APPLICATION_CLOUD_SETTING_KEY_TYPES.get(setting_key)
    if not setting_type:
        return f"Unsupported setting key: {setting_key}"

    if not isinstance(setting_value, str):
        return f"Invalid setting value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_STRING:
        return ""

    if setting_type == CLOUD_SETTING_TYPE_NUMBER:
        try:
            float(setting_value)
            return ""
        except (TypeError, ValueError):
            return f"Invalid number value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_BOOLEAN:
        if setting_value in ("true", "false"):
            return ""
        return f"Invalid boolean value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP:
        try:
            parsed = json.loads(setting_value)
        except (TypeError, ValueError, json.JSONDecodeError):
            return f"Invalid JSON value for {setting_key}"

        if not isinstance(parsed, dict):
            return f"Invalid map value for {setting_key}"

        for map_key, map_value in parsed.items():
            if not isinstance(map_key, str) or not isinstance(map_value, bool):
                return f"Invalid map value for {setting_key}"
        return ""

    return f"Unsupported setting type for {setting_key}"


def _normalize_application_cloud_settings(settings: list[dict]) -> list[dict]:
    """标准化应用云同步设置请求。"""
    normalized_settings = []

    for setting in settings:
        normalized_settings.append(
            {
                "setting_key": str(setting.get("settingKey", "") or "").strip(),
                "setting_value": str(setting.get("settingValue", "") or ""),
            }
        )

    return normalized_settings


@bp.route("/profile/cloud-settings", methods=["GET", "PUT", "DELETE"])
@log_method
@require_auth
def profile_cloud_settings():
    """获取、更新或禁用用户应用云同步设置。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if request.method == "GET":
            settings = _load_application_cloud_settings(db, user_id, loop)
            loop.close()
            return jsonify({"success": True, "result": settings or False})

        if request.method == "DELETE":
            loop.run_until_complete(db.delete_user_application_cloud_settings(user_id))
            loop.close()
            return jsonify({"success": True, "result": True})

        data = request.get_json(silent=True) or {}
        settings = data.get("settings", [])
        full_update = bool(data.get("fullUpdate", False))

        if not isinstance(settings, list):
            loop.close()
            return jsonify({"success": False, "error": "Bad Request", "message": "settings must be an array"}), 400

        for setting in settings:
            error_message = _validate_application_cloud_setting(setting)
            if error_message:
                loop.close()
                return jsonify({"success": False, "error": "Bad Request", "message": error_message}), 400

        normalized_settings = _normalize_application_cloud_settings(settings)
        loop.run_until_complete(
            db.update_user_application_cloud_settings(user_id, normalized_settings, full_update=full_update)
        )
        loop.close()

        return jsonify({"success": True, "result": True})

    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("处理用户应用云同步设置失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
