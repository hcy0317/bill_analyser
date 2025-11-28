"""
Logger Module - 异步日志模块

提供异步日志记录功能，支持自动滚动清理和统一的日志格式。
日志格式：时间戳 | 线程名 | 级别 | 类名.方法名 | 消息
"""

import asyncio
import logging
import threading
import queue
import sys
from datetime import datetime, timedelta
from logging.handlers import RotatingFileHandler, QueueHandler, QueueListener
from pathlib import Path
from typing import Optional
import functools
import inspect


class SafeStreamHandler(logging.StreamHandler):
    """安全的流处理器，避免写入已关闭的流"""
    def emit(self, record):
        try:
            if hasattr(self.stream, 'closed') and self.stream.closed:
                return
            super().emit(record)
        except (ValueError, OSError):
            # 忽略 I/O operation on closed file 错误
            pass

class AsyncLogger:
    """异步日志管理器"""

    def __init__(self):
        """初始化日志管理器"""
        self.log_dir = Path(__file__).parent.parent.parent / "logs"
        self.log_dir.mkdir(parents=True, exist_ok=True)

        # 配置参数
        self.max_log_size_mb = 10
        self.max_log_age_days = 7  # 7天自动清理

        # 创建日志队列
        self.log_queue = queue.Queue(-1)

        # 初始化
        self._setup_loggers()
        self._start_cleanup_task()

    def _setup_loggers(self):
        """配置日志器"""
        # 创建自定义格式化器
        # 格式：时间戳 | 线程名 | 级别 | 类名.方法名 | 消息
        formatter = logging.Formatter(
            '%(asctime)s | %(threadName)s | %(levelname)-8s | %(name)s | %(message)s',
            datefmt='%Y-%m-%d %H:%M:%S'
        )

        # 实际的处理器列表
        handlers = []

        # 添加控制台处理器
        if 'pytest' not in sys.modules:
            console_handler = SafeStreamHandler(sys.stdout)
            console_handler.setLevel(logging.INFO)
            console_handler.setFormatter(formatter)
            handlers.append(console_handler)
# ...existing code...

        # 添加文件处理器（按大小轮转）
        log_file = self.log_dir / f"bill_analyser_{datetime.now().strftime('%Y%m%d')}.log"
        file_handler = RotatingFileHandler(
            log_file,
            maxBytes=self.max_log_size_mb * 1024 * 1024,
            backupCount=5,
            encoding='utf-8'
        )
        file_handler.setLevel(logging.DEBUG)
        file_handler.setFormatter(formatter)
        handlers.append(file_handler)

        # 添加错误日志文件处理器
        error_log_file = self.log_dir / f"error_{datetime.now().strftime('%Y%m%d')}.log"
        error_handler = RotatingFileHandler(
            error_log_file,
            maxBytes=self.max_log_size_mb * 1024 * 1024,
            backupCount=5,
            encoding='utf-8'
        )
        error_handler.setLevel(logging.ERROR)
        error_handler.setFormatter(formatter)
        handlers.append(error_handler)

        # 配置队列监听器
        self.queue_listener = QueueListener(self.log_queue, *handlers, respect_handler_level=True)
        self.queue_listener.start()

        # 配置根日志器使用QueueHandler
        self.logger = logging.getLogger('bill_analyser')
        self.logger.setLevel(logging.DEBUG)
        self.logger.propagate = False

        # 清除旧处理器并添加QueueHandler
        self.logger.handlers.clear()
        queue_handler = QueueHandler(self.log_queue)
        self.logger.addHandler(queue_handler)

    def _start_cleanup_task(self):
        """启动日志清理任务"""
        def cleanup_worker():
            """清理工作线程"""
            while True:
                try:
                    self._cleanup_old_logs()
                    # 每天清理一次
                    threading.Event().wait(86400)
                except Exception as e:
                    # 避免递归记录错误
                    print(f"日志清理任务出错: {e}")

        cleanup_thread = threading.Thread(
            target=cleanup_worker,
            name="LogCleanupThread",
            daemon=True
        )
        cleanup_thread.start()

    def _cleanup_old_logs(self):
        """清理过期日志文件"""
        try:
            cutoff_date = datetime.now() - timedelta(days=self.max_log_age_days)

            for log_file in self.log_dir.glob("*.log*"):
                try:
                    if log_file.stat().st_mtime < cutoff_date.timestamp():
                        log_file.unlink()
                        self.logger.info(f"已删除过期日志: {log_file.name}")
                except Exception as e:
                    self.logger.error(f"删除日志文件失败 {log_file}: {e}")
        except Exception as e:
            self.logger.error(f"清理日志目录失败: {e}")

    def get_logger(self, name: Optional[str] = None) -> logging.Logger:
        """获取日志器"""
        if name:
            return logging.getLogger(f'bill_analyser.{name}')
        return self.logger

    # 移除旧的 async_log 方法，因为 QueueHandler 已经是异步（非阻塞）的了

    def stop(self):
        """停止日志系统"""
        if hasattr(self, 'queue_listener'):
            self.queue_listener.stop()


# 全局日志实例
_logger_instance = AsyncLogger()


def get_logger(name: Optional[str] = None) -> logging.Logger:
    """
    获取日志器

    Args:
        name: 日志器名称

    Returns:
        logging.Logger: 日志器实例
    """
    return _logger_instance.get_logger(name)


def log_method(func):
    """
    方法日志装饰器

    自动记录方法的入口、出口、参数和返回值
    """
    @functools.wraps(func)
    async def async_wrapper(*args, **kwargs):
        # 获取类名和方法名
        class_name = args[0].__class__.__name__ if args else "Unknown"
        method_name = func.__name__
        logger_name = f"{class_name}.{method_name}"
        logger = get_logger(logger_name)

        # 格式化参数
        try:
            sig = inspect.signature(func)
            bound_args = sig.bind(*args, **kwargs)
            bound_args.apply_defaults()

            # 过滤掉 self 参数
            params = {k: v for k, v in bound_args.arguments.items() if k != 'self'}
            # 截断过长的参数值
            param_str_parts = []
            for k, v in params.items():
                v_str = repr(v)
                if len(v_str) > 100:
                    v_str = v_str[:100] + "..."
                param_str_parts.append(f"{k}={v_str}")
            param_str = ", ".join(param_str_parts)
        except Exception:
            param_str = "无法解析参数"

        # 记录方法入口
        start_time = datetime.now()
        try:
            if sys.meta_path:
                logger.debug(f"进入方法 | 参数: {param_str if param_str else '无'}")
        except (ImportError, Exception):
            pass

        try:
            # 执行方法
            result = await func(*args, **kwargs)

            # 计算执行时间
            duration = (datetime.now() - start_time).total_seconds() * 1000

            # 记录方法出口和返回值
            try:
                result_str = repr(result) if result is not None else "None"
                if len(result_str) > 200:
                    result_str = result_str[:200] + "..."
            except Exception:
                result_str = "无法解析返回值"

            try:
                if sys.meta_path:
                    logger.debug(f"退出方法 | 耗时: {duration:.2f}ms | 返回值: {result_str}")
            except (ImportError, Exception):
                pass

            return result
        except Exception as e:
            # 记录异常
            logger.error(f"方法异常 | 错误: {type(e).__name__}: {str(e)}")
            raise

    @functools.wraps(func)
    def sync_wrapper(*args, **kwargs):
        # 获取类名和方法名
        class_name = args[0].__class__.__name__ if args else "Unknown"
        method_name = func.__name__
        logger_name = f"{class_name}.{method_name}"
        logger = get_logger(logger_name)

        # 格式化参数
        try:
            sig = inspect.signature(func)
            bound_args = sig.bind(*args, **kwargs)
            bound_args.apply_defaults()

            # 过滤掉 self 参数
            params = {k: v for k, v in bound_args.arguments.items() if k != 'self'}
            # 截断过长的参数值
            param_str_parts = []
            for k, v in params.items():
                v_str = repr(v)
                if len(v_str) > 100:
                    v_str = v_str[:100] + "..."
                param_str_parts.append(f"{k}={v_str}")
            param_str = ", ".join(param_str_parts)
        except Exception:
            param_str = "无法解析参数"

        # 记录方法入口
        start_time = datetime.now()
        try:
            if sys.meta_path:
                logger.debug(f"进入方法 | 参数: {param_str if param_str else '无'}")
        except (ImportError, Exception):
            # 忽略解释器关闭时的错误
            pass

        try:
            # 执行方法
            result = func(*args, **kwargs)

            # 计算执行时间
            duration = (datetime.now() - start_time).total_seconds() * 1000

            # 记录方法出口和返回值
            try:
                result_str = repr(result) if result is not None else "None"
                if len(result_str) > 200:
                    result_str = result_str[:200] + "..."
            except Exception:
                result_str = "无法解析返回值"

            try:
                if sys.meta_path:
                    logger.debug(f"退出方法 | 耗时: {duration:.2f}ms | 返回值: {result_str}")
            except (ImportError, Exception):
                pass

            return result
        except Exception as e:
            # 记录异常
            logger.error(f"方法异常 | 错误: {type(e).__name__}: {str(e)}")
            raise

    # 判断是否为异步函数
    if asyncio.iscoroutinefunction(func):
        return async_wrapper
    return sync_wrapper


def log_step(step_name: str):
    """
    记录关键步骤

    Args:
        step_name: 步骤名称
    """
    def decorator(func):
        @functools.wraps(func)
        async def async_wrapper(*args, **kwargs):
            class_name = args[0].__class__.__name__ if args else "Unknown"
            method_name = func.__name__
            logger_name = f"{class_name}.{method_name}"
            logger = get_logger(logger_name)

            logger.info(f"开始步骤: {step_name}")
            try:
                result = await func(*args, **kwargs)
                logger.info(f"完成步骤: {step_name}")
                return result
            except Exception as e:
                logger.error(f"步骤失败: {step_name} | 错误: {type(e).__name__}: {str(e)}")
                raise

        @functools.wraps(func)
        def sync_wrapper(*args, **kwargs):
            class_name = args[0].__class__.__name__ if args else "Unknown"
            method_name = func.__name__
            logger_name = f"{class_name}.{method_name}"
            logger = get_logger(logger_name)

            logger.info(f"开始步骤: {step_name}")
            try:
                result = func(*args, **kwargs)
                logger.info(f"完成步骤: {step_name}")
                return result
            except Exception as e:
                logger.error(f"步骤失败: {step_name} | 错误: {type(e).__name__}: {str(e)}")
                raise

        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        return sync_wrapper

    return decorator
