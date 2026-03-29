"""
Config Module - 配置管理模块

统一加载和管理配置文件（JSON），支持缓存和热重载。
"""

import asyncio
import json
import threading
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from watchdog.events import FileModifiedEvent, FileSystemEventHandler
from watchdog.observers import Observer

from bill_analyser.constants import CONFIG_DIR, LEGACY_CONFIG_DIR

from .logger import get_logger, log_method


SERVER_CONFIG_FILENAME = "server_config.json"
INSECURE_JWT_SECRET_VALUES = frozenset(
    {
        "default_secret_key_change_in_production",
        "CHANGE_THIS_TO_A_RANDOM_SECRET_KEY_IN_PRODUCTION",
    }
)

DEFAULT_AUTH_CONFIG: dict[str, Any] = {
    "jwt_algorithm": "HS256",
    "jwt_expiration_days": 7,
    "refresh_token_expiration_days": 30,
    "password_min_length": 8,
    "enable_user_registration": True,
    "max_login_attempts": 5,
    "lockout_duration_minutes": 15,
}

DEFAULT_API_RUNTIME_CONFIG: dict[str, Any] = {
    "host": "127.0.0.1",
    "port": 5000,
    "debug": False,
    "threaded": True,
    "cors": {
        "origins": ["http://localhost:8081", "http://127.0.0.1:8081"],
        "methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
        "allow_headers": [
            "Content-Type",
            "Authorization",
            "X-Timezone-Offset",
            "X-Language",
            "Accept",
            "Accept-Language",
        ],
        "expose_headers": ["Content-Type", "Authorization"],
        "supports_credentials": True,
        "max_age_seconds": 3600,
        "send_wildcard": False,
        "always_send": True,
    },
}

DEFAULT_DEFAULT_USER_PROFILE: dict[str, Any] = {
    "auto_create": True,
    "nickname": "管理员",
    "language": "zh_Hans",
    "default_currency": "CNY",
    "first_day_of_week": 1,
}


class ConfigValidationError(RuntimeError):
    """配置合法性校验失败。"""


def _deep_merge_dicts(base: dict[str, Any], override: Mapping[str, Any] | None) -> dict[str, Any]:
    """深度合并配置字典，避免调用方自己散落地补默认值。"""
    result: dict[str, Any] = {}

    for key, value in base.items():
        if isinstance(value, dict):
            result[key] = _deep_merge_dicts(value, None)
        elif isinstance(value, list):
            result[key] = value.copy()
        else:
            result[key] = value

    if not override:
        return result

    for key, value in override.items():
        if isinstance(value, Mapping) and isinstance(result.get(key), dict):
            result[key] = _deep_merge_dicts(result[key], value)
        elif isinstance(value, list):
            result[key] = value.copy()
        else:
            result[key] = value

    return result


def _normalize_string_list(raw_value: Any, fallback: list[str]) -> list[str]:
    """将配置中的字符串列表标准化，支持逗号分隔字符串。"""
    if isinstance(raw_value, str):
        values = [item.strip() for item in raw_value.split(",") if item.strip()]
        return values or fallback.copy()

    if isinstance(raw_value, list):
        values = [str(item).strip() for item in raw_value if str(item).strip()]
        return values or fallback.copy()

    return fallback.copy()


def _coerce_port(raw_value: Any, fallback: int) -> int:
    """将端口配置解析为有效整数。"""
    try:
        port = int(raw_value)
    except (TypeError, ValueError):
        return fallback

    if port <= 0:
        return fallback

    return port


def get_server_config(use_cache: bool = True) -> dict[str, Any]:
    """加载服务端主配置文件。"""
    return get_config(SERVER_CONFIG_FILENAME, use_cache=use_cache)


def load_auth_settings(use_cache: bool = True) -> dict[str, Any]:
    """加载认证配置；不再回退到代码内置 JWT secret。"""
    config = _deep_merge_dicts(DEFAULT_AUTH_CONFIG, get_server_config(use_cache))
    jwt_secret = str(config.get("jwt_secret", "") or "").strip()

    if not jwt_secret:
        raise ConfigValidationError("server_config.json 缺少 jwt_secret，认证功能无法启动")

    if jwt_secret in INSECURE_JWT_SECRET_VALUES:
        _config_manager.logger.warning("检测到占位 jwt_secret，请在生产环境替换为真实密钥")

    config["jwt_secret"] = jwt_secret
    return config


def load_api_runtime_settings(use_cache: bool = True) -> dict[str, Any]:
    """加载 API 运行时配置（监听地址、端口、CORS 等）。"""
    server_config = get_server_config(use_cache)
    api_override = server_config.get("api") if isinstance(server_config.get("api"), dict) else {}
    runtime_config = _deep_merge_dicts(DEFAULT_API_RUNTIME_CONFIG, api_override)

    if "api_host" in server_config:
        runtime_config["host"] = str(server_config.get("api_host") or runtime_config["host"]).strip() or runtime_config[
            "host"
        ]

    runtime_config["port"] = _coerce_port(server_config.get("api_port", runtime_config.get("port")), runtime_config["port"])
    runtime_config["debug"] = bool(runtime_config.get("debug", False))
    runtime_config["threaded"] = bool(runtime_config.get("threaded", True))

    cors_defaults = DEFAULT_API_RUNTIME_CONFIG["cors"]
    cors_override = api_override.get("cors") if isinstance(api_override.get("cors"), dict) else {}
    cors_config = _deep_merge_dicts(cors_defaults, cors_override)
    cors_config["origins"] = _normalize_string_list(cors_config.get("origins"), cors_defaults["origins"])
    cors_config["methods"] = _normalize_string_list(cors_config.get("methods"), cors_defaults["methods"])
    cors_config["allow_headers"] = _normalize_string_list(
        cors_config.get("allow_headers"), cors_defaults["allow_headers"]
    )
    cors_config["expose_headers"] = _normalize_string_list(
        cors_config.get("expose_headers"), cors_defaults["expose_headers"]
    )
    cors_config["supports_credentials"] = bool(cors_config.get("supports_credentials", True))
    cors_config["max_age_seconds"] = _coerce_port(cors_config.get("max_age_seconds"), cors_defaults["max_age_seconds"])
    cors_config["send_wildcard"] = bool(cors_config.get("send_wildcard", False))
    cors_config["always_send"] = bool(cors_config.get("always_send", True))

    runtime_config["cors"] = cors_config
    return runtime_config


def load_default_user_settings(use_cache: bool = True) -> dict[str, Any] | None:
    """加载默认管理员引导配置；仅在显式配置存在时启用。"""
    server_config = get_server_config(use_cache)
    raw_default_user = server_config.get("default_user")

    if not isinstance(raw_default_user, dict):
        return None

    default_user = _deep_merge_dicts(DEFAULT_DEFAULT_USER_PROFILE, raw_default_user)
    default_user["auto_create"] = bool(default_user.get("auto_create", True))
    if not default_user["auto_create"]:
        return None

    required_fields = ("username", "password", "email")
    missing_fields = [field for field in required_fields if not str(default_user.get(field, "") or "").strip()]
    if missing_fields:
        raise ConfigValidationError(
            f"default_user 配置缺少必填字段: {', '.join(missing_fields)}"
        )

    default_user["username"] = str(default_user["username"]).strip()
    default_user["password"] = str(default_user["password"])
    default_user["email"] = str(default_user["email"]).strip()
    default_user["nickname"] = str(default_user.get("nickname", "") or default_user["username"]).strip()
    return default_user


class ConfigFileHandler(FileSystemEventHandler):
    """配置文件监听处理器"""

    def __init__(self, config_manager):
        """初始化"""
        self.config_manager = config_manager
        self.logger = get_logger("ConfigFileHandler")

    def on_modified(self, event):
        """文件修改事件处理"""
        if isinstance(event, FileModifiedEvent) and not event.is_directory:
            file_path = Path(event.src_path)
            if file_path.suffix == ".json":
                self.logger.info("检测到配置文件变化: %s", file_path.name)
                self.config_manager.reload_config(str(file_path))


class ConfigManager:
    """配置管理器"""

    _instance = None
    _lock = threading.Lock()

    def __new__(cls):
        """单例模式"""
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self):
        """初始化配置管理器"""
        if hasattr(self, "_initialized"):
            return

        self._initialized = True
        self.logger = get_logger("ConfigManager")
        # 配置目录固定为 data/config，兼容读取旧的顶层 config 目录。
        self.config_dir = CONFIG_DIR
        self.legacy_config_dir = LEGACY_CONFIG_DIR
        self.config_dir.mkdir(parents=True, exist_ok=True)

        # 配置缓存
        self._cache: dict[str, dict[str, Any]] = {}
        self._cache_timestamps: dict[str, float] = {}
        self._cache_lock = threading.RLock()

        # 文件监听器
        self._observer: Observer | None = None
        self._auto_reload_enabled = False

        self.logger.info("配置管理器初始化完成")

    @log_method
    def load_config(self, filename: str, use_cache: bool = True) -> dict[str, Any]:
        """
        加载配置文件

        Args:
            filename: 配置文件名（不含路径）
            use_cache: 是否使用缓存

        Returns:
            Dict[str, Any]: 配置字典
        """
        config_path = self.config_dir / filename
        legacy_config_path = self.legacy_config_dir / filename
        source_path = config_path if config_path.exists() or not legacy_config_path.exists() else legacy_config_path

        with self._cache_lock:
            # 检查缓存
            if use_cache and filename in self._cache:
                # 验证文件是否被修改
                current_mtime = source_path.stat().st_mtime
                cached_mtime = self._cache_timestamps.get(filename, 0)

                if current_mtime <= cached_mtime:
                    self.logger.debug("使用缓存的配置: %s", filename)
                    return self._cache[filename].copy()

            # 加载配置文件
            try:
                self.logger.info("加载配置文件: %s", filename)
                if source_path == legacy_config_path:
                    self.logger.warning("配置文件 %s 仍位于旧目录 %s，当前以兼容模式读取", filename, legacy_config_path)

                with open(source_path, encoding="utf-8") as f:
                    config = json.load(f)

                # 更新缓存
                self._cache[filename] = config
                self._cache_timestamps[filename] = source_path.stat().st_mtime

                self.logger.info("配置文件加载成功: %s", filename)
                return config.copy()

            except FileNotFoundError:
                self.logger.error("配置文件不存在: %s", filename)
                return {}
            except json.JSONDecodeError as e:
                self.logger.error("配置文件JSON格式错误 %s: %s", filename, e)
                return {}
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("加载配置文件失败 %s: %s", filename, e)
                return {}

    @log_method
    async def async_load_config(self, filename: str, use_cache: bool = True) -> dict[str, Any]:
        """
        异步加载配置文件

        Args:
            filename: 配置文件名
            use_cache: 是否使用缓存

        Returns:
            Dict[str, Any]: 配置字典
        """
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(None, self.load_config, filename, use_cache)

    @log_method
    def save_config(self, filename: str, config: dict[str, Any]) -> bool:
        """
        保存配置文件

        Args:
            filename: 配置文件名
            config: 配置字典

        Returns:
            bool: 是否保存成功
        """
        config_path = self.config_dir / filename

        try:
            self.logger.info("保存配置文件: %s", filename)

            # 创建临时文件
            temp_path = config_path.with_suffix(".tmp")
            with open(temp_path, "w", encoding="utf-8") as f:
                json.dump(config, f, ensure_ascii=False, indent=2)

            # 原子性替换
            temp_path.replace(config_path)

            # 更新缓存
            with self._cache_lock:
                self._cache[filename] = config.copy()
                self._cache_timestamps[filename] = config_path.stat().st_mtime

            self.logger.info("配置文件保存成功: %s", filename)
            return True

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("保存配置文件失败 %s: %s", filename, e)
            return False

    @log_method
    async def async_save_config(self, filename: str, config: dict[str, Any]) -> bool:
        """
        异步保存配置文件

        Args:
            filename: 配置文件名
            config: 配置字典

        Returns:
            bool: 是否保存成功
        """
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(None, self.save_config, filename, config)

    def reload_config(self, file_path: str):
        """对外暴露的配置重载入口，供文件监听器调用。"""
        self._reload_config(file_path)

    def _reload_config(self, file_path: str):
        """
        重新加载配置文件

        Args:
            file_path: 配置文件路径
        """
        try:
            filename = Path(file_path).name

            with self._cache_lock:
                # 清除缓存
                if filename in self._cache:
                    del self._cache[filename]
                if filename in self._cache_timestamps:
                    del self._cache_timestamps[filename]

            # 重新加载
            self.load_config(filename, use_cache=False)
            self.logger.info("配置文件已重新加载: %s", filename)

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("重新加载配置文件失败 %s: %s", file_path, e)

    @log_method
    def enable_auto_reload(self):
        """启用自动热重载"""
        if self._auto_reload_enabled:
            self.logger.warning("自动重载已启用")
            return

        try:
            self.logger.info("启用配置文件自动重载")

            # 创建文件监听器
            event_handler = ConfigFileHandler(self)
            self._observer = Observer()
            self._observer.schedule(event_handler, str(self.config_dir), recursive=False)
            self._observer.start()

            self._auto_reload_enabled = True
            self.logger.info("配置文件自动重载已启用")

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("启用自动重载失败: %s", e)

    @log_method
    def disable_auto_reload(self):
        """禁用自动热重载"""
        if not self._auto_reload_enabled:
            return

        try:
            self.logger.info("禁用配置文件自动重载")

            if self._observer:
                self._observer.stop()
                self._observer.join(timeout=5)
                self._observer = None

            self._auto_reload_enabled = False
            self.logger.info("配置文件自动重载已禁用")

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("禁用自动重载失败: %s", e)

    @log_method
    def clear_cache(self):
        """清除所有缓存"""
        with self._cache_lock:
            count = len(self._cache)
            self._cache.clear()
            self._cache_timestamps.clear()
            self.logger.info("已清除 %s 个配置文件缓存", count)

    def __del__(self):
        """析构函数"""
        observer = getattr(self, "_observer", None)
        if observer is None:
            return

        try:
            observer.stop()

            should_join = observer is not threading.current_thread()
            if should_join:
                try:
                    should_join = bool(observer.is_alive())
                except Exception:  # pylint: disable=broad-except
                    should_join = True

            if should_join:
                observer.join(timeout=1)
        except Exception:  # pylint: disable=broad-except
            pass

        self._observer = None
        self._auto_reload_enabled = False


# 全局配置管理器实例
_config_manager = ConfigManager()


def get_config(filename: str, use_cache: bool = True) -> dict[str, Any]:
    """
    获取配置

    Args:
        filename: 配置文件名
        use_cache: 是否使用缓存

    Returns:
        Dict[str, Any]: 配置字典
    """
    return _config_manager.load_config(filename, use_cache)


async def async_get_config(filename: str, use_cache: bool = True) -> dict[str, Any]:
    """
    异步获取配置

    Args:
        filename: 配置文件名
        use_cache: 是否使用缓存

    Returns:
        Dict[str, Any]: 配置字典
    """
    return await _config_manager.async_load_config(filename, use_cache)


def save_config(filename: str, config: dict[str, Any]) -> bool:
    """
    保存配置

    Args:
        filename: 配置文件名
        config: 配置字典

    Returns:
        bool: 是否保存成功
    """
    return _config_manager.save_config(filename, config)


async def async_save_config(filename: str, config: dict[str, Any]) -> bool:
    """
    异步保存配置

    Args:
        filename: 配置文件名
        config: 配置字典

    Returns:
        bool: 是否保存成功
    """
    return await _config_manager.async_save_config(filename, config)


def enable_auto_reload():
    """启用配置自动重载"""
    _config_manager.enable_auto_reload()


def disable_auto_reload():
    """禁用配置自动重载"""
    _config_manager.disable_auto_reload()
