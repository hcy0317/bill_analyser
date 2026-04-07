"""Legacy user/security schema migration regression coverage."""

from __future__ import annotations

import sqlite3
from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


def _create_legacy_users_table(db_path: Path) -> None:
    connection = sqlite3.connect(db_path)
    try:
        connection.execute(
            """
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                nickname TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            """
        )
        connection.execute(
            """
            INSERT INTO users (username, email, password_hash, nickname, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (
                "legacy-user",
                "legacy-user@example.com",
                "pytest-hash",
                "Legacy User",
                "2026-04-01 00:00:00",
                "2026-04-01 00:00:00",
            ),
        )
        connection.commit()
    finally:
        connection.close()


def _get_table_columns(connection: sqlite3.Connection, table_name: str) -> set[str]:
    return {row[1] for row in connection.execute(f"PRAGMA table_info({table_name})")}


@pytest.mark.asyncio
async def test_init_db_migrates_legacy_user_security_columns_and_support_tables(
    tmp_path: Path,
) -> None:
    """init_db() should backfill missing user/security columns for legacy databases."""
    db_path = tmp_path / "test_db_user_security_schema_paths.db"
    _create_legacy_users_table(db_path)

    db = Database(str(db_path))
    try:
        await db.init_db()
        await db.init_db()

        created_user_id = await db.create_user(
            {
                "username": "migrated-user",
                "email": "migrated-user@example.com",
                "password_hash": "pytest-hash",
                "nickname": "Migrated User",
            }
        )
        assert created_user_id > 0

        assert await db.increment_failed_login(1) is True
        await db.update_user_last_login(1, ip_address="127.0.0.1")
    finally:
        await db.close()

    connection = sqlite3.connect(db_path)
    try:
        user_columns = _get_table_columns(connection, "users")
        assert {
            "cash_account_id",
            "cash_transfer_category_id",
            "import_learning_enabled",
            "investment_platform_keywords",
            "investment_product_keywords",
            "investment_exclude_keywords",
        }.issubset(user_columns)

        legacy_user = connection.execute(
            """
            SELECT
                avatar,
                language,
                default_currency,
                first_day_of_week,
                is_active,
                email_verified,
                two_factor_enabled,
                failed_login_attempts,
                locked_until,
                last_login_at,
                last_login_ip,
                cash_account_id,
                cash_transfer_category_id,
                import_learning_enabled,
                investment_platform_keywords,
                investment_product_keywords,
                investment_exclude_keywords
            FROM users
            WHERE username = ?
            """,
            ("legacy-user",),
        ).fetchone()
        assert legacy_user is not None
        assert legacy_user[0] == ""
        assert legacy_user[1] == "zh_Hans"
        assert legacy_user[2] == "CNY"
        assert legacy_user[3] == 1
        assert legacy_user[4] == 1
        assert legacy_user[5] == 0
        assert legacy_user[6] == 0
        assert legacy_user[7] == 0
        assert legacy_user[8] is None
        assert legacy_user[9] is not None
        assert legacy_user[10] == "127.0.0.1"
        assert legacy_user[11] is None
        assert legacy_user[12] is None
        assert legacy_user[13] == 1
        assert legacy_user[14] is None
        assert legacy_user[15] is None
        assert legacy_user[16] is None

        table_names = {
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_master WHERE type = 'table'"
            )
        }
        assert {
            "sessions",
            "auth_logs",
            "user_two_factor_recovery_codes",
            "user_external_auths",
            "user_application_cloud_settings",
            "app_settings",
            "audit_logs",
            "backup_records",
            "backup_jobs",
        }.issubset(table_names)
    finally:
        connection.close()
