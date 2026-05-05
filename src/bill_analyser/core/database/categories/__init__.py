"""Category-domain persistence helpers for the split database facade."""

from __future__ import annotations

import sqlite3
from typing import Any

import aiosqlite

from bill_analyser.core import category_rust_bridge
from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso


class DatabaseCategoriesMixin(DatabaseFacadeBase):
    """Category CRUD, lookup, statistics, and cascade helpers."""

    def _should_use_rust_category_bridge(self) -> bool:
        """Use Rust category CRUD only for regular file DBs sharing the SQLite file."""
        db_path_text = str(self.db_path)
        if db_path_text in {":memory:", "file::memory:?cache=shared"}:
            return False
        encryption_config = getattr(self, "_encryption_config", None)
        return not bool(getattr(encryption_config, "enabled", False))

    @log_method
    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取所有分类（从 categories 表）。"""
        if self._should_use_rust_category_bridge():
            return category_rust_bridge.list_categories(self.db_path, user_id=user_id)
        return await self._get_all_categories_python(user_id=user_id)

    async def _get_all_categories_python(self, user_id: int = 1) -> list[dict[str, Any]]:
        """通过 aiosqlite fallback 获取所有分类。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        self.logger.debug(
            "[get_all_categories] 查询分类 (user_id=%s)，排序：priority ASC",
            user_id,
        )
        async with conn.execute(
            (
                "SELECT * FROM categories WHERE user_id = ? "
                "ORDER BY priority ASC, main_category, sub_category"
            ),
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            categories = [dict(row) for row in rows]

        self.logger.debug("[get_all_categories] 返回 %s 个分类", len(categories))
        if categories:
            for index, category in enumerate(categories[:5], start=1):
                self.logger.debug(
                    "[get_all_categories] #%s 分类: %s/%s, priority=%s",
                    index,
                    category.get("main_category"),
                    category.get("sub_category"),
                    category.get("priority", 0),
                )

        if categories:
            return categories

        self.logger.info("categories 表为空，从 bills 表提取分类")
        async with conn.execute(
            "SELECT DISTINCT main_category, sub_category FROM bills "
            "WHERE main_category IS NOT NULL AND user_id = ? "
            "ORDER BY main_category, sub_category",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()

        return [
            {
                "id": 0,
                "main_category": row[0],
                "sub_category": row[1],
                "description": "",
                "priority": 0,
                "keywords": "",
            }
            for row in rows
        ]

    @log_method
    async def get_category_by_name(
        self,
        main_category: str,
        sub_category: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """根据名称获取分类。"""
        if self._should_use_rust_category_bridge():
            return category_rust_bridge.get_category_by_name(
                self.db_path,
                main_category,
                sub_category,
                user_id=user_id,
            )
        return await self._get_category_by_name_python(
            main_category,
            sub_category,
            user_id=user_id,
        )

    async def _get_category_by_name_python(
        self,
        main_category: str,
        sub_category: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """通过 aiosqlite fallback 根据名称获取分类。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM categories WHERE main_category = ? AND sub_category = ? AND user_id = ?",
            (main_category, sub_category, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_category(self, category_data: dict[str, Any], user_id: int = 1) -> int | None:
        """创建分类。"""
        if self._should_use_rust_category_bridge():
            category_id = category_rust_bridge.create_category(
                self.db_path,
                category_data,
                user_id=user_id,
            )
            if category_id is not None:
                self._clear_cache("category_mappings")
            return category_id
        return await self._create_category_python(category_data, user_id=user_id)

    async def _create_category_python(
        self,
        category_data: dict[str, Any],
        user_id: int = 1,
    ) -> int | None:
        """通过 aiosqlite fallback 创建分类。"""
        conn = await self._get_connection()
        main_category = category_data.get("main_category")
        sub_category = category_data.get("sub_category", "")

        self.logger.info(
            "开始创建分类: %s/%s (user_id=%s)",
            main_category,
            sub_category,
            user_id,
        )

        try:
            columns = [
                "type",
                "main_category",
                "sub_category",
                "description",
                "priority",
                "keywords",
                "hidden",
                "icon",
                "color",
                "created_at",
                "user_id",
            ]
            placeholders = ", ".join(["?" for _ in columns])
            values = [
                category_data.get("type", 1),
                main_category,
                sub_category,
                category_data.get("description", ""),
                category_data.get("priority", 0),
                category_data.get("keywords", ""),
                category_data.get("hidden", False),
                category_data.get("icon", ""),
                category_data.get("color", ""),
                utc_now_iso(),
                user_id,
            ]

            self.logger.debug("执行插入: columns=%s, values=%s", columns, values)
            cursor = await conn.execute(
                f"INSERT INTO categories ({', '.join(columns)}) VALUES ({placeholders})",
                values,
            )
            await conn.commit()

            category_id = cursor.lastrowid
            self.logger.info("创建分类成功: ID=%s, %s/%s", category_id, main_category, sub_category)
            self._clear_cache("category_mappings")
            return category_id
        except sqlite3.IntegrityError as exc:
            self.logger.warning(
                "分类已存在（UNIQUE约束）: %s/%s - %s",
                main_category,
                sub_category,
                exc,
            )
            return None
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("创建分类失败: %s: %s", type(exc).__name__, exc, exc_info=True)
            return None

    @log_method
    async def ensure_categories(
        self,
        categories: list[dict[str, Any]],
        user_id: int = 1,
    ) -> dict[str, int]:
        """批量创建缺失分类，不覆盖已有分类。"""
        if self._should_use_rust_category_bridge():
            result = category_rust_bridge.ensure_categories(
                self.db_path,
                categories,
                user_id=user_id,
            )
            if result["created"] > 0:
                self._clear_cache("category_mappings")
            return result

        created = 0
        skipped = 0
        for category_data in categories:
            category_id = await self._create_category_python(category_data, user_id=user_id)
            if category_id is None:
                skipped += 1
            else:
                created += 1
        return {"created": created, "skipped": skipped}

    @log_method
    async def update_category(
        self,
        category_id: int,
        updates: dict[str, Any],
        user_id: int = 1,
    ) -> bool:
        """更新分类。"""
        if self._should_use_rust_category_bridge():
            updated = category_rust_bridge.update_category(
                self.db_path,
                category_id,
                updates,
                user_id=user_id,
            )
            if updated:
                self._clear_cache("category_mappings")
            return updated
        return await self._update_category_python(category_id, updates, user_id=user_id)

    async def _update_category_python(
        self,
        category_id: int,
        updates: dict[str, Any],
        user_id: int = 1,
    ) -> bool:
        """通过 aiosqlite fallback 更新分类。"""
        conn = await self._get_connection()
        try:
            valid_fields = [
                "type",
                "main_category",
                "sub_category",
                "description",
                "priority",
                "keywords",
                "hidden",
                "icon",
                "color",
            ]
            safe_updates = {key: value for key, value in updates.items() if key in valid_fields}
            if not safe_updates:
                return False

            set_clause = ", ".join(f"{key} = ?" for key in safe_updates)
            values = [*safe_updates.values(), category_id, user_id]
            cursor = await conn.execute(
                f"UPDATE categories SET {set_clause} WHERE id = ? AND user_id = ?",
                values,
            )
            await conn.commit()

            if cursor.rowcount == 0:
                self.logger.warning("分类 ID %s 不存在或不属于用户 %s", category_id, user_id)
                return False

            self.logger.info("已更新分类 ID: %s (user_id=%s)", category_id, user_id)
            self._clear_cache("category_mappings")
            return True
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error("更新分类失败: %s", exc, exc_info=True)
            raise

    @log_method
    async def delete_category(self, category_id: int, user_id: int = 1) -> bool:
        """删除分类（父分类级联删除子分类）。"""
        if self._should_use_rust_category_bridge():
            deleted = category_rust_bridge.delete_category(
                self.db_path,
                category_id,
                user_id=user_id,
            )
            if deleted:
                self._clear_cache("category_mappings")
            return deleted
        return await self._delete_category_python(category_id, user_id=user_id)

    async def _delete_category_python(self, category_id: int, user_id: int = 1) -> bool:
        """通过 aiosqlite fallback 删除分类（父分类级联删除子分类）。"""
        conn = await self._get_connection()
        try:
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                (
                    "SELECT id, main_category, sub_category FROM categories "
                    "WHERE id = ? AND user_id = ?"
                ),
                (category_id, user_id),
            ) as cursor:
                category = await cursor.fetchone()

            if not category:
                self.logger.warning(
                    "分类ID %s 不存在或不属于用户 %s",
                    category_id,
                    user_id,
                )
                return False

            category_dict = dict(category)
            main_category = category_dict["main_category"]
            sub_category = category_dict["sub_category"]

            if sub_category in {"", None}:
                self.logger.info(
                    "删除父级分类 '%s' 及其所有子分类 (user_id=%s)",
                    main_category,
                    user_id,
                )
                async with conn.execute(
                    "SELECT COUNT(*) as count FROM categories "
                    "WHERE main_category = ? AND sub_category != '' AND user_id = ?",
                    (main_category, user_id),
                ) as cursor:
                    result = await cursor.fetchone()
                    child_count = result[0] if result else 0

                cursor = await conn.execute(
                    "DELETE FROM categories WHERE main_category = ? AND user_id = ?",
                    (main_category, user_id),
                )
                deleted_count = cursor.rowcount
                await conn.commit()

                self.logger.info(
                    "已删除父级分类 '%s' (ID: %s) 及其 %s 个子分类，共删除 %s 条记录 (user_id=%s)",
                    main_category,
                    category_id,
                    child_count,
                    deleted_count,
                    user_id,
                )
                self._clear_cache("category_mappings")
                return True

            self.logger.info(
                "删除子分类 '%s/%s' (ID: %s, user_id=%s)",
                main_category,
                sub_category,
                category_id,
                user_id,
            )
            await conn.execute(
                "DELETE FROM categories WHERE id = ? AND user_id = ?",
                (category_id, user_id),
            )
            await conn.commit()
            self.logger.info(
                "已删除子分类 '%s/%s' (ID: %s, user_id=%s)",
                main_category,
                sub_category,
                category_id,
                user_id,
            )
            self._clear_cache("category_mappings")
            return True
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("删除分类失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def get_category_by_id(self, category_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """获取单个分类。"""
        if self._should_use_rust_category_bridge():
            return category_rust_bridge.get_category(
                self.db_path,
                category_id,
                user_id=user_id,
            )
        return await self._get_category_by_id_python(category_id, user_id=user_id)

    async def _get_category_by_id_python(
        self,
        category_id: int,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """通过 aiosqlite fallback 根据 ID 获取分类。"""
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM categories WHERE id = ? AND user_id = ?",
            (category_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_category_statistics(
        self,
        period: str = "month",
        start_date: str | None = None,
        end_date: str | None = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """获取分类统计。"""
        _ = period
        conn = await self._get_connection()

        query = """
        SELECT
            main_category,
            sub_category,
            type,
            COUNT(*) as count,
            SUM(amount) as total_amount,
            AVG(amount) as avg_amount,
            MIN(amount) as min_amount,
            MAX(amount) as max_amount
        FROM bills
        WHERE main_category IS NOT NULL AND user_id = ?
        """
        params: list[Any] = [user_id]

        if start_date:
            query += " AND date >= ?"
            params.append(start_date)
        if end_date:
            query += " AND date <= ?"
            params.append(end_date)

        query += " GROUP BY main_category, sub_category, type ORDER BY total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def delete_categories_by_main_category(
        self,
        main_category: str,
        user_id: int = 1,
    ) -> bool:
        """根据主分类名称删除所有相关分类。"""
        if self._should_use_rust_category_bridge():
            deleted = category_rust_bridge.delete_categories_by_main_category(
                self.db_path,
                main_category,
                user_id=user_id,
            )
            if deleted:
                self._clear_cache("category_mappings")
            return deleted
        return await self._delete_categories_by_main_category_python(
            main_category,
            user_id=user_id,
        )

    async def _delete_categories_by_main_category_python(
        self,
        main_category: str,
        user_id: int = 1,
    ) -> bool:
        """通过 aiosqlite fallback 根据主分类名称删除所有相关分类。"""
        conn = await self._get_connection()
        try:
            cursor = await conn.execute(
                "DELETE FROM categories WHERE main_category = ? AND user_id = ?",
                (main_category, user_id),
            )
            await conn.commit()
            deleted = cursor.rowcount > 0
            self.logger.info(
                "已删除主分类及其子分类: %s (user_id=%s, deleted=%s)",
                main_category,
                user_id,
                deleted,
            )
            if deleted:
                self._clear_cache("category_mappings")
            return deleted
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error("删除主分类失败: %s", exc, exc_info=True)
            raise

    @log_method
    async def update_main_category_name(
        self,
        old_name: str,
        new_name: str,
        user_id: int = 1,
    ) -> bool:
        """更新主分类名称（级联更新所有子分类）。"""
        if self._should_use_rust_category_bridge():
            updated = category_rust_bridge.update_main_category_name(
                self.db_path,
                old_name,
                new_name,
                user_id=user_id,
            )
            if updated:
                self._clear_cache("account_mappings")
                self._clear_cache("category_mappings")
            return updated
        return await self._update_main_category_name_python(
            old_name,
            new_name,
            user_id=user_id,
        )

    async def _update_main_category_name_python(
        self,
        old_name: str,
        new_name: str,
        user_id: int = 1,
    ) -> bool:
        """通过 aiosqlite fallback 更新主分类名称（级联更新所有子分类）。"""
        conn = await self._get_connection()
        try:
            cursor = await conn.execute(
                "UPDATE categories SET main_category = ? WHERE main_category = ? AND user_id = ?",
                (new_name, old_name, user_id),
            )
            await conn.commit()
            if cursor.rowcount == 0:
                self.logger.info(
                    "主分类名称更新未命中任何分类: %s -> %s (user_id=%s)",
                    old_name,
                    new_name,
                    user_id,
                )
                return False
            self.logger.info(
                "已更新主分类名称: %s -> %s (user_id=%s)",
                old_name,
                new_name,
                user_id,
            )
            self._clear_cache("account_mappings")
            self._clear_cache("category_mappings")
            return True
        except sqlite3.IntegrityError as exc:
            await conn.rollback()
            self.logger.warning(
                "更新主分类名称冲突: %s -> %s (user_id=%s): %s",
                old_name,
                new_name,
                user_id,
                exc,
            )
            return False
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error("更新主分类名称失败: %s", exc, exc_info=True)
            raise
