"""Category-rules persistence helpers for the split database facade."""

from __future__ import annotations

import sqlite3
from typing import Any

import aiosqlite

from ..utils.logger import log_method
from .category_engine import escape_rule_expression_term
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


_LEGACY_RULE_PREFIXES = ("OR:", "AND:", "NOT:", "REGEX:")
_LEGACY_RULE_OPERATORS = (
    ("OR:", "OR", "|"),
    ("AND:", "AND", "|"),
    ("NOT:", "NOT", "|"),
    ("REGEX:", "REGEX", None),
)


def _split_legacy_delimited_text(text: str, separator: str) -> list[str]:
    """Split legacy keyword text while allowing ``\\|``/``\\&`` literals."""
    parts: list[str] = []
    current: list[str] = []
    escape_pending = False

    for char in text:
        if escape_pending:
            if char == separator:
                current.append(separator)
            else:
                current.extend(["\\", char])
            escape_pending = False
            continue
        if char == "\\":
            escape_pending = True
            continue
        if char == separator:
            parts.append("".join(current))
            current = []
            continue
        current.append(char)

    if escape_pending:
        current.append("\\")
    parts.append("".join(current))
    return parts


def _has_legacy_rule_prefix(keyword_text: str) -> bool:
    """Return whether old keyword text uses legacy operator prefixes."""
    return any(
        part.strip().upper().startswith(_LEGACY_RULE_PREFIXES)
        for part in _split_legacy_delimited_text(keyword_text, "&")
    )


def _format_rule_clause(operator: str, keywords: list[str]) -> str:
    """Format escaped terms as one rule-expression clause."""
    escaped_keywords = [
        escape_rule_expression_term(keyword.strip())
        for keyword in keywords
        if keyword.strip()
    ]
    if not escaped_keywords:
        return ""
    return f"{operator}=" + "{" + ",".join(escaped_keywords) + "}"


class DatabaseCategoryRulesMixin(DatabaseFacadeBase):
    """Category rules CRUD, reorder, and keyword migration helpers."""

    @log_method
    async def get_category_rules(
        self,
        user_id: int = 1,
        category_id: int | None = None,
        enabled_only: bool = True,
        include_category_priority: bool = False,
    ) -> list[dict[str, Any]]:
        """获取分类规则列表。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        select_fields = (
            "SELECT cr.*, c.main_category, c.sub_category, "
            "c.type AS category_type "
        )
        if include_category_priority:
            select_fields = (
                "SELECT cr.*, c.main_category, c.sub_category, "
                "c.type AS category_type, c.priority AS category_priority "
            )
        query = select_fields + (
            "FROM category_rules cr "
            "JOIN categories c ON cr.category_id = c.id "
            "WHERE cr.user_id = ?"
        )
        params: list[Any] = [user_id]

        if category_id is not None:
            query += " AND cr.category_id = ?"
            params.append(category_id)

        if enabled_only:
            query += " AND cr.enabled = 1"

        if include_category_priority:
            query += " ORDER BY c.priority ASC, c.id ASC, cr.id ASC"
        else:
            query += " ORDER BY cr.priority ASC, cr.id ASC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_category_rule(
        self,
        rule_id: int,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """获取单条分类规则。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        async with conn.execute(
            (
                "SELECT cr.*, c.main_category, c.sub_category, c.type AS category_type "
                "FROM category_rules cr "
                "JOIN categories c ON cr.category_id = c.id "
                "WHERE cr.id = ? AND cr.user_id = ?"
            ),
            (rule_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_category_rule(
        self,
        data: dict[str, Any],
        user_id: int = 1,
    ) -> int | None:
        """创建分类规则。"""
        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            cursor = await conn.execute(
                (
                    "INSERT INTO category_rules "
                    "(user_id, category_id, name, priority, rule_expression, "
                    "regex_enabled, enabled, created_at, updated_at) "
                    "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                ),
                (
                    user_id,
                    data["category_id"],
                    data.get("name", ""),
                    data.get("priority", 100),
                    data["rule_expression"],
                    data.get("regex_enabled", False),
                    data.get("enabled", True),
                    now,
                    now,
                ),
            )
            await conn.commit()
            rule_id = cursor.lastrowid
            self.logger.info("创建分类规则成功: ID=%s (user_id=%s)", rule_id, user_id)
            return rule_id
        except sqlite3.IntegrityError as exc:
            self.logger.warning("创建分类规则失败（完整性约束）: %s", exc)
            return None
        except sqlite3.Error as exc:
            self.logger.error("创建分类规则失败: %s", exc, exc_info=True)
            return None

    @log_method
    async def update_category_rule(
        self,
        rule_id: int,
        updates: dict[str, Any],
        user_id: int = 1,
    ) -> bool:
        """更新分类规则。"""
        conn = await self._get_connection()
        valid_fields = [
            "category_id",
            "name",
            "priority",
            "rule_expression",
            "regex_enabled",
            "enabled",
        ]
        safe_updates = {k: v for k, v in updates.items() if k in valid_fields}
        if not safe_updates:
            return False

        safe_updates["updated_at"] = utc_now_iso()
        set_clause = ", ".join(f"{k} = ?" for k in safe_updates)
        values = [*safe_updates.values(), rule_id, user_id]
        try:
            cursor = await conn.execute(
                f"UPDATE category_rules SET {set_clause} WHERE id = ? AND user_id = ?",
                values,
            )
            await conn.commit()
            if cursor.rowcount == 0:
                self.logger.warning(
                    "分类规则 ID %s 不存在或不属于用户 %s", rule_id, user_id
                )
                return False
            self.logger.info("已更新分类规则 ID: %s (user_id=%s)", rule_id, user_id)
            return True
        except sqlite3.Error as exc:
            await conn.rollback()
            self.logger.error("更新分类规则失败: %s", exc, exc_info=True)
            raise

    @log_method
    async def delete_category_rule(
        self,
        rule_id: int,
        user_id: int = 1,
    ) -> bool:
        """删除分类规则。"""
        conn = await self._get_connection()
        try:
            cursor = await conn.execute(
                "DELETE FROM category_rules WHERE id = ? AND user_id = ?",
                (rule_id, user_id),
            )
            await conn.commit()
            deleted = cursor.rowcount > 0
            self.logger.info(
                "删除分类规则: ID=%s, user_id=%s, deleted=%s",
                rule_id,
                user_id,
                deleted,
            )
            return deleted
        except sqlite3.Error as exc:
            self.logger.error("删除分类规则失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def reorder_category_rules(
        self,
        rule_ids: list[int],
        user_id: int = 1,
    ) -> bool:
        """按顺序重新设置规则优先级。"""
        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            for priority, rid in enumerate(rule_ids, start=1):
                await conn.execute(
                    "UPDATE category_rules SET priority = ?, updated_at = ? "
                    "WHERE id = ? AND user_id = ?",
                    (priority, now, rid, user_id),
                )
            await conn.commit()
            self.logger.info(
                "已重排 %d 条分类规则 (user_id=%s)", len(rule_ids), user_id
            )
            return True
        except sqlite3.Error as exc:
            await conn.rollback()
            self.logger.error("重排分类规则失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def migrate_keywords_to_rules(
        self,
        user_id: int = 1,
    ) -> dict[str, int]:
        """将旧 categories.keywords 迁移为 category_rules 行。

        旧语法: ``OR:k1|k2&AND:k3&NOT:k4``
        新语法: ``OR={k1,k2}+AND={k3}+NOT={k4}``

        Returns:
            ``{"migrated": n, "skipped": n}``
        """
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        async with conn.execute(
            (
                "SELECT * FROM categories "
                "WHERE user_id = ? AND keywords IS NOT NULL AND keywords != ''"
            ),
            (user_id,),
        ) as cursor:
            categories = [dict(row) for row in await cursor.fetchall()]

        migrated = 0
        skipped = 0
        now = utc_now_iso()

        for cat in categories:
            cat_id = cat["id"]
            old_kw: str = cat["keywords"]
            new_expr = self._convert_old_keyword_syntax(old_kw)

            try:
                async with conn.execute(
                    (
                        "SELECT 1 FROM category_rules "
                        "WHERE user_id = ? AND category_id = ? "
                        "AND rule_expression = ? AND regex_enabled = 0 "
                        "LIMIT 1"
                    ),
                    (
                        user_id,
                        cat_id,
                        new_expr,
                    ),
                ) as duplicate_cursor:
                    if await duplicate_cursor.fetchone() is not None:
                        skipped += 1
                        continue

                await conn.execute(
                    (
                        "INSERT INTO category_rules "
                        "(user_id, category_id, name, priority, rule_expression, "
                        "regex_enabled, enabled, created_at, updated_at) "
                        "VALUES (?, ?, ?, ?, ?, 0, 1, ?, ?)"
                    ),
                    (
                        user_id,
                        cat_id,
                        f"migrated:{cat.get('main_category', '')}/{cat.get('sub_category', '')}",
                        cat.get("priority", 0),
                        new_expr,
                        now,
                        now,
                    ),
                )
                migrated += 1
            except sqlite3.Error as exc:
                self.logger.warning("迁移分类 %s 的关键词失败: %s", cat_id, exc)
                skipped += 1

        await conn.commit()
        self.logger.info(
            "关键词迁移完成: migrated=%d, skipped=%d (user_id=%s)",
            migrated,
            skipped,
            user_id,
        )
        return {"migrated": migrated, "skipped": skipped}

    @staticmethod
    def _convert_old_keyword_syntax(old_kw: str) -> str:
        """Convert old keyword syntax to new rule_expression syntax.

        Old: ``OR:k1|k2&AND:k3&NOT:k4`` or plain ``keyword``
        New: ``OR={k1,k2}+AND={k3}+NOT={k4}``

        Literal terms are escaped when serialized so legacy keywords containing
        expression delimiters such as ``,``, ``+``, ``×``, ``/``, ``{}``, or
        ``|`` remain a single literal after migration.
        """
        if not old_kw:
            return ""

        old_kw = old_kw.strip()
        if not _has_legacy_rule_prefix(old_kw):
            return _format_rule_clause("OR", [old_kw])

        parts = _split_legacy_delimited_text(old_kw, "&")
        blocks: list[str] = []

        for part in parts:
            part = part.strip()
            if not part:
                continue

            matched_operator = False
            for prefix, operator, separator in _LEGACY_RULE_OPERATORS:
                if not part.upper().startswith(prefix):
                    continue

                payload = part[len(prefix) :]
                keywords = (
                    [payload.strip()]
                    if separator is None
                    else _split_legacy_delimited_text(payload, separator)
                )
                clause = _format_rule_clause(operator, keywords)
                if clause:
                    blocks.append(clause)
                matched_operator = True
                break

            if not matched_operator:
                # Simple keyword
                clause = _format_rule_clause("OR", [part])
                if clause:
                    blocks.append(clause)

        return "+".join(blocks) if blocks else old_kw
