"""Template DTO serialization helpers."""

# pylint: disable=line-too-long,protected-access,unused-argument

from __future__ import annotations

from typing import Any


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
