"""Investment transaction category persistence tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.api.adapters.transaction_adapter import TransactionAdapter
from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_investment_account(db: Database) -> int:
    account_id = await db.create_account(
        {
            "name": "投资账户",
            "type": 1,
            "category": "asset",
            "currency": "CNY",
            "icon": "",
            "color": "",
            "balance": 0.0,
            "initial_balance": 0.0,
            "hidden": False,
            "display_order": 0,
            "comment": "",
            "aliases": [],
        }
    )
    assert account_id is not None
    return int(account_id)


async def _create_investment_category(db: Database) -> int:
    category_id = await db.create_category(
        {
            "type": 5,
            "main_category": "投资",
            "sub_category": "基金",
            "description": "",
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": "",
            "color": "",
        }
    )
    assert category_id is not None
    return int(category_id)


async def _create_investment_bill(db: Database, *, account_id: int) -> int:
    bill_id = await db.create_bill(
        {
            "date": "2026-05-12 10:30:00",
            "type": "投资",
            "amount": 500.0,
            "counterparty": "测试投资",
            "description": "测试投资分类持久化",
            "main_category": "投资",
            "sub_category": "基金",
            "source_account_id": account_id,
            "destination_account_id": account_id,
        }
    )
    assert bill_id is not None
    return int(bill_id)


async def _adapt_bill(db: Database, bill_id: int) -> dict:
    bill = await db.get_bill_by_id(bill_id)
    assert bill is not None
    tags = await db.get_tags_for_bill(bill_id)
    adapter = TransactionAdapter(db)
    return await adapter.backend_to_frontend(bill, tags=tags)


@pytest.mark.asyncio
async def test_investment_category_persistence(tmp_path: Path) -> None:
    """Investment bill adapters keep category data after reload-like access."""
    db = Database(str(tmp_path / "investment_category_persistence.db"))
    await db.init_db()
    try:
        account_id = await _create_investment_account(db)
        category_id = await _create_investment_category(db)
        bill_id = await _create_investment_bill(db, account_id=account_id)

        first_view = await _adapt_bill(db, bill_id)
        second_view = await _adapt_bill(db, bill_id)

        for adapted in (first_view, second_view):
            assert adapted["categoryId"] == str(category_id)
            assert adapted["categoryName"] == "投资"
            assert adapted["subCategoryName"] == "基金"
            assert adapted["category"]["id"] == str(category_id)
            assert adapted["category"]["name"] == "基金"
    finally:
        await db.close()
