from __future__ import annotations

import asyncio

from flask import Flask, request
import pytest

from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
    get_required_request_str,
    run_async_in_new_loop,
)


@pytest.fixture
def flask_app() -> Flask:
    """Create a minimal Flask app for request-context tests."""
    return Flask(__name__)



def test_run_async_in_new_loop_returns_result_and_closes_loop() -> None:
    """异步桥接应返回协程结果，并在结束后关闭临时事件循环。"""

    async def sample_coroutine() -> tuple[asyncio.AbstractEventLoop, str]:
        return asyncio.get_running_loop(), "done"

    try:
        created_loop, result = run_async_in_new_loop(sample_coroutine())
    finally:
        asyncio.set_event_loop(None)

    assert result == "done"
    assert created_loop.is_closed() is True



def test_get_required_request_int_accepts_positive_integer_strings(flask_app: Flask) -> None:
    """带认证上下文的整型字段应能被安全解析。"""
    with flask_app.test_request_context("/"):
        setattr(request, "user_id", " 12 ")
        assert get_required_request_int("user_id") == 12



def test_get_required_request_int_rejects_missing_invalid_and_non_positive_values(flask_app: Flask) -> None:
    """缺失、非法和非正整数都应被拒绝。"""
    with flask_app.test_request_context("/"):
        with pytest.raises(RuntimeError, match="missing user_id"):
            get_required_request_int("user_id")

        setattr(request, "user_id", "oops")
        with pytest.raises(RuntimeError, match="invalid user_id"):
            get_required_request_int("user_id")

        setattr(request, "user_id", "0")
        with pytest.raises(RuntimeError, match="invalid user_id"):
            get_required_request_int("user_id")

        setattr(request, "user_id", -1)
        with pytest.raises(RuntimeError, match="invalid user_id"):
            get_required_request_int("user_id")



def test_get_required_request_str_trims_and_requires_content(flask_app: Flask) -> None:
    """字符串字段应去除首尾空白，空值则抛出缺失异常。"""
    with flask_app.test_request_context("/"):
        setattr(request, "email", "  user@example.com  ")
        assert get_required_request_str("email") == "user@example.com"

        setattr(request, "email", "   ")
        with pytest.raises(RuntimeError, match="missing email"):
            get_required_request_str("email")
