"""Regression tests for shared pytest test-user cleanup helpers."""

from __future__ import annotations

from pathlib import Path

import pytest

from bill_analyser.core.db import Database
from tests import user_cleanup_support as cleanup_support_module
from tests.user_cleanup_support import (
    begin_test_user_cleanup_tracking,
    cleanup_registered_test_users_async,
    get_tracked_test_user_ids,
    purge_test_user_by_username_async,
    register_test_user_for_cleanup_async,
)


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_user_cleanup_support.db"))
    await db.init_db()
    return db


async def _create_user(db: Database, username: str) -> int:
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "pytest-hash",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


async def _count_rows_for_user(db: Database, table_name: str, user_id: int) -> int:
    """Count rows in one user-scoped table for one user."""
    conn = await db._get_connection()  # pylint: disable=protected-access
    async with conn.execute(
        f'SELECT COUNT(*) FROM "{table_name}" WHERE user_id = ?',
        (user_id,),
    ) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


@pytest.mark.asyncio
async def test_purge_test_user_by_username_removes_auth_and_business_residue(
    tmp_path: Path,
) -> None:
    """Purging one tracked test user should remove only that user's residue."""
    db = await _create_database(tmp_path)

    try:
        victim_id = await _create_user(db, "test_cleanup_target")
        keeper_id = await _create_user(db, "test_cleanup_keeper")

        await db.create_session(
            {
                "user_id": victim_id,
                "token_hash": "pytest-token-hash",
                "refresh_token_hash": "pytest-refresh-hash",
                "expires_at": "2099-01-01T00:00:00",
                "refresh_expires_at": "2099-02-01T00:00:00",
                "user_agent": "pytest",
                "ip_address": "127.0.0.1",
            }
        )
        await db.create_auth_log(
            {
                "user_id": victim_id,
                "username": "test_cleanup_target",
                "event_type": "login",
                "ip_address": "127.0.0.1",
                "user_agent": "pytest",
                "success": True,
                "error_message": None,
                "metadata": "{}",
            }
        )

        await db.create_account(
            {
                "name": "pytest-cleanup-account",
                "category": 1,
                "type": 1,
                "icon": "1",
                "color": "00ccff",
                "currency": "CNY",
                "balance": 0,
                "comment": "pytest cleanup",
                "hidden": False,
                "aliases": [],
            },
            user_id=victim_id,
        )

        conn = await db._get_connection()  # pylint: disable=protected-access
        await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, enabled, created_at, updated_at
            ) VALUES (
                ?, 'description', 'cleanup-target', 'cleanup-target',
                '支出', 1, '2026-04-16T00:00:00', '2026-04-16T00:00:00'
            )
            """,
            (victim_id,),
        )
        await conn.commit()

        assert await db.get_user_by_id(victim_id) is not None
        assert await _count_rows_for_user(db, "sessions", victim_id) == 1
        assert await _count_rows_for_user(db, "auth_logs", victim_id) == 1
        assert await _count_rows_for_user(db, "accounts", victim_id) == 1
        assert await _count_rows_for_user(db, "import_learning_rules", victim_id) == 1

        deleted = await purge_test_user_by_username_async(db, "test_cleanup_target")

        assert deleted is True
        assert await db.get_user_by_id(victim_id) is None
        assert await _count_rows_for_user(db, "sessions", victim_id) == 0
        assert await _count_rows_for_user(db, "auth_logs", victim_id) == 0
        assert await _count_rows_for_user(db, "accounts", victim_id) == 0
        assert await _count_rows_for_user(db, "import_learning_rules", victim_id) == 0
        assert await db.get_user_by_id(keeper_id) is not None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_cleanup_registered_test_users_async_only_removes_tracked_users(
    tmp_path: Path,
) -> None:
    """Registry cleanup should preserve untracked users."""
    db = await _create_database(tmp_path)

    try:
        tracked_id = await _create_user(db, "test_cleanup_tracked")
        untracked_id = await _create_user(db, "test_cleanup_untracked")

        begin_test_user_cleanup_tracking()
        registered_user_id = await register_test_user_for_cleanup_async(
            db,
            "test_cleanup_tracked",
        )

        assert registered_user_id == tracked_id
        assert get_tracked_test_user_ids() == {tracked_id}

        cleaned_ids = await cleanup_registered_test_users_async(db)

        assert cleaned_ids == [tracked_id]
        assert await db.get_user_by_id(tracked_id) is None
        assert await db.get_user_by_id(untracked_id) is not None
        assert get_tracked_test_user_ids() == set()
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_purge_skips_configured_default_user_even_when_name_looks_like_test_user(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Configured default users must remain protected even if their names start with test_."""
    db = await _create_database(tmp_path)

    try:
        protected_username = "test_configured_default_user"
        protected_user_id = await _create_user(db, protected_username)
        monkeypatch.setattr(
            cleanup_support_module,
            "load_default_user_settings",
            lambda use_cache=True: {"username": protected_username, "auto_create": True},
        )

        deleted = await purge_test_user_by_username_async(db, protected_username)

        assert deleted is False
        assert await db.get_user_by_id(protected_user_id) is not None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_cleanup_registered_test_users_uses_original_db_instance(
    tmp_path: Path,
) -> None:
    """Tracked users should be cleaned from the DB where they were originally registered."""
    primary_db = await _create_database(tmp_path / "primary")
    secondary_db = await _create_database(tmp_path / "secondary")

    try:
        tracked_username = "test_cleanup_bound_db"
        tracked_user_id = await _create_user(primary_db, tracked_username)

        begin_test_user_cleanup_tracking()
        registered_user_id = await register_test_user_for_cleanup_async(
            primary_db,
            tracked_username,
        )

        assert registered_user_id == tracked_user_id
        cleaned_ids = await cleanup_registered_test_users_async(secondary_db)

        assert cleaned_ids == [tracked_user_id]
        assert await primary_db.get_user_by_id(tracked_user_id) is None
    finally:
        await primary_db.close()
        await secondary_db.close()
