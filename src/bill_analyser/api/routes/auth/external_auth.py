"""External authentication profile routes."""

from __future__ import annotations

from .support import *  # noqa: F403


def _build_external_auth_info(external_auth: dict) -> dict:
    """构建统一的第三方登录响应。"""
    return {
        "externalAuthCategory": external_auth.get("external_auth_category", ""),
        "externalAuthType": external_auth.get("external_auth_type", ""),
        "linked": bool(external_auth.get("linked", True)),
        "externalUsername": external_auth.get("external_username") or "",
        "createdAt": _datetime_to_unix_millis(external_auth.get("created_at", "")),
    }


@bp.route("/profile/external-auths", methods=["GET"])
@log_method
@require_auth
def list_profile_external_auths():
    """获取当前用户第三方登录绑定列表（REST）。"""
    loop = None
    try:
        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        external_auths = loop.run_until_complete(db.get_user_external_auths(user_id))
        result = [_build_external_auth_info(item) for item in external_auths]

        oauth2_enabled = bool(config.get("enable_oauth2", False))
        oauth2_provider = str(config.get("oauth2_provider", "") or "").strip()
        linked_types = {item.get("externalAuthType") for item in result}

        if oauth2_enabled and oauth2_provider and oauth2_provider not in linked_types:
            result.append(
                {
                    "externalAuthCategory": "oauth2",
                    "externalAuthType": oauth2_provider,
                    "linked": False,
                    "externalUsername": "",
                    "createdAt": 0,
                }
            )

        loop.close()

        result.sort(
            key=lambda item: (
                0 if item.get("linked") else 1,
                item.get("externalAuthType", ""),
                -(item.get("createdAt") or 0),
            )
        )

        return jsonify({"success": True, "result": result})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取第三方登录列表失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc), "errorMessage": str(exc)}
        ), 500


@bp.route("/profile/external-auths/unlink", methods=["POST"])
@log_method
@require_auth
def unlink_profile_external_auth():
    """解绑当前用户第三方登录（REST）。"""
    loop = None
    try:
        data = request.get_json() or {}
        external_auth_type = str(data.get("externalAuthType", "") or "").strip()
        password = str(data.get("password", "") or "")

        if not external_auth_type:
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "externalAuthType is required",
                    "errorMessage": "externalAuthType is required",
                }
            ), 400

        if not password:
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "password is required",
                    "errorMessage": "password is required",
                }
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found", "errorMessage": "User not found"}), 404

        if not _verify_user_password(user, password):
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "Invalid password",
                    "errorMessage": "Invalid password",
                }
            ), 400

        existing = loop.run_until_complete(db.get_user_external_auth(user_id, external_auth_type))
        if not existing:
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Not Found",
                    "message": "Third-party login is not linked",
                    "errorMessage": "Third-party login is not linked",
                }
            ), 404

        success = loop.run_until_complete(db.delete_user_external_auth(user_id, external_auth_type))
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "external_auth_unlinked",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": success,
                    "metadata": json.dumps(
                        {
                            "external_auth_type": external_auth_type,
                            "external_auth_category": existing.get("external_auth_category", ""),
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": success})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("解绑第三方登录失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc), "errorMessage": str(exc)}
        ), 500


__all__ = [name for name in globals() if not name.startswith("__")]
