"""Integration coverage for full user-account purging."""

from __future__ import annotations

# pylint: disable=line-too-long,duplicate-code
from pathlib import Path

import pytest

from bill_analyser.core.database.users import data as db_user_data_module
from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_user_purge_account.db"))
    await db.init_db()
    return db


async def _create_user(db: Database, username: str, email: str) -> int:
    return int(
        await db.create_user(
            {
                "username": username,
                "email": email,
                "password_hash": "pytest-hash",
                "nickname": username,
                "language": "zh_Hans",
                "default_currency": "CNY",
                "first_day_of_week": 1,
                "is_active": 1,
                "email_verified": 1,
            }
        )
    )


async def _count_rows(conn, table_name: str, *, user_id: int) -> int:
    async with conn.execute(f'SELECT COUNT(*) FROM "{table_name}" WHERE user_id = ?', (user_id,)) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


@pytest.mark.asyncio
async def test_purge_user_account_removes_user_and_residue(tmp_path: Path) -> None:
    """Full purge should remove the user row plus auth and business residue."""
    db = await _create_database(tmp_path)

    try:
        victim_id = await _create_user(db, "test_runtime_cleanup_target", "test_runtime_cleanup_target@example.com")
        keeper_id = await _create_user(db, "keeper_user", "keeper@example.com")

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
                "username": "test_runtime_cleanup_target",
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
            ) VALUES (?, 'description', 'pytest-learn', 'pytest-learn', '支出', 1, '2026-04-17T00:00:00', '2026-04-17T00:00:00')
            """,
            (victim_id,),
        )
        await conn.commit()

        result = await db.purge_user_account(victim_id)

        assert result["success"] is True
        assert result["deleted"] is True
        assert await db.get_user_by_id(victim_id) is None
        assert await _count_rows(conn, "sessions", user_id=victim_id) == 0
        assert await _count_rows(conn, "auth_logs", user_id=victim_id) == 0
        assert await _count_rows(conn, "accounts", user_id=victim_id) == 0
        assert await _count_rows(conn, "import_learning_rules", user_id=victim_id) == 0
        assert await db.get_user_by_id(keeper_id) is not None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_purge_user_account_refuses_protected_users(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Protected default users should not be purged automatically."""
    db = await _create_database(tmp_path)

    try:
        protected_user_id = await _create_user(db, "admin", "admin@example.com")
        monkeypatch.setattr(
            db_user_data_module,
            "load_default_user_settings",
            lambda: {"username": "admin", "auto_create": True},
        )

        result = await db.purge_user_account(protected_user_id)

        assert result["success"] is False
        assert result["deleted"] is False
        assert "protected" in str(result["message"]).lower()
        assert await db.get_user_by_id(protected_user_id) is not None
    finally:
        await db.close()
