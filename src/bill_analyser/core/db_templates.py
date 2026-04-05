"""Template and recurring-bill persistence helpers for the split database facade."""

# pylint: disable=line-too-long,broad-exception-caught,too-many-locals,too-many-branches

from __future__ import annotations

from datetime import date, datetime, timedelta
from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseTemplatesMixin(DatabaseFacadeBase):
    """Template and recurring-bill persistence helpers."""

    @log_method
    async def get_all_templates(self, user_id: int = 1, template_type: int | None = None) -> list[dict[str, Any]]:
        """获取所有模板（按模板类型统一返回前端 DTO 结构）。"""
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

    @log_method
    async def get_recurring_candidates_for_bill(
        self,
        bill_id: int,
        user_id: int = 1,
        tolerance_days: int = 3,
    ) -> dict[str, Any]:
        """获取账单可匹配的定时交易候选。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return {"bill": None, "linked_recurring_id": None, "linked_recurring_name": "", "candidates": []}

        linked_recurring_id = bill.get("created_from_recurring")
        linked_recurring_name = ""
        if linked_recurring_id:
            async with conn.execute(
                "SELECT name FROM recurring_bills WHERE id = ? AND user_id = ?",
                (linked_recurring_id, user_id),
            ) as cursor:
                row = await cursor.fetchone()
                if row:
                    linked_recurring_name = str(row["name"] or "")

        recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
        candidates = self.build_recurring_candidates_for_bill_data(
            bill,
            recurring_rows,
            linked_recurring_id=linked_recurring_id,
            tolerance_days=tolerance_days,
        )
        return {
            "bill": bill,
            "linked_recurring_id": linked_recurring_id,
            "linked_recurring_name": linked_recurring_name,
            "candidates": candidates,
        }

    @log_method
    async def get_enabled_recurring_templates(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取当前用户启用中的定时交易模板。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM recurring_bills
            WHERE user_id = ? AND enabled = 1
            ORDER BY COALESCE(display_order, 0), name
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    def build_recurring_candidates_for_bill_data(
        self,
        bill: dict[str, Any],
        recurring_rows: list[dict[str, Any]],
        linked_recurring_id: Any = None,
        tolerance_days: int = 3,
    ) -> list[dict[str, Any]]:
        """基于账单数据构建定时账单候选列表。"""
        bill_date = self._parse_date_value(bill.get("date"))
        if not bill_date:
            return []

        bill_type = self._normalize_template_transaction_type(bill.get("type"))
        bill_amount_cents = round(abs(float(bill.get("amount") or 0)) * 100)
        bill_source_account = str(bill.get("source_account_id") or "0")
        bill_destination_account = str(bill.get("destination_account_id") or "0")

        candidates: list[dict[str, Any]] = []
        for recurring in recurring_rows:
            recurring_type = self._normalize_template_transaction_type(recurring.get("type"))
            if recurring_type != bill_type:
                continue

            recurring_amount_cents = round(abs(float(recurring.get("amount") or 0)))
            if recurring_amount_cents != bill_amount_cents:
                continue

            matched_occurrence = self._find_recurring_occurrence_near_date(
                recurring,
                bill_date,
                tolerance_days=tolerance_days,
            )
            if not matched_occurrence:
                continue

            score = 80
            reasons: list[str] = ["type", "amount", "schedule"]
            recurring_source_account = str(recurring.get("account") or "0")
            recurring_destination_account = str(recurring.get("counterparty") or "0")

            if recurring_source_account == bill_source_account:
                reasons.append("source_account")
                score += 10
            if bill_destination_account not in ("", "0") and recurring_destination_account == bill_destination_account:
                reasons.append("destination_account")
                score += 10

            days_offset = abs((matched_occurrence - bill_date).days)
            score += max(0, 10 - days_offset * 2)

            candidate = self._serialize_template_row(recurring, template_type=2)
            candidate.update(
                {
                    "matchScore": score,
                    "matchReasons": reasons,
                    "matchedOccurrenceDate": matched_occurrence.isoformat(),
                    "matchedDayOffset": days_offset,
                    "linked": int(linked_recurring_id or 0) == int(recurring.get("id") or 0),
                }
            )
            candidates.append(candidate)

        candidates.sort(
            key=lambda item: (
                -int(item.get("matchScore") or 0),
                int(item.get("matchedDayOffset") or 999),
                str(item.get("name") or ""),
            )
        )
        return candidates

    @log_method
    async def bind_bill_to_recurring(self, bill_id: int, recurring_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """将账单绑定到定时交易，并推进 next_date。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return None

        previous_recurring_id = bill.get("created_from_recurring")
        async with conn.execute(
            "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?",
            (recurring_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            recurring = dict(row) if row else None

        if not recurring:
            return None

        bill_date = self._parse_date_value(bill.get("date"))
        next_occurrence = None
        if bill_date:
            next_date = self._get_next_recurring_occurrence_after(recurring, bill_date)
            next_occurrence = next_date.isoformat() if next_date else None

        now = utc_now_iso()
        await conn.execute(
            "UPDATE bills SET created_from_recurring = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (recurring_id, now, bill_id, user_id),
        )
        await conn.execute(
            "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (next_occurrence or recurring.get("next_date"), now, recurring_id, user_id),
        )

        if previous_recurring_id and str(previous_recurring_id) != str(recurring_id):
            await self._recalculate_recurring_next_date(conn, int(previous_recurring_id), user_id, now)

        await conn.commit()
        return {
            "billId": bill_id,
            "recurringId": recurring_id,
            "nextScheduledDate": next_occurrence or recurring.get("next_date"),
        }

    async def _recalculate_recurring_next_date(
        self,
        conn,
        recurring_id: int,
        user_id: int,
        now: str | None = None,
    ) -> None:
        """根据当前已绑定账单重算定时模板的下一次计划日期。"""
        async with conn.execute(
            "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?",
            (recurring_id, user_id),
        ) as recurring_cursor:
            recurring_row = await recurring_cursor.fetchone()

        recurring = dict(recurring_row) if recurring_row else None
        if not recurring:
            return

        async with conn.execute(
            """
            SELECT date FROM bills
            WHERE user_id = ? AND created_from_recurring = ?
            ORDER BY date DESC
            LIMIT 1
            """,
            (user_id, recurring_id),
        ) as linked_cursor:
            latest_linked_row = await linked_cursor.fetchone()

        latest_linked_date = self._parse_date_value(latest_linked_row["date"] if latest_linked_row else None)
        if latest_linked_date:
            next_date = self._get_next_recurring_occurrence_after(recurring, latest_linked_date)
        else:
            next_date = self._get_first_recurring_occurrence(recurring)

        next_occurrence = next_date.isoformat() if next_date else recurring.get("next_date")
        await conn.execute(
            "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (next_occurrence, now or utc_now_iso(), recurring_id, user_id),
        )

    @log_method
    async def unbind_bill_from_recurring(self, bill_id: int, user_id: int = 1) -> bool:
        """取消账单与定时交易的绑定。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return False

        recurring_id = bill.get("created_from_recurring")
        now = utc_now_iso()
        cursor = await conn.execute(
            "UPDATE bills SET created_from_recurring = NULL, updated_at = ? WHERE id = ? AND user_id = ?",
            (now, bill_id, user_id),
        )

        if recurring_id:
            await self._recalculate_recurring_next_date(conn, int(recurring_id), user_id, now)

        await conn.commit()
        return cursor.rowcount > 0

    def _get_first_recurring_occurrence(self, recurring: dict[str, Any], max_search_days: int = 370) -> date | None:
        """获取定时模板的首次计划日期（包含 start_date 当天）。"""
        start_date = self._parse_date_value(recurring.get("start_date"))
        if not start_date:
            return self._parse_date_value(recurring.get("next_date"))

        for offset in range(0, max_search_days + 1):
            candidate = start_date + timedelta(days=offset)
            if self._is_recurring_due_on_date(recurring, candidate):
                return candidate

        return self._parse_date_value(recurring.get("next_date"))

    def _get_template_table(self, template_type: int | None) -> str:
        """根据模板类型返回表名。"""
        return "recurring_bills" if int(template_type or 1) == 2 else "bill_templates"

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

    def _serialize_template_tag_ids(self, tag_ids: Any) -> str:
        """序列化模板标签 ID 列表。"""
        if isinstance(tag_ids, list):
            return ",".join(str(tag_id) for tag_id in tag_ids if str(tag_id).strip())
        return str(tag_ids or "")

    def _deserialize_template_tag_ids(self, raw_value: Any) -> list[str]:
        """反序列化模板标签 ID 列表。"""
        if not raw_value:
            return []
        if isinstance(raw_value, list):
            return [str(tag_id) for tag_id in raw_value if str(tag_id).strip()]
        return [item.strip() for item in str(raw_value).split(",") if item.strip()]

    def _serialize_template_row(self, row: dict[str, Any], template_type: int) -> dict[str, Any]:
        """将模板表记录统一转换为前端模板 DTO。"""
        source_amount = float(row.get("amount") or 0)
        destination_amount = float(row.get("destination_amount") or 0)
        source_account_id = str(row.get("account") or "0")
        destination_account_id = str(row.get("counterparty") or "0")

        return {
            "id": str(row.get("id")),
            "timeSequenceId": "",
            "templateType": template_type,
            "name": row.get("name", ""),
            "type": self._normalize_template_transaction_type(row.get("type")),
            "categoryId": str(row.get("category") or ""),
            "time": int(row.get("scheduled_at") or 0),
            "utcOffset": int(row.get("utc_offset") or 0),
            "sourceAccountId": source_account_id,
            "destinationAccountId": destination_account_id,
            "sourceAmount": source_amount,
            "destinationAmount": destination_amount,
            "hideAmount": bool(row.get("hide_amount")),
            "tagIds": self._deserialize_template_tag_ids(row.get("tag")),
            "comment": row.get("comment", "") or "",
            "editable": True,
            "displayOrder": int(row.get("display_order") or 0),
            "hidden": bool(row.get("hidden")),
            "scheduledFrequencyType": int(row.get("scheduled_frequency_type") or 0) if template_type == 2 else None,
            "scheduledFrequency": row.get("frequency") if template_type == 2 else None,
            "scheduledStartDate": row.get("start_date") if template_type == 2 else None,
            "scheduledEndDate": row.get("end_date") if template_type == 2 else None,
            "scheduledAt": None,
        }

    def _normalize_template_transaction_type(self, raw_value: Any) -> int:
        """将历史模板类型值统一映射到前端数字枚举。"""
        mapping = {
            "2": 2,
            "3": 3,
            "4": 4,
            "5": 5,
            "income": 2,
            "expense": 3,
            "transfer": 4,
            "investment": 5,
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        }
        if raw_value is None:
            return 3

        textual = str(raw_value).strip().lower()
        return mapping.get(textual, 3)

    def _parse_date_value(self, raw_value: Any) -> date | None:
        """解析 YYYY-MM-DD 或 YYYY-MM-DD HH:MM:SS 格式日期。"""
        if not raw_value:
            return None

        text = str(raw_value).strip()
        if not text:
            return None

        try:
            return datetime.fromisoformat(text[:19]).date()
        except ValueError:
            pass

        try:
            return date.fromisoformat(text[:10])
        except ValueError:
            return None

    def _parse_schedule_frequency_values(self, raw_value: Any) -> list[int]:
        """解析定时频率值，例如 '1,15'。"""
        if not raw_value:
            return []

        values: list[int] = []
        for item in str(raw_value).split(","):
            item = item.strip()
            if not item:
                continue
            try:
                values.append(int(item))
            except ValueError:
                continue
        return sorted(set(values))

    def _weekday_sunday_first(self, target_date: date) -> int:
        """将 Python weekday(Monday=0) 转为 Sunday=0。"""
        return (target_date.weekday() + 1) % 7

    def _is_recurring_active_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
        """判断定时模板在指定日期是否生效。"""
        start_date = self._parse_date_value(recurring.get("start_date"))
        end_date = self._parse_date_value(recurring.get("end_date"))
        if start_date and target_date < start_date:
            return False
        if end_date and target_date > end_date:
            return False
        return True

    def _is_recurring_due_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
        """判断定时模板是否在某天应发生。"""
        if not self._is_recurring_active_on_date(recurring, target_date):
            return False

        frequency_type = int(recurring.get("scheduled_frequency_type") or 0)
        frequency_values = self._parse_schedule_frequency_values(recurring.get("frequency"))
        start_date = self._parse_date_value(recurring.get("start_date"))
        next_date = self._parse_date_value(recurring.get("next_date"))

        if frequency_type == 1:
            if frequency_values:
                valid_weekdays = frequency_values
            elif start_date:
                valid_weekdays = [self._weekday_sunday_first(start_date)]
            else:
                valid_weekdays = []
            return self._weekday_sunday_first(target_date) in valid_weekdays

        if frequency_type == 2:
            if frequency_values:
                valid_days = frequency_values
            elif start_date:
                valid_days = [start_date.day]
            else:
                valid_days = []
            return target_date.day in valid_days

        if next_date:
            return target_date == next_date
        return bool(start_date and target_date == start_date)

    def _find_recurring_occurrence_near_date(
        self,
        recurring: dict[str, Any],
        target_date: date,
        tolerance_days: int,
    ) -> date | None:
        """在容差窗口内寻找最近的计划发生日期。"""
        nearest_date: date | None = None
        nearest_diff: int | None = None

        for offset in range(-tolerance_days, tolerance_days + 1):
            current_date = target_date + timedelta(days=offset)
            if not self._is_recurring_due_on_date(recurring, current_date):
                continue

            diff = abs(offset)
            if nearest_date is None or (nearest_diff is not None and diff < nearest_diff):
                nearest_date = current_date
                nearest_diff = diff

        return nearest_date

    def _get_next_recurring_occurrence_after(
        self,
        recurring: dict[str, Any],
        after_date: date,
        max_search_days: int = 370,
    ) -> date | None:
        """获取指定日期后的下一次计划发生日期。"""
        for offset in range(1, max_search_days + 1):
            candidate = after_date + timedelta(days=offset)
            if self._is_recurring_due_on_date(recurring, candidate):
                return candidate
        return None

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
