"""认证请求上下文与异步桥接辅助函数。"""

import asyncio

from flask import request


def run_async_in_new_loop(coroutine):
    """在独立事件循环中执行异步调用。"""
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


def get_required_request_int(field_name: str) -> int:
    """获取认证中间件注入的必填整型请求字段。"""
    raw_value = getattr(request, field_name, None)
    if raw_value in (None, ""):
        raise RuntimeError(f"Authenticated request context missing {field_name}")

    try:
        value = int(raw_value)
    except (TypeError, ValueError) as exc:
        raise RuntimeError(f"Authenticated request context has invalid {field_name}") from exc

    if value <= 0:
        raise RuntimeError(f"Authenticated request context has invalid {field_name}")

    return value


def get_required_request_str(field_name: str) -> str:
    """获取认证中间件注入的必填字符串请求字段。"""
    value = str(getattr(request, field_name, "") or "").strip()
    if not value:
        raise RuntimeError(f"Authenticated request context missing {field_name}")
    return value
