"""Credential login and logout routes."""

from __future__ import annotations

from .support import *  # noqa: F403
from .cloud_settings import _load_application_cloud_settings
from .profile import _build_user_profile_info


def _build_auth_success_result(user: dict, tokens: dict, application_cloud_settings: list[dict] | None = None) -> dict:
    """构建统一的认证成功响应结构。"""
    return {
        "token": tokens["access_token"],
        "refreshToken": tokens.get("refresh_token"),
        "need2FA": False,
        "user": user,
        "applicationCloudSettings": application_cloud_settings or [],
    }


def _create_new_session_payload(user_id: int, username: str, config: dict, db, loop) -> dict:
    """生成新的会话 token 并写入 session。"""
    tokens = generate_jwt_token(user_id, username, config)

    new_token_hash = calculate_token_hash(tokens["access_token"])
    new_refresh_hash = calculate_token_hash(tokens["refresh_token"])

    loop.run_until_complete(
        db.create_session(
            {
                "user_id": user_id,
                "token_hash": new_token_hash,
                "refresh_token_hash": new_refresh_hash,
                "expires_at": tokens["expires_at"],
                "refresh_expires_at": tokens["refresh_expires_at"],
                "user_agent": request.headers.get("User-Agent", ""),
                "ip_address": get_client_ip(),
            }
        )
    )

    return tokens


@bp.route("/auth/login", methods=["POST"])
@log_method
def login():
    """
    用户登录端点

    请求体：
        {
            "loginName": "username or email",
            "password": "password"
        }

    响应：
        {
            "success": true,
            "result": {
                "token": "jwt_access_token",
                "user": {
                    "username": "...",
                    "email": "...",
                    ...
                }
            }
        }
    """
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            data = {}
        login_name = data.get("loginName", "").strip()
        password = data.get("password", "")

        if not login_name or not password:
            logger.warning("登录失败: 缺少用户名或密码")
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Username and password are required"}
            ), 400

        # 获取配置和数据库
        config = load_auth_config()
        db = get_app_context()

        # 获取客户端信息
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        # 查找用户（支持用户名或邮箱登录）
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        logger.info("查找用户: login_name=%s", login_name)
        user = loop.run_until_complete(db.get_user_by_username(login_name))
        if not user:
            user = loop.run_until_complete(db.get_user_by_email(login_name))

        if not user:
            logger.warning("登录失败: 用户不存在 - %s", login_name)

            # 记录失败日志
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": login_name,
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "User not found",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid username or password"}
            ), 401

        # 检查账户是否被锁定
        logger.info("检查账户锁定状态: user_id=%s", user["id"])
        is_locked = loop.run_until_complete(db.is_user_locked(user["id"]))
        if is_locked:
            logger.warning("登录失败: 账户被锁定 - user_id=%s", user["id"])

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Account locked",
                    }
                )
            )
            loop.close()

            return jsonify(
                {
                    "success": False,
                    "error": "Account locked",
                    "message": "Account is temporarily locked due to multiple failed login attempts",
                }
            ), 403

        # 验证密码
        logger.info("验证密码: user_id=%s", user["id"])
        password_hash = user["password_hash"]
        if not bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8")):
            logger.warning("登录失败: 密码错误 - user_id=%s", user["id"])

            # 增加失败次数
            lockout_minutes = config.get("lockout_duration_minutes", 15)
            loop.run_until_complete(db.increment_failed_login(user["id"], lockout_minutes))

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Invalid password",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid username or password"}
            ), 401

        # 检查账户是否激活
        if not user.get("is_active"):
            logger.warning("登录失败: 账户未激活 - user_id=%s", user["id"])

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Account not active",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Account not active", "message": "Your account has been deactivated"}
            ), 403

        if user.get("two_factor_enabled"):
            pending_token = generate_action_token(
                user["id"], user["username"], user.get("email", ""), config, "pending_2fa", expires_in_hours=1
            )

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_2fa_pending",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": True,
                    }
                )
            )
            loop.close()

            return jsonify({"success": True, "result": {"token": pending_token, "need2FA": True}})

        # 生成JWT令牌
        logger.info("生成JWT令牌: user_id=%s", user["id"])
        tokens = generate_jwt_token(user["id"], user["username"], config)

        # 计算令牌哈希
        token_hash = calculate_token_hash(tokens["access_token"])
        refresh_token_hash = calculate_token_hash(tokens["refresh_token"])

        # 创建会话
        logger.info("创建会话: user_id=%s", user["id"])
        session_id = loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user["id"],
                    "token_hash": token_hash,
                    "refresh_token_hash": refresh_token_hash,
                    "expires_at": tokens["expires_at"],
                    "refresh_expires_at": tokens["refresh_expires_at"],
                    "user_agent": user_agent,
                    "ip_address": ip_address,
                }
            )
        )

        # 更新最后登录时间
        loop.run_until_complete(db.update_user_last_login(user["id"], ip_address or ""))

        # 记录成功日志
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_success",
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                    "success": True,
                    "metadata": json.dumps({"session_id": session_id}),
                }
            )
        )

        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)

        loop.close()

        # 构建用户信息响应
        logger.info(
            "[login] 数据库用户记录: id=%s, username=%s, fiscal_year_start=%s",
            user["id"],
            user["username"],
            user.get("fiscal_year_start", "NOT_SET"),
        )

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]

        logger.info("登录成功: user_id=%s, username=%s, ip=%s", user["id"], user["username"], ip_address)
        logger.info(
            "[login] 返回给前端的user_info: id=%s, username=%s, fiscalYearStart=%s (0x%x)",
            user_info["id"],
            user_info["username"],
            user_info["fiscalYearStart"],
            user_info["fiscalYearStart"],
        )

        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )

    except Exception as error:
        logger.error("登录过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/auth/logout", methods=["POST"])
@log_method
def logout():
    """
    用户登出端点

    需要 Authorization 请求头。
    """
    try:
        auth_header = request.headers.get("Authorization", "")

        if not auth_header:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != "bearer":
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid authorization header"}), 401

        token = parts[1]
        token_hash = calculate_token_hash(token)

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取会话信息
        session = loop.run_until_complete(db.get_session_by_token_hash(token_hash))

        if session:
            # 使会话失效
            logger.info("使会话失效: session_id=%s", session["id"])
            loop.run_until_complete(db.invalidate_session(token_hash))

            # 记录登出日志
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": session["user_id"],
                        "username": session["username"],
                        "event_type": "logout",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": True,
                    }
                )
            )

            logger.info("登出成功: user_id=%s, username=%s", session["id"], session["username"])
        else:
            logger.warning("登出时未找到会话: token_hash=%s...", token_hash[:16])

        loop.close()

        # 🆕 返回 result 字段以匹配前端期望（stores/index.ts 第 428 行）
        return jsonify(
            {
                "success": True,
                "result": True,  # ← 前端会检查这个字段！
                "message": "Logged out successfully",
            }
        )

    except Exception as error:
        logger.error("登出过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
