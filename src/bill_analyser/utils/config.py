"""
Config Module - 配置管理模块

统一加载和管理配置文件（JSON），支持缓存和热重载。
"""

import asyncio
import json
import threading
from pathlib import Path
from typing import Any

from watchdog.events import FileModifiedEvent, FileSystemEventHandler
from watchdog.observers import Observer

from bill_analyser.constants import PROJECT_ROOT

from .logger import get_logger, log_method


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
        # 配置目录指向项目根目录的 config 文件夹
        self.config_dir = PROJECT_ROOT / "config"
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

        with self._cache_lock:
            # 检查缓存
            if use_cache and filename in self._cache:
                # 验证文件是否被修改
                current_mtime = config_path.stat().st_mtime
                cached_mtime = self._cache_timestamps.get(filename, 0)

                if current_mtime <= cached_mtime:
                    self.logger.debug("使用缓存的配置: %s", filename)
                    return self._cache[filename].copy()

            # 加载配置文件
            try:
                self.logger.info("加载配置文件: %s", filename)
                with open(config_path, encoding="utf-8") as f:
                    config = json.load(f)

                # 更新缓存
                self._cache[filename] = config
                self._cache_timestamps[filename] = config_path.stat().st_mtime

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
        self.disable_auto_reload()


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
