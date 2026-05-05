"""Historical bill matching persistence helpers."""

from __future__ import annotations

import json
import sqlite3
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import parse_bill_datetime
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.investment.matching import score_investment_candidate
from bill_analyser.core.investment.settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from bill_analyser.core.matching import build_transfer_pair_candidate, build_transfer_pair_candidates
from bill_analyser.core.matching.candidate_ids import build_learning_rule_revision, normalize_learning_rule_revision
from .base import MatchingBaseMixin

_UNSET = MatchingBaseMixin._UNSET


class MatchingSettingsMixin(object):
        async def _get_user_investment_keyword_config(
            self,
            *,
            user_id: int = 1,
            conn: Any | None = None,
        ) -> dict[str, list[str]]:
            active_conn = conn or await self._get_connection()
            async with active_conn.execute(
                """
                SELECT investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
                FROM users
                WHERE id = ?
                LIMIT 1
                """,
                (user_id,),
            ) as cursor:
                user_row = await cursor.fetchone()
            return build_user_investment_keyword_settings(
                dict(user_row) if user_row else None,
            )

        @log_method
        async def get_pairing_investment_settings(
            self,
            *,
            user_id: int = 1,
            conn: Any | None = None,
        ) -> dict[str, Any] | None:
            """Return pairing-center investment settings backed by the current user row."""
            normalized_user_id = int(user_id)
            if normalized_user_id <= 0:
                return None

            active_conn = conn or await self._get_connection()
            async with active_conn.execute(
                """
                SELECT id, import_learning_enabled,
                       investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
                FROM users
                WHERE id = ?
                LIMIT 1
                """,
                (normalized_user_id,),
            ) as cursor:
                user_row = await cursor.fetchone()

            if not user_row:
                return None

            row_dict = dict(user_row)
            keyword_settings = build_user_investment_keyword_settings(row_dict)
            return {
                "user_id": normalized_user_id,
                "import_learning_enabled": bool(row_dict.get("import_learning_enabled", True)),
                "investment_platform_keywords": keyword_settings["platform_keywords"],
                "investment_product_keywords": keyword_settings["product_keywords"],
                "investment_exclude_keywords": keyword_settings["exclude_keywords"],
            }

        @log_method
        async def update_pairing_investment_settings(
            self,
            *,
            user_id: int = 1,
            import_learning_enabled: Any = _UNSET,
            investment_platform_keywords: Any = _UNSET,
            investment_product_keywords: Any = _UNSET,
            investment_exclude_keywords: Any = _UNSET,
        ) -> dict[str, Any] | None:
            """Update pairing-center investment settings while keeping storage on ``users``."""
            normalized_user_id = int(user_id)
            if normalized_user_id <= 0:
                return None

            update_data: dict[str, Any] = {}
            if import_learning_enabled is not self._UNSET:
                update_data["import_learning_enabled"] = 1 if bool(import_learning_enabled) else 0
            if investment_platform_keywords is not self._UNSET:
                update_data["investment_platform_keywords"] = serialize_keyword_list(investment_platform_keywords)
            if investment_product_keywords is not self._UNSET:
                update_data["investment_product_keywords"] = serialize_keyword_list(investment_product_keywords)
            if investment_exclude_keywords is not self._UNSET:
                update_data["investment_exclude_keywords"] = serialize_keyword_list(investment_exclude_keywords)

            conn = await self._get_connection()
            try:
                await conn.execute("BEGIN IMMEDIATE")

                async with conn.execute(
                    "SELECT id FROM users WHERE id = ? LIMIT 1",
                    (normalized_user_id,),
                ) as cursor:
                    user_row = await cursor.fetchone()
                if not user_row:
                    await conn.rollback()
                    return None

                if update_data:
                    set_clause = ", ".join(f"{field_name} = ?" for field_name in update_data)
                    cursor = await conn.execute(
                        f"UPDATE users SET {set_clause} WHERE id = ?",
                        [*update_data.values(), normalized_user_id],
                    )
                    if int(cursor.rowcount or 0) != 1:
                        await conn.rollback()
                        return None

                settings = await self.get_pairing_investment_settings(
                    user_id=normalized_user_id,
                    conn=conn,
                )
                await conn.commit()
                return settings
            except Exception:
                await conn.rollback()
                raise
