"""Sensitive operation step-up auth routes."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/security/step-up/verify", methods=["POST"])
@log_method
@require_auth
def verify_security_step_up():
    """为敏感操作签发短期 step-up token。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = str(data.get("password", "") or "")
        passcode = str(data.get("passcode", "") or "").strip()

        if not password and not passcode:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "password or passcode is required"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        verified_via = ""
        if password:
            if not _verify_sensitive_operation_password(db, user, password, loop):
                loop.close()
                return jsonify(
                    {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
                ), 401
            verified_via = "password"
        else:
            secret = str(user.get("two_factor_secret") or "").strip().replace(" ", "")
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
            verified_via = "passcode"

        step_up_token = generate_action_token(
            user_id,
            username,
            user.get("email", ""),
            config,
            "step_up",
            expires_in_hours=1,
        )
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "step_up_verified",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                    "metadata": json.dumps({"verified_via": verified_via}, ensure_ascii=False),
                }
            )
        )
        loop.close()

        return jsonify(
            {
                "success": True,
                "result": {
                    "stepUpToken": step_up_token,
                    "verifiedVia": verified_via,
                },
            }
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("step-up 验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
