from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast

import pytest
from watchdog.events import FileModifiedEvent

from bill_analyser.utils import config as config_module


class LoggerRecorder:
    """Collect logger calls without touching the real logging system."""

    def __init__(self) -> None:
        self.debug_messages: list[str] = []
        self.info_messages: list[str] = []
        self.warning_messages: list[str] = []
        self.error_messages: list[str] = []

    def debug(self, message: str, *args: object) -> None:
        self.debug_messages.append(message % args if args else message)

    def info(self, message: str, *args: object) -> None:
        self.info_messages.append(message % args if args else message)

    def warning(self, message: str, *args: object) -> None:
        self.warning_messages.append(message % args if args else message)

    def error(self, message: str, *args: object) -> None:
        self.error_messages.append(message % args if args else message)


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



def test_config_helpers_merge_nested_values_and_normalize_lists() -> None:
    """纯辅助函数应深拷贝嵌套结构，并标准化列表与端口。"""
    merged = config_module._deep_merge_dicts(
        {
            "api": {"port": 5000, "cors": ["GET"]},
            "scopes": ["read"],
            "debug": False,
        },
        {
            "api": {"port": 8080},
            "scopes": ["write"],
            "debug": True,
        },
    )

    assert merged == {
        "api": {"port": 8080, "cors": ["GET"]},
        "scopes": ["write"],
        "debug": True,
    }
    assert config_module._normalize_string_list("a, b, , c", ["fallback"]) == ["a", "b", "c"]
    assert config_module._normalize_string_list([" a ", "", "b"], ["fallback"]) == ["a", "b"]
    assert config_module._normalize_string_list(None, ["fallback"]) == ["fallback"]
    assert config_module._coerce_port("8081", 5000) == 8081
    assert config_module._coerce_port("bad", 5000) == 5000
    assert config_module._coerce_port(0, 5000) == 5000



def test_load_auth_settings_validates_missing_secret_and_warns_for_insecure_secret(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证配置缺少 jwt_secret 时应失败，占位密钥应报警。"""
    logger = LoggerRecorder()
    monkeypatch.setattr(config_module._config_manager, "logger", logger)
    monkeypatch.setattr(config_module, "get_server_config", lambda use_cache=True: {})
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {})

    with pytest.raises(config_module.ConfigValidationError, match="JWT secret"):
        config_module.load_auth_settings()

    insecure_secret = next(iter(config_module.INSECURE_JWT_SECRET_VALUES))
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {"jwt_secret": insecure_secret, "password_min_length": 12},
    )
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {})

    auth_settings = config_module.load_auth_settings()

    assert auth_settings["jwt_secret"] == insecure_secret
    assert auth_settings["password_min_length"] == 12
    assert logger.warning_messages


def test_load_auth_settings_accepts_secure_secret_without_warning(monkeypatch: pytest.MonkeyPatch) -> None:
    """非占位 jwt_secret 应直接通过，不触发异常路径。"""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {"jwt_secret": "real-secret", "jwt_algorithm": "HS512"},
    )
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {})

    auth_settings = config_module.load_auth_settings()

    assert auth_settings["jwt_secret"] == "real-secret"
    assert auth_settings["jwt_algorithm"] == "HS512"



def test_load_api_runtime_settings_applies_overrides_and_normalizes_lists(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """运行时配置应深度合并 API 覆盖项，并标准化 CORS 字段。"""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {
            "api_host": "0.0.0.0",
            "api_port": "8080",
            "api": {
                "debug": 1,
                "threaded": 0,
                "cors": {
                    "origins": "https://a.example.com, https://b.example.com",
                    "methods": ["GET", "PATCH", ""],
                    "allow_headers": ["X-Test", "", "Authorization"],
                    "expose_headers": "X-Expose",
                    "supports_credentials": 0,
                    "max_age_seconds": "oops",
                    "send_wildcard": 1,
                    "always_send": 0,
                },
            },
        },
    )

    runtime_settings = config_module.load_api_runtime_settings()

    assert runtime_settings["host"] == "0.0.0.0"
    assert runtime_settings["port"] == 8080
    assert runtime_settings["debug"] is True
    assert runtime_settings["threaded"] is False
    assert runtime_settings["cors"]["origins"] == ["https://a.example.com", "https://b.example.com"]
    assert runtime_settings["cors"]["methods"] == ["GET", "PATCH"]
    assert runtime_settings["cors"]["allow_headers"] == ["X-Test", "Authorization"]
    assert runtime_settings["cors"]["expose_headers"] == ["X-Expose"]
    assert runtime_settings["cors"]["supports_credentials"] is False
    assert runtime_settings["cors"]["max_age_seconds"] == 3600
    assert runtime_settings["cors"]["send_wildcard"] is True
    assert runtime_settings["cors"]["always_send"] is False


def test_load_api_runtime_settings_prefers_env_host_and_port(monkeypatch: pytest.MonkeyPatch) -> None:
    """Rust primary startup can move the Python fallback to a different local port."""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {
            "api": {
                "host": "127.0.0.1",
                "port": 5000,
            }
        },
    )
    monkeypatch.setattr(
        config_module,
        "load_env_settings",
        lambda: {
            "BILL_ANALYSER_API_HOST": "127.0.0.1",
            "BILL_ANALYSER_API_PORT": "5001",
        },
    )

    runtime_settings = config_module.load_api_runtime_settings()

    assert runtime_settings["host"] == "127.0.0.1"
    assert runtime_settings["port"] == 5001



def test_load_default_user_settings_handles_disabled_missing_and_normalized_profiles(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """默认用户配置应支持关闭、必填校验和字段规范化。"""
    monkeypatch.setattr(config_module, "get_server_config", lambda use_cache=True: {})
    assert config_module.load_default_user_settings() is None

    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {"default_user": {"auto_create": False, "username": "ignored"}},
    )
    assert config_module.load_default_user_settings() is None

    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {"default_user": {"username": "admin", "password": "pwd"}},
    )
    with pytest.raises(config_module.ConfigValidationError, match="email"):
        config_module.load_default_user_settings()

    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {
            "default_user": {
                "username": " admin ",
                "password": "secret",
                "email": " admin@example.com ",
                "nickname": "   ",
            }
        },
    )

    default_user = config_module.load_default_user_settings()

    assert default_user == {
        "auto_create": True,
        "nickname": "",
        "language": "zh_Hans",
        "default_currency": "CNY",
        "first_day_of_week": 1,
        "username": "admin",
        "password": "secret",
        "email": "admin@example.com",
    }



def test_config_manager_save_load_reload_and_legacy_fallback(
    isolated_config_manager: config_module.ConfigManager,
) -> None:
    """配置管理器应支持保存、缓存、重载、旧目录兼容和错误回退。"""
    manager = isolated_config_manager

    assert manager.save_config("demo.json", {"flag": 1}) is True
    loaded_config = manager.load_config("demo.json")
    loaded_config["flag"] = 99
    assert manager.load_config("demo.json")["flag"] == 1

    demo_file = manager.config_dir / "demo.json"
    demo_file.write_text(json.dumps({"flag": 3}, ensure_ascii=False), encoding="utf-8")
    manager.reload_config(str(demo_file))
    assert manager.load_config("demo.json")["flag"] == 3

    (manager.config_dir / "broken.json").write_text("{bad json", encoding="utf-8")
    assert manager.load_config("broken.json", use_cache=False) == {}
    assert manager.load_config("missing.json", use_cache=False) == {}

    legacy_file = manager.legacy_config_dir / "legacy.json"
    legacy_file.write_text(json.dumps({"legacy": True}, ensure_ascii=False), encoding="utf-8")
    assert manager.load_config("legacy.json", use_cache=False) == {"legacy": True}
    logger = cast("LoggerRecorder", manager.logger)
    assert any("旧目录" in message for message in logger.warning_messages)


@pytest.mark.asyncio
async def test_config_manager_async_load_and_save(
    isolated_config_manager: config_module.ConfigManager,
) -> None:
    """异步保存/读取应复用同步逻辑并返回相同结果。"""
    manager = isolated_config_manager

    assert await manager.async_save_config("async.json", {"value": 7}) is True
    assert await manager.async_load_config("async.json") == {"value": 7}



def test_config_file_handler_reloads_only_modified_json_files(
    isolated_config_manager: config_module.ConfigManager,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """文件监听器只应对 JSON 文件修改触发重载。"""
    manager = isolated_config_manager
    reloaded_paths: list[str] = []
    monkeypatch.setattr(manager, "reload_config", lambda file_path: reloaded_paths.append(file_path))
    handler = config_module.ConfigFileHandler(manager)

    json_path = tmp_path / "watched.json"
    text_path = tmp_path / "ignored.txt"
    json_path.write_text("{}", encoding="utf-8")
    text_path.write_text("ignored", encoding="utf-8")

    handler.on_modified(FileModifiedEvent(str(json_path)))
    handler.on_modified(FileModifiedEvent(str(text_path)))

    directory_event = FileModifiedEvent(str(json_path))
    directory_event.is_directory = True
    handler.on_modified(directory_event)

    assert reloaded_paths == [str(json_path)]


def test_config_manager_singleton_stale_cache_refresh_and_partial_reload_cleanup(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """配置管理器应保持单例、在缓存过期时刷新，并清理不完整缓存条目。"""
    manager = isolated_config_manager

    assert config_module.ConfigManager() is manager

    assert manager.save_config("stale.json", {"value": 1}) is True
    assert manager.load_config("stale.json") == {"value": 1}

    stale_file = manager.config_dir / "stale.json"
    stale_file.write_text(json.dumps({"value": 2}, ensure_ascii=False), encoding="utf-8")
    with manager._cache_lock:
        manager._cache_timestamps["stale.json"] = 0
    assert manager.load_config("stale.json") == {"value": 2}

    with manager._cache_lock:
        manager._cache.pop("ghost.json", None)
        manager._cache_timestamps["ghost.json"] = 1

    monkeypatch.setattr(manager, "load_config", lambda *_args, **_kwargs: {})
    manager._reload_config(str(manager.config_dir / "ghost.json"))

    assert "ghost.json" not in manager._cache
    assert "ghost.json" not in manager._cache_timestamps

    with manager._cache_lock:
        manager._cache["cache-only.json"] = {"cached": True}
        manager._cache_timestamps.pop("cache-only.json", None)

    manager._reload_config(str(manager.config_dir / "cache-only.json"))

    assert "cache-only.json" not in manager._cache
    assert "cache-only.json" not in manager._cache_timestamps


def test_config_manager_new_returns_existing_instance_when_initialized_during_lock(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """双重检查锁的内层 false 分支应在竞争者先完成初始化时返回既有实例。"""

    class FakeLock:
        def __enter__(self):
            config_module.ConfigManager._instance = sentinel
            return self

        def __exit__(self, exc_type, exc, tb) -> bool:
            _ = (exc_type, exc, tb)
            return False

    sentinel = object()
    monkeypatch.setattr(config_module.ConfigManager, "_instance", None, raising=False)
    monkeypatch.setattr(config_module.ConfigManager, "_lock", FakeLock(), raising=False)

    assert config_module.ConfigManager() is sentinel



def test_config_manager_enable_disable_auto_reload_and_clear_cache(
    isolated_config_manager: config_module.ConfigManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """自动重载生命周期应正确启停，并允许清空缓存。"""
    manager = isolated_config_manager

    class FakeObserver:
        def __init__(self) -> None:
            self.started = False
            self.stopped = False
            self.join_timeouts: list[int | None] = []
            self.scheduled_paths: list[str] = []

        def schedule(self, _handler: object, path: str, recursive: bool = False) -> None:
            assert recursive is False
            self.scheduled_paths.append(path)

        def start(self) -> None:
            self.started = True

        def stop(self) -> None:
            self.stopped = True

        def join(self, timeout: int | None = None) -> None:
            self.join_timeouts.append(timeout)

    monkeypatch.setattr(config_module, "Observer", FakeObserver)

    manager.enable_auto_reload()
    assert manager._auto_reload_enabled is True
    assert manager._observer is not None
    assert manager._observer.started is True
    assert manager._observer.scheduled_paths == [str(manager.config_dir)]

    manager.enable_auto_reload()
    logger = cast("LoggerRecorder", manager.logger)
    assert any("自动重载已启用" in message for message in logger.warning_messages)

    with manager._cache_lock:
        manager._cache["demo.json"] = {"cached": True}
        manager._cache_timestamps["demo.json"] = 1
    manager.clear_cache()
    assert manager._cache == {}
    assert manager._cache_timestamps == {}

    observer = manager._observer
    manager.disable_auto_reload()
    assert observer is not None
    assert observer.stopped is True
    assert observer.join_timeouts == [5]
    assert manager._observer is None
    assert manager._auto_reload_enabled is False


@pytest.mark.asyncio
async def test_module_level_config_helpers_delegate_to_global_manager(monkeypatch: pytest.MonkeyPatch) -> None:
    """模块级 helper 应委托给全局配置管理器实例。"""

    class FakeManager:
        def __init__(self) -> None:
            self.calls: list[tuple[str, Any, Any]] = []

        def load_config(self, filename: str, use_cache: bool = True) -> dict[str, Any]:
            self.calls.append(("load", filename, use_cache))
            return {"filename": filename, "cached": use_cache}

        async def async_load_config(self, filename: str, use_cache: bool = True) -> dict[str, Any]:
            self.calls.append(("async_load", filename, use_cache))
            return {"filename": filename, "cached": use_cache, "async": True}

        def save_config(self, filename: str, config: dict[str, Any]) -> bool:
            self.calls.append(("save", filename, config))
            return True

        async def async_save_config(self, filename: str, config: dict[str, Any]) -> bool:
            self.calls.append(("async_save", filename, config))
            return True

        def enable_auto_reload(self) -> None:
            self.calls.append(("enable", None, None))

        def disable_auto_reload(self) -> None:
            self.calls.append(("disable", None, None))

    fake_manager = FakeManager()
    monkeypatch.setattr(config_module, "_config_manager", fake_manager)

    assert config_module.get_config("demo.json", use_cache=False) == {"filename": "demo.json", "cached": False}
    assert config_module.get_server_config(use_cache=False) == {
        "filename": config_module.SERVER_CONFIG_FILENAME,
        "cached": False,
    }
    assert await config_module.async_get_config("demo.json") == {
        "filename": "demo.json",
        "cached": True,
        "async": True,
    }
    assert config_module.save_config("demo.json", {"value": 1}) is True
    assert await config_module.async_save_config("demo.json", {"value": 2}) is True
    config_module.enable_auto_reload()
    config_module.disable_auto_reload()

    assert fake_manager.calls == [
        ("load", "demo.json", False),
        ("load", config_module.SERVER_CONFIG_FILENAME, False),
        ("async_load", "demo.json", True),
        ("save", "demo.json", {"value": 1}),
        ("async_save", "demo.json", {"value": 2}),
        ("enable", None, None),
        ("disable", None, None),
    ]
