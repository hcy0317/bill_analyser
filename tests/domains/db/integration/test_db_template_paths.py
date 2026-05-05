from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core import template_rust_bridge
from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_template_paths.db"))
    await db.init_db()
    return db


async def _create_user(db: Database, username: str) -> int:
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "pytest-hash",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


async def _create_account(db: Database, *, user_id: int, name: str) -> int:
    account_id = await db.create_account(
        {
            "name": name,
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
        },
        user_id=user_id,
    )
    assert account_id is not None
    return int(account_id)


async def _create_category(db: Database, *, user_id: int, main_category: str, sub_category: str) -> int:
    category_id = await db.create_category(
        {
            "type": 3,
            "main_category": main_category,
            "sub_category": sub_category,
            "description": "",
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": "",
            "color": "",
        },
        user_id=user_id,
    )
    assert category_id is not None
    return int(category_id)


async def _create_bill(
    db: Database,
    *,
    user_id: int,
    source_account_id: int,
    destination_account_id: int,
    **overrides: Any,
) -> int:
    payload = {
        "date": "2026-03-09 12:00:00",
        "type": "支出",
        "amount": -12.0,
        "counterparty": "模板匹配商户",
        "description": "模板匹配账单",
        "payment_method": "支付宝",
        "main_category": "模板主类",
        "sub_category": "模板子类",
        "source_account_id": source_account_id,
        "destination_account_id": destination_account_id,
        "destination_amount": 0.0,
    }
    payload.update(overrides)
    bill_id = await db.create_bill(payload, user_id=user_id)
    assert bill_id is not None
    return int(bill_id)


def _build_recurring_template_payload(
    *,
    name: str,
    category_id: int,
    source_account_id: int,
    destination_account_id: int,
    start_date: str,
    tag_ids: list[str] | None = None,
) -> dict[str, Any]:
    return {
        "templateType": 2,
        "name": name,
        "type": 3,
        "categoryId": str(category_id),
        "sourceAccountId": str(source_account_id),
        "destinationAccountId": str(destination_account_id),
        "sourceAmount": 1200,
        "destinationAmount": 0,
        "hideAmount": True,
        "tagIds": tag_ids or ["8", "9"],
        "comment": f"{name} 备注",
        "hidden": True,
        "utcOffset": 0,
        "scheduledFrequencyType": 1,
        "scheduledFrequency": "1",
        "scheduledStartDate": start_date,
        "scheduledEndDate": "2026-12-31",
    }


async def _get_raw_next_date(db: Database, *, recurring_id: int, user_id: int) -> str | None:
    conn = await db._get_connection()
    async with conn.execute(
        "SELECT next_date FROM recurring_bills WHERE id = ? AND user_id = ?",
        (recurring_id, user_id),
    ) as cursor:
        row = await cursor.fetchone()
    return str(row["next_date"]) if row and row["next_date"] else None


@pytest.mark.asyncio
async def test_recurring_template_round_trip_updates_schedule_and_tag_ids(tmp_path: Path) -> None:
    """Recurring 模板应支持 DTO round-trip 更新，并同步 next_date。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_template_round_trip")
        source_account_id = await _create_account(db, user_id=user_id, name="模板源账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="模板目标账户")
        category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="模板主类",
            sub_category="模板子类",
        )

        recurring_id = await db.create_template(
            _build_recurring_template_payload(
                name="每周订阅",
                category_id=category_id,
                source_account_id=source_account_id,
                destination_account_id=destination_account_id,
                start_date="2026-03-02",
            ),
            user_id=user_id,
        )

        created = await db.get_template_by_id(recurring_id, user_id=user_id, template_type=2)
        assert created is not None
        assert created["templateType"] == 2
        assert created["tagIds"] == ["8", "9"]
        assert created["scheduledFrequencyType"] == 1
        assert created["scheduledFrequency"] == "1"
        assert created["scheduledStartDate"] == "2026-03-02"
        assert created["hideAmount"] is True
        assert created["hidden"] is True

        updated = await db.update_template(
            recurring_id,
            {
                "tagIds": ["9", "10"],
                "scheduledFrequencyType": 2,
                "scheduledFrequency": "7,21",
                "scheduledStartDate": "2026-04-07",
                "scheduledEndDate": "2026-12-25",
                "hideAmount": False,
                "hidden": False,
            },
            user_id=user_id,
            template_type=2,
        )
        assert updated is True

        refreshed = await db.get_template_by_id(recurring_id, user_id=user_id, template_type=2)
        assert refreshed is not None
        assert refreshed["tagIds"] == ["9", "10"]
        assert refreshed["scheduledFrequencyType"] == 2
        assert refreshed["scheduledFrequency"] == "7,21"
        assert refreshed["scheduledStartDate"] == "2026-04-07"
        assert refreshed["scheduledEndDate"] == "2026-12-25"
        assert refreshed["hideAmount"] is False
        assert refreshed["hidden"] is False
        assert await _get_raw_next_date(db, recurring_id=recurring_id, user_id=user_id) == "2026-04-07"
        assert len(await db.get_all_templates(user_id=user_id, template_type=2)) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_template_crud_keeps_python_fallback_for_in_memory_db(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """In-memory databases must not invoke the Rust subprocess bridge."""
    db = Database(":memory:")
    await db.init_db()
    try:
        user_id = await _create_user(db, "db_template_memory_fallback")

        def _fail_bridge(*_args: Any, **_kwargs: Any) -> Any:
            raise AssertionError("template Rust bridge should not be called for in-memory DBs")

        monkeypatch.setattr(template_rust_bridge, "create_template", _fail_bridge)
        monkeypatch.setattr(template_rust_bridge, "list_templates", _fail_bridge)
        monkeypatch.setattr(template_rust_bridge, "get_template", _fail_bridge)
        monkeypatch.setattr(template_rust_bridge, "update_template", _fail_bridge)
        monkeypatch.setattr(template_rust_bridge, "delete_template", _fail_bridge)
        monkeypatch.setattr(template_rust_bridge, "update_display_orders", _fail_bridge)

        template_id = await db.create_template(
            {
                "templateType": 1,
                "name": "内存模板",
                "type": 3,
                "categoryId": "101",
                "sourceAccountId": "11",
                "sourceAmount": 123,
            },
            user_id=user_id,
        )
        assert await db.get_template_by_id(template_id, user_id=user_id, template_type=1)
        assert len(await db.get_all_templates(user_id=user_id, template_type=1)) == 1
        assert await db.update_template(
            template_id,
            {"name": "内存模板更新"},
            user_id=user_id,
            template_type=1,
        )
        assert await db.update_template_display_orders(
            [(template_id, 1)],
            template_type=1,
            user_id=user_id,
        )
        assert await db.delete_template(template_id, user_id=user_id, template_type=1)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_bind_rebind_and_unbind_bill_from_recurring_recalculate_next_dates(tmp_path: Path) -> None:
    """账单绑定/改绑/解绑 recurring 时，应推进或回退 next_date。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_template_binding")
        source_account_id = await _create_account(db, user_id=user_id, name="扣款账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="收款账户")
        category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="模板主类",
            sub_category="模板子类",
        )
        recurring_a_id = await db.create_template(
            _build_recurring_template_payload(
                name="订阅 A",
                category_id=category_id,
                source_account_id=source_account_id,
                destination_account_id=destination_account_id,
                start_date="2026-03-02",
            ),
            user_id=user_id,
        )
        recurring_b_id = await db.create_template(
            _build_recurring_template_payload(
                name="订阅 B",
                category_id=category_id,
                source_account_id=source_account_id,
                destination_account_id=destination_account_id,
                start_date="2026-03-02",
                tag_ids=["10"],
            ),
            user_id=user_id,
        )
        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            destination_account_id=destination_account_id,
        )

        candidates_payload = await db.get_recurring_candidates_for_bill(bill_id, user_id=user_id, tolerance_days=2)
        assert {int(candidate["id"]) for candidate in candidates_payload["candidates"]} == {
            recurring_a_id,
            recurring_b_id,
        }

        bind_a = await db.bind_bill_to_recurring(bill_id, recurring_a_id, user_id=user_id)
        assert bind_a == {
            "billId": bill_id,
            "recurringId": recurring_a_id,
            "nextScheduledDate": "2026-03-16",
        }
        bill_after_first_bind = await db.get_bill_by_id(bill_id, user_id=user_id)
        assert bill_after_first_bind is not None
        assert int(bill_after_first_bind["created_from_recurring"]) == recurring_a_id
        assert await _get_raw_next_date(db, recurring_id=recurring_a_id, user_id=user_id) == "2026-03-16"

        bind_b = await db.bind_bill_to_recurring(bill_id, recurring_b_id, user_id=user_id)
        assert bind_b == {
            "billId": bill_id,
            "recurringId": recurring_b_id,
            "nextScheduledDate": "2026-03-16",
        }
        bill_after_rebind = await db.get_bill_by_id(bill_id, user_id=user_id)
        assert bill_after_rebind is not None
        assert int(bill_after_rebind["created_from_recurring"]) == recurring_b_id
        assert await _get_raw_next_date(db, recurring_id=recurring_a_id, user_id=user_id) == "2026-03-02"
        assert await _get_raw_next_date(db, recurring_id=recurring_b_id, user_id=user_id) == "2026-03-16"

        assert await db.unbind_bill_from_recurring(bill_id, user_id=user_id) is True
        bill_after_unbind = await db.get_bill_by_id(bill_id, user_id=user_id)
        assert bill_after_unbind is not None
        assert bill_after_unbind["created_from_recurring"] is None
        assert await _get_raw_next_date(db, recurring_id=recurring_b_id, user_id=user_id) == "2026-03-02"
    finally:
        await db.close()
