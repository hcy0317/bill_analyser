"""Two-factor login verification routes."""

from __future__ import annotations

from .support import *  # noqa: F403
from .cloud_settings import _load_application_cloud_settings
from .profile import _build_user_profile_info
from .session import _build_auth_success_result, _create_new_session_payload
from .two_factor_support import _consume_persistent_recovery_code, _create_two_factor_audit_log


@bp.route("/2fa/verify", methods=["POST"])
@log_method
def verify_2fa_login():
    """使用 TOTP 验证待登录 2FA 令牌。"""
    loop = None
    try:
        token = _extract_bearer_token()
        data = request.get_json(silent=True) or {}
        passcode = (data.get("passcode") or "").strip()

        if not token:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        if not passcode:
            return jsonify(
                {"success": False, "error": "Bad Request", "errorCode": 203005, "message": "Passcode is required"}
            ), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "pending_2fa")
        if not payload:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            loop.close()
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        secret = (user.get("two_factor_secret") or "").strip().replace(" ", "")
        if not user.get("two_factor_enabled") or not secret:
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Two-factor authentication is not enabled"}
            ), 400

        if not pyotp.TOTP(secret).verify(passcode, valid_window=1):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid passcode", "message": "The current passcode is incorrect"}
            ), 401

        tokens = _create_new_session_payload(user["id"], user["username"], config, db, loop)
        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_2fa_success",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]
        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("2FA 登录验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/recovery/verify", methods=["POST"])
@log_method
def verify_2fa_login_by_recovery_code():
    """使用恢复码验证待登录 2FA 令牌。"""
    loop = None
    try:
        token = _extract_bearer_token()
        data = request.get_json(silent=True) or {}
        recovery_code = (data.get("recoveryCode") or "").strip()

        if not token:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        if not recovery_code:
            return jsonify({"success": False, "error": "Bad Request", "message": "Recovery code is required"}), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "pending_2fa")
        if not payload:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        if not user.get("two_factor_enabled"):
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Two-factor authentication is not enabled"}
            ), 400

        if not _consume_persistent_recovery_code(db, user_id, recovery_code, loop):
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid recovery code",
                    "message": "Recovery code is invalid or already used",
                }
            ), 401

        tokens = _create_new_session_payload(user["id"], user["username"], config, db, loop)
        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_2fa_recovery_success",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        _create_two_factor_audit_log(
            db,
            loop,
            operation_type="2fa_recovery_code_used",
            user_id=user_id,
            details={"verification": "recovery_code"},
            affected_count=1,
        )
        loop.close()

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]
        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("2FA 恢复码登录验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
