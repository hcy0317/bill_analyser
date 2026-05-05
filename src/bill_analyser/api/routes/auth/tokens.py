"""Token list, refresh, revoke, and personal token routes."""

from __future__ import annotations

# pylint: disable=wildcard-import,unused-wildcard-import,undefined-variable
# pylint: disable=line-too-long,mixed-line-endings,broad-exception-caught
# pylint: disable=too-many-locals,too-many-return-statements,too-many-branches

from bill_analyser.core.auth_rust_bridge import (
    AuthRustBridgeUnavailable,
    AuthRustValidationError,
    validate_refresh_token_claims as rust_validate_refresh_token_claims,
)

from .support import *  # noqa: F403
from .cloud_settings import _load_application_cloud_settings
from .profile import _build_user_profile_info


def _build_api_base_url() -> str:
    """构建 API token 示例使用的 API 基础地址。"""
    return f"{request.url_root.rstrip('/')}/api"


def _build_mcp_url() -> str:
    """构建 MCP token 示例地址。"""
    return f"{request.url_root.rstrip('/')}/mcp"


def _get_token_user_agent(token_kind: str) -> str:
    """为生成的 token 构造稳定的 user_agent 标识。"""
    if token_kind == "api":
        return "Bill Analyser API Token"
    if token_kind == "mcp":
        return "Bill Analyser MCP Token"
    return request.headers.get("User-Agent", "")


def _infer_token_type(user_agent: str) -> int:
    """根据 user_agent 推断 token 类型，兼容前端 Session 解析。"""
    user_agent_lower = (user_agent or "").lower()
    if "mcp token" in user_agent_lower:
        return TOKEN_TYPE_MCP
    if "api token" in user_agent_lower:
        return TOKEN_TYPE_API
    return TOKEN_TYPE_DEFAULT


@bp.route("/tokens/refresh", methods=["POST"])
@log_method
def refresh_token():
    """
    刷新令牌端点

    请求体：
        {
            "refreshToken": "refresh_token_string"
        }
    """
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            data = {}
        refresh_token_str = data.get("refreshToken", "")

        if not refresh_token_str:
            return jsonify({"success": False, "error": "Invalid request", "message": "Refresh token is required"}), 400

        config = load_auth_config()
        jwt_secret = config.get("jwt_secret")
        jwt_algorithm = config.get("jwt_algorithm", "HS256")

        # 验证刷新令牌
        try:
            payload = jwt.decode(refresh_token_str, jwt_secret, algorithms=[jwt_algorithm])

            try:
                refresh_claims = rust_validate_refresh_token_claims(payload)
            except AuthRustValidationError as validation_error:
                return jsonify(
                    {"success": False, "error": validation_error.error, "message": validation_error.message},
                ), validation_error.status

            user_id = refresh_claims.user_id
            username = refresh_claims.username

        except jwt.ExpiredSignatureError:
            return jsonify({"success": False, "error": "Token expired", "message": "Refresh token has expired"}), 401
        except jwt.InvalidTokenError:
            return jsonify({"success": False, "error": "Invalid token", "message": "Invalid refresh token"}), 401
        except AuthRustBridgeUnavailable:
            return (
                jsonify(
                    {
                        "success": False,
                        "error": "Service Unavailable",
                        "message": "Authentication temporarily unavailable",
                    }
                ),
                503,
            )

        # 生成新的访问令牌
        logger.info("生成新令牌: user_id=%s", user_id)
        tokens = generate_jwt_token(user_id, username, config)

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found", "message": "User does not exist"}), 404

        # 查找旧会话并更新
        # old_refresh_hash = calculate_token_hash(refresh_token_str)  # 保留以便后续使用
        new_token_hash = calculate_token_hash(tokens["access_token"])
        new_refresh_hash = calculate_token_hash(tokens["refresh_token"])

        # 这里简化处理：创建新会话
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user_id,
                    "token_hash": new_token_hash,
                    "refresh_token_hash": new_refresh_hash,
                    "expires_at": tokens["expires_at"],
                    "refresh_expires_at": tokens["refresh_expires_at"],
                    "user_agent": user_agent,
                    "ip_address": ip_address,
                }
            )
        )

        application_cloud_settings = _load_application_cloud_settings(db, user_id, loop)

        loop.close()

        logger.info("令牌刷新成功: user_id=%s, username=%s", user_id, username)

        return jsonify(
            {
                "success": True,
                "result": {
                    "token": tokens["access_token"],
                    "refreshToken": tokens["refresh_token"],
                    "newToken": tokens["access_token"],  # 兼容前端期望的字段名
                    "user": _build_user_profile_info(user),
                    "applicationCloudSettings": application_cloud_settings,
                },
            }
        )

    except Exception as error:
        logger.error("刷新令牌过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/tokens/api", methods=["POST"])
@log_method
@require_auth
def generate_api_token():
    """生成 API token（需要认证与当前密码确认）。"""
    return _generate_personal_token("api")


@bp.route("/tokens/mcp", methods=["POST"])
@log_method
@require_auth
def generate_mcp_token():
    """生成 MCP token（需要认证与当前密码确认）。"""
    return _generate_personal_token("mcp")


def _generate_personal_token(token_kind: str):
    """生成个人访问 token 的共享实现。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        expires_in_seconds = int(data.get("expiresInSeconds", 0) or 0)

        if not password:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Current password is required"}
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
            return jsonify({"success": False, "error": "User not found", "message": "User not found"}), 404

        if not _verify_user_password(user, password):
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user_id,
                        "username": username,
                        "event_type": f"{token_kind}_token_generate_failed",
                        "ip_address": get_client_ip(),
                        "user_agent": request.headers.get("User-Agent", ""),
                        "success": False,
                        "error_message": "Invalid password",
                    }
                )
            )
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        token_data = generate_access_token(
            user_id,
            username,
            config,
            expires_in_seconds=expires_in_seconds,
            token_kind=token_kind,
        )
        token_hash = calculate_token_hash(token_data["access_token"])
        session_id = loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user_id,
                    "token_hash": token_hash,
                    "refresh_token_hash": None,
                    "expires_at": token_data["expires_at"],
                    "refresh_expires_at": None,
                    "user_agent": _get_token_user_agent(token_kind),
                    "ip_address": get_client_ip(),
                }
            )
        )
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": f"{token_kind}_token_generate_success",
                    "ip_address": get_client_ip(),
                    "user_agent": _get_token_user_agent(token_kind),
                    "success": True,
                    "metadata": json.dumps({"session_id": session_id}),
                }
            )
        )
        loop.close()

        result = {
            "token": token_data["access_token"],
        }
        if token_kind == "api":
            result["apiBaseUrl"] = _build_api_base_url()
        else:
            result["mcpUrl"] = _build_mcp_url()

        return jsonify({"success": True, "result": result})
    except (TypeError, ValueError):
        if loop and not loop.is_closed():
            loop.close()
        return jsonify(
            {"success": False, "error": "Invalid request", "message": "expiresInSeconds must be a valid integer"}
        ), 400
    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("生成 %s token 失败: %s", token_kind, e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/tokens/<token_id>", methods=["DELETE"])
@log_method
@require_auth
def revoke_token(token_id: str):
    """撤销指定 token 会话。"""
    loop = None
    try:
        session_id = int(token_id)
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.invalidate_session_by_id(session_id, _get_request_user_id()))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Not Found", "message": "Token not found"}), 404

        return jsonify({"success": True, "result": True})
    except ValueError:
        if loop and not loop.is_closed():
            loop.close()
        return jsonify(
            {"success": False, "error": "Invalid request", "message": "tokenId must be a valid integer"}
        ), 400
    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("撤销 token 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/tokens", methods=["GET", "DELETE"])
@log_method
@require_auth
def list_tokens():
    """获取用户所有令牌或撤销其他令牌（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        session_id = _get_request_session_id()

        if request.method == "DELETE":
            revoked_count = loop.run_until_complete(db.invalidate_other_user_sessions(user_id, session_id))
            loop.close()
            return jsonify({"success": True, "result": True, "revokedCount": revoked_count})

        # 先清理过期会话
        loop.run_until_complete(db.cleanup_expired_sessions())

        # 获取所有活跃会话
        sessions = loop.run_until_complete(db.get_user_sessions(user_id))
        loop.close()

        # 转换为前端期望的格式
        tokens = []
        # seen_devices = set()  # 未使用

        for session in sessions:
            user_agent = session.get("user_agent", "")
            ip_address = session.get("ip_address", "")
            token_type = _infer_token_type(user_agent)
            is_current = session["id"] == session_id
            last_seen = _datetime_to_unix_millis(session.get("last_activity_at") or session.get("created_at", ""))

            # 简单设备标识：IP + UA 的前 50 个字符
            # device_key = f"{ip_address}_{user_agent[:50]}" # 未使用

            # 解析User-Agent提取设备信息
            device_name = parse_user_agent(user_agent)

            tokens.append(
                {
                    "tokenId": str(session["id"]),
                    "tokenType": token_type,
                    "userAgent": user_agent,
                    "deviceName": device_name,
                    "ipAddress": ip_address,
                    "createdAt": session.get("created_at", ""),
                    "expiresAt": session.get("expires_at", ""),
                    "lastActivityAt": session.get("last_activity_at", ""),
                    "lastSeen": last_seen,
                    "isCurrent": is_current,
                    "isCurrentToken": is_current,
                }
            )

        logger.info("返回会话列表: user_id=%s, count=%s", user_id, len(tokens))

        return jsonify({"success": True, "result": tokens})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取令牌列表失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


def parse_user_agent(user_agent: str) -> str:
    """解析User-Agent字符串,返回友好的设备名称"""
    if not user_agent:
        return "未知设备"

    ua_lower = user_agent.lower()

    # 检测操作系统
    if "windows" in ua_lower:
        os_name = "Windows"
        if "windows nt 10" in ua_lower:
            os_name = "Windows 10"
        elif "windows nt 11" in ua_lower:
            os_name = "Windows 11"
    elif "iphone" in ua_lower or "ipad" in ua_lower:
        os_name = "iOS"
    elif "mac os" in ua_lower or "macos" in ua_lower:
        os_name = "macOS"
    elif "android" in ua_lower:
        os_name = "Android"
    elif "linux" in ua_lower:
        os_name = "Linux"
    else:
        os_name = "其他系统"

    # 检测浏览器
    if "edg/" in ua_lower or "edge/" in ua_lower:
        browser = "Edge"
    elif "chrome/" in ua_lower and "edg/" not in ua_lower:
        browser = "Chrome"
    elif "firefox/" in ua_lower:
        browser = "Firefox"
    elif "safari/" in ua_lower and "chrome" not in ua_lower:
        browser = "Safari"
    else:
        browser = "其他浏览器"

    return f"{os_name} ({browser})"


__all__ = [name for name in globals() if not name.startswith("__")]
