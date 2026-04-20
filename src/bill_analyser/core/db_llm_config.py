"""LLM configuration persistence helpers for the split database facade."""

from __future__ import annotations

from typing import Any

import aiosqlite

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseLLMConfigMixin(DatabaseFacadeBase):
    """CRUD helpers for persisted LLM provider configurations."""

    @log_method
    async def get_llm_configs(self, user_id: int = 1) -> list[dict[str, Any]]:
        """List all LLM configs for a user, ordered by is_active DESC, updated_at DESC."""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM llm_configs WHERE user_id = ? ORDER BY is_active DESC, updated_at DESC",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_active_llm_config(self, user_id: int = 1) -> dict[str, Any] | None:
        """Return the currently active LLM config, or None."""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM llm_configs WHERE user_id = ? AND is_active = 1 LIMIT 1",
            (user_id,),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    @log_method
    async def create_llm_config(
        self,
        user_id: int = 1,
        *,
        name: str,
        provider: str = "openai",
        model: str = "",
        api_key: str = "",
        base_url: str = "",
        is_active: bool = False,
    ) -> dict[str, Any]:
        """Create a new LLM config. If is_active, deactivate others first."""
        conn = await self._get_connection()
        now = utc_now_iso()

        if is_active:
            await conn.execute(
                "UPDATE llm_configs SET is_active = 0, updated_at = ? WHERE user_id = ? AND is_active = 1",
                (now, user_id),
            )

        cursor = await conn.execute(
            """INSERT INTO llm_configs (user_id, name, provider, model, api_key, base_url, is_active, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (user_id, name, provider, model, api_key, base_url, 1 if is_active else 0, now, now),
        )
        await conn.commit()
        config_id = cursor.lastrowid

        conn.row_factory = aiosqlite.Row
        async with conn.execute("SELECT * FROM llm_configs WHERE id = ?", (config_id,)) as cur:
            row = await cur.fetchone()
        return dict(row) if row else {"id": config_id}

    @log_method
    async def update_llm_config(
        self,
        config_id: int,
        user_id: int = 1,
        **fields: Any,
    ) -> dict[str, Any] | None:
        """Update an existing LLM config. Allowed fields: name, provider, model, api_key, base_url, is_active."""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        async with conn.execute(
            "SELECT * FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id)
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return None

        now = utc_now_iso()
        allowed = {"name", "provider", "model", "api_key", "base_url", "is_active"}
        updates: list[str] = ["updated_at = ?"]
        params: list[Any] = [now]

        for key, value in fields.items():
            if key in allowed:
                if key == "is_active":
                    value = 1 if value else 0
                updates.append(f"{key} = ?")
                params.append(value)

        # If activating this config, deactivate others
        if fields.get("is_active"):
            await conn.execute(
                "UPDATE llm_configs SET is_active = 0, updated_at = ? WHERE user_id = ? AND id != ?",
                (now, user_id, config_id),
            )

        params.extend([config_id, user_id])
        await conn.execute(
            f"UPDATE llm_configs SET {', '.join(updates)} WHERE id = ? AND user_id = ?",
            tuple(params),
        )
        await conn.commit()

        async with conn.execute("SELECT * FROM llm_configs WHERE id = ?", (config_id,)) as cur:
            row = await cur.fetchone()
        return dict(row) if row else None

    @log_method
    async def delete_llm_config(self, config_id: int, user_id: int = 1) -> bool:
        """Delete an LLM config. Returns False if not found."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT id FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id)
        ) as cursor:
            if not await cursor.fetchone():
                return False
        await conn.execute("DELETE FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id))
        await conn.commit()
        return True

    @log_method
    async def activate_llm_config(self, config_id: int, user_id: int = 1) -> bool:
        """Set a config as the active one, deactivating all others."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT id FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id)
        ) as cursor:
            if not await cursor.fetchone():
                return False
        now = utc_now_iso()
        await conn.execute(
            "UPDATE llm_configs SET is_active = 0, updated_at = ? WHERE user_id = ?",
            (now, user_id),
        )
        await conn.execute(
            "UPDATE llm_configs SET is_active = 1, updated_at = ? WHERE id = ? AND user_id = ?",
            (now, config_id, user_id),
        )
        await conn.commit()
        return True
