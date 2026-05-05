"""Email verification and password recovery routes."""

from __future__ import annotations

from .support import *  # noqa: F403
from .profile import _build_user_profile_info
from .registration import validate_password
from .session import _create_new_session_payload


@bp.route("/auth/email/verify", methods=["POST"])
@log_method
def verify_email_by_token():
    """使用动作令牌验证邮箱（REST）。"""
    loop = None
    try:
        data = request.json or {}
        token = str(data.get("token", "") or "").strip()
        request_new_token = bool(data.get("requestNewToken", False))

        if not token:
            return jsonify({"success": False, "error": "Bad Request", "message": "Verification token is required"}), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "verify_email")
        if not payload:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Verification token is invalid or expired"}
            ), 400

        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Verification token is invalid"},
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        loop.run_until_complete(db.update_user(user_id, {"email_verified": 1}))
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        new_token = None
        if request_new_token and user:
            tokens = _create_new_session_payload(user_id, user["username"], config, db, loop)
            new_token = tokens["access_token"]

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": user["username"],
                    "event_type": "email_verified",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        return jsonify(
            {
                "success": True,
                "result": {
                    "newToken": new_token,
                    "user": _build_user_profile_info(user),
                    "notificationContent": "",
                },
            }
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("邮箱验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/email/resend-verification", methods=["POST"])
@log_method
def resend_verification_email_unauthed():
    """未登录用户重发验证邮件（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        password = data.get("password", "")

        if not email or not password:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email and password are required"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_email(email))
        if not user or not _verify_user_password(user, password):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid email or password"}
            ), 401

        verification_token = generate_action_token(
            user["id"], user["username"], user.get("email", ""), config, "verify_email"
        )

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "verification_email_resend_requested",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                    "metadata": json.dumps(
                        {
                            "email": email,
                            "delivery": "not_configured_mock_success",
                            "verification_token": verification_token,
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("未登录重发验证邮件失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/password/forgot", methods=["POST"])
@log_method
def request_password_reset():
    """请求密码重置（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        if not email:
            return jsonify({"success": False, "error": "Bad Request", "message": "Email is required"}), 400

        config = load_auth_config()
        if not config.get("enable_user_forget_password", False):
            return jsonify(
                {
                    "success": False,
                    "error": "Forget password disabled",
                    "message": "Forget password is currently disabled",
                }
            ), 403

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user = loop.run_until_complete(db.get_user_by_email(email))

        if user:
            reset_token = generate_action_token(
                user["id"], user["username"], user.get("email", ""), config, "reset_password"
            )
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "password_reset_requested",
                        "ip_address": get_client_ip(),
                        "user_agent": request.headers.get("User-Agent", ""),
                        "success": True,
                        "metadata": json.dumps(
                            {"email": email, "delivery": "not_configured_mock_success", "reset_token": reset_token},
                            ensure_ascii=False,
                        ),
                    }
                )
            )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("请求重置密码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/password/reset", methods=["POST"])
@log_method
def reset_password_by_token():
    """使用动作令牌重置密码（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        password = data.get("password", "")
        token = str(data.get("token", "") or "").strip()

        if not email or not password or not token:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email, password and token are required"}
            ), 400

        config = load_auth_config()
        if not config.get("enable_user_forget_password", False):
            return jsonify(
                {
                    "success": False,
                    "error": "Forget password disabled",
                    "message": "Forget password is currently disabled",
                }
            ), 403

        is_valid, error_msg = validate_password(password, config)
        if not is_valid:
            return jsonify({"success": False, "error": "Invalid password", "message": error_msg}), 400

        payload = decode_action_token(token, config, "reset_password")
        if not payload:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Reset password token is invalid or expired"}
            ), 400

        if payload.get("email") != email:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Reset password token does not match email"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            loop.close()
            return jsonify({"success": False, "error": "Invalid token"}), 400

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user or (user.get("email") or "").strip() != email:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")
        loop.run_until_complete(db.update_user(user["id"], {"password_hash": password_hash}))
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "password_reset_completed",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重置密码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/oauth2/authorize", methods=["POST"])
@log_method
def authorize_oauth2_callback():
    """OAuth2 回调授权入口（REST）。"""
    try:
        config = load_auth_config()
        if not config.get("enable_oauth2", False):
            return jsonify(
                {"success": False, "error": "OAuth2 disabled", "message": "OAuth2 login is currently disabled"}
            ), 403

        return jsonify(
            {
                "success": False,
                "error": "Not Implemented",
                "message": "OAuth2 callback authorization is not implemented in this workspace build",
            }
        ), 501
    except Exception as exc:
        logger.error("OAuth2 回调授权失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
