from __future__ import annotations
# pyright: reportPrivateUsage=false

from pathlib import Path

import pytest

from bill_analyser.utils import config as config_module


class LoggerRecorder:
    """Collect logger calls without touching the real logging system."""

    def __init__(self) -> None:
        self.error_messages: list[str] = []

    def error(self, message: str, *args: object) -> None:
        self.error_messages.append(message % args if args else message)

    def info(self, _message: str, *args: object) -> None:
        _ = args

    def warning(self, _message: str, *args: object) -> None:
        _ = args

    def debug(self, _message: str, *args: object) -> None:
        _ = args


class FakeObserver:
    """Minimal observer stub for destructor and lifecycle tests."""

    def __init__(self, *, alive: bool = True, stop_error: Exception | None = None) -> None:
        self.alive = alive
        self.stop_error = stop_error
        self.stop_calls = 0
        self.join_timeouts: list[int | None] = []

    def stop(self) -> None:
        self.stop_calls += 1
        if self.stop_error is not None:
            raise self.stop_error

    def is_alive(self) -> bool:
        return self.alive

    def join(self, timeout: int | None = None) -> None:
        self.join_timeouts.append(timeout)


class ExplodingAliveObserver(FakeObserver):
    """Observer stub whose is_alive path fails."""

    def is_alive(self) -> bool:
        raise RuntimeError("is_alive failed")


@pytest.fixture
def isolated_config_manager(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> config_module.ConfigManager:
    """Provide the singleton config manager with isolated temp directories."""
    manager = config_module.ConfigManager()
    manager.disable_auto_reload()
    manager.clear_cache()

    config_dir = tmp_path / "config"
    legacy_dir = tmp_path / "legacy"
    config_dir.mkdir()
    legacy_dir.mkdir()

    monkeypatch.setattr(manager, "config_dir", config_dir)
    monkeypatch.setattr(manager, "legacy_config_dir", legacy_dir)
    monkeypatch.setattr(manager, "logger", LoggerRecorder())
    manager._observer = None
    manager._auto_reload_enabled = False

    with manager._cache_lock:
        manager._cache.clear()
        manager._cache_timestamps.clear()

    return manager



def test_config_manager_destructor_joins_only_when_alive_and_not_current_thread(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """析构器应在 observer 仍存活且不是当前线程时再 join。"""
    manager = isolated_config_manager
    observer = FakeObserver(alive=True)
    manager._observer = observer
    manager._auto_reload_enabled = True
    monkeypatch.setattr(config_module.threading, "current_thread", lambda: object())

    manager.__del__()

    assert observer.stop_calls == 1
    assert observer.join_timeouts == [1]
    assert manager._observer is None
    assert manager._auto_reload_enabled is False


def test_config_manager_destructor_handles_missing_observer_and_is_alive_failures(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """析构器在无 observer 时直接返回，is_alive 失败时退化为仍然 join。"""
    manager = isolated_config_manager

    manager._observer = None
    manager._auto_reload_enabled = True
    manager.__del__()
    assert manager._observer is None
    assert manager._auto_reload_enabled is True

    exploding_observer = ExplodingAliveObserver(alive=True)
    manager._observer = exploding_observer
    manager._auto_reload_enabled = True
    monkeypatch.setattr(config_module.threading, "current_thread", lambda: object())

    manager.__del__()

    assert exploding_observer.stop_calls == 1
    assert exploding_observer.join_timeouts == [1]
    assert manager._observer is None
    assert manager._auto_reload_enabled is False



def test_config_manager_destructor_skips_join_for_dead_or_current_thread_observer(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """析构器对已停止 observer 或“当前线程即 observer”应跳过 join。"""
    manager = isolated_config_manager

    dead_observer = FakeObserver(alive=False)
    manager._observer = dead_observer
    monkeypatch.setattr(config_module.threading, "current_thread", lambda: object())
    manager.__del__()
    assert dead_observer.stop_calls == 1
    assert dead_observer.join_timeouts == []

    current_thread_observer = FakeObserver(alive=True)
    manager._observer = current_thread_observer
    monkeypatch.setattr(config_module.threading, "current_thread", lambda: current_thread_observer)
    manager.__del__()
    assert current_thread_observer.stop_calls == 1
    assert current_thread_observer.join_timeouts == []



def test_config_manager_destructor_swallows_observer_stop_errors(
    isolated_config_manager: config_module.ConfigManager,
) -> None:
    """observer.stop 抛错时析构器也应静默清理状态。"""
    manager = isolated_config_manager
    manager._observer = FakeObserver(stop_error=RuntimeError("stop failed"))
    manager._auto_reload_enabled = True

    manager.__del__()

    assert manager._observer is None
    assert manager._auto_reload_enabled is False



def test_save_config_and_reload_config_cover_error_paths(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """配置管理器的保存与重载错误路径应返回 False/记录错误，而不是抛异常。"""
    manager = isolated_config_manager
    monkeypatch.setattr(config_module.json, "dump", lambda *_args, **_kwargs: (_ for _ in ()).throw(OSError("disk full")))
    assert manager.save_config("broken.json", {"value": 1}) is False

    with manager._cache_lock:
        manager._cache["demo.json"] = {"cached": True}
        manager._cache_timestamps["demo.json"] = 1

    monkeypatch.setattr(manager, "load_config", lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("reload failed")))
    manager._reload_config(str(manager.config_dir / "demo.json"))

    logger = manager.logger
    assert isinstance(logger, LoggerRecorder)
    assert any("重新加载配置文件失败" in message for message in logger.error_messages)
    assert manager._cache == {}
    assert manager._cache_timestamps == {}


def test_load_config_returns_empty_dict_on_unexpected_read_errors(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """读取阶段遇到非文件缺失/非 JSON 错误时应走通用兜底分支。"""
    manager = isolated_config_manager
    config_file = manager.config_dir / "denied.json"
    config_file.write_text('{"flag": true}', encoding="utf-8")

    monkeypatch.setattr(
        "builtins.open",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(PermissionError("denied")),
    )

    assert manager.load_config("denied.json", use_cache=False) == {}


@pytest.mark.asyncio
async def test_config_manager_async_helpers_delegate_via_event_loop_executor(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """异步 helper 应通过事件循环 executor 委托到同步实现。"""
    manager = isolated_config_manager
    calls: list[tuple[str, tuple[object, ...]]] = []

    class FakeLoop:
        async def run_in_executor(self, _executor: object, func, *args):
            calls.append((func.__name__, args))
            return func(*args)

    monkeypatch.setattr(config_module.asyncio, "get_event_loop", lambda: FakeLoop())
    monkeypatch.setattr(manager, "load_config", lambda filename, use_cache=True: {"filename": filename, "cached": use_cache})
    monkeypatch.setattr(manager, "save_config", lambda filename, config: filename == "demo.json" and config == {"value": 2})

    assert await manager.async_load_config("demo.json", use_cache=False) == {"filename": "demo.json", "cached": False}
    assert await manager.async_save_config("demo.json", {"value": 2}) is True
    assert calls == [
        ("<lambda>", ("demo.json", False)),
        ("<lambda>", ("demo.json", {"value": 2})),
    ]


def test_enable_and_disable_auto_reload_log_errors_on_observer_failures(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """自动重载启停失败时应记录错误，而不是抛异常。"""
    manager = isolated_config_manager

    class StartFailObserver(FakeObserver):
        def schedule(self, _handler: object, _path: str, recursive: bool = False) -> None:
            assert recursive is False

        def start(self) -> None:
            raise RuntimeError("start failed")

    monkeypatch.setattr(config_module, "Observer", StartFailObserver)
    manager.enable_auto_reload()

    logger = manager.logger
    assert isinstance(logger, LoggerRecorder)
    assert any("启用自动重载失败" in message for message in logger.error_messages)
    assert manager._auto_reload_enabled is False

    manager._observer = FakeObserver(stop_error=RuntimeError("stop failed"))
    manager._auto_reload_enabled = True
    manager.disable_auto_reload()

    assert any("禁用自动重载失败" in message for message in logger.error_messages)


def test_disable_auto_reload_without_observer_still_clears_enabled_flag(
    isolated_config_manager: config_module.ConfigManager,
) -> None:
    """当 auto_reload 标记开启但 observer 已缺失时，disable 仍应落回关闭状态。"""
    manager = isolated_config_manager
    manager._observer = None
    manager._auto_reload_enabled = True

    manager.disable_auto_reload()

    assert manager._auto_reload_enabled is False
