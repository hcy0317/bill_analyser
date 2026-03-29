from __future__ import annotations

import copy
from datetime import datetime
from typing import Any

import pytest

from bill_analyser.api.adapters.transaction_adapter import TransactionAdapter
from bill_analyser.utils.constants import TransactionType


class FakeDB:
    """Minimal async DB stub for adapter tests."""

    def __init__(self) -> None:
        self.account_user_ids: list[int] = []
        self.category_user_ids: list[int] = []

    async def get_all_accounts(self, user_id: int) -> list[dict[str, Any]]:
        self.account_user_ids.append(user_id)
        return [
            {"id": 1, "name": "现金", "parent_id": 0, "currency": "CNY", "balance": 12.34, "aliases": [], "hidden": 0},
            {"id": 2, "name": "银行卡", "parent_id": 0, "currency": "CNY", "balance": 56.78, "aliases": [], "hidden": 0},
        ]

    async def get_all_categories(self, user_id: int) -> list[dict[str, Any]]:
        self.category_user_ids.append(user_id)
        return [
            {
                "id": 10,
                "main_category": "工资",
                "sub_category": "奖金",
                "type": int(TransactionType.INCOME),
                "priority": 3,
                "hidden": 0,
            }
        ]


@pytest.mark.parametrize(
    ("raw_value", "expected"),
    [
        (None, 0),
        ("", 0),
        ("bad", 0),
        (1735689600, 1735689600),
        (1735689600000, 1735689600),
        (-1735689600000, -1735689600),
    ],
)
def test_normalize_frontend_unix_time_handles_multiple_formats(raw_value: Any, expected: int) -> None:
    """时间戳归一化应兼容秒、毫秒与异常输入。"""
    assert TransactionAdapter._normalize_frontend_unix_time(raw_value) == expected



def test_frontend_to_backend_maps_expense_and_income_without_mutating_input() -> None:
    """前端交易转后端账单时应正确处理时间、金额符号与标签过滤。"""
    adapter = TransactionAdapter()
    expense_payload = {
        "type": int(TransactionType.EXPENSE),
        "time": "bad-time",
        "sourceAmount": "12345",
        "destinationAmount": "500",
        "sourceAccountId": "7",
        "destinationAccountId": "8",
        "remark": "午饭",
        "categoryId": "9",
        "tagIds": ["1", "0", "", " 2 ", "3"],
    }
    original_expense_payload = copy.deepcopy(expense_payload)

    backend_expense, expense_metadata = adapter.frontend_to_backend(expense_payload)

    assert backend_expense == {
        "type": "支出",
        "date": "",
        "amount": -123.45,
        "destination_amount": 5.0,
        "source_account_id": 7,
        "destination_account_id": 8,
        "description": "午饭",
    }
    assert expense_metadata == {
        "category_id": "9",
        "source_account_id": 7,
        "destination_account_id": 8,
        "tag_ids": [1, 2, 3],
        "auto_invest_account": False,
    }
    assert expense_payload == original_expense_payload

    income_payload = {
        "type": int(TransactionType.INCOME),
        "time": int(datetime(2025, 1, 2, 3, 4, 5).timestamp()),
        "sourceAmount": 2000,
        "destinationAmount": 0,
        "sourceAccountId": "1",
        "destinationAccountId": "0",
        "comment": "工资",
        "tagIds": [],
    }

    backend_income, income_metadata = adapter.frontend_to_backend(income_payload)

    assert backend_income["type"] == "收入"
    assert backend_income["amount"] == 20.0
    assert backend_income["date"] == "2025-01-02 03:04:05"
    assert backend_income["description"] == "工资"
    assert income_metadata["tag_ids"] == []


@pytest.mark.asyncio
async def test_backend_to_frontend_enriches_category_accounts_tags_and_date_fields() -> None:
    """后端账单应补齐分类、账户、标签和日期展示字段。"""
    adapter = TransactionAdapter()
    account_map = {
        "id_to_account": {
            1: {"id": 1, "name": "现金", "parent_id": 0, "currency": "CNY", "balance": 12.34, "aliases": [], "hidden": 0},
            2: {"id": 2, "name": "银行卡", "parent_id": 0, "currency": "CNY", "balance": 56.78, "aliases": [], "hidden": 0},
        }
    }
    category_map = {
        "id_to_category": {
            10: {
                "id": 10,
                "main_category": "转账",
                "sub_category": "内部转账",
                "type": int(TransactionType.TRANSFER),
                "priority": 1,
                "hidden": 0,
            }
        },
        "name_to_id": {("转账", "内部转账"): 10},
    }

    frontend_bill = await adapter.backend_to_frontend(
        {
            "id": 5,
            "time_sequence_id": 6,
            "type": "转账",
            "date": "2025-01-02 03:04:05",
            "amount": -12.34,
            "destination_amount": 11.11,
            "source_account_id": 1,
            "destination_account_id": 2,
            "main_category": "转账",
            "sub_category": "内部转账",
            "description": "账户互转",
            "utc_offset": 480,
            "hide_amount": 1,
        },
        account_map=account_map,
        category_map=category_map,
        tags=[{"id": 7, "name": "已对账"}, {"id": None, "name": "ignored"}],
    )

    assert frontend_bill["id"] == "5"
    assert frontend_bill["timeSequenceId"] == "6"
    assert frontend_bill["type"] == int(TransactionType.TRANSFER)
    assert frontend_bill["categoryId"] == "10"
    assert frontend_bill["amount"] == 1234
    assert frontend_bill["sourceAmount"] == 1234
    assert frontend_bill["destinationAmount"] == 1111
    assert frontend_bill["hideAmount"] is True
    assert frontend_bill["tagIds"] == ["7"]
    assert frontend_bill["tags"] == [{"id": "7", "name": "已对账"}]
    assert frontend_bill["comment"] == "账户互转"
    assert frontend_bill["category"]["parentId"] == "virtual_转账"
    assert frontend_bill["sourceAccount"]["id"] == "1"
    assert frontend_bill["destinationAccount"]["id"] == "2"
    assert frontend_bill["gregorianCalendarYearDashMonthDashDay"] == "2025-01-02"
    assert frontend_bill["gregorianCalendarDayOfMonth"] == 2
    assert frontend_bill["displayDayOfWeek"] == 5


@pytest.mark.asyncio
async def test_backend_to_frontend_falls_back_for_unknown_type_invalid_date_and_missing_maps() -> None:
    """未知类型、坏日期和缺失映射应走稳妥回退分支。"""
    adapter = TransactionAdapter()

    frontend_bill = await adapter.backend_to_frontend(
        {
            "id": 9,
            "time_sequence_id": 10,
            "type": "退款",
            "date": "not-a-date",
            "amount": 9.99,
            "destination_amount": 0,
            "source_account_id": 0,
            "destination_account_id": 0,
            "main_category": "",
            "sub_category": "",
            "description": "退款回退",
        }
    )

    assert frontend_bill["type"] == int(TransactionType.EXPENSE)
    assert frontend_bill["time"] == 0
    assert frontend_bill["categoryId"] == "0"
    assert frontend_bill["amount"] == 999
    assert frontend_bill["destinationAmount"] == 999
    assert "category" not in frontend_bill
    assert "sourceAccount" not in frontend_bill
    assert "destinationAccount" not in frontend_bill
    assert "gregorianCalendarYearDashMonthDashDay" not in frontend_bill
    assert "displayDayOfWeek" not in frontend_bill


@pytest.mark.asyncio
async def test_backend_list_to_frontend_fetches_maps_from_db_and_builds_page() -> None:
    """批量转换应从数据库拉取映射，并返回统一分页结构。"""
    fake_db = FakeDB()
    adapter = TransactionAdapter(db=fake_db, user_id=42)

    response = await adapter.backend_list_to_frontend(
        [
            {
                "id": 11,
                "time_sequence_id": 11,
                "type": "收入",
                "date": "2025-02-03",
                "amount": 88.8,
                "destination_amount": 0,
                "source_account_id": 1,
                "destination_account_id": 2,
                "main_category": "工资",
                "sub_category": "奖金",
                "description": "季度奖",
            }
        ],
        total=5,
        page=2,
        page_size=10,
    )

    assert fake_db.account_user_ids == [42]
    assert fake_db.category_user_ids == [42]
    assert response["success"] is True
    assert response["result"]["totalCount"] == 5
    assert response["result"]["page"] == 2
    assert response["result"]["pageSize"] == 10
    assert response["result"]["total"] == 5
    assert len(response["result"]["items"]) == 1
    assert response["result"]["items"][0]["sourceAccount"]["name"] == "现金"
    assert response["result"]["items"][0]["destinationAccount"]["name"] == "银行卡"
    assert response["result"]["items"][0]["category"]["id"] == "10"
