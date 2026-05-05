from __future__ import annotations

from datetime import UTC, datetime
from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core.database.users import auth as db_users_auth_module
from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_users_auth_paths.db"))
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


async def _count_rows(db: Database, query: str, params: tuple[Any, ...]) -> int:
    conn = await db._get_connection()
    async with conn.execute(query, params) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


def _build_timestamp_feeder(*timestamps: str):
    if not timestamps:
        raise ValueError("timestamps cannot be empty")

    remaining_values = iter(timestamps)
    last_value = timestamps[-1]

    def _next_timestamp() -> str:
        return next(remaining_values, last_value)

    return _next_timestamp


def _naive_utc_iso(year: int, month: int, day: int, hour: int, minute: int, second: int) -> str:
    return datetime(year, month, day, hour, minute, second, tzinfo=UTC).replace(tzinfo=None).isoformat()


@pytest.mark.asyncio
async def test_auth_logs_roundtrip_filters_limit_and_cleanup(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证日志应支持写入、按用户/事件筛选、按时间倒序返回，并清理过期记录。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "auth_log_user")
        other_user_id = await _create_user(db, "auth_log_other")
        monkeypatch.setattr(
            db_users_auth_module,
            "utc_now_iso",
            _build_timestamp_feeder(
                _naive_utc_iso(2026, 1, 1, 8, 0, 0),
                _naive_utc_iso(2026, 4, 5, 9, 30, 0),
                _naive_utc_iso(2026, 4, 6, 10, 45, 0),
                _naive_utc_iso(2026, 4, 7, 11, 15, 0),
            ),
        )

        await db.create_auth_log(
            {
                "user_id": user_id,
                "username": "auth_log_user",
                "event_type": "login",
                "ip_address": "10.0.0.1",
                "user_agent": "pytest-agent",
                "success": False,
                "error_message": "bad password",
                "metadata": '{"stage": "old"}',
            }
        )
        await db.create_auth_log(
            {
                "user_id": user_id,
                "username": "auth_log_user",
                "event_type": "login",
                "ip_address": "10.0.0.2",
                "user_agent": "pytest-agent",
                "success": True,
                "metadata": '{"stage": "recent"}',
            }
        )
        await db.create_auth_log(
            {
                "user_id": other_user_id,
                "username": "auth_log_other",
                "event_type": "logout",
                "ip_address": "10.0.0.3",
                "user_agent": "pytest-agent",
                "success": True,
                "metadata": '{"scope": "other-user"}',
            }
        )
        await db.create_auth_log(
            {
                "user_id": user_id,
                "username": "auth_log_user",
                "event_type": "password_reset",
                "ip_address": "10.0.0.4",
                "user_agent": "pytest-agent",
                "success": True,
                "metadata": '{"scope": "latest"}',
            }
        )

        latest_login = await db.get_auth_logs(user_id=user_id, event_type="login", limit=1)
        assert len(latest_login) == 1
        assert latest_login[0]["created_at"] == _naive_utc_iso(2026, 4, 5, 9, 30, 0)
        assert latest_login[0]["ip_address"] == "10.0.0.2"
        assert bool(latest_login[0]["success"]) is True

        user_logs = await db.get_auth_logs(user_id=user_id, limit=10)
        assert [log["event_type"] for log in user_logs] == ["password_reset", "login", "login"]
        assert [log["created_at"] for log in user_logs] == [
            _naive_utc_iso(2026, 4, 7, 11, 15, 0),
            _naive_utc_iso(2026, 4, 5, 9, 30, 0),
            _naive_utc_iso(2026, 1, 1, 8, 0, 0),
        ]

        monkeypatch.setattr(db_users_auth_module, "utc_now", lambda: datetime(2026, 4, 7, 12, 0, 0, tzinfo=UTC))
        deleted_count = await db.cleanup_old_auth_logs(days=30)
        assert deleted_count == 1

        remaining_logs = await db.get_auth_logs(limit=10)
        assert [log["event_type"] for log in remaining_logs] == ["password_reset", "logout", "login"]
        assert all(log["created_at"] != _naive_utc_iso(2026, 1, 1, 8, 0, 0) for log in remaining_logs)
        assert await _count_rows(db, "SELECT COUNT(*) FROM auth_logs", ()) == 3
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_cleanup_old_auth_logs_keeps_entries_exactly_on_cutoff(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证日志清理应删除早于 cutoff 的记录，但保留恰好等于 cutoff 的记录。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "auth_log_cutoff_user")
        monkeypatch.setattr(
            db_users_auth_module,
            "utc_now_iso",
            _build_timestamp_feeder(
                _naive_utc_iso(2026, 3, 8, 11, 59, 59),
                _naive_utc_iso(2026, 3, 8, 12, 0, 0),
            ),
        )

        await db.create_auth_log(
            {
                "user_id": user_id,
                "username": "auth_log_cutoff_user",
                "event_type": "login",
                "success": False,
            }
        )
        await db.create_auth_log(
            {
                "user_id": user_id,
                "username": "auth_log_cutoff_user",
                "event_type": "login",
                "success": True,
            }
        )

        monkeypatch.setattr(db_users_auth_module, "utc_now", lambda: datetime(2026, 4, 7, 12, 0, 0, tzinfo=UTC))
        deleted_count = await db.cleanup_old_auth_logs(days=30)
        assert deleted_count == 1

        remaining_logs = await db.get_auth_logs(user_id=user_id, event_type="login", limit=10)
        assert len(remaining_logs) == 1
        assert remaining_logs[0]["created_at"] == _naive_utc_iso(2026, 3, 8, 12, 0, 0)
        assert bool(remaining_logs[0]["success"]) is True
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_app_setting_roundtrip_upsert_and_read_latest_value(tmp_path: Path) -> None:
    """应用设置应支持缺失返回 None、单行 upsert 与最新值读取。"""
    db = await _create_database(tmp_path)
    try:
        assert await db.get_app_setting("missing-setting") is None

        assert (
            await db.set_app_setting(
                "operation_password",
                "first-secret",
                value_type="secret",
                description="first description",
                is_encrypted=True,
            )
            is True
        )
        assert (
            await db.set_app_setting(
                "operation_password",
                "updated-secret",
                value_type="password",
                description="updated description",
                is_encrypted=False,
            )
            is True
        )

        assert await db.get_app_setting("operation_password") == "updated-secret"
        assert await _count_rows(db, "SELECT COUNT(*) FROM app_settings WHERE key = ?", ("operation_password",)) == 1

        conn = await db._get_connection()
        async with conn.execute(
            "SELECT value, value_type, description, is_encrypted FROM app_settings WHERE key = ?",
            ("operation_password",),
        ) as cursor:
            row = await cursor.fetchone()
        assert row is not None
        assert row["value"] == "updated-secret"
        assert row["value_type"] == "password"
        assert row["description"] == "updated description"
        assert row["is_encrypted"] == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_user_external_auth_upsert_fetch_and_delete_are_user_scoped(tmp_path: Path) -> None:
    """第三方登录绑定应支持 upsert、按用户查询，并保持不同用户互不影响。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "external_auth_user")
        other_user_id = await _create_user(db, "external_auth_other")

        external_auth_id = await db.create_user_external_auth(
            {
                "user_id": user_id,
                "external_auth_category": "social",
                "external_auth_type": "wechat",
                "external_user_id": "wx-user-1",
                "external_username": "wechat-user",
            }
        )
        await db.create_user_external_auth(
            {
                "user_id": user_id,
                "external_auth_category": "social",
                "external_auth_type": "wechat",
                "external_user_id": "wx-user-1-updated",
                "external_username": "wechat-user-updated",
            }
        )
        await db.create_user_external_auth(
            {
                "user_id": other_user_id,
                "external_auth_category": "social",
                "external_auth_type": "wechat",
                "external_user_id": "wx-user-2",
                "external_username": "wechat-user-other",
            }
        )

        assert external_auth_id > 0

        user_external_auth = await db.get_user_external_auth(user_id, "wechat")
        assert user_external_auth is not None
        assert user_external_auth["external_user_id"] == "wx-user-1-updated"
        assert user_external_auth["external_username"] == "wechat-user-updated"

        user_external_auths = await db.get_user_external_auths(user_id)
        assert len(user_external_auths) == 1
        assert user_external_auths[0]["external_auth_type"] == "wechat"

        other_external_auths = await db.get_user_external_auths(other_user_id)
        assert len(other_external_auths) == 1
        assert other_external_auths[0]["external_user_id"] == "wx-user-2"

        assert await db.delete_user_external_auth(user_id, "wechat") is True
        assert await db.delete_user_external_auth(user_id, "wechat") is False
        assert await db.get_user_external_auth(user_id, "wechat") is None
        assert len(await db.get_user_external_auths(other_user_id)) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_user_application_cloud_settings_full_update_replaces_missing_keys(tmp_path: Path) -> None:
    """应用云设置应归一化键名、支持全量更新删除缺失项，并可按用户清空。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "cloud_settings_user")
        other_user_id = await _create_user(db, "cloud_settings_other")

        assert (
            await db.update_user_application_cloud_settings(
                user_id,
                [
                    {"setting_key": " theme ", "setting_value": "dark"},
                    {"setting_key": "auto_sync", "setting_value": "enabled"},
                    {"setting_key": "   ", "setting_value": "ignored"},
                ],
            )
            is True
        )
        assert (
            await db.update_user_application_cloud_settings(
                other_user_id,
                [{"setting_key": "theme", "setting_value": "light"}],
            )
            is True
        )

        initial_settings = await db.get_user_application_cloud_settings(user_id)
        assert len(initial_settings) == 2
        assert {item["setting_key"]: item["setting_value"] for item in initial_settings} == {
            "theme": "dark",
            "auto_sync": "enabled",
        }

        assert (
            await db.update_user_application_cloud_settings(
                user_id,
                [{"setting_key": "theme", "setting_value": "system"}],
                full_update=True,
            )
            is True
        )

        updated_settings = await db.get_user_application_cloud_settings(user_id)
        assert [(item["setting_key"], item["setting_value"]) for item in updated_settings] == [
            ("theme", "system"),
        ]

        other_settings = await db.get_user_application_cloud_settings(other_user_id)
        assert len(other_settings) == 1
        assert {item["setting_key"]: item["setting_value"] for item in other_settings} == {
            "theme": "light",
        }

        assert await db.delete_user_application_cloud_settings(user_id) is True
        assert await db.get_user_application_cloud_settings(user_id) == []
        assert len(await db.get_user_application_cloud_settings(other_user_id)) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_verify_operation_password_prefers_env_then_db_then_unset_fallback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """操作密码校验应优先环境变量，其次 DB 设置，最后保留未配置时的兼容放行契约。"""
    db = await _create_database(tmp_path)
    try:
        await db.set_app_setting("operation_password", "db-secret")

        monkeypatch.setenv("BILL_ANALYSER_OPERATION_PASSWORD", "env-secret")
        assert await db.verify_operation_password("env-secret") is True
        assert await db.verify_operation_password("db-secret") is False

        monkeypatch.delenv("BILL_ANALYSER_OPERATION_PASSWORD", raising=False)
        assert await db.verify_operation_password("db-secret") is True
        assert await db.verify_operation_password("wrong-secret") is False

        conn = await db._get_connection()
        await conn.execute("DELETE FROM app_settings WHERE key = ?", ("operation_password",))
        await conn.commit()

        assert await db.verify_operation_password("anything-goes") is True
    finally:
        monkeypatch.delenv("BILL_ANALYSER_OPERATION_PASSWORD", raising=False)
        await db.close()
