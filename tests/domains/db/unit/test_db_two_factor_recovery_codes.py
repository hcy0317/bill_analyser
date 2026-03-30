from __future__ import annotations

from pathlib import Path

import pytest

from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_two_factor_recovery_codes.db"))
    await db.init_db()
    return db


async def _create_user(db: Database, username: str) -> int:
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "hashed-password",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


@pytest.mark.asyncio
async def test_replace_and_consume_two_factor_recovery_codes_is_single_use(tmp_path: Path) -> None:
    """恢复码应哈希持久化，并且每个码只能成功消费一次。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "recovery_single_use")

        replaced_count = await db.replace_two_factor_recovery_codes(user_id, ["ABCD-1234", "EFGH-5678"])
        assert replaced_count == 2
        assert await db.count_active_two_factor_recovery_codes(user_id) == 2

        assert await db.consume_two_factor_recovery_code(user_id, "abcd-1234") is True
        assert await db.consume_two_factor_recovery_code(user_id, "ABCD-1234") is False
        assert await db.consume_two_factor_recovery_code(user_id, "NOT-EXIST") is False
        assert await db.count_active_two_factor_recovery_codes(user_id) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_replace_two_factor_recovery_codes_invalidates_previous_batch(tmp_path: Path) -> None:
    """重新生成恢复码后，旧批次应立即失效，仅新批次仍可使用。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "recovery_replace_batch")

        await db.replace_two_factor_recovery_codes(user_id, ["OLD1-1111", "OLD2-2222"])
        await db.replace_two_factor_recovery_codes(user_id, ["NEW1-3333", "NEW2-4444"])

        assert await db.consume_two_factor_recovery_code(user_id, "OLD1-1111") is False
        assert await db.consume_two_factor_recovery_code(user_id, "NEW1-3333") is True
        assert await db.count_active_two_factor_recovery_codes(user_id) == 1

        cleared_count = await db.clear_two_factor_recovery_codes(user_id)
        assert cleared_count == 2
        assert await db.count_active_two_factor_recovery_codes(user_id) == 0
    finally:
        await db.close()
