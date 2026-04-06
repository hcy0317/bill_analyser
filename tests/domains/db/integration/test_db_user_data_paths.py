from __future__ import annotations

import json
from datetime import UTC, datetime
from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core import db_user_data as db_user_data_module
from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_user_data_paths.db"))
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


async def _create_bill(db: Database, *, user_id: int, account_id: int, category_suffix: str) -> int:
    bill_id = await db.create_bill(
        {
            "date": "2026-04-05 10:00:00",
            "type": "支出",
            "amount": -32.5,
            "counterparty": f"商户{category_suffix}",
            "description": f"描述{category_suffix}",
            "payment_method": "支付宝",
            "main_category": f"主类{category_suffix}",
            "sub_category": f"子类{category_suffix}",
            "source_account_id": account_id,
            "destination_account_id": 0,
            "destination_amount": 0.0,
        },
        user_id=user_id,
    )
    assert bill_id is not None
    return int(bill_id)


async def _insert_saved_filter(db: Database, *, user_id: int, name: str) -> None:
    conn = await db._get_connection()
    now = datetime.now(UTC).isoformat()
    await conn.execute(
        """
        INSERT INTO saved_filters (user_id, name, filter_data, description, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (user_id, name, json.dumps({"keyword": name}, ensure_ascii=False), f"{name} 描述", now, now),
    )
    await conn.commit()


async def _seed_user_domain_records(db: Database, *, user_id: int, suffix: str) -> dict[str, Any]:
    account_id = await _create_account(db, user_id=user_id, name=f"账户{suffix}")
    category_id = await _create_category(
        db,
        user_id=user_id,
        main_category=f"主类{suffix}",
        sub_category=f"子类{suffix}",
    )
    tag_id = await db.create_tag(
        {
            "name": f"标签{suffix}",
            "color": "#123456",
            "icon": "mdi-tag",
            "hidden": False,
        },
        user_id=user_id,
    )
    bill_id = await _create_bill(db, user_id=user_id, account_id=account_id, category_suffix=suffix)
    assert await db.add_tags_to_bill(bill_id, [tag_id], user_id=user_id) is True

    template_id = await db.create_template(
        {
            "templateType": 1,
            "name": f"普通模板{suffix}",
            "type": 3,
            "categoryId": str(category_id),
            "sourceAccountId": str(account_id),
            "destinationAccountId": "0",
            "sourceAmount": 1880,
            "destinationAmount": 0,
            "hideAmount": False,
            "tagIds": [str(tag_id)],
            "comment": f"模板备注{suffix}",
            "hidden": False,
            "utcOffset": 0,
        },
        user_id=user_id,
    )
    recurring_id = await db.create_template(
        {
            "templateType": 2,
            "name": f"定时模板{suffix}",
            "type": 3,
            "categoryId": str(category_id),
            "sourceAccountId": str(account_id),
            "destinationAccountId": "0",
            "sourceAmount": 1880,
            "destinationAmount": 0,
            "hideAmount": False,
            "tagIds": [str(tag_id)],
            "comment": f"定时模板备注{suffix}",
            "hidden": False,
            "utcOffset": 0,
            "scheduledFrequencyType": 1,
            "scheduledFrequency": "1",
            "scheduledStartDate": "2026-04-06",
            "scheduledEndDate": "2026-12-31",
        },
        user_id=user_id,
    )
    budget_timestamp = datetime.now(UTC).strftime("%Y-%m-%d %H:%M:%S")
    budget_id = await db.create_budget(
        {
            "name": f"预算{suffix}",
            "category": f"预算主类{suffix}",
            "sub_category": "",
            "period_type": "monthly",
            "amount": 300.0,
            "start_date": "2026-04-01",
            "end_date": None,
            "alert_threshold": 80,
            "enabled": 1,
            "created_at": budget_timestamp,
            "updated_at": budget_timestamp,
        },
        user_id=user_id,
    )
    session_id = f"session-{suffix}"
    await db.create_import_session(session_id, user_id=user_id, file_count=1)
    preview_id = await db.insert_preview_bill(
        session_id,
        {
            "preview_date": "2026-04-05 10:00:00",
            "preview_type": "支出",
            "preview_amount": 32.5,
            "preview_main_category": f"主类{suffix}",
            "preview_sub_category": f"子类{suffix}",
            "preview_counterparty": f"预览商户{suffix}",
            "preview_payment_method": "支付宝",
            "preview_description": f"预览备注{suffix}",
        },
        user_id=user_id,
    )
    await _insert_saved_filter(db, user_id=user_id, name=f"筛选{suffix}")

    conn = await db._get_connection()
    now = datetime.now(UTC).isoformat()
    await conn.execute(
        """
        INSERT INTO budget_history (
            user_id, budget_id, period_start, period_end,
            budget_amount, spent_amount, remaining_amount,
            execution_rate, status, filter_summary, calculated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            budget_id,
            "2026-04-01",
            "2026-04-30",
            300.0,
            128.5,
            171.5,
            0.4283,
            "normal",
            "pytest",
            now,
        ),
    )
    await conn.execute(
        """
        INSERT INTO bills_parser_template (
            session_id, user_id, parser_date, parser_amount, parser_type,
            parser_description, parser_id, parser_counterparty, parser_payment_method,
            parser_original_type, parser_original_category, parser_account_id,
            parser_is_processed, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)
        """,
        (
            session_id,
            user_id,
            "2026-04-05 09:59:59",
            32.5,
            "支出",
            f"解析备注{suffix}",
            "wechat",
            f"解析对手方{suffix}",
            "微信支付",
            "消费",
            "",
            account_id,
            now,
        ),
    )
    await conn.commit()

    return {
        "account_id": account_id,
        "category_id": category_id,
        "tag_id": tag_id,
        "bill_id": bill_id,
        "template_id": template_id,
        "recurring_id": recurring_id,
        "budget_id": budget_id,
        "preview_id": preview_id,
        "session_id": session_id,
    }


async def _count_rows(db: Database, query: str, params: tuple[Any, ...]) -> int:
    conn = await db._get_connection()
    async with conn.execute(query, params) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


@pytest.mark.asyncio
async def test_clear_user_transactions_removes_only_current_user_bills_and_bill_tags(tmp_path: Path) -> None:
    """清空交易只应删除当前用户账单与 bill_tags，并保留其他业务主数据。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_user_tx_clear")
        other_user_id = await _create_user(db, "db_user_tx_clear_other")
        user_records = await _seed_user_domain_records(db, user_id=user_id, suffix="tx-user")
        other_records = await _seed_user_domain_records(db, user_id=other_user_id, suffix="tx-other")

        result = await db.clear_user_transactions(user_id=user_id)
        assert result == {"success": True, "deleted_count": 1}
        assert await db.get_bills(user_id=user_id) == []
        assert len(await db.get_bills(user_id=other_user_id)) == 1
        assert await _count_rows(
            db,
            "SELECT COUNT(*) FROM bill_tags WHERE bill_id = ?",
            (user_records["bill_id"],),
        ) == 0
        assert await _count_rows(
            db,
            "SELECT COUNT(*) FROM bill_tags WHERE bill_id = ?",
            (other_records["bill_id"],),
        ) == 1
        assert len(await db.get_all_accounts(user_id=user_id)) == 1
        assert await _count_rows(
            db,
            "SELECT COUNT(*) FROM categories WHERE user_id = ? AND id = ?",
            (user_id, user_records["category_id"]),
        ) == 1

        second_result = await db.clear_user_transactions(user_id=user_id)
        assert second_result == {"success": True, "deleted_count": 0}
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_clear_user_data_cascades_across_business_tables_for_one_user_only(tmp_path: Path) -> None:
    """清空全部数据应覆盖导入预览、预算、模板等多表，并保持其他用户数据不变。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_user_data_clear")
        other_user_id = await _create_user(db, "db_user_data_clear_other")
        user_records = await _seed_user_domain_records(db, user_id=user_id, suffix="clear-user")
        other_records = await _seed_user_domain_records(db, user_id=other_user_id, suffix="clear-other")

        result = await db.clear_user_data(user_id=user_id)

        assert result["success"] is True
        assert result["counts"]["bills"] == 1
        assert result["counts"]["accounts"] == 1
        assert result["counts"]["categories"] == 1
        assert result["counts"]["tags"] == 1
        assert result["counts"]["templates"] == 1
        assert result["counts"]["recurring_bills"] == 1
        assert result["counts"]["budgets"] >= 1

        assert await _count_rows(db, "SELECT COUNT(*) FROM bills WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM accounts WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM categories WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM tags WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM budgets WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(
            db,
            "SELECT COUNT(*) FROM budget_history WHERE budget_id = ?",
            (user_records["budget_id"],),
        ) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM import_sessions WHERE user_id = ?", (user_id,)) == 0
        assert await _count_rows(db, "SELECT COUNT(*) FROM bills_preview WHERE user_id = ?", (user_id,)) == 0
        assert (
            await _count_rows(
                db,
                "SELECT COUNT(*) FROM bills_parser_template WHERE user_id = ?",
                (user_id,),
            )
            == 0
        )
        assert await _count_rows(db, "SELECT COUNT(*) FROM saved_filters WHERE user_id = ?", (user_id,)) == 0

        user_statistics = await db.get_user_data_statistics(user_id=user_id)
        assert user_statistics == {
            "billCount": 0,
            "accountCount": 0,
            "categoryCount": 0,
            "tagCount": 0,
            "templateCount": 0,
        }

        assert await _count_rows(db, "SELECT COUNT(*) FROM bills WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM accounts WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM categories WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM tags WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM budgets WHERE user_id = ?", (other_user_id,)) >= 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM import_sessions WHERE user_id = ?", (other_user_id,)) == 1
        assert await _count_rows(db, "SELECT COUNT(*) FROM bills_preview WHERE user_id = ?", (other_user_id,)) == 1
        assert (
            await _count_rows(
                db,
                "SELECT COUNT(*) FROM bills_parser_template WHERE user_id = ?",
                (other_user_id,),
            )
            == 1
        )
        assert await _count_rows(db, "SELECT COUNT(*) FROM saved_filters WHERE user_id = ?", (other_user_id,)) == 1
        assert len(await db.get_all_templates(user_id=other_user_id)) == 2
        assert await _count_rows(
            db,
            "SELECT COUNT(*) FROM budget_history WHERE budget_id = ?",
            (other_records["budget_id"],),
        ) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_user_custom_exchange_rates_round_trip_is_user_scoped(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """自定义汇率应大小写归一、支持覆盖更新，并保持不同用户互不影响。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_user_rates")
        other_user_id = await _create_user(db, "db_user_rates_other")
        patched_now = datetime(2026, 4, 6, 9, 30, 0, tzinfo=UTC)
        monkeypatch.setattr(db_user_data_module, "utc_now_iso", lambda: patched_now.isoformat())
        monkeypatch.setattr(db_user_data_module, "utc_now", lambda: patched_now)

        result = await db.upsert_user_custom_exchange_rate("cny", "usd", 7.21, user_id=user_id)
        other_result = await db.upsert_user_custom_exchange_rate("cny", "eur", 0.13, user_id=other_user_id)
        assert result["success"] is True
        assert other_result["success"] is True
        assert result["from_currency"] == "CNY"
        assert result["to_currency"] == "USD"

        updated_result = await db.upsert_user_custom_exchange_rate("CNY", "USD", 7.11, user_id=user_id)
        assert updated_result["success"] is True
        assert updated_result["rate"] == pytest.approx(7.11)

        user_rates = await db.get_user_custom_exchange_rates("cny", user_id=user_id)
        other_user_rates = await db.get_user_custom_exchange_rates("CNY", user_id=other_user_id)
        assert [(rate["from_currency"], rate["to_currency"], rate["rate"]) for rate in user_rates] == [
            ("CNY", "USD", pytest.approx(7.11)),
        ]
        assert [(rate["from_currency"], rate["to_currency"], rate["rate"]) for rate in other_user_rates] == [
            ("CNY", "EUR", pytest.approx(0.13)),
        ]

        assert await db.delete_user_custom_exchange_rate("cny", "usd", user_id=user_id) is True
        assert await db.get_user_custom_exchange_rates("CNY", user_id=user_id) == []
        remaining_other_rates = await db.get_user_custom_exchange_rates("cny", user_id=other_user_id)
        assert len(remaining_other_rates) == 1
        assert remaining_other_rates[0]["to_currency"] == "EUR"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_user_custom_exchange_rates_allow_same_currency_pair_across_users(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """不同用户在同一时间写入同一货币对时，也应各自拥有独立记录。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_user_same_pair")
        other_user_id = await _create_user(db, "db_user_same_pair_other")
        patched_now = datetime(2026, 4, 6, 12, 0, 0, tzinfo=UTC)
        monkeypatch.setattr(db_user_data_module, "utc_now_iso", lambda: patched_now.isoformat())
        monkeypatch.setattr(db_user_data_module, "utc_now", lambda: patched_now)

        first_result = await db.upsert_user_custom_exchange_rate("cny", "usd", 7.25, user_id=user_id)
        second_result = await db.upsert_user_custom_exchange_rate("CNY", "USD", 6.98, user_id=other_user_id)

        assert first_result["success"] is True
        assert second_result["success"] is True

        user_rates = await db.get_user_custom_exchange_rates("cny", user_id=user_id)
        other_user_rates = await db.get_user_custom_exchange_rates("cny", user_id=other_user_id)
        assert len(user_rates) == 1
        assert len(other_user_rates) == 1
        assert user_rates[0]["to_currency"] == "USD"
        assert other_user_rates[0]["to_currency"] == "USD"
        assert user_rates[0]["rate"] == pytest.approx(7.25)
        assert other_user_rates[0]["rate"] == pytest.approx(6.98)
    finally:
        await db.close()
