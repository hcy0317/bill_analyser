"""Focused coverage for db_runtime helper branches."""

# pylint: disable=missing-function-docstring,too-few-public-methods,protected-access

from __future__ import annotations

import sqlite3
from datetime import timedelta
from pathlib import Path
from typing import Any, cast

import pytest

from bill_analyser.constants import TEST_DB_DIR_ENV
from bill_analyser.core.database import runtime as db_runtime
from bill_analyser.core.database.runtime import DatabaseRuntimeMixin, _resolve_database_path
from bill_analyser.core.database.time import utc_now


class DummyRuntime(DatabaseRuntimeMixin):
    """Small runtime test double with controllable account/category payloads."""

    def __init__(
        self,
        db_path: str | None = None,
        *,
        accounts: list[dict[str, Any]] | None = None,
        categories: list[dict[str, Any]] | None = None,
    ) -> None:
        self.init_db_calls = 0
        self.get_all_accounts_calls = 0
        self.get_all_categories_calls = 0
        self._accounts = list(accounts or [])
        self._categories = list(categories or [])
        super().__init__(db_path)

    async def init_db(self) -> None:
        self.init_db_calls += 1

    async def get_all_accounts(self, user_id: int = 1) -> list[dict[str, Any]]:
        del user_id
        self.get_all_accounts_calls += 1
        return list(self._accounts)

    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        del user_id
        self.get_all_categories_calls += 1
        return list(self._categories)


class FakeConnection:
    """Async connection stub used to exercise close() branches."""

    def __init__(self, *, execute_error: Exception | None = None) -> None:
        self.execute_error = execute_error
        self.executed_sql: list[str] = []
        self.commit_calls = 0
        self.close_calls = 0

    async def execute(self, sql: str) -> None:
        self.executed_sql.append(sql)
        if self.execute_error is not None:
            raise self.execute_error

    async def commit(self) -> None:
        self.commit_calls += 1

    async def close(self) -> None:
        self.close_calls += 1


class BrokenResolvePath:
    """Path-like helper whose resolve() always fails."""

    def __init__(self, raw_path: Path) -> None:
        self._raw_path = raw_path

    def resolve(self) -> Path:
        raise OSError("broken resolve")

    def __str__(self) -> str:
        return str(self._raw_path)


class BrokenDataDir:
    """DATA_DIR stand-in that forces the resolution fallback branch."""

    def resolve(self) -> Path:
        raise OSError("broken data dir")


def test_resolve_database_path_covers_default_and_passthrough_branches(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Path resolution should preserve default, memory, and no-test-dir branches."""
    data_dir = tmp_path / "data"
    monkeypatch.setattr(db_runtime, "DATA_DIR", data_dir)
    monkeypatch.delenv(TEST_DB_DIR_ENV, raising=False)

    assert _resolve_database_path(None) == data_dir / "bills.db"
    assert _resolve_database_path("test_passthrough.db") == Path("test_passthrough.db")

    redirected_root = tmp_path / "redirected"
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(redirected_root))
    assert _resolve_database_path(None) == redirected_root / "pytest_default.db"
    assert _resolve_database_path(":memory:") == Path(":memory:")
    assert _resolve_database_path("file::memory:?cache=shared") == Path(
        "file::memory:?cache=shared"
    )
    assert _resolve_database_path("production.db") == Path("production.db")


def test_resolve_database_path_redirects_relative_and_absolute_test_dbs(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Relative and absolute test DBs under DATA_DIR should redirect into the test root."""
    data_dir = tmp_path / "data"
    data_dir.mkdir()
    redirect_root = tmp_path / "redirected"
    monkeypatch.setattr(db_runtime, "DATA_DIR", data_dir)
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(redirect_root))

    monkeypatch.chdir(data_dir)
    assert _resolve_database_path("test_relative.db") == redirect_root / "test_relative.db"
    assert _resolve_database_path("data/test_nested.db") == redirect_root / "test_nested.db"

    monkeypatch.chdir(tmp_path)
    assert _resolve_database_path("test_outside.db") == Path("test_outside.db")

    absolute_candidate = data_dir / "nested" / "test_absolute.db"
    assert _resolve_database_path(str(absolute_candidate)) == (
        redirect_root / "nested" / "test_absolute.db"
    )


def test_resolve_database_path_returns_candidate_when_resolution_raises_os_error(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolution failures should fall back to the original candidate path."""
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(tmp_path / "redirected"))
    monkeypatch.setattr(db_runtime, "DATA_DIR", BrokenDataDir())
    monkeypatch.chdir(tmp_path)

    assert _resolve_database_path("test_broken_relative.db") == Path("test_broken_relative.db")

    absolute_candidate = tmp_path / "test_broken_absolute.db"
    assert _resolve_database_path(str(absolute_candidate)) == absolute_candidate


def test_runtime_init_creates_parent_dir_for_non_memory_path(tmp_path: Path) -> None:
    """Non-memory database paths should create their parent directory eagerly."""
    db_path = tmp_path / "nested" / "db" / "test_runtime_helpers.db"

    runtime = DummyRuntime(str(db_path))

    assert db_path.parent.exists()
    assert runtime.db_path == db_path


def test_cache_helpers_clear_scoped_entries_and_reject_missing_expiry(tmp_path: Path) -> None:
    """Cache helpers should reject entries without expiry and clear scoped keys together."""
    runtime = DummyRuntime(str(tmp_path / "test_runtime_cache.db"))
    valid_until = utc_now() + timedelta(seconds=30)

    runtime._cache["account_mappings"] = {"stale": True}
    runtime._cache["category_mappings"] = {"root": True}
    runtime._cache_expiry["category_mappings"] = valid_until
    runtime._cache["category_mappings:1"] = {"user": True}
    runtime._cache_expiry["category_mappings:1"] = valid_until

    assert runtime._is_cache_valid("account_mappings") is False

    runtime._clear_cache("category_mappings")
    assert "category_mappings" not in runtime._cache
    assert "category_mappings:1" not in runtime._cache

    runtime._clear_cache()
    assert not runtime._cache
    assert not runtime._cache_expiry


@pytest.mark.asyncio
async def test_async_context_manager_calls_init_db_and_close(tmp_path: Path) -> None:
    """Async context manager should initialize and close through the runtime hooks."""
    runtime = DummyRuntime(str(tmp_path / "test_runtime_context.db"))
    fake_connection = FakeConnection()
    runtime._connection = cast("Any", fake_connection)

    async with runtime as opened_runtime:
        assert opened_runtime is runtime
        assert runtime.init_db_calls == 1

    assert fake_connection.close_calls == 1
    assert runtime._connection is None


@pytest.mark.asyncio
async def test_get_account_mappings_builds_cache_and_reuses_it(tmp_path: Path) -> None:
    """Account mappings should populate cache on first call and reuse it on subsequent hits."""
    runtime = DummyRuntime(
        str(tmp_path / "test_account_mappings.db"),
        accounts=[
            {"id": 1, "name": "现金账户"},
            {"id": 2, "name": "银行卡"},
        ],
    )

    first_result = await runtime.get_account_mappings()
    second_result = await runtime.get_account_mappings()

    assert first_result["name_to_id"] == {"现金账户": 1, "银行卡": 2}
    assert first_result["id_to_name"] == {1: "现金账户", 2: "银行卡"}
    assert second_result is first_result
    assert runtime.get_all_accounts_calls == 1
    assert runtime._is_cache_valid("account_mappings") is True


@pytest.mark.asyncio
async def test_get_category_mappings_builds_cache_per_user_and_reuses_it(tmp_path: Path) -> None:
    """Category mappings should cache independently per user and reuse valid entries."""
    runtime = DummyRuntime(
        str(tmp_path / "test_category_mappings.db"),
        categories=[
            {"id": 11, "main_category": "餐饮", "sub_category": "早餐"},
            {"id": 12, "main_category": "交通", "sub_category": "地铁"},
        ],
    )

    first_result = await runtime.get_category_mappings(user_id=7)
    second_result = await runtime.get_category_mappings(user_id=7)

    assert first_result["name_to_id"] == {("餐饮", "早餐"): 11, ("交通", "地铁"): 12}
    assert first_result["id_to_name"] == {11: ("餐饮", "早餐"), 12: ("交通", "地铁")}
    assert second_result is first_result
    assert runtime.get_all_categories_calls == 1
    assert runtime._is_cache_valid("category_mappings:7") is True


@pytest.mark.asyncio
async def test_close_without_connection_is_a_safe_no_op(tmp_path: Path) -> None:
    """close() should simply return when no connection has been opened yet."""
    runtime = DummyRuntime(str(tmp_path / "test_close_no_connection.db"))

    await runtime.close()

    assert runtime._connection is None


@pytest.mark.asyncio
async def test_close_runs_wal_checkpoint_for_test_db(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """close() should checkpoint test DBs before closing the connection."""
    redirect_root = tmp_path / "redirected"
    redirect_root.mkdir()
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(redirect_root))

    runtime = DummyRuntime(str(redirect_root / "test_close_runtime.db"))
    fake_connection = FakeConnection()
    runtime._connection = cast("Any", fake_connection)

    await runtime.close()

    assert fake_connection.executed_sql == ["PRAGMA wal_checkpoint(TRUNCATE)"]
    assert fake_connection.commit_calls == 1
    assert fake_connection.close_calls == 1
    assert runtime._connection is None


@pytest.mark.asyncio
async def test_close_skips_checkpoint_when_db_path_resolution_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """close() should still close cleanly when db_path.resolve() raises OSError."""
    redirect_root = tmp_path / "redirected"
    redirect_root.mkdir()
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(redirect_root))

    runtime = DummyRuntime(str(redirect_root / "test_close_broken.db"))
    fake_connection = FakeConnection()
    runtime._connection = cast("Any", fake_connection)
    runtime.db_path = cast("Any", BrokenResolvePath(redirect_root / "test_close_broken.db"))

    await runtime.close()

    assert not fake_connection.executed_sql
    assert fake_connection.commit_calls == 0
    assert fake_connection.close_calls == 1
    assert runtime._connection is None


@pytest.mark.asyncio
async def test_close_swallows_checkpoint_errors_and_still_closes(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Checkpoint failures should not prevent close() from releasing the connection."""
    redirect_root = tmp_path / "redirected"
    redirect_root.mkdir()
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(redirect_root))

    runtime = DummyRuntime(str(redirect_root / "test_close_error.db"))
    fake_connection = FakeConnection(execute_error=sqlite3.OperationalError("checkpoint boom"))
    runtime._connection = cast("Any", fake_connection)

    await runtime.close()

    assert fake_connection.executed_sql == ["PRAGMA wal_checkpoint(TRUNCATE)"]
    assert fake_connection.commit_calls == 0
    assert fake_connection.close_calls == 1
    assert runtime._connection is None
