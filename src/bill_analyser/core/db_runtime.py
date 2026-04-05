"""Runtime lifecycle, connection, cache, and path-resolution helpers."""

# pylint: disable=line-too-long,too-many-return-statements

from __future__ import annotations

import os
import sqlite3
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

import aiosqlite

from bill_analyser.constants import DATA_DIR, TEST_DB_DIR_ENV

from ..utils.logger import get_logger, log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now


def _is_relative_to(path: Path, parent: Path) -> bool:
    """兼容 Python 版本差异的 Path 前缀判断。"""
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def _resolve_database_path(db_path: str | None) -> Path:
    """解析数据库路径，并在测试场景下将测试库重定向到 tests 运行目录。"""
    test_db_dir = os.environ.get(TEST_DB_DIR_ENV, "").strip()

    if db_path is None:
        if test_db_dir:
            return Path(test_db_dir) / "pytest_default.db"
        return DATA_DIR / "bills.db"

    if db_path in {":memory:", "file::memory:?cache=shared"}:
        return Path(db_path)

    candidate = Path(db_path)
    if not test_db_dir:
        return candidate

    filename = candidate.name.lower()
    is_test_db_name = filename.startswith("test") and filename.endswith(".db")
    if not is_test_db_name:
        return candidate

    redirected_root = Path(test_db_dir)

    if not candidate.is_absolute():
        parts = tuple(part.lower() for part in candidate.parts)
        if parts and parts[0] == "data":
            return redirected_root / Path(*candidate.parts[1:])

        try:
            resolved_candidate = (Path.cwd() / candidate).resolve()
            resolved_data_dir = DATA_DIR.resolve()
        except OSError:
            return candidate

        if _is_relative_to(resolved_candidate, resolved_data_dir):
            return redirected_root / resolved_candidate.relative_to(resolved_data_dir)

        return candidate

    try:
        resolved_candidate = candidate.resolve()
        resolved_data_dir = DATA_DIR.resolve()
    except OSError:
        return candidate

    if _is_relative_to(resolved_candidate, resolved_data_dir):
        return redirected_root / resolved_candidate.relative_to(resolved_data_dir)

    return candidate


class DatabaseRuntimeMixin(DatabaseFacadeBase):
    """Database lifecycle, connection, path, and shared cache helpers."""

    def __init__(self, db_path: str | None = None):
        self.logger = get_logger("Database")
        self.db_path = _resolve_database_path(db_path)

        if str(self.db_path) not in {":memory:", "file::memory:?cache=shared"}:
            self.db_path.parent.mkdir(parents=True, exist_ok=True)

        self._connection: aiosqlite.Connection | None = None
        self.batch_size = 1000
        self._cache: dict[str, Any] = {}
        self._cache_expiry: dict[str, datetime] = {}
        self._cache_ttl = 60

        self.logger.info("数据库管理器已初始化: %s", self.db_path)

    @log_method
    async def _get_connection(self) -> aiosqlite.Connection:
        """获取数据库连接。"""
        if self._connection is None:
            self._connection = await aiosqlite.connect(str(self.db_path))
            self._connection.row_factory = aiosqlite.Row
        return self._connection

    def _clear_cache(self, key: str | None = None) -> None:
        """清除缓存。"""
        if key:
            self._cache.pop(key, None)
            self._cache_expiry.pop(key, None)
            self.logger.debug("已清除缓存: %s", key)
            return

        self._cache.clear()
        self._cache_expiry.clear()
        self.logger.debug("已清除所有缓存")

    def _is_cache_valid(self, key: str) -> bool:
        """检查缓存是否有效。"""
        if key not in self._cache:
            return False

        expiry = self._cache_expiry.get(key)
        if not expiry:
            return False

        return utc_now() < expiry

    @log_method
    async def close(self) -> None:
        """关闭数据库连接。"""
        if self._connection is None:
            return

        try:
            test_db_dir = os.environ.get(TEST_DB_DIR_ENV, "").strip()
            should_checkpoint = False

            if test_db_dir and str(self.db_path) not in {":memory:", "file::memory:?cache=shared"}:
                try:
                    should_checkpoint = _is_relative_to(self.db_path.resolve(), Path(test_db_dir).resolve())
                except OSError:
                    should_checkpoint = False

            if should_checkpoint:
                await self._connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
                await self._connection.commit()
        except (sqlite3.Error, aiosqlite.Error, OSError, RuntimeError, ValueError) as exc:
            self.logger.debug("关闭数据库前执行 WAL checkpoint 失败: %s", exc)

        await self._connection.close()
        self._connection = None
        self.logger.info("数据库连接已关闭")

    @log_method
    async def __aenter__(self):
        """异步上下文管理器入口。"""
        await self.init_db()
        return self

    @log_method
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """异步上下文管理器退出。"""
        await self.close()

    @log_method
    async def get_account_mappings(self) -> dict[str, Any]:
        """获取账户映射（带缓存）。"""
        cache_key = "account_mappings"
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的账户映射")
            return self._cache[cache_key]

        accounts = await self.get_all_accounts()
        mappings = {
            "id_to_account": {account["id"]: account for account in accounts},
            "name_to_id": {account["name"]: account["id"] for account in accounts},
            "id_to_name": {account["id"]: account["name"] for account in accounts},
        }

        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = utc_now() + timedelta(seconds=self._cache_ttl)
        self.logger.info("已构建账户映射: %s 个账户", len(accounts))
        return mappings

    @log_method
    async def get_category_mappings(self) -> dict[str, Any]:
        """获取分类映射（带缓存）。"""
        cache_key = "category_mappings"
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的分类映射")
            return self._cache[cache_key]

        categories = await self.get_all_categories()
        mappings = {
            "id_to_category": {category["id"]: category for category in categories},
            "name_to_id": {
                (category["main_category"], category["sub_category"]): category["id"]
                for category in categories
            },
            "id_to_name": {
                category["id"]: (category["main_category"], category["sub_category"])
                for category in categories
            },
        }

        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = utc_now() + timedelta(seconds=self._cache_ttl)
        self.logger.info("已构建分类映射: %s 个分类", len(categories))
        return mappings
