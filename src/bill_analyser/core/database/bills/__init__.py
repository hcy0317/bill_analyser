"""Bill-domain queries, CRUD, deduplication, and saved-filter helpers."""

# pylint: disable=line-too-long,too-many-branches,too-many-statements,broad-exception-caught

from __future__ import annotations

import hashlib
import json
import sqlite3
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class DatabaseBillsMixin(DatabaseFacadeBase):
    """Bill CRUD, filtering, deduplication, and saved-filter helpers."""

    BILL_CREATE_COLUMNS = (
        "user_id",
        "date",
        "type",
        "amount",
        "counterparty",
        "description",
        "payment_method",
        "main_category",
        "sub_category",
        "batch_id",
        "hash",
        "created_at",
        "updated_at",
        "source_account_id",
        "destination_account_id",
        "destination_amount",
        "created_from_template",
        "created_from_recurring",
        "import_history_id",
    )
    BILL_UPDATE_COLUMNS = (
        "date",
        "type",
        "amount",
        "counterparty",
        "description",
        "payment_method",
        "main_category",
        "sub_category",
        "batch_id",
        "hash",
        "source_account_id",
        "destination_account_id",
        "destination_amount",
        "created_from_template",
        "created_from_recurring",
        "import_history_id",
    )

    def _calculate_hash(self, bill: dict[str, Any]) -> str:
        """根据账单稳定字段生成去重哈希。"""
        parts = [
            str(bill.get("date", "")),
            str(bill.get("type", "")),
            str(bill.get("amount", "")),
            str(bill.get("counterparty", "")),
            str(bill.get("description", "")),
        ]
        return hashlib.md5("|".join(parts).encode()).hexdigest()

    def _apply_amount_filter(
        self,
        conditions: list[str],
        params: list[Any],
        amount_filter: str,
    ) -> None:
        """解析并应用 amount_filter DSL。"""
        parts = str(amount_filter or "").split(":")
        if len(parts) < 2:
            return

        filter_type = parts[0].lower()
        try:
            if filter_type == "eq":
                amount = float(parts[1])
                conditions.append("amount = ?")
                params.append(amount)
            elif filter_type == "ne":
                amount = float(parts[1])
                conditions.append("amount != ?")
                params.append(amount)
            elif filter_type == "gt":
                amount = float(parts[1])
                conditions.append("amount > ?")
                params.append(amount)
            elif filter_type == "lt":
                amount = float(parts[1])
                conditions.append("amount < ?")
                params.append(amount)
            elif filter_type == "gte":
                amount = float(parts[1])
                conditions.append("amount >= ?")
                params.append(amount)
            elif filter_type == "lte":
                amount = float(parts[1])
                conditions.append("amount <= ?")
                params.append(amount)
            elif filter_type == "between" and len(parts) >= 3:
                min_amount = float(parts[1])
                max_amount = float(parts[2])
                conditions.append("amount BETWEEN ? AND ?")
                params.extend([min_amount, max_amount])
            else:
                self.logger.warning("未知的金额过滤器类型: %s", filter_type)
        except (TypeError, ValueError, IndexError) as exc:
            self.logger.error("金额过滤器格式错误: %s, 错误: %s", amount_filter, exc)

    def _build_bill_filter_conditions(
        self,
        filters: dict[str, Any] | None,
        params: list[Any],
    ) -> list[str]:
        """构建 bills 查询条件。"""
        if not filters:
            return []

        conditions: list[str] = []
        if "id" in filters:
            conditions.append("id = ?")
            params.append(filters["id"])

        date_from = filters.get("date_from") or filters.get("start_date")
        if date_from:
            conditions.append("date >= ?")
            params.append(date_from)

        date_to = filters.get("date_to") or filters.get("end_date")
        if date_to:
            conditions.append("date <= ?")
            params.append(date_to)

        if "type" in filters:
            conditions.append("type = ?")
            params.append(filters["type"])
        if "main_category" in filters:
            conditions.append("main_category = ?")
            params.append(filters["main_category"])
        if "sub_category" in filters:
            conditions.append("sub_category = ?")
            params.append(filters["sub_category"])
        if "batch_id" in filters:
            conditions.append("batch_id = ?")
            params.append(filters["batch_id"])
        if "counterparty" in filters:
            conditions.append("counterparty LIKE ?")
            params.append(f"%{filters['counterparty']}%")
        if "description" in filters:
            conditions.append("description LIKE ?")
            params.append(f"%{filters['description']}%")

        if filters.get("keyword"):
            keyword_pattern = f"%{filters['keyword']}%"
            conditions.append("(description LIKE ? OR counterparty LIKE ?)")
            params.extend([keyword_pattern, keyword_pattern])

        account_ids = filters.get("account_ids")
        if isinstance(account_ids, list) and account_ids:
            placeholders = ",".join(["?"] * len(account_ids))
            conditions.append(f"(source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))")
            params.extend(account_ids)
            params.extend(account_ids)

        categories = filters.get("categories")
        if isinstance(categories, list) and categories:
            category_conditions: list[str] = []
            for category in categories:
                main = category.get("main")
                sub = category.get("sub")
                if main and sub:
                    category_conditions.append("(main_category = ? AND sub_category = ?)")
                    params.extend([main, sub])
                elif main:
                    category_conditions.append("(main_category = ?)")
                    params.append(main)
            if category_conditions:
                conditions.append("(" + " OR ".join(category_conditions) + ")")

        tag_ids = filters.get("tag_ids")
        if isinstance(tag_ids, list) and tag_ids:
            placeholders = ",".join(["?"] * len(tag_ids))
            conditions.append(f"id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))")
            params.extend(tag_ids)

        if "min_amount" in filters and filters["min_amount"] is not None:
            conditions.append("amount >= ?")
            params.append(float(filters["min_amount"]))
        if "max_amount" in filters and filters["max_amount"] is not None:
            conditions.append("amount <= ?")
            params.append(float(filters["max_amount"]))

        if filters.get("amount_filter"):
            self._apply_amount_filter(conditions, params, str(filters["amount_filter"]))

        return conditions

    @log_method
    async def insert_bills(self, bills: list[dict[str, Any]], batch_id: str | None = None, user_id: int = 1) -> int:
        """批量插入账单。"""
        if not bills:
            self.logger.warning("账单列表为空")
            return 0

        resolved_batch_id = batch_id or utc_now().strftime("%Y%m%d%H%M%S")
        conn = await self._get_connection()
        inserted_count = 0

        for index in range(0, len(bills), self.batch_size):
            batch = bills[index : index + self.batch_size]
            for bill in batch:
                try:
                    bill_hash = self._calculate_hash(bill)
                    now = utc_now_iso()
                    await conn.execute(
                        """
                        INSERT INTO bills (
                            user_id, date, type, amount, counterparty, description,
                            payment_method, main_category, sub_category, batch_id, hash,
                            created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            user_id,
                            bill.get("date"),
                            bill.get("type"),
                            bill.get("amount"),
                            bill.get("counterparty"),
                            bill.get("description"),
                            bill.get("payment_method"),
                            bill.get("main_category"),
                            bill.get("sub_category"),
                            resolved_batch_id,
                            bill_hash,
                            now,
                            now,
                        ),
                    )
                    inserted_count += 1
                except sqlite3.IntegrityError:
                    self.logger.debug("跳过重复账单: %s %s", bill.get("date"), bill.get("description"))
                except Exception as exc:  # pragma: no cover - defensive logging branch
                    self.logger.error("插入账单失败: %s", exc)
            await conn.commit()

        self.logger.info("成功插入 %s 条账单", inserted_count)
        return inserted_count

    @log_method
    async def insert_bill(self, bill: dict[str, Any], user_id: int = 1) -> int:
        """插入单条账单。"""
        payload = dict(bill)
        if "payment_method" not in payload and payload.get("channel") not in (None, ""):
            payload["payment_method"] = payload.pop("channel")
        if "main_category" not in payload and payload.get("category") not in (None, ""):
            payload["main_category"] = payload.pop("category")

        bill_id = await self.create_bill(payload, user_id=user_id)
        return int(bill_id or 0)

    @log_method
    async def check_duplicate(self, bill: dict[str, Any], user_id: int = 1) -> bool:
        """检查账单是否重复。"""
        bill_hash = self._calculate_hash(bill)
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT COUNT(*) as count FROM bills WHERE user_id = ? AND hash = ?",
            (user_id, bill_hash),
        ) as cursor:
            row = await cursor.fetchone()
        return bool(row and row["count"] > 0)

    @log_method
    async def get_bills_by_date_range(self, start_date: str, end_date: str, user_id: int = 1) -> list[dict[str, Any]]:
        """按日期范围查询账单（用于去重对比）。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT id, date, amount, counterparty, description,
                   type, main_category, sub_category, source_account_id
            FROM bills
            WHERE user_id = ?
              AND date >= ?
              AND date <= ?
            ORDER BY date
            """,
            [user_id, start_date, end_date + " 23:59:59"],
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_bills(
        self,
        filters: dict[str, Any] | None = None,
        limit: int | None = None,
        offset: int = 0,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """查询账单列表。"""
        conn = await self._get_connection()
        params: list[Any] = [user_id]
        conditions = self._build_bill_filter_conditions(filters, params)

        query = "SELECT * FROM bills WHERE user_id = ?"
        if conditions:
            query += " AND " + " AND ".join(conditions)
        query += " ORDER BY date DESC"
        if limit is not None:
            query += f" LIMIT {int(limit)} OFFSET {int(offset)}"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        bills = [dict(row) for row in rows]
        self.logger.info("查询到 %s 条账单", len(bills))
        return bills

    @log_method
    async def create_bill(self, bill_data: dict[str, Any], user_id: int = 1) -> int | None:
        """创建单条账单。"""
        conn = await self._get_connection()
        try:
            bill_payload = dict(bill_data)
            if "payment_method" not in bill_payload and bill_payload.get("channel") not in (None, ""):
                bill_payload["payment_method"] = bill_payload.pop("channel")

            now = utc_now_iso()
            bill_payload.setdefault("created_at", now)
            bill_payload.setdefault("updated_at", now)
            bill_payload.setdefault("user_id", user_id)
            invalid_columns = sorted(set(bill_payload) - set(self.BILL_CREATE_COLUMNS))
            if invalid_columns:
                raise ValueError(f"unsupported bill create fields: {', '.join(invalid_columns)}")

            for field in ["date", "type", "amount", "description"]:
                if field not in bill_payload:
                    self.logger.error("缺少必填字段: %s", field)
                    return None

            columns = [column for column in self.BILL_CREATE_COLUMNS if column in bill_payload]
            placeholders = ", ".join(["?" for _ in columns])
            values = [bill_payload[column] for column in columns]
            cursor = await conn.execute(
                f"INSERT INTO bills ({', '.join(columns)}) VALUES ({placeholders})",
                values,
            )
            await conn.commit()
            bill_id = int(cursor.lastrowid or 0)
            self.logger.info("已创建账单 ID: %s", bill_id)
            return bill_id
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("创建账单失败: %s", exc)
            return None

    @log_method
    async def delete_bill(self, bill_id: int, user_id: int = 1) -> bool:
        """删除账单。"""
        conn = await self._get_connection()
        try:
            await self._delete_bill_pair_links_for_bill_ids(conn, [bill_id], user_id=user_id)
            await self._delete_bill_transfer_pair_suppressions_for_bill_ids(conn, [bill_id], user_id=user_id)
            await self._delete_bill_investment_pair_suppressions_for_bill_ids(conn, [bill_id], user_id=user_id)
            await self._delete_bill_learning_rule_suppressions_for_bill_ids(conn, [bill_id], user_id=user_id)
            cursor = await conn.execute("DELETE FROM bills WHERE id = ? AND user_id = ?", (bill_id, user_id))
            await conn.commit()
            return cursor.rowcount > 0
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("删除账单失败 ID=%s: %s", bill_id, exc)
            return False

    @log_method
    async def update_bill(self, bill_id: int, updates: dict[str, Any], user_id: int = 1) -> bool:
        """更新账单。"""
        if not updates:
            return False
        invalid_update_fields = sorted(set(updates) - set(self.BILL_UPDATE_COLUMNS))
        if invalid_update_fields:
            raise ValueError(f"unsupported bill update fields: {', '.join(invalid_update_fields)}")
        conn = await self._get_connection()
        try:
            update_payload = {
                column: updates[column]
                for column in self.BILL_UPDATE_COLUMNS
                if column in updates
            }
            update_payload["updated_at"] = utc_now_iso()
            set_clause = ", ".join(f"{key} = ?" for key in update_payload)
            values = [*update_payload.values(), bill_id, user_id]
            cursor = await conn.execute(
                f"UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?",
                values,
            )
            await conn.commit()
            return cursor.rowcount > 0
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("更新账单失败 ID=%s: %s", bill_id, exc)
            return False

    @log_method
    async def query_bills(
        self,
        page: int = 1,
        page_size: int = 20,
        filters: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> tuple[list[dict[str, Any]], int]:
        """分页查询账单。"""
        offset = (page - 1) * page_size
        bills = await self.get_bills(filters=filters, limit=page_size, offset=offset, user_id=user_id)

        conn = await self._get_connection()
        params: list[Any] = [user_id]
        conditions = self._build_bill_filter_conditions(filters, params)
        count_query = "SELECT COUNT(*) as total FROM bills WHERE user_id = ?"
        if conditions:
            count_query += " AND " + " AND ".join(conditions)

        async with conn.execute(count_query, params) as cursor:
            row = await cursor.fetchone()
        total = int(row["total"] if row else 0)
        return bills, total

    @log_method
    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """根据 ID 获取账单。"""
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM bills WHERE id = ? AND user_id = ?", (bill_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def batch_delete_bills(self, bill_ids: list[int], user_id: int = 1) -> int:
        """批量删除账单。"""
        if not bill_ids:
            return 0

        conn = await self._get_connection()
        await self._delete_bill_pair_links_for_bill_ids(conn, bill_ids, user_id=user_id)
        await self._delete_bill_transfer_pair_suppressions_for_bill_ids(conn, bill_ids, user_id=user_id)
        await self._delete_bill_investment_pair_suppressions_for_bill_ids(conn, bill_ids, user_id=user_id)
        await self._delete_bill_learning_rule_suppressions_for_bill_ids(conn, bill_ids, user_id=user_id)
        placeholders = ",".join(["?" for _ in bill_ids])
        cursor = await conn.execute(
            f"DELETE FROM bills WHERE id IN ({placeholders}) AND user_id = ?",
            [*bill_ids, user_id],
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def batch_update_bills(
        self,
        bill_ids: list[int],
        updates: dict[str, Any],
        user_id: int = 1,
    ) -> dict[str, Any]:
        """批量更新账单。"""
        if not bill_ids or not updates:
            return {"success_count": 0, "failed_count": 0, "failed_ids": []}

        invalid_update_fields = sorted(set(updates) - set(self.BILL_UPDATE_COLUMNS))
        if invalid_update_fields:
            raise ValueError(f"unsupported batch update fields: {', '.join(invalid_update_fields)}")

        conn = await self._get_connection()
        success_count = 0
        failed_ids: list[int] = []
        update_payload = {
            column: updates[column]
            for column in self.BILL_UPDATE_COLUMNS
            if column in updates
        }
        update_payload["updated_at"] = utc_now_iso()
        set_clause = ", ".join(f"{key} = ?" for key in update_payload)

        for bill_id in bill_ids:
            try:
                cursor = await conn.execute(
                    f"UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?",
                    [*update_payload.values(), bill_id, user_id],
                )
                if cursor.rowcount > 0:
                    success_count += 1
                else:
                    failed_ids.append(bill_id)
            except Exception as exc:  # pragma: no cover - defensive logging branch
                failed_ids.append(bill_id)
                self.logger.error("更新账单失败 ID=%s: %s", bill_id, exc)

        await conn.commit()
        return {
            "success_count": success_count,
            "failed_count": len(failed_ids),
            "failed_ids": failed_ids,
        }

    @log_method
    async def batch_update_categories(
        self,
        bill_ids: list[int],
        main_category: str,
        sub_category: str,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """批量修改账单分类。"""
        return await self.batch_update_bills(
            bill_ids,
            {"main_category": main_category, "sub_category": sub_category},
            user_id=user_id,
        )

    @log_method
    async def deduplicate(self) -> int:
        """去除重复账单。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT id, user_id, date, type, amount, counterparty, description, hash
            FROM bills
            ORDER BY id ASC
            """
        ) as cursor:
            bill_rows = await cursor.fetchall()

        duplicate_ids_by_user: dict[int, list[int]] = {}
        seen_group_keys: set[tuple[Any, ...]] = set()
        for row in bill_rows:
            row_id = int(row["id"] or 0)
            row_user_id = int(row["user_id"] or 0)
            row_hash = str(row["hash"] or "").strip()

            if row_hash:
                dedup_group_key: tuple[Any, ...] = ("hash", row_user_id, row_hash)
            else:
                dedup_group_key = (
                    "legacy-null-hash",
                    row_user_id,
                    str(row["date"] or ""),
                    str(row["type"] or ""),
                    f"{float(row['amount'] or 0.0):.10f}",
                    str(row["counterparty"] or ""),
                    str(row["description"] or ""),
                )

            if dedup_group_key in seen_group_keys:
                duplicate_ids_by_user.setdefault(row_user_id, []).append(row_id)
                continue

            seen_group_keys.add(dedup_group_key)

        duplicate_ids = [bill_id for bill_ids in duplicate_ids_by_user.values() for bill_id in bill_ids]
        if not duplicate_ids:
            return 0

        for row_user_id, bill_ids in duplicate_ids_by_user.items():
            await self._delete_bill_pair_links_for_bill_ids(
                conn,
                bill_ids,
                user_id=row_user_id,
            )
            await self._delete_bill_transfer_pair_suppressions_for_bill_ids(
                conn,
                bill_ids,
                user_id=row_user_id,
            )
            await self._delete_bill_investment_pair_suppressions_for_bill_ids(
                conn,
                bill_ids,
                user_id=row_user_id,
            )
            await self._delete_bill_learning_rule_suppressions_for_bill_ids(
                conn,
                bill_ids,
                user_id=row_user_id,
            )

        placeholders = ",".join(["?" for _ in duplicate_ids])
        cursor = await conn.execute(
            f"DELETE FROM bills WHERE id IN ({placeholders})",
            duplicate_ids,
        )
        deleted_count = int(cursor.rowcount or 0)
        await conn.commit()
        return deleted_count

    @log_method
    async def get_statistics(self) -> dict[str, Any]:
        """获取数据库统计信息。"""
        conn = await self._get_connection()
        stats: dict[str, Any] = {}

        async with conn.execute("SELECT COUNT(*) FROM bills") as cursor:
            row = await cursor.fetchone()
            stats["total_bills"] = row[0] if row else 0

        async with conn.execute(
            """
            SELECT type, COUNT(*) as count, SUM(amount) as total
            FROM bills
            GROUP BY type
            """
        ) as cursor:
            rows = await cursor.fetchall()
            stats["by_type"] = {row[0]: {"count": row[1], "total": row[2]} for row in rows}

        async with conn.execute(
            """
            SELECT main_category, COUNT(*) as count, SUM(amount) as total
            FROM bills
            WHERE main_category IS NOT NULL
            GROUP BY main_category
            """
        ) as cursor:
            rows = await cursor.fetchall()
            stats["by_category"] = {row[0]: {"count": row[1], "total": row[2]} for row in rows}

        return stats

    @log_method
    async def save_filter(self, name: str, filter_data: dict[str, Any], description: str | None = None) -> int:
        """保存筛选条件。"""
        conn = await self._get_connection()
        now = utc_now_iso()
        filter_json = json.dumps(filter_data, ensure_ascii=False)

        try:
            cursor = await conn.execute(
                """
                INSERT INTO saved_filters (name, filter_data, description, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?)
                """,
                (name, filter_json, description, now, now),
            )
            await conn.commit()
            return int(cursor.lastrowid or 0)
        except sqlite3.IntegrityError:
            await conn.execute(
                """
                UPDATE saved_filters
                SET filter_data = ?, description = ?, updated_at = ?
                WHERE name = ?
                """,
                (filter_json, description, now, name),
            )
            await conn.commit()
            async with conn.execute("SELECT id FROM saved_filters WHERE name = ?", (name,)) as cursor:
                row = await cursor.fetchone()
            return int(row[0] if row else 0)

    @log_method
    async def get_saved_filters(self) -> list[dict[str, Any]]:
        """获取所有保存的筛选条件。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT id, name, filter_data, description, created_at, updated_at
            FROM saved_filters
            ORDER BY updated_at DESC
            """
        ) as cursor:
            rows = await cursor.fetchall()

        saved_filters: list[dict[str, Any]] = []
        for row in rows:
            filter_item = dict(row)
            filter_item["filter_data"] = json.loads(filter_item["filter_data"])
            saved_filters.append(filter_item)
        return saved_filters

    @log_method
    async def get_saved_filter(self, filter_id: int | None = None, name: str | None = None) -> dict[str, Any] | None:
        """获取单个保存的筛选条件。"""
        if not filter_id and not name:
            return None

        conn = await self._get_connection()
        if filter_id:
            query = "SELECT id, name, filter_data, description, created_at, updated_at FROM saved_filters WHERE id = ?"
            params = (filter_id,)
        else:
            query = (
                "SELECT id, name, filter_data, description, created_at, updated_at FROM saved_filters WHERE name = ?"
            )
            params = (name,)

        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
        if not row:
            return None

        filter_item = dict(row)
        filter_item["filter_data"] = json.loads(filter_item["filter_data"])
        return filter_item

    @log_method
    async def delete_saved_filter(self, filter_id: int | None = None, name: str | None = None) -> bool:
        """删除保存的筛选条件。"""
        if not filter_id and not name:
            return False

        conn = await self._get_connection()
        if filter_id:
            cursor = await conn.execute("DELETE FROM saved_filters WHERE id = ?", (filter_id,))
        else:
            cursor = await conn.execute("DELETE FROM saved_filters WHERE name = ?", (name,))
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def get_ml_training_data(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取用于 ML 分类器训练的已分类账单数据。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT counterparty, description, main_category, sub_category
            FROM bills
            WHERE user_id = ?
              AND main_category IS NOT NULL
              AND main_category != ''
            ORDER BY date DESC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        result = [dict(row) for row in rows]
        self.logger.info("[ML训练数据] user_id=%d, 获取 %d 条已分类账单", user_id, len(result))
        return result
