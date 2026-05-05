"""User, session, 2FA, and app-setting persistence helpers."""

# pylint: disable=missing-function-docstring,line-too-long,too-many-arguments,too-many-positional-arguments,broad-exception-caught,too-many-public-methods

from __future__ import annotations

import hashlib
import os
from datetime import datetime, timedelta
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class DatabaseUsersAuthMixin(DatabaseFacadeBase):
    """User profile, auth, session, 2FA, and app-setting helpers."""

    @staticmethod
    def _normalize_two_factor_recovery_code(recovery_code: str | None) -> str:
        if recovery_code is None:
            return ""
        return "".join(str(recovery_code).strip().upper().split())

    @classmethod
    def _hash_two_factor_recovery_code(cls, recovery_code: str | None) -> str:
        normalized_code = cls._normalize_two_factor_recovery_code(recovery_code)
        if not normalized_code:
            return ""
        return hashlib.sha256(f"2fa-recovery:{normalized_code}".encode()).hexdigest()

    @log_method
    async def create_user(self, data: dict[str, Any]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            INSERT INTO users (
                username, email, password_hash, nickname, avatar,
                language, default_currency, first_day_of_week,
                is_active, email_verified, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("username"),
                data.get("email"),
                data.get("password_hash"),
                data.get("nickname", data.get("username")),
                data.get("avatar", ""),
                data.get("language", "zh_Hans"),
                data.get("default_currency", "CNY"),
                data.get("first_day_of_week", 1),
                data.get("is_active", 1),
                data.get("email_verified", 0),
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def get_user_by_username(self, username: str) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM users WHERE username = ?", (username,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_email(self, email: str) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM users WHERE email = ?", (email,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
        if not data:
            return False
        conn = await self._get_connection()
        update_data = {**data, "updated_at": utc_now_iso()}
        set_clause = ", ".join(f"{key} = ?" for key in update_data)
        cursor = await conn.execute(
            f"UPDATE users SET {set_clause} WHERE id = ?",
            [*update_data.values(), user_id],
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def create_user_external_auth(self, data: dict[str, Any]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            INSERT INTO user_external_auths (
                user_id, external_auth_category, external_auth_type,
                external_user_id, external_username, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, external_auth_type) DO UPDATE SET
                external_auth_category = excluded.external_auth_category,
                external_user_id = excluded.external_user_id,
                external_username = excluded.external_username,
                updated_at = excluded.updated_at
            """,
            (
                data.get("user_id"),
                data.get("external_auth_category"),
                data.get("external_auth_type"),
                data.get("external_user_id"),
                data.get("external_username"),
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def get_user_external_auths(self, user_id: int) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM user_external_auths
            WHERE user_id = ?
            ORDER BY created_at DESC, id DESC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_user_external_auth(self, user_id: int, external_auth_type: str) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM user_external_auths
            WHERE user_id = ? AND external_auth_type = ?
            LIMIT 1
            """,
            (user_id, external_auth_type),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def delete_user_external_auth(self, user_id: int, external_auth_type: str) -> bool:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "DELETE FROM user_external_auths WHERE user_id = ? AND external_auth_type = ?",
            (user_id, external_auth_type),
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def get_user_application_cloud_settings(self, user_id: int) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT setting_key, setting_value, created_at, updated_at
            FROM user_application_cloud_settings
            WHERE user_id = ?
            ORDER BY created_at ASC, id ASC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def update_user_application_cloud_settings(
        self,
        user_id: int,
        settings: list[dict[str, Any]],
        full_update: bool = False,
    ) -> bool:
        conn = await self._get_connection()
        now = utc_now_iso()
        normalized_settings = [
            {
                "setting_key": str(setting.get("setting_key", "") or "").strip(),
                "setting_value": str(setting.get("setting_value", "") or ""),
            }
            for setting in settings
            if str(setting.get("setting_key", "") or "").strip()
        ]

        if full_update:
            if normalized_settings:
                keep_keys = [setting["setting_key"] for setting in normalized_settings]
                placeholders = ",".join("?" for _ in keep_keys)
                await conn.execute(
                    f"DELETE FROM user_application_cloud_settings WHERE user_id = ? "
                    f"AND setting_key NOT IN ({placeholders})",
                    [user_id, *keep_keys],
                )
            else:
                await conn.execute("DELETE FROM user_application_cloud_settings WHERE user_id = ?", (user_id,))

        for setting in normalized_settings:
            await conn.execute(
                """
                INSERT INTO user_application_cloud_settings (
                    user_id, setting_key, setting_value, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(user_id, setting_key) DO UPDATE SET
                    setting_value = excluded.setting_value,
                    updated_at = excluded.updated_at
                """,
                (user_id, setting["setting_key"], setting["setting_value"], now, now),
            )

        await conn.commit()
        return True

    @log_method
    async def delete_user_application_cloud_settings(self, user_id: int) -> bool:
        conn = await self._get_connection()
        await conn.execute("DELETE FROM user_application_cloud_settings WHERE user_id = ?", (user_id,))
        await conn.commit()
        return True

    @log_method
    async def update_user_last_login(self, user_id: int, ip_address: str | None = None) -> None:
        conn = await self._get_connection()
        await conn.execute(
            """
            UPDATE users
            SET last_login_at = ?, last_login_ip = ?, failed_login_attempts = 0, locked_until = NULL
            WHERE id = ?
            """,
            (utc_now_iso(), ip_address, user_id),
        )
        await conn.commit()

    @log_method
    async def increment_failed_login(self, user_id: int, lockout_minutes: int = 15) -> bool:
        conn = await self._get_connection()
        async with conn.execute("SELECT failed_login_attempts FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
        if not row:
            return False

        failed_attempts = int(row[0] or 0) + 1
        if failed_attempts >= 5:
            locked_until = (utc_now() + timedelta(minutes=lockout_minutes)).replace(tzinfo=None).isoformat()
            await conn.execute(
                "UPDATE users SET failed_login_attempts = ?, locked_until = ? WHERE id = ?",
                (failed_attempts, locked_until, user_id),
            )
        else:
            await conn.execute(
                "UPDATE users SET failed_login_attempts = ? WHERE id = ?",
                (failed_attempts, user_id),
            )
        await conn.commit()
        return True

    @log_method
    async def is_user_locked(self, user_id: int) -> bool:
        conn = await self._get_connection()
        async with conn.execute("SELECT locked_until FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
        if not row or not row[0]:
            return False

        locked_until = datetime.fromisoformat(row[0])
        if utc_now().replace(tzinfo=None) < locked_until:
            return True

        await conn.execute(
            "UPDATE users SET locked_until = NULL, failed_login_attempts = 0 WHERE id = ?",
            (user_id,),
        )
        await conn.commit()
        return False

    @log_method
    async def create_session(self, data: dict[str, Any]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            INSERT INTO sessions (
                user_id, token_hash, refresh_token_hash,
                expires_at, refresh_expires_at,
                user_agent, ip_address, is_active,
                last_activity_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("user_id"),
                data.get("token_hash"),
                data.get("refresh_token_hash"),
                data.get("expires_at"),
                data.get("refresh_expires_at"),
                data.get("user_agent"),
                data.get("ip_address"),
                1,
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def get_session_by_token_hash(self, token_hash: str) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT s.*, u.username, u.email, u.is_active as user_is_active
            FROM sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token_hash = ? AND s.is_active = 1
            """,
            (token_hash,),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_sessions(self, user_id: int) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM sessions WHERE user_id = ? AND is_active = 1 ORDER BY created_at DESC",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def update_session_activity(self, session_id: int) -> None:
        conn = await self._get_connection()
        await conn.execute(
            "UPDATE sessions SET last_activity_at = ? WHERE id = ?",
            (utc_now_iso(), session_id),
        )
        await conn.commit()

    @log_method
    async def invalidate_session(self, token_hash: str) -> bool:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "UPDATE sessions SET is_active = 0 WHERE token_hash = ?",
            (token_hash,),
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def invalidate_user_sessions(self, user_id: int) -> int:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "UPDATE sessions SET is_active = 0 WHERE user_id = ?",
            (user_id,),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def invalidate_session_by_id(self, session_id: int, user_id: int) -> bool:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "UPDATE sessions SET is_active = 0 WHERE id = ? AND user_id = ?",
            (session_id, user_id),
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def invalidate_other_user_sessions(self, user_id: int, current_session_id: int) -> int:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "UPDATE sessions SET is_active = 0 WHERE user_id = ? AND id != ? AND is_active = 1",
            (user_id, current_session_id),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def cleanup_expired_sessions(self) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            DELETE FROM sessions
            WHERE expires_at < ?
               OR (refresh_expires_at IS NOT NULL AND refresh_expires_at < ?)
            """,
            (now, now),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def create_auth_log(self, data: dict[str, Any]) -> None:
        conn = await self._get_connection()
        await conn.execute(
            """
            INSERT INTO auth_logs (
                user_id, username, event_type, ip_address, user_agent,
                success, error_message, metadata, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("user_id"),
                data.get("username"),
                data.get("event_type"),
                data.get("ip_address"),
                data.get("user_agent"),
                data.get("success", False),
                data.get("error_message"),
                data.get("metadata"),
                utc_now_iso(),
            ),
        )
        await conn.commit()

    @log_method
    async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            seen_hashes: set[str] = set()
            hashed_payloads: list[tuple[int, str, str, str]] = []
            for recovery_code in recovery_codes:
                code_hash = self._hash_two_factor_recovery_code(recovery_code)
                if not code_hash or code_hash in seen_hashes:
                    continue
                seen_hashes.add(code_hash)
                hashed_payloads.append((user_id, code_hash, now, now))

            await conn.execute("DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?", (user_id,))
            if hashed_payloads:
                await conn.executemany(
                    """
                    INSERT INTO user_two_factor_recovery_codes (
                        user_id, code_hash, created_at, updated_at
                    ) VALUES (?, ?, ?, ?)
                    """,
                    hashed_payloads,
                )
            await conn.commit()
            return len(hashed_payloads)
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def consume_two_factor_recovery_code(self, user_id: int, recovery_code: str) -> bool:
        code_hash = self._hash_two_factor_recovery_code(recovery_code)
        if not code_hash:
            return False

        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            UPDATE user_two_factor_recovery_codes
            SET used_at = ?, updated_at = ?
            WHERE user_id = ? AND code_hash = ? AND used_at IS NULL
            """,
            (now, now, user_id, code_hash),
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def clear_two_factor_recovery_codes(self, user_id: int) -> int:
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?", (user_id,))
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def count_active_two_factor_recovery_codes(self, user_id: int) -> int:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT COUNT(*) AS count FROM user_two_factor_recovery_codes WHERE user_id = ? AND used_at IS NULL",
            (user_id,),
        ) as cursor:
            row = await cursor.fetchone()
        return int((row["count"] if row else 0) or 0)

    @log_method
    async def get_auth_logs(
        self,
        user_id: int | None = None,
        event_type: str | None = None,
        limit: int = 100,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM auth_logs WHERE 1=1"
        params: list[Any] = []
        if user_id:
            query += " AND user_id = ?"
            params.append(user_id)
        if event_type:
            query += " AND event_type = ?"
            params.append(event_type)
        query += " ORDER BY created_at DESC LIMIT ?"
        params.append(limit)

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def cleanup_old_auth_logs(self, days: int = 90) -> int:
        conn = await self._get_connection()
        cutoff_date = (utc_now() - timedelta(days=days)).replace(tzinfo=None).isoformat()
        cursor = await conn.execute("DELETE FROM auth_logs WHERE created_at < ?", (cutoff_date,))
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def get_app_setting(self, key: str) -> str | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT value, is_encrypted FROM app_settings WHERE key = ?", (key,)) as cursor:
            row = await cursor.fetchone()
        if not row:
            return None
        return row["value"]

    @log_method
    async def set_app_setting(
        self,
        key: str,
        value: str,
        value_type: str = "string",
        description: str | None = None,
        is_encrypted: bool = False,
    ) -> bool:
        conn = await self._get_connection()
        now = utc_now_iso()
        encrypted_value = value
        try:
            await conn.execute(
                """
                INSERT INTO app_settings (key, value, value_type, description, is_encrypted, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    value_type = excluded.value_type,
                    description = excluded.description,
                    is_encrypted = excluded.is_encrypted,
                    updated_at = excluded.updated_at
                """,
                (key, encrypted_value, value_type, description, is_encrypted, now, now),
            )
            await conn.commit()
            return True
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("设置应用配置失败: key=%s, error=%s", key, exc)
            return False

    @log_method
    async def verify_operation_password(self, password: str) -> bool:
        env_password = os.getenv("BILL_ANALYSER_OPERATION_PASSWORD")
        if env_password:
            return password == env_password

        stored_password = await self.get_app_setting("operation_password")
        if not stored_password:
            self.logger.warning("未配置操作密码，默认允许操作（不安全！）")
            return True
        return password == stored_password
