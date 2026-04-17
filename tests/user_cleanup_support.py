"""Helpers for cleaning test-created users from shared pytest databases."""

from __future__ import annotations

import asyncio
from typing import Any

from bill_analyser.utils.config import load_default_user_settings

_FALLBACK_PROTECTED_USERNAMES = {"admin"}
_TEST_USERNAME_PREFIXES = ("test_", "pytest_")
_TRACKED_TEST_USERS: list[tuple[Any, int]] | None = None


def _get_protected_usernames() -> set[str]:
    """Return usernames that cleanup helpers must never purge."""
    protected_usernames = set(_FALLBACK_PROTECTED_USERNAMES)
    try:
        default_user = load_default_user_settings()
    except Exception:  # pylint: disable=broad-exception-caught  # pragma: no cover
        default_user = None

    if isinstance(default_user, dict):
        username = str(default_user.get("username") or "").strip().lower()
        if username:
            protected_usernames.add(username)

    return protected_usernames


def _is_cleanup_candidate(username: str) -> bool:
    """Return True when a username is an eligible test-only cleanup target."""
    normalized_username = str(username or "").strip().lower()
    if not normalized_username:
        return False

    if normalized_username in _get_protected_usernames():
        return False

    return normalized_username.startswith(_TEST_USERNAME_PREFIXES)


def begin_test_user_cleanup_tracking() -> None:
    """Start a fresh per-test cleanup registry."""
    global _TRACKED_TEST_USERS  # pylint: disable=global-statement

    _TRACKED_TEST_USERS = []


def get_tracked_test_user_ids() -> set[int]:
    """Return a snapshot of the currently tracked test user ids."""
    return {user_id for _, user_id in (_TRACKED_TEST_USERS or [])}


def register_test_user_for_cleanup(db: Any, username: str) -> int | None:
    """Synchronously register a test user for end-of-test cleanup."""
    return asyncio.run(register_test_user_for_cleanup_async(db, username))


def cleanup_registered_test_users(db: Any | None = None) -> list[int]:
    """Synchronously purge all test users tracked for the current test."""
    return asyncio.run(cleanup_registered_test_users_async(db))


def purge_test_user_by_username(db: Any, username: str) -> bool:
    """Synchronously purge a test user by username."""
    return asyncio.run(purge_test_user_by_username_async(db, username))


async def register_test_user_for_cleanup_async(db: Any, username: str) -> int | None:
    """Register a test user id in the current per-test cleanup registry."""
    normalized_username = str(username or "").strip()
    if _TRACKED_TEST_USERS is None or not normalized_username:
        return None

    if not _is_cleanup_candidate(normalized_username):
        return None

    user = await db.get_user_by_username(normalized_username)
    if not user or user.get("id") is None:
        return None

    user_id = int(user["id"])
    tracked_entry = (db, user_id)
    if tracked_entry not in _TRACKED_TEST_USERS:
        _TRACKED_TEST_USERS.append(tracked_entry)
    return user_id


async def cleanup_registered_test_users_async(db: Any | None = None) -> list[int]:
    """Purge all currently tracked test users and reset the registry."""
    global _TRACKED_TEST_USERS  # pylint: disable=global-statement

    tracked_users = sorted(
        _TRACKED_TEST_USERS or [],
        key=lambda entry: entry[1],
        reverse=True,
    )
    cleaned_ids: list[int] = []

    try:
        for tracked_db, user_id in tracked_users:
            cleanup_db = tracked_db if tracked_db is not None else db
            if cleanup_db is None:
                continue
            if await purge_test_user_by_id_async(cleanup_db, user_id):
                cleaned_ids.append(user_id)
        return cleaned_ids
    finally:
        _TRACKED_TEST_USERS = None


async def purge_test_user_by_username_async(db: Any, username: str) -> bool:
    """Purge a non-protected test user by username."""
    normalized_username = str(username or "").strip()
    if not _is_cleanup_candidate(normalized_username):
        return False

    user = await db.get_user_by_username(normalized_username)
    if not user or user.get("id") is None:
        return False

    return await purge_test_user_by_id_async(db, int(user["id"]))


async def purge_test_user_by_id_async(db: Any, user_id: int) -> bool:
    """Purge one non-protected user and all of its user-scoped data."""
    if int(user_id) <= 0:
        return False

    user = await db.get_user_by_id(int(user_id))
    if not user:
        return False

    username = str(user.get("username") or "").strip()
    if not _is_cleanup_candidate(username):
        raise ValueError(f"Refusing to purge protected test user: {username}")

    clear_result = await db.clear_user_data(user_id=int(user_id))
    if not clear_result.get("success"):
        raise RuntimeError(
            "Failed to clear business data for test user "
            f"{username}: {clear_result}"
        )

    conn = await db._get_connection()  # pylint: disable=protected-access
    user_scoped_tables = await _list_user_scoped_tables(conn)

    for table_name in user_scoped_tables:
        if table_name == "users":
            continue
        await conn.execute(f'DELETE FROM "{table_name}" WHERE user_id = ?', (int(user_id),))

    cursor = await conn.execute("DELETE FROM users WHERE id = ?", (int(user_id),))
    await conn.commit()

    clear_cache = getattr(db, "_clear_cache", None)
    if callable(clear_cache):
        clear_cache()

    return bool(cursor.rowcount > 0)


async def _list_user_scoped_tables(conn: Any) -> list[str]:
    """Return all application tables that expose a user_id column."""
    async with conn.execute(
        "SELECT name FROM sqlite_master "
        "WHERE type = 'table' AND name NOT LIKE 'sqlite_%' "
        "ORDER BY name ASC"
    ) as cursor:
        table_rows = await cursor.fetchall()

    result: list[str] = []
    for row in table_rows:
        table_name = str(row[0] if not isinstance(row, dict) else row.get("name") or "").strip()
        if not table_name:
            continue

        async with conn.execute(f'PRAGMA table_info("{table_name}")') as table_info_cursor:
            columns = await table_info_cursor.fetchall()

        column_names = {
            str(column[1] if not isinstance(column, dict) else column.get("name") or "")
            for column in columns
        }
        if "user_id" in column_names:
            result.append(table_name)

    return result
