"""
认证路由

提供用户登录、注册、登出等认证功能。
"""

# pylint: disable=too-many-lines,broad-exception-caught,line-too-long,import-outside-toplevel,duplicate-code
# pylint: disable=too-many-return-statements,too-many-branches,too-many-locals,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments

import asyncio
import base64
import csv
import hashlib
import io
import json
import mimetypes
import secrets
from datetime import datetime, timedelta
from typing import Any, cast

import bcrypt
import jwt
import pyotp
import qrcode
from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser import __version__
from bill_analyser.api.config.auth import (
    AUTH_DEFAULT_ACCOUNT_TEMPLATES,
    CLOUD_SETTING_TYPE_BOOLEAN,
    CLOUD_SETTING_TYPE_NUMBER,
    CLOUD_SETTING_TYPE_STRING,
    CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    SUPPORTED_APPLICATION_CLOUD_SETTING_KEY_TYPES,
    TOKEN_TYPE_API,
    TOKEN_TYPE_DEFAULT,
    TOKEN_TYPE_MCP,
)
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
    get_required_request_str,
)
from bill_analyser.core.investment.settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from bill_analyser.core.default_category_seed import ensure_default_category_seed
from bill_analyser.utils.config import load_auth_settings as load_server_auth_settings
from bill_analyser.utils.constants import FRONTEND_TO_BACKEND_TYPE
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("AuthAPI")

bp = Blueprint("auth", __name__)

TWO_FACTOR_RECOVERY_CODES: dict[int, list[str]] = {}


def load_auth_config():
    """加载认证配置"""
    return load_server_auth_settings()


def get_app_context():
    """获取应用上下文"""
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    if db is None:
        # 回退方案：尝试从模块导入
        import bill_analyser.api.app as app_module

        db = app_module.db
    if db is None:
        raise RuntimeError("Database not initialized. Please ensure server is properly started.")
    return db


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return get_required_request_int("user_id")


def _get_request_username() -> str:
    """获取认证中间件注入的当前用户名。"""
    return get_required_request_str("username")


def _get_request_session_id() -> int:
    """获取认证中间件注入的当前会话 ID。"""
    return get_required_request_int("session_id")


def get_client_ip():
    """获取客户端IP地址"""
    forwarded_for = request.headers.get("X-Forwarded-For")
    if forwarded_for:
        return forwarded_for.split(",")[0].strip()

    real_ip = request.headers.get("X-Real-IP")
    if real_ip:
        return real_ip

    return request.remote_addr or ""


def calculate_token_hash(token: str) -> str:
    """计算令牌哈希值"""
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def generate_access_token(
    user_id: int,
    username: str,
    config: dict,
    expires_in_seconds: int = 0,
    token_kind: str = "session",
) -> dict:
    """生成单个访问令牌，用于 API/MCP token 与普通会话。"""
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")

    now = datetime.now()
    if expires_in_seconds and expires_in_seconds > 0:
        expires_at = now + timedelta(seconds=expires_in_seconds)
    else:
        # 前端的“不过期”语义使用超长有效期实现。
        expires_at = now + timedelta(days=365 * 100)

    payload = {
        "user_id": user_id,
        "username": username,
        "type": "access",
        "token_kind": token_kind,
        "iat": int(now.timestamp()),
        "exp": int(expires_at.timestamp()),
        "nonce": secrets.token_hex(16),
    }
    access_token = jwt.encode(payload, jwt_secret, algorithm=jwt_algorithm)

    return {
        "access_token": access_token,
        "expires_at": expires_at.isoformat(),
    }


def _verify_user_password(user: dict, password: str) -> bool:
    """校验当前用户密码。"""
    password_hash = (user or {}).get("password_hash", "")
    if not password_hash or not password:
        return False
    return bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8"))


def _verify_sensitive_operation_password(db, user: dict | None, password: str, loop) -> bool:
    """优先校验当前用户密码，必要时回退到操作密码。"""
    if user:
        try:
            if _verify_user_password(user, password):
                return True
        except ValueError as exc:
            logger.warning(
                "敏感操作密码校验遇到无效哈希，回退到操作密码验证: user_id=%s, error=%s",
                user.get("id"),
                exc,
            )

    verify_operation_password = getattr(db, "verify_operation_password", None)
    if callable(verify_operation_password):
        return bool(loop.run_until_complete(verify_operation_password(password)))

    return False


def generate_jwt_token(user_id: int, username: str, config: dict) -> dict:
    """
    生成JWT令牌和刷新令牌

    返回：
        dict: {
            'access_token': str,
            'refresh_token': str,
            'expires_at': str,
            'refresh_expires_at': str
        }
    """
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")
    access_exp_days = config.get("jwt_expiration_days", 7)
    refresh_exp_days = config.get("refresh_token_expiration_days", 30)

    now = datetime.now()
    access_expires_at = now + timedelta(days=access_exp_days)
    refresh_expires_at = now + timedelta(days=refresh_exp_days)

    # 生成访问令牌（添加nonce确保唯一性）
    access_payload = {
        "user_id": user_id,
        "username": username,
        "type": "access",
        "iat": int(now.timestamp()),
        "exp": int(access_expires_at.timestamp()),
        "nonce": secrets.token_hex(16),  # 添加32字符随机值
    }
    access_token = jwt.encode(access_payload, jwt_secret, algorithm=jwt_algorithm)

    # 生成刷新令牌（添加nonce确保唯一性）
    refresh_payload = {
        "user_id": user_id,
        "username": username,
        "type": "refresh",
        "iat": int(now.timestamp()),
        "exp": int(refresh_expires_at.timestamp()),
        "nonce": secrets.token_hex(16),  # 添加32字符随机值
    }
    refresh_token_str = jwt.encode(refresh_payload, jwt_secret, algorithm=jwt_algorithm)

    return {
        "access_token": access_token,
        "refresh_token": refresh_token_str,
        "expires_at": access_expires_at.isoformat(),
        "refresh_expires_at": refresh_expires_at.isoformat(),
    }


def generate_action_token(
    user_id: int,
    username: str,
    email: str,
    config: dict,
    token_type: str,
    expires_in_hours: int = 24,
) -> str:
    """生成用于邮箱验证/重置密码的一次性动作令牌。"""
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")
    now = datetime.now()
    expires_at = now + timedelta(hours=expires_in_hours)

    payload = {
        "user_id": user_id,
        "username": username,
        "email": email,
        "type": token_type,
        "iat": int(now.timestamp()),
        "exp": int(expires_at.timestamp()),
        "nonce": secrets.token_hex(16),
    }
    return jwt.encode(payload, jwt_secret, algorithm=jwt_algorithm)


def decode_action_token(token: str, config: dict, expected_type: str) -> dict | None:
    """解码并校验动作令牌。"""
    if not token:
        return None

    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")

    try:
        payload = jwt.decode(token, jwt_secret, algorithms=[jwt_algorithm])
    except jwt.ExpiredSignatureError:
        logger.warning("动作令牌已过期: expected_type=%s", expected_type)
        return None
    except jwt.InvalidTokenError:
        logger.warning("动作令牌无效: expected_type=%s", expected_type)
        return None

    if payload.get("type") != expected_type:
        logger.warning("动作令牌类型不匹配: expected=%s actual=%s", expected_type, payload.get("type"))
        return None

    return payload


def _verify_step_up_token(config: dict, step_up_token: str, expected_user_id: int) -> bool:
    """校验敏感操作使用的 step-up token。"""
    payload = decode_action_token(step_up_token, config, "step_up")
    if not payload:
        return False

    token_user_id = payload.get("user_id")
    return isinstance(token_user_id, int) and token_user_id == expected_user_id


def _resolve_sensitive_operation_auth(
    *,
    db,
    user: dict | None,
    config: dict,
    payload: dict,
    loop,
) -> tuple[bool, str]:
    """解析敏感操作鉴权：支持 password 或 stepUpToken。"""
    password = str(payload.get("password", "") or "")
    step_up_token = str(payload.get("stepUpToken", "") or "").strip()

    if step_up_token:
        user_id = int((user or {}).get("id") or 0)
        if _verify_step_up_token(config, step_up_token, user_id):
            return True, "step_up"
        return False, "invalid_step_up"

    if not password:
        return False, "missing_credentials"

    if _verify_sensitive_operation_password(db, user, password, loop):
        return True, "password"

    return False, "invalid_password"


def _extract_bearer_token() -> str:
    """从请求头提取 Bearer token。"""
    auth_header = request.headers.get("Authorization", "")
    parts = auth_header.split()

    if len(parts) != 2 or parts[0].lower() != "bearer":
        return ""

    return parts[1]


def _datetime_to_unix_millis(value: str) -> int:
    """将 ISO 时间字符串转换为 Unix 毫秒。"""
    if not value:
        return 0

    try:
        return int(datetime.fromisoformat(value).timestamp() * 1000)
    except (TypeError, ValueError):
        return 0


__all__ = [name for name in globals() if not (name.startswith('__') and name != '__version__')]
