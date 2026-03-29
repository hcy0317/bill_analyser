from __future__ import annotations

import io
import logging
import os
import sys
from datetime import datetime, timedelta
from logging.handlers import QueueHandler
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.utils import logger as logger_module


class FakeLogger:
    """Simple logger spy for decorator tests."""

    def __init__(self) -> None:
        self.debug_messages: list[str] = []
        self.info_messages: list[str] = []
        self.error_messages: list[str] = []

    def debug(self, message: str, *args: object) -> None:
        self.debug_messages.append(message % args if args else message)

    def info(self, message: str, *args: object) -> None:
        self.info_messages.append(message % args if args else message)

    def error(self, message: str, *args: object) -> None:
        self.error_messages.append(message % args if args else message)


class FakeHandler(logging.Handler):
    """In-memory handler used to avoid filesystem side effects."""

    def __init__(self) -> None:
        super().__init__()
        self.flushed = False
        self.closed_called = False

    def emit(self, record: logging.LogRecord) -> None:  # pragma: no cover - no runtime behavior needed
        _ = record

    def flush(self) -> None:
        self.flushed = True

    def close(self) -> None:
        self.closed_called = True
        super().close()


class FakeQueueListener:
    """Minimal queue listener spy."""

    def __init__(self, _queue: object, *handlers: logging.Handler, respect_handler_level: bool = False) -> None:
        self.handlers = list(handlers)
        self.started = False
        self.stopped = False
        self.respect_handler_level = respect_handler_level

    def start(self) -> None:
        self.started = True

    def stop(self) -> None:
        self.stopped = True



def _make_log_record(message: str = "hello") -> logging.LogRecord:
    return logging.LogRecord("bill_analyser.tests", logging.INFO, __file__, 1, message, (), None)



def test_safe_stream_handler_ignores_closed_streams_and_emit_value_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """安全流处理器应忽略关闭流和常见 I/O 异常。"""
    closed_stream = io.StringIO()
    closed_stream.close()
    closed_handler = logger_module.SafeStreamHandler(closed_stream)
    closed_handler.emit(_make_log_record("closed"))

    def raise_value_error(self: logging.StreamHandler, record: logging.LogRecord) -> None:
        _ = (self, record)
        raise ValueError("stream closed")

    monkeypatch.setattr(logging.StreamHandler, "emit", raise_value_error)
    handler = logger_module.SafeStreamHandler(io.StringIO())
    handler.emit(_make_log_record("ignored"))



def test_safe_stream_handler_handle_error_delegates_only_non_io_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """handleError 应吞掉常见 I/O 错误，其余异常交给父类。"""
    handler = logger_module.SafeStreamHandler(io.StringIO())
    delegated = {"count": 0}

    def fake_handle_error(self: logging.StreamHandler, record: logging.LogRecord) -> None:
        _ = (self, record)
        delegated["count"] += 1

    monkeypatch.setattr(logging.StreamHandler, "handleError", fake_handle_error)

    try:
        raise ValueError("closed")
    except ValueError:
        handler.handleError(_make_log_record())
    assert delegated["count"] == 0

    try:
        raise RuntimeError("boom")
    except RuntimeError:
        handler.handleError(_make_log_record())
    assert delegated["count"] == 1



def test_daily_file_handler_rolls_over_and_swallows_emit_errors(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """按日滚动文件处理器应生成当天文件，并吞掉权限错误。"""
    handler = logger_module.DailyFileHandler(tmp_path, prefix="unit")
    today_name = f"unit_{datetime.now().strftime('%Y%m%d')}.log"

    handler.emit(_make_log_record("first"))
    handler.flush()
    assert "first" in (tmp_path / today_name).read_text(encoding="utf-8")

    handler._current_date = "19000101"
    handler.emit(_make_log_record("rolled"))
    handler.flush()
    assert handler.baseFilename.endswith(today_name)
    assert "rolled" in (tmp_path / today_name).read_text(encoding="utf-8")

    def raise_permission_error(self: logging.FileHandler, record: logging.LogRecord) -> None:
        _ = (self, record)
        raise PermissionError("locked")

    monkeypatch.setattr(logging.FileHandler, "emit", raise_permission_error)
    handler.emit(_make_log_record("ignored"))
    handler.close()



def test_daily_file_handler_handle_error_delegates_only_non_io_errors(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """文件处理器的 handleError 也应只放行非 I/O 异常。"""
    handler = logger_module.DailyFileHandler(tmp_path, prefix="unit")
    delegated = {"count": 0}

    def fake_handle_error(self: logging.FileHandler, record: logging.LogRecord) -> None:
        _ = (self, record)
        delegated["count"] += 1

    monkeypatch.setattr(logging.FileHandler, "handleError", fake_handle_error)

    try:
        raise PermissionError("locked")
    except PermissionError:
        handler.handleError(_make_log_record())
    assert delegated["count"] == 0

    try:
        raise RuntimeError("boom")
    except RuntimeError:
        handler.handleError(_make_log_record())
    assert delegated["count"] == 1
    handler.close()



def test_async_logger_under_pytest_uses_null_handler_and_cleans_old_logs(tmp_path: Path) -> None:
    """pytest 环境下应避免启动异步队列线程，但仍能清理旧日志。"""
    async_logger = logger_module.AsyncLogger()
    async_logger.log_dir = tmp_path
    async_logger.max_log_age_days = 7

    old_log = tmp_path / "old.log"
    new_log = tmp_path / "new.log"
    old_log.write_text("old", encoding="utf-8")
    new_log.write_text("new", encoding="utf-8")
    old_timestamp = (datetime.now() - timedelta(days=10)).timestamp()
    os.utime(old_log, (old_timestamp, old_timestamp))

    async_logger._cleanup_old_logs()

    assert async_logger.queue_listener is None
    assert any(isinstance(handler, logging.NullHandler) for handler in async_logger.logger.handlers)
    assert async_logger.get_logger("demo").name == "bill_analyser.demo"
    assert old_log.exists() is False
    assert new_log.exists() is True



def test_async_logger_full_setup_and_stop_when_pytest_module_is_absent(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """非 pytest 分支应配置队列监听器、处理器并能安全停止。"""
    monkeypatch.delitem(sys.modules, "pytest", raising=False)
    monkeypatch.setattr(logger_module, "LOG_DIR", tmp_path)
    monkeypatch.setattr(logger_module, "DailyFileHandler", lambda *args, **kwargs: FakeHandler())
    monkeypatch.setattr(logger_module, "QueueListener", FakeQueueListener)
    monkeypatch.setattr(
        logger_module.AsyncLogger,
        "_start_cleanup_task",
        lambda self: setattr(self, "cleanup_started", True),
    )

    async_logger = logger_module.AsyncLogger()

    assert isinstance(async_logger.queue_listener, FakeQueueListener)
    assert async_logger.queue_listener.started is True
    assert getattr(async_logger, "cleanup_started", False) is True
    assert any(isinstance(handler, QueueHandler) for handler in async_logger.logger.handlers)
    assert async_logger.get_logger("demo").name == "bill_analyser.demo"

    async_logger.stop()

    assert async_logger.queue_listener.stopped is True
    assert async_logger.logger.handlers == []
    closed_handler_count = sum(
        1 for handler in async_logger.queue_listener.handlers if getattr(handler, "closed_called", False)
    )
    assert closed_handler_count == 2



def test_shutdown_async_logger_ignores_stop_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """解释器退出钩子不应因为 stop 报错而再次抛异常。"""

    class BrokenLogger:
        def stop(self) -> None:
            raise RuntimeError("boom")

    monkeypatch.setattr(logger_module, "_logger_instance", BrokenLogger())
    logger_module._shutdown_async_logger()


@pytest.mark.asyncio
async def test_log_method_wraps_sync_and_async_functions_and_logs_exceptions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """log_method 应覆盖同步、异步和异常分支。"""
    fake_logger = FakeLogger()
    monkeypatch.setattr(logger_module, "get_logger", lambda name=None: fake_logger)
    monkeypatch.setattr(logger_module, "LOG_METHOD_VERBOSE", True)

    class DemoService:
        @logger_module.log_method
        def double(self, value: int) -> int:
            return value * 2

        @logger_module.log_method
        async def triple(self, value: int) -> int:
            return value * 3

        @logger_module.log_method
        def explode(self) -> None:
            raise ValueError("boom")

    service = DemoService()

    assert service.double(4) == 8
    assert await service.triple(4) == 12
    with pytest.raises(ValueError, match="boom"):
        service.explode()

    assert any("进入方法" in message for message in fake_logger.debug_messages)
    assert any("退出方法" in message for message in fake_logger.debug_messages)
    assert any("ValueError" in message for message in fake_logger.error_messages)


@pytest.mark.asyncio
async def test_log_step_wraps_sync_and_async_functions_and_logs_failures(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """log_step 应记录步骤开始/结束，并覆盖失败路径。"""
    fake_logger = FakeLogger()
    monkeypatch.setattr(logger_module, "get_logger", lambda name=None: fake_logger)

    class DemoWorkflow:
        @logger_module.log_step("同步步骤")
        def sync_step(self) -> str:
            return "sync-ok"

        @logger_module.log_step("异步步骤")
        async def async_step(self) -> str:
            return "async-ok"

        @logger_module.log_step("失败步骤")
        def failed_step(self) -> None:
            raise RuntimeError("bad")

    workflow = DemoWorkflow()

    assert workflow.sync_step() == "sync-ok"
    assert await workflow.async_step() == "async-ok"
    with pytest.raises(RuntimeError, match="bad"):
        workflow.failed_step()

    assert any("开始步骤" in message for message in fake_logger.info_messages)
    assert any("完成步骤" in message for message in fake_logger.info_messages)
    assert any("步骤失败" in message for message in fake_logger.error_messages)
