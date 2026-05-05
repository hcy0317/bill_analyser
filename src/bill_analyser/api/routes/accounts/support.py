"""
账户 API 路由

重构后使用统一账户适配器处理账户数据格式转换。
"""

import asyncio
from typing import Any, cast

import bcrypt
from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.adapters.account_adapter import AccountAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("AccountsAPI")

bp = Blueprint("accounts", __name__)
account_adapter = AccountAdapter()


def get_app_context():
    """获取应用上下文中的服务实例"""
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _run_async(coroutine):
    """在独立事件循环中执行异步数据库调用。"""
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return int(getattr(request, "user_id", 0) or 0)


def _verify_sensitive_operation_password(db, user_id: int, password: str) -> bool:
    """优先验证当前用户密码，必要时回退到全局操作密码。"""
    user = _run_async(db.get_user_by_id(user_id)) if hasattr(db, "get_user_by_id") else None
    password_hash = (user or {}).get("password_hash", "")
    if password_hash and password:
        try:
            if bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8")):
                return True
        except ValueError:
            logger.warning("敏感操作密码校验失败：password_hash 格式无效, user_id=%s", user_id)

    return bool(_run_async(db.verify_operation_password(password))) if hasattr(db, "verify_operation_password") else False

__all__ = [name for name in globals() if not name.startswith("__")]
