"""Two-factor setup and recovery-code management routes."""

from __future__ import annotations

from .support import *  # noqa: F403
from .session import _create_new_session_payload
from .two_factor_support import *  # noqa: F403


@bp.route("/2fa/status", methods=["GET"])
@log_method
@require_auth
def get_2fa_status():
    """获取用户2FA状态（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()

        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        # 返回2FA状态
        is_enabled = user.get("two_factor_enabled", False)

        return jsonify({"success": True, "result": {"enable": bool(is_enabled), "isEnabled": bool(is_enabled)}})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取2FA状态失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/2fa/enable/request", methods=["POST"])
@log_method
@require_auth
def enable_2fa_request():
    """请求启用 2FA，返回 secret 与二维码。"""
    try:
        secret = pyotp.random_base32()
        qrcode_data = _generate_2fa_qrcode_data_url(_get_request_username(), secret)

        return jsonify({"success": True, "result": {"secret": secret, "qrcode": qrcode_data}})
    except Exception as exc:
        logger.error("请求启用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/enable/confirm", methods=["POST"])
@log_method
@require_auth
def enable_2fa_confirm():
    """确认启用 2FA。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        secret = (data.get("secret") or "").strip().replace(" ", "")
        passcode = (data.get("passcode") or "").strip()

        if not secret or not passcode:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Secret and passcode are required"}
            ), 400

        if not pyotp.TOTP(secret).verify(passcode, valid_window=1):
            return jsonify(
                {"success": False, "error": "Invalid passcode", "message": "The current passcode is incorrect"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        success = loop.run_until_complete(
            db.update_user(user_id, {"two_factor_enabled": 1, "two_factor_secret": secret})
        )
        if not success:
            loop.close()
            return jsonify(
                {"success": False, "error": "Update failed", "message": "Failed to enable two-factor authentication"}
            ), 500

        recovery_codes = _generate_recovery_codes()
        try:
            stored_count = _replace_persistent_recovery_codes(db, user_id, recovery_codes, loop)
        except Exception:
            loop.run_until_complete(db.update_user(user_id, {"two_factor_enabled": 0, "two_factor_secret": ""}))
            try:
                _clear_persistent_recovery_codes(db, user_id, loop)
            except Exception as clear_exc:  # pylint: disable=broad-except
                logger.warning("2FA 启用补偿清理恢复码失败: user_id=%s, error=%s", user_id, clear_exc)
            raise

        if stored_count != len(recovery_codes):
            loop.run_until_complete(db.update_user(user_id, {"two_factor_enabled": 0, "two_factor_secret": ""}))
            _clear_persistent_recovery_codes(db, user_id, loop)
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Update failed",
                    "message": "Failed to persist two-factor recovery codes",
                }
            ), 500

        tokens = _create_new_session_payload(user_id, username, config, db, loop)
        _create_two_factor_audit_log(
            db,
            loop,
            operation_type="2fa_enabled",
            user_id=user_id,
            details={"recovery_code_count": stored_count},
            affected_count=stored_count,
        )
        loop.close()

        return jsonify(
            {
                "success": True,
                "result": {
                    "token": tokens["access_token"],
                    "refreshToken": tokens["refresh_token"],
                    "recoveryCodes": recovery_codes,
                },
            }
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("确认启用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/disable", methods=["POST"])
@log_method
@require_auth
def disable_2fa():
    """禁用 2FA。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        auth_ok, auth_mode = _resolve_sensitive_operation_auth(
            db=db,
            user=user,
            config=config,
            payload=data,
            loop=loop,
        )
        if not auth_ok:
            loop.close()
            if auth_mode == "missing_credentials":
                return jsonify(
                    {"success": False, "error": "Bad Request", "message": "Current password or stepUpToken is required"}
                ), 400
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        success = loop.run_until_complete(
            db.update_user(user_id, {"two_factor_enabled": 0, "two_factor_secret": ""})
        )
        if not success:
            loop.close()
            return jsonify(
                {"success": False, "error": "Update failed", "message": "Failed to disable two-factor authentication"}
            ), 500

        cleared_count = _clear_persistent_recovery_codes(db, user_id, loop)
        _create_two_factor_audit_log(
            db,
            loop,
            operation_type="2fa_disabled",
            user_id=user_id,
            details={"cleared_recovery_code_count": cleared_count, "auth_mode": auth_mode},
            affected_count=cleared_count,
        )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("禁用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/recovery/regenerate", methods=["POST"])
@log_method
@require_auth
def regenerate_2fa_recovery_codes():
    """重新生成 2FA 恢复码。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        auth_ok, auth_mode = _resolve_sensitive_operation_auth(
            db=db,
            user=user,
            config=config,
            payload=data,
            loop=loop,
        )
        if not auth_ok:
            loop.close()
            if auth_mode == "missing_credentials":
                return jsonify(
                    {"success": False, "error": "Bad Request", "message": "Current password or stepUpToken is required"}
                ), 400
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        if not user.get("two_factor_enabled"):
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Two-factor authentication is not enabled"}
            ), 400

        recovery_codes = _generate_recovery_codes()
        stored_count = _replace_persistent_recovery_codes(db, user_id, recovery_codes, loop)
        if stored_count != len(recovery_codes):
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Update failed",
                    "message": "Failed to persist two-factor recovery codes",
                }
            ), 500

        _create_two_factor_audit_log(
            db,
            loop,
            operation_type="2fa_recovery_regenerated",
            user_id=user_id,
            details={"recovery_code_count": stored_count, "auth_mode": auth_mode},
            affected_count=stored_count,
        )
        loop.close()

        return jsonify({"success": True, "result": {"recoveryCodes": recovery_codes}})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重新生成2FA恢复码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
