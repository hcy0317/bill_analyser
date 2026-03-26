"""
高级异步日志系统

提供完整的日志功能:
1. 自动滚动清理 (按大小和时间)
2. 方法入口/出口装饰器
3. 参数和返回值记录
4. 关键步骤状态记录
5. 统一格式: [时间戳] [线程] [级别] [类名.方法名] 内容
6. 异步写入 (不阻塞主线程)

作者: AI Assistant
日期: 2025-11-16
"""

import sys
import logging
import asyncio
import threading
import functools
import traceback
from pathlib import Path
from datetime import datetime, timedelta
from typing import Any, Callable, Optional
from logging.handlers import RotatingFileHandler
from concurrent.futures import ThreadPoolExecutor


class AsyncLogHandler(logging.Handler):
    """异步日志处理器,使用线程池避免阻塞主线程"""

    def __init__(self, base_handler: logging.Handler, max_workers: int = 2):
        """
        初始化异步日志处理器

        Args:
            base_handler: 基础处理器 (如FileHandler)
            max_workers: 最大工作线程数
        """
        super().__init__()
        self.base_handler = base_handler
        self.executor = ThreadPoolExecutor(max_workers=max_workers)
        self.setFormatter(base_handler.formatter)
        self.setLevel(base_handler.level)

    def emit(self, record: logging.LogRecord) -> None:
        """异步发送日志记录"""
        try:
            self.executor.submit(self.base_handler.emit, record)
        except Exception:  # pylint: disable=broad-except
            self.handleError(record)

    def close(self) -> None:
        """关闭处理器"""
        self.executor.shutdown(wait=True)
        self.base_handler.close()
        super().close()


class AdvancedLogger:
    """高级日志记录器"""

    # 默认配置
    DEFAULT_LOG_DIR = "logs"
    DEFAULT_LOG_FILE = "bill_analyser.log"
    DEFAULT_MAX_BYTES = 10 * 1024 * 1024  # 10MB
    DEFAULT_BACKUP_COUNT = 10
    DEFAULT_RETENTION_DAYS = 30
    DEFAULT_FORMAT = (
        "[%(asctime)s] [%(threadName)-10s] [%(levelname)-8s] "
        "[%(name)s.%(funcName)s] %(message)s"
    )
    DEFAULT_DATE_FORMAT = "%Y-%m-%d %H:%M:%S"

    def __init__(
        self,
        name: str,
        log_dir: Optional[str] = None,
        log_file: Optional[str] = None,
        level: int = logging.INFO,
        max_bytes: int = DEFAULT_MAX_BYTES,
        backup_count: int = DEFAULT_BACKUP_COUNT,
        retention_days: int = DEFAULT_RETENTION_DAYS,
        enable_async: bool = True,
        enable_console: bool = True
    ):
        """
        初始化高级日志记录器

        Args:
            name: 日志记录器名称
            log_dir: 日志目录
            log_file: 日志文件名
            level: 日志级别
            max_bytes: 单个日志文件最大字节数
            backup_count: 保留的备份文件数量
            retention_days: 日志保留天数
            enable_async: 是否启用异步写入
            enable_console: 是否输出到控制台
        """
        self.name = name
        self.log_dir = Path(log_dir or self.DEFAULT_LOG_DIR)
        self.log_file = log_file or self.DEFAULT_LOG_FILE
        self.level = level
        self.max_bytes = max_bytes
        self.backup_count = backup_count
        self.retention_days = retention_days
        self.enable_async = enable_async
        self.enable_console = enable_console

        # 创建日志目录
        self.log_dir.mkdir(parents=True, exist_ok=True)

        # 创建logger
        self.logger = logging.getLogger(name)
        self.logger.setLevel(level)
        self.logger.propagate = False  # 防止日志传播到父logger

        # 清除已有的handler
        self.logger.handlers.clear()

        # 创建formatter
        self.formatter = logging.Formatter(
            self.DEFAULT_FORMAT,
            datefmt=self.DEFAULT_DATE_FORMAT
        )

        # 添加handlers
        self._setup_file_handler()
        if enable_console:
            self._setup_console_handler()

        # 启动日志清理任务
        self._start_cleanup_task()

    def _setup_file_handler(self) -> None:
        """设置文件处理器"""
        log_path = self.log_dir / self.log_file

        # 使用RotatingFileHandler实现按大小滚动
        file_handler = RotatingFileHandler(
            filename=str(log_path),
            maxBytes=self.max_bytes,
            backupCount=self.backup_count,
            encoding="utf-8"
        )
        file_handler.setLevel(self.level)
        file_handler.setFormatter(self.formatter)

        # 如果启用异步,包装为异步处理器
        if self.enable_async:
            file_handler = AsyncLogHandler(file_handler)

        self.logger.addHandler(file_handler)

    def _setup_console_handler(self) -> None:
        """设置控制台处理器"""
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(self.level)
        console_handler.setFormatter(self.formatter)

        # 控制台handler通常不需要异步
        self.logger.addHandler(console_handler)

    def _start_cleanup_task(self) -> None:
        """启动日志清理任务"""
        def cleanup_worker():
            """清理工作线程"""
            while True:
                try:
                    self._cleanup_old_logs()
                except Exception as e:  # pylint: disable=broad-except
                    self.logger.error(f"清理日志失败: {e}")

                # 每天清理一次
                threading.Event().wait(86400)  # 24小时

        cleanup_thread = threading.Thread(
            target=cleanup_worker,
            daemon=True,
            name="LogCleanup"
        )
        cleanup_thread.start()

    def _cleanup_old_logs(self) -> None:
        """清理过期的日志文件"""
        cutoff_date = datetime.now() - timedelta(days=self.retention_days)

        for log_file in self.log_dir.glob("*.log*"):
            try:
                # 获取文件修改时间
                file_mtime = datetime.fromtimestamp(log_file.stat().st_mtime)

                if file_mtime < cutoff_date:
                    log_file.unlink()
                    self.logger.info(f"已删除过期日志文件: {log_file.name}")
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error(f"删除日志文件失败 {log_file.name}: {e}")

    def debug(self, msg: str, *args, **kwargs) -> None:
        """记录DEBUG级别日志"""
        self.logger.debug(msg, *args, **kwargs)

    def info(self, msg: str, *args, **kwargs) -> None:
        """记录INFO级别日志"""
        self.logger.info(msg, *args, **kwargs)

    def warning(self, msg: str, *args, **kwargs) -> None:
        """记录WARNING级别日志"""
        self.logger.warning(msg, *args, **kwargs)

    def error(self, msg: str, *args, **kwargs) -> None:
        """记录ERROR级别日志"""
        self.logger.error(msg, *args, **kwargs)

    def critical(self, msg: str, *args, **kwargs) -> None:
        """记录CRITICAL级别日志"""
        self.logger.critical(msg, *args, **kwargs)

    def exception(self, msg: str, *args, **kwargs) -> None:
        """记录异常信息"""
        self.logger.exception(msg, *args, **kwargs)


# 全局logger实例缓存
_logger_cache = {}
_cache_lock = threading.Lock()


def get_logger(
    name: str,
    log_dir: Optional[str] = None,
    level: int = logging.INFO,
    **kwargs
) -> AdvancedLogger:
    """
    获取或创建logger实例

    Args:
        name: logger名称
        log_dir: 日志目录
        level: 日志级别
        **kwargs: 其他AdvancedLogger参数

    Returns:
        AdvancedLogger实例
    """
    cache_key = (name, log_dir or AdvancedLogger.DEFAULT_LOG_DIR)

    with _cache_lock:
        if cache_key not in _logger_cache:
            _logger_cache[cache_key] = AdvancedLogger(
                name=name,
                log_dir=log_dir,
                level=level,
                **kwargs
            )

        return _logger_cache[cache_key]


def log_method(func: Optional[Callable] = None, *, log_args: bool = True,
               log_result: bool = True) -> Callable:
    """
    方法日志装饰器

    记录方法的入口、出口、参数和返回值

    Args:
        func: 被装饰的函数
        log_args: 是否记录参数
        log_result: 是否记录返回值

    Returns:
        装饰后的函数

    Example:
        @log_method
        def my_function(x, y):
            return x + y

        @log_method(log_args=True, log_result=False)
        async def async_function(a, b):
            return a * b
    """
    def decorator(f: Callable) -> Callable:
        # 确定logger名称
        if hasattr(f, '__self__'):
            # 实例方法
            logger_name = f.__self__.__class__.__name__
        elif hasattr(f, '__qualname__') and '.' in f.__qualname__:
            # 类方法或静态方法
            logger_name = f.__qualname__.rsplit('.', 1)[0]
        else:
            # 普通函数
            logger_name = f.__module__

        logger = get_logger(logger_name)

        if asyncio.iscoroutinefunction(f):
            # 异步函数
            @functools.wraps(f)
            async def async_wrapper(*args, **kwargs):
                # 记录入口
                if log_args:
                    args_repr = _format_args(args, kwargs)
                    logger.info(f">>> 进入方法: {f.__name__}({args_repr})")
                else:
                    logger.info(f">>> 进入方法: {f.__name__}()")

                try:
                    result = await f(*args, **kwargs)

                    # 记录返回值
                    if log_result:
                        result_repr = _format_result(result)
                        logger.info(f"<<< 方法返回: {f.__name__} -> {result_repr}")
                    else:
                        logger.info(f"<<< 方法返回: {f.__name__}")

                    return result

                except Exception as e:
                    logger.error(
                        f"!!! 方法异常: {f.__name__} - {type(e).__name__}: {e}"
                    )
                    logger.debug(f"异常堆栈: {traceback.format_exc()}")
                    raise

            return async_wrapper

        # 同步函数
        @functools.wraps(f)
        def sync_wrapper(*args, **kwargs):
            # 记录入口
            if log_args:
                args_repr = _format_args(args, kwargs)
                logger.info(f">>> 进入方法: {f.__name__}({args_repr})")
            else:
                logger.info(f">>> 进入方法: {f.__name__}()")

            try:
                result = f(*args, **kwargs)

                # 记录返回值
                if log_result:
                    result_repr = _format_result(result)
                    logger.info(f"<<< 方法返回: {f.__name__} -> {result_repr}")
                else:
                    logger.info(f"<<< 方法返回: {f.__name__}")

                return result

            except Exception as e:
                logger.error(
                    f"!!! 方法异常: {f.__name__} - {type(e).__name__}: {e}"
                )
                logger.debug(f"异常堆栈: {traceback.format_exc()}")
                raise

        return sync_wrapper

    # 处理 @log_method 和 @log_method() 两种用法
    if func is None:
        return decorator
    return decorator(func)


def log_step(step_name: str) -> Callable:
    """
    关键步骤日志装饰器

    Args:
        step_name: 步骤名称

    Returns:
        装饰器函数

    Example:
        @log_step("验证用户权限")
        def check_permission(user_id):
            pass
    """
    def decorator(func: Callable) -> Callable:
        logger_name = func.__module__
        logger = get_logger(logger_name)

        if asyncio.iscoroutinefunction(func):
            @functools.wraps(func)
            async def async_wrapper(*args, **kwargs):
                logger.info(f"[步骤] {step_name} - 开始")
                try:
                    result = await func(*args, **kwargs)
                    logger.info(f"[步骤] {step_name} - 完成")
                    return result
                except Exception as e:
                    logger.error(f"[步骤] {step_name} - 失败: {e}")
                    raise

            return async_wrapper

        @functools.wraps(func)
        def sync_wrapper(*args, **kwargs):
            logger.info(f"[步骤] {step_name} - 开始")
            try:
                result = func(*args, **kwargs)
                logger.info(f"[步骤] {step_name} - 完成")
                return result
            except Exception as e:
                logger.error(f"[步骤] {step_name} - 失败: {e}")
                raise

        return sync_wrapper

    return decorator


def _format_args(args: tuple, kwargs: dict) -> str:
    """
    格式化函数参数

    Args:
        args: 位置参数
        kwargs: 关键字参数

    Returns:
        格式化后的参数字符串
    """
    parts = []

    # 格式化位置参数
    for arg in args:
        parts.append(_format_value(arg))

    # 格式化关键字参数
    for key, value in kwargs.items():
        parts.append(f"{key}={_format_value(value)}")

    return ", ".join(parts)


def _format_result(result: Any) -> str:
    """
    格式化返回值

    Args:
        result: 返回值

    Returns:
        格式化后的字符串
    """
    return _format_value(result)


def _format_value(value: Any, max_len: int = 200) -> str:
    """
    格式化值

    Args:
        value: 要格式化的值
        max_len: 最大长度

    Returns:
        格式化后的字符串
    """
    try:
        if value is None:
            return "None"
        if isinstance(value, (str, int, float, bool)):
            value_str = repr(value)
        elif isinstance(value, (list, tuple)):
            if len(value) == 0:
                value_str = "[]" if isinstance(value, list) else "()"
            elif len(value) <= 3:
                value_str = repr(value)
            else:
                value_str = f"[{len(value)} items]"
        elif isinstance(value, dict):
            if len(value) == 0:
                value_str = "{}"
            elif len(value) <= 3:
                value_str = repr(value)
            else:
                value_str = f"{{{len(value)} items}}"
        else:
            value_str = f"<{type(value).__name__}>"

        # 截断过长的字符串
        if len(value_str) > max_len:
            value_str = value_str[:max_len] + "..."

        return value_str

    except Exception:  # pylint: disable=broad-except
        return f"<{type(value).__name__}>"


# 导出主要接口
__all__ = [
    'AdvancedLogger',
    'get_logger',
    'log_method',
    'log_step',
    'AsyncLogHandler',
]
