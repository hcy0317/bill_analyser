"""LLM configuration persistence helpers for the split database facade."""

from __future__ import annotations

import json
from typing import Any

import aiosqlite

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso

_PROMPT_TEXT_LIMIT = 12000
_ALLOWED_REASONING_DEPTHS = {"", "low", "medium", "high"}


def normalize_llm_advanced_settings(settings: Any) -> dict[str, Any]:
    """Normalize persisted LLM advanced settings into a safe JSON object."""
    if isinstance(settings, str):
        try:
            loaded_settings = json.loads(settings) if settings.strip() else {}
        except json.JSONDecodeError:
            loaded_settings = {}
    elif isinstance(settings, dict):
        loaded_settings = settings
    else:
        loaded_settings = {}

    if not isinstance(loaded_settings, dict):
        loaded_settings = {}

    normalized: dict[str, Any] = {}

    reasoning_depth = str(loaded_settings.get("reasoning_depth") or "").strip().lower()
    if reasoning_depth in {"auto", "default", "none"}:
        reasoning_depth = ""
    if reasoning_depth in _ALLOWED_REASONING_DEPTHS and reasoning_depth:
        normalized["reasoning_depth"] = reasoning_depth

    temperature = _normalize_float_setting(
        loaded_settings.get("temperature"),
        minimum=0.0,
        maximum=2.0,
    )
    if temperature is not None:
        normalized["temperature"] = temperature

    max_tokens = _normalize_int_setting(
        loaded_settings.get("max_tokens"),
        minimum=1,
        maximum=200000,
    )
    if max_tokens is not None:
        normalized["max_tokens"] = max_tokens

    for key in ("system_prompt", "classification_prompt_template", "rule_prompt_template"):
        prompt_text = _normalize_prompt_text(loaded_settings.get(key))
        if prompt_text:
            normalized[key] = prompt_text

    return normalized


def _normalize_float_setting(value: Any, *, minimum: float, maximum: float) -> float | None:
    if value in (None, ""):
        return None
    try:
        normalized = float(value)
    except (TypeError, ValueError):
        return None
    if normalized < minimum or normalized > maximum:
        return None
    return normalized


def _normalize_int_setting(value: Any, *, minimum: int, maximum: int) -> int | None:
    if value in (None, ""):
        return None
    try:
        normalized = int(value)
    except (TypeError, ValueError):
        return None
    if normalized < minimum or normalized > maximum:
        return None
    return normalized


def _normalize_prompt_text(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip()[:_PROMPT_TEXT_LIMIT]


def _serialize_llm_advanced_settings(settings: Any) -> str:
    return json.dumps(normalize_llm_advanced_settings(settings), ensure_ascii=False)


def _decode_llm_config_row(row: aiosqlite.Row | dict[str, Any] | None) -> dict[str, Any] | None:
    if not row:
        return None
    item = dict(row)
    item["advanced_settings"] = normalize_llm_advanced_settings(item.get("advanced_settings"))
    return item


class DatabaseLLMConfigMixin(DatabaseFacadeBase):
    """CRUD helpers for persisted LLM provider configurations."""

    # pylint: disable=too-many-arguments,too-many-locals
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
        return [item for row in rows if (item := _decode_llm_config_row(row)) is not None]

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
        return _decode_llm_config_row(row)

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
        advanced_settings: dict[str, Any] | str | None = None,
        is_active: bool = False,
    ) -> dict[str, Any]:
        """Create a new LLM config. If is_active, deactivate others first."""
        conn = await self._get_connection()
        now = utc_now_iso()
        serialized_advanced_settings = _serialize_llm_advanced_settings(advanced_settings)

        if is_active:
            await conn.execute(
                (
                    "UPDATE llm_configs SET is_active = 0, updated_at = ? "
                    "WHERE user_id = ? AND is_active = 1"
                ),
                (now, user_id),
            )

        cursor = await conn.execute(
            """INSERT INTO llm_configs (
                   user_id, name, provider, model, api_key, base_url,
                   advanced_settings, is_active, created_at, updated_at
               )
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                user_id,
                name,
                provider,
                model,
                api_key,
                base_url,
                serialized_advanced_settings,
                1 if is_active else 0,
                now,
                now,
            ),
        )
        await conn.commit()
        config_id = cursor.lastrowid

        conn.row_factory = aiosqlite.Row
        async with conn.execute("SELECT * FROM llm_configs WHERE id = ?", (config_id,)) as cur:
            row = await cur.fetchone()
        return _decode_llm_config_row(row) or {"id": config_id}

    @log_method
    async def update_llm_config(
        self,
        config_id: int,
        user_id: int = 1,
        **fields: Any,
    ) -> dict[str, Any] | None:
        """Update an existing LLM config.

        Allowed fields: name, provider, model, api_key, base_url, advanced_settings, is_active.
        """
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        async with conn.execute(
            "SELECT * FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id)
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return None

        now = utc_now_iso()
        allowed = {
            "name",
            "provider",
            "model",
            "api_key",
            "base_url",
            "advanced_settings",
            "is_active",
        }
        updates: list[str] = ["updated_at = ?"]
        params: list[Any] = [now]

        for key, value in fields.items():
            if key in allowed:
                if key == "is_active":
                    value = 1 if value else 0
                elif key == "advanced_settings":
                    value = _serialize_llm_advanced_settings(value)
                updates.append(f"{key} = ?")
                params.append(value)

        # If activating this config, deactivate others
        if fields.get("is_active"):
            await conn.execute(
                (
                    "UPDATE llm_configs SET is_active = 0, updated_at = ? "
                    "WHERE user_id = ? AND id != ?"
                ),
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
        return _decode_llm_config_row(row)

    @log_method
    async def delete_llm_config(self, config_id: int, user_id: int = 1) -> bool:
        """Delete an LLM config. Returns False if not found."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT id FROM llm_configs WHERE id = ? AND user_id = ?", (config_id, user_id)
        ) as cursor:
            if not await cursor.fetchone():
                return False
        await conn.execute(
            "DELETE FROM llm_configs WHERE id = ? AND user_id = ?",
            (config_id, user_id),
        )
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
