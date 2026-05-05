"""Template CRUD persistence helpers."""

# pylint: disable=line-too-long,broad-exception-caught,protected-access,unused-argument,too-many-branches

from __future__ import annotations

from typing import Any

from bill_analyser.core import template_rust_bridge
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.utils.logger import log_method


@log_method
async def get_all_templates(self, user_id: int = 1, template_type: int | None = None) -> list[dict[str, Any]]:
    """获取所有模板（按模板类型统一返回前端 DTO 结构）。"""
    if self._should_use_rust_template_bridge():
        return template_rust_bridge.list_templates(
            self.db_path,
            user_id=user_id,
            template_type=template_type,
        )

    conn = await self._get_connection()
    templates: list[dict[str, Any]] = []

    if template_type in (None, 1):
        async with conn.execute(
            """
            SELECT * FROM bill_templates
            WHERE user_id = ?
            ORDER BY COALESCE(display_order, 0), is_favorite DESC, use_count DESC, name
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            templates.extend(self._serialize_template_row(dict(row), template_type=1) for row in rows)

    if template_type in (None, 2):
        async with conn.execute(
            """
            SELECT * FROM recurring_bills
            WHERE user_id = ?
            ORDER BY COALESCE(display_order, 0), name
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            templates.extend(self._serialize_template_row(dict(row), template_type=2) for row in rows)

    return templates


@log_method
async def get_template_by_id(
    self,
    template_id: int,
    user_id: int = 1,
    template_type: int | None = None,
) -> dict[str, Any] | None:
    """根据 ID 获取模板。"""
    if self._should_use_rust_template_bridge():
        return template_rust_bridge.get_template(
            self.db_path,
            template_id,
            user_id=user_id,
            template_type=template_type,
        )

    conn = await self._get_connection()

    if template_type == 1:
        table_candidates = [("bill_templates", 1)]
    elif template_type == 2:
        table_candidates = [("recurring_bills", 2)]
    else:
        table_candidates = [("bill_templates", 1), ("recurring_bills", 2)]

    for table_name, resolved_type in table_candidates:
        async with conn.execute(
            f"SELECT * FROM {table_name} WHERE id = ? AND user_id = ?",
            (template_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                return self._serialize_template_row(dict(row), template_type=resolved_type)

    return None


@log_method
async def create_template(self, data: dict[str, Any], user_id: int = 1) -> int:
    """创建模板。"""
    if self._should_use_rust_template_bridge():
        return template_rust_bridge.create_template(self.db_path, data, user_id=user_id)

    conn = await self._get_connection()
    now = utc_now_iso()

    template_type = int(data.get("templateType") or 1)
    display_order = await self._get_next_template_display_order(conn, template_type, user_id)

    if template_type == 2:
        cursor = await conn.execute(
            """
            INSERT INTO recurring_bills (
                user_id, template_id, name, description, type, category, amount,
                account, counterparty, destination_amount, hide_amount, tag,
                comment, frequency, scheduled_frequency_type, start_date, end_date,
                next_date, hidden, display_order, utc_offset, enabled, auto_create,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                None,
                data.get("name", ""),
                data.get("description", ""),
                data.get("type"),
                data.get("categoryId", ""),
                float(data.get("sourceAmount") or 0),
                data.get("sourceAccountId", "0"),
                data.get("destinationAccountId", "0"),
                float(data.get("destinationAmount") or 0),
                1 if data.get("hideAmount") else 0,
                self._serialize_template_tag_ids(data.get("tagIds")),
                data.get("comment", ""),
                data.get("scheduledFrequency", ""),
                int(data.get("scheduledFrequencyType") or 0),
                data.get("scheduledStartDate"),
                data.get("scheduledEndDate"),
                data.get("scheduledStartDate") or now[:10],
                1 if data.get("hidden") else 0,
                display_order,
                int(data.get("utcOffset") or 0),
                1,
                0,
                now,
                now,
            ),
        )
    else:
        cursor = await conn.execute(
            """
            INSERT INTO bill_templates (
                user_id, name, description, type, category, amount, account,
                counterparty, destination_amount, hide_amount, tag, comment,
                is_favorite, display_order, hidden, utc_offset, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                data.get("name", ""),
                data.get("description", ""),
                data.get("type"),
                data.get("categoryId", ""),
                float(data.get("sourceAmount") or 0),
                data.get("sourceAccountId", "0"),
                data.get("destinationAccountId", "0"),
                float(data.get("destinationAmount") or 0),
                1 if data.get("hideAmount") else 0,
                self._serialize_template_tag_ids(data.get("tagIds")),
                data.get("comment", ""),
                0,
                display_order,
                1 if data.get("hidden") else 0,
                int(data.get("utcOffset") or 0),
                now,
                now,
            ),
        )

    await conn.commit()
    return int(cursor.lastrowid or 0)


@log_method
async def update_template(
    self,
    template_id: int,
    data: dict[str, Any],
    user_id: int = 1,
    template_type: int | None = None,
) -> bool:
    """更新模板。"""
    if not data:
        return False

    if self._should_use_rust_template_bridge():
        return template_rust_bridge.update_template(
            self.db_path,
            template_id,
            data,
            user_id=user_id,
            template_type=template_type,
        )

    conn = await self._get_connection()
    normalized = self._build_template_update_payload(data, template_type)
    normalized["updated_at"] = utc_now_iso()

    table_name = self._get_template_table(template_type)
    set_clause = ", ".join(f"{key} = ?" for key in normalized)
    values = [*normalized.values(), template_id, user_id]
    cursor = await conn.execute(f"UPDATE {table_name} SET {set_clause} WHERE id = ? AND user_id = ?", values)
    await conn.commit()
    return cursor.rowcount > 0


@log_method
async def delete_template(self, template_id: int, user_id: int = 1, template_type: int | None = None) -> bool:
    """删除模板。"""
    if self._should_use_rust_template_bridge():
        return template_rust_bridge.delete_template(
            self.db_path,
            template_id,
            user_id=user_id,
            template_type=template_type,
        )

    conn = await self._get_connection()
    table_name = self._get_template_table(template_type)
    cursor = await conn.execute(f"DELETE FROM {table_name} WHERE id = ? AND user_id = ?", (template_id, user_id))
    await conn.commit()
    return cursor.rowcount > 0


@log_method
async def update_template_display_orders(self, orders: list[tuple], template_type: int, user_id: int = 1) -> bool:
    """批量更新模板显示顺序。"""
    if not orders:
        return True

    if self._should_use_rust_template_bridge():
        return template_rust_bridge.update_display_orders(
            self.db_path,
            orders,
            template_type=template_type,
            user_id=user_id,
        )

    conn = await self._get_connection()
    table_name = self._get_template_table(template_type)
    now = utc_now_iso()

    try:
        for template_id, display_order in orders:
            await conn.execute(
                f"UPDATE {table_name} SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                (display_order, now, template_id, user_id),
            )

        await conn.commit()
        return True
    except Exception as exc:  # pragma: no cover - defensive logging branch
        self.logger.error("更新模板显示顺序失败: %s", exc, exc_info=True)
        await conn.rollback()
        return False


def _get_template_table(self, template_type: int | None) -> str:
    """根据模板类型返回表名。"""
    return "recurring_bills" if int(template_type or 1) == 2 else "bill_templates"


def _should_use_rust_template_bridge(self) -> bool:
    """Use Rust template CRUD only for regular file DBs that share the same SQLite file."""
    db_path_text = str(self.db_path)
    if db_path_text in {":memory:", "file::memory:?cache=shared"}:
        return False
    encryption_config = getattr(self, "_encryption_config", None)
    return not bool(getattr(encryption_config, "enabled", False))


async def _get_next_template_display_order(self, conn, template_type: int, user_id: int) -> int:
    """获取下一显示顺序。"""
    table_name = self._get_template_table(template_type)
    async with conn.execute(
        f"SELECT COALESCE(MAX(display_order), 0) FROM {table_name} WHERE user_id = ?",
        (user_id,),
    ) as cursor:
        row = await cursor.fetchone()
        max_order = row[0] if row and row[0] is not None else 0
        return int(max_order) + 1


def _build_template_update_payload(self, data: dict[str, Any], template_type: int | None) -> dict[str, Any]:
    """构建模板更新字段。"""
    normalized: dict[str, Any] = {}
    field_mapping = {
        "name": "name",
        "type": "type",
        "categoryId": "category",
        "sourceAccountId": "account",
        "destinationAccountId": "counterparty",
        "sourceAmount": "amount",
        "destinationAmount": "destination_amount",
        "hideAmount": "hide_amount",
        "comment": "comment",
        "hidden": "hidden",
        "displayOrder": "display_order",
        "utcOffset": "utc_offset",
    }

    for source_key, target_key in field_mapping.items():
        if source_key not in data:
            continue
        value = data[source_key]
        if source_key in {"hideAmount", "hidden"}:
            normalized[target_key] = 1 if value else 0
        elif source_key in {"sourceAmount", "destinationAmount"}:
            normalized[target_key] = float(value or 0)
        elif source_key in {"displayOrder", "utcOffset"}:
            normalized[target_key] = int(value or 0)
        else:
            normalized[target_key] = value

    if "tagIds" in data:
        normalized["tag"] = self._serialize_template_tag_ids(data.get("tagIds"))

    if int(template_type or 1) == 2:
        recurring_mapping = {
            "scheduledFrequencyType": "scheduled_frequency_type",
            "scheduledFrequency": "frequency",
            "scheduledStartDate": "start_date",
            "scheduledEndDate": "end_date",
        }
        for source_key, target_key in recurring_mapping.items():
            if source_key not in data:
                continue
            value = data[source_key]
            if source_key == "scheduledFrequencyType":
                normalized[target_key] = int(value or 0)
            else:
                normalized[target_key] = value

        if "scheduledStartDate" in data:
            normalized["next_date"] = data.get("scheduledStartDate")

    return normalized
