from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from datetime import datetime
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_account_learning.db"))
    await db.init_db()
    return db


async def _create_account(
    db: Database,
    *,
    name: str,
    aliases: Any = None,
    initial_balance: float = 0.0,
    balance: float | None = None,
) -> int:
    return await db.create_account(
        {
            "name": name,
            "type": 1,
            "category": "asset",
            "currency": "CNY",
            "icon": "",
            "color": "",
            "balance": initial_balance if balance is None else balance,
            "initial_balance": initial_balance,
            "hidden": False,
            "display_order": 0,
            "comment": "",
            "aliases": aliases,
        },
        user_id=1,
    )


async def _insert_bill(
    db: Database,
    *,
    bill_type: str,
    amount: float,
    date_text: str,
    source_account_id: int = 0,
    destination_account_id: int = 0,
    destination_amount: float = 0.0,
    payment_method: str = "",
    counterparty: str = "",
    description: str = "",
) -> None:
    conn = await db._get_connection()
    now = datetime.now().isoformat()
    await conn.execute(
        """
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description, payment_method,
            created_at, updated_at, source_account_id, destination_account_id, destination_amount
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            1,
            date_text,
            bill_type,
            amount,
            counterparty,
            description,
            payment_method,
            now,
            now,
            source_account_id,
            destination_account_id,
            destination_amount,
        ),
    )
    await conn.commit()



def test_import_learning_text_helpers_normalize_and_build_stable_composite_keys() -> None:
    """导入学习 helper 应稳定归一文本并生成有序复合匹配键。"""
    assert Database._normalize_import_learning_text(None) == ""
    assert Database._normalize_import_learning_text("  早餐铺 | 早餐铺   ") == "早餐铺 | 早餐铺"
    assert Database._normalize_import_learning_text("  招商 银行  ") == "招商 银行"

    features = Database.build_composite_match_features(
        parser_id=" WeChat ",
        counterparty=" 早餐铺 ",
        description="  ",
        payment_method=" 微信支付 ",
    )
    assert features == {
        "parser_id": "wechat",
        "counterparty": "早餐铺",
        "payment_method": "微信支付",
    }

    assert Database.build_composite_match_hash("", "早餐铺", "", "") is None
    assert Database.build_composite_match_hash("WeChat", "早餐铺", "共同描述", "微信支付") == (
        "c=早餐铺|d=共同描述|p=wechat|m=微信支付"
    )


@pytest.mark.asyncio
async def test_get_account_alias_mapping_includes_account_names_and_skips_invalid_alias_json(tmp_path: Path) -> None:
    """账户别名映射应同时包含账户名大小写键，并在坏 JSON 时安全跳过。"""
    db = await _create_database(tmp_path)
    try:
        wallet_id = await _create_account(db, name="支付宝", aliases=["小荷包", " 备用钱包 "])
        bank_id = await _create_account(db, name="招行卡", aliases="not-json")
        cash_id = await _create_account(db, name="现金", aliases=None)

        alias_map = await db.get_account_alias_mapping(user_id=1)

        assert alias_map["支付宝"] == wallet_id
        assert alias_map["支付宝".lower()] == wallet_id
        assert alias_map["小荷包"] == wallet_id
        assert alias_map["小荷包".lower()] == wallet_id
        assert alias_map["备用钱包"] == wallet_id
        assert alias_map["招行卡"] == bank_id
        assert alias_map["招行卡".lower()] == bank_id
        assert alias_map["现金"] == cash_id
        assert "not-json" not in alias_map
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_historical_account_suggestions_score_matches_and_exclude_source_account(tmp_path: Path) -> None:
    """历史源/目标账户建议应根据匹配字段得分，并排除与源账户相同的目标账户。"""
    db = await _create_database(tmp_path)
    try:
        source_best = await _create_account(db, name="招商银行卡")
        source_other = await _create_account(db, name="微信零钱")
        destination_best = await _create_account(db, name="转入账户A")
        destination_other = await _create_account(db, name="转入账户B")

        await _insert_bill(
            db,
            bill_type="支出",
            amount=15.0,
            date_text="2025-01-02 08:00:00",
            source_account_id=source_best,
            payment_method="招商银行卡",
            counterparty="早餐铺",
            description="豆浆油条",
        )
        await _insert_bill(
            db,
            bill_type="支出",
            amount=15.0,
            date_text="2025-01-03 08:00:00",
            source_account_id=source_best,
            payment_method="招商银行卡",
            counterparty="早餐铺",
            description="豆浆油条",
        )
        await _insert_bill(
            db,
            bill_type="支出",
            amount=15.0,
            date_text="2025-01-04 08:00:00",
            source_account_id=source_other,
            payment_method="招商银行卡",
            counterparty="早餐铺",
            description="别的描述",
        )

        await _insert_bill(
            db,
            bill_type="转账",
            amount=200.0,
            date_text="2025-01-05 09:00:00",
            source_account_id=source_best,
            destination_account_id=destination_best,
            destination_amount=200.0,
            counterparty="转入账户",
            description="自动转入",
        )
        await _insert_bill(
            db,
            bill_type="转账",
            amount=100.0,
            date_text="2025-01-06 09:00:00",
            source_account_id=source_best,
            destination_account_id=destination_other,
            destination_amount=100.0,
            counterparty="转入账户",
            description="其他说明",
        )

        source_suggestion = await db.get_historical_source_account_suggestion(
            user_id=1,
            payment_method="招商银行卡",
            counterparty="早餐铺",
            description="豆浆油条",
            bill_type="支出",
        )
        assert source_suggestion is not None
        assert source_suggestion["account_id"] == source_best
        assert source_suggestion["score"] > 0
        assert set(source_suggestion["reasons"]) == {"payment_method", "counterparty", "description", "type"}
        assert source_suggestion["hits"] == 2

        assert (
            await db.get_historical_source_account_suggestion(
                user_id=1,
                payment_method="",
                counterparty="",
                description="",
                bill_type="支出",
            )
            is None
        )

        destination_suggestion = await db.get_historical_destination_account_suggestion(
            user_id=1,
            counterparty="转入账户",
            description="自动转入",
            bill_type="转账",
            source_account_id=destination_best,
        )
        assert destination_suggestion is not None
        assert destination_suggestion["account_id"] == destination_other
        assert set(destination_suggestion["reasons"]) == {"counterparty", "type"}
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_calculate_and_sync_account_balances_cover_single_and_batch_paths(tmp_path: Path) -> None:
    """余额计算与同步应覆盖收入/支出/转账/投资的主链路径。"""
    db = await _create_database(tmp_path)
    try:
        primary_account = await _create_account(db, name="主账户", initial_balance=1000.0, balance=900.0)
        peer_account = await _create_account(db, name="对手账户", initial_balance=200.0, balance=100.0)

        await _insert_bill(
            db,
            bill_type="收入",
            amount=500.0,
            date_text="2025-01-02 10:00:00",
            source_account_id=primary_account,
            counterparty="公司",
            description="工资",
        )
        await _insert_bill(
            db,
            bill_type="支出",
            amount=200.0,
            date_text="2025-01-03 10:00:00",
            source_account_id=primary_account,
            counterparty="餐厅",
            description="午餐",
        )
        await _insert_bill(
            db,
            bill_type="转账",
            amount=300.0,
            date_text="2025-01-04 10:00:00",
            source_account_id=primary_account,
            destination_account_id=peer_account,
            destination_amount=300.0,
            counterparty="内部转账",
            description="转出",
        )
        await _insert_bill(
            db,
            bill_type="转账",
            amount=100.0,
            date_text="2025-01-05 10:00:00",
            source_account_id=peer_account,
            destination_account_id=primary_account,
            destination_amount=100.0,
            counterparty="内部转账",
            description="转入",
        )
        await _insert_bill(
            db,
            bill_type="投资",
            amount=150.0,
            date_text="2025-01-06 10:00:00",
            source_account_id=primary_account,
            destination_account_id=peer_account,
            destination_amount=150.0,
            counterparty="基金公司",
            description="申购",
        )
        await _insert_bill(
            db,
            bill_type="投资",
            amount=50.0,
            date_text="2025-01-07 10:00:00",
            source_account_id=peer_account,
            destination_account_id=primary_account,
            destination_amount=50.0,
            counterparty="基金公司",
            description="赎回",
        )

        assert await db.calculate_account_balance(primary_account, "主账户") == pytest.approx(1000.0)
        assert await db.calculate_account_balance(peer_account, "对手账户") == pytest.approx(500.0)
        assert await db.calculate_account_balance(999999) == 0.0

        assert await db.sync_account_balance(primary_account) is True
        assert await db.sync_account_balance(999999) is False

        result = await db.sync_all_account_balances(user_id=1)
        assert result["total_accounts"] == 2
        assert result["synced_accounts"] == 2
        assert result["errors"] == []
        assert len(result["discrepancies"]) == 1
        assert {item["account_id"] for item in result["discrepancies"]} == {peer_account}

        refreshed_primary = await db.get_account_by_id(primary_account, user_id=1)
        refreshed_peer = await db.get_account_by_id(peer_account, user_id=1)
        assert refreshed_primary is not None and refreshed_primary["balance"] == pytest.approx(1000.0)
        assert refreshed_peer is not None and refreshed_peer["balance"] == pytest.approx(500.0)
    finally:
        await db.close()
