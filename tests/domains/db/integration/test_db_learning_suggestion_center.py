"""独立学习建议中心 — DB mixin 与 API 路由回归测试。"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.db import Database


# ── Helpers ───────────────────────────────────────────────────────────


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_learning_suggestion_center.db"))
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
            "balance": 0,
            "include_in_total": 1,
        },
        user_id=user_id,
    )
    return account_id


async def _create_preview_with_annotation(
    db: Database,
    *,
    session_id: str,
    user_id: int,
    preview_data: dict[str, Any],
    annotation: dict[str, Any],
) -> int:
    """Insert a preview row + annotation sample for mining tests."""
    conn = await db._get_connection()
    now = "2025-01-01T00:00:00Z"

    await conn.execute(
        """
        INSERT INTO bills_preview (
            session_id, user_id,
            preview_date, preview_amount, preview_description,
            preview_counterparty, preview_payment_method, preview_parser_id,
            preview_type, preview_main_category, preview_sub_category,
            created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            session_id,
            user_id,
            preview_data.get("date", "2025-01-15"),
            preview_data.get("amount", 100),
            preview_data.get("description", "测试描述"),
            preview_data.get("counterparty", "测试商户"),
            preview_data.get("payment_method", "微信支付"),
            preview_data.get("parser_id", "wechat"),
            preview_data.get("type", "支出"),
            preview_data.get("main_category", "餐饮"),
            preview_data.get("sub_category", ""),
            now,
        ),
    )
    async with conn.execute("SELECT last_insert_rowid() AS id") as cur:
        row = await cur.fetchone()
        preview_id = int(row["id"] if isinstance(row, dict) else row[0])

    await db.save_import_annotation_samples(
        session_id,
        [
            {
                "preview_id": preview_id,
                "annotated_type": annotation.get("type", "支出"),
                "annotated_category_id": annotation.get("category_id"),
                "annotated_source_account_id": annotation.get("source_account_id"),
                "annotated_destination_account_id": annotation.get("destination_account_id"),
            },
        ],
        user_id=user_id,
    )
    return preview_id


async def _create_composite_rule(
    db: Database,
    *,
    user_id: int,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    learned_type: str,
) -> int:
    conn = await db._get_connection()  # pylint: disable=protected-access
    now = "2026-08-01T09:00:00"
    composite_hash = db.build_composite_match_hash(
        parser_id=parser_id,
        counterparty=counterparty,
        description=description,
        payment_method=payment_method,
    )
    match_features = db.build_composite_match_features(
        parser_id=parser_id,
        counterparty=counterparty,
        description=description,
        payment_method=payment_method,
    )
    assert composite_hash is not None
    assert match_features is not None
    cursor = await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, enabled, parser_id, composite_match_hash,
            match_features_json, created_at, updated_at
        ) VALUES (?, 'composite', ?, ?, ?, 1, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            composite_hash,
            composite_hash,
            learned_type,
            parser_id,
            composite_hash,
            json.dumps(match_features, ensure_ascii=False, sort_keys=True),
            now,
            now,
        ),
    )
    await conn.commit()
    return int(cursor.lastrowid or 0)


# ── DB Mixin Tests ────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_mine_learning_suggestions_creates_pending_suggestions(tmp_path: Path) -> None:
    """mine_learning_suggestions 应从历史标注中生成 pending 建议。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "miner_test_user")
        session_id = "mine-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "星巴克",
                "description": "拿铁咖啡",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "支出", "category_id": 10},
        )
        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "星巴克",
                "description": "拿铁咖啡",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "支出", "category_id": 10},
        )

        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["total_annotations"] == 2
        assert stats["mined"] >= 1
        assert stats["created"] >= 1

        suggestions = await db.get_learning_suggestions(user_id=user_id)
        assert len(suggestions) >= 1
        assert suggestions[0]["status"] == "pending"
        assert suggestions[0]["sample_count"] == 2

        total = await db.count_learning_suggestions(user_id=user_id, status="pending")
        assert total >= 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_mine_learning_suggestions_uses_durable_corpus_after_session_cleanup(tmp_path: Path) -> None:
    """历史挖掘应使用 durable corpus，不能因 session 临时表清理而丢失语料。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "durable_corpus_miner")
        session_id = "durable-corpus-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "耐久商户",
                "description": "耐久描述",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "支出", "category_id": 10},
        )

        corpus_samples = await db.get_import_learning_corpus_samples(user_id=user_id)
        assert len(corpus_samples) == 1
        assert corpus_samples[0]["preview_id"] == preview_id
        assert corpus_samples[0]["composite_match_hash"] == "c=耐久商户|d=耐久描述|p=wechat|m=微信支付"

        cleared = await db.clear_session_data(session_id, user_id=user_id)
        assert cleared["annotation_count"] == 1
        assert await db.get_import_annotation_samples(session_id, user_id=user_id) == []

        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["total_annotations"] == 1
        assert stats["mined"] == 1
        assert stats["created"] == 1

        suggestions = await db.get_learning_suggestions(user_id=user_id, status="pending")
        assert len(suggestions) == 1
        assert suggestions[0]["source_session_ids_json"] == f'["{session_id}"]'
        assert suggestions[0]["source_preview_ids_json"] == f"[{preview_id}]"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_mine_learning_suggestions_skips_conflicting_annotations(tmp_path: Path) -> None:
    """同一 composite_hash 下不同标注应被跳过（冲突）。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "conflict_miner")
        session_id = "conflict-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "同商户",
                "description": "同描述",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "支出", "category_id": 10},
        )
        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "同商户",
                "description": "同描述",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "收入", "category_id": 20},
        )

        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["skipped_conflict"] >= 1

        suggestions = await db.get_learning_suggestions(user_id=user_id, status="pending")
        assert len(suggestions) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_mine_learning_suggestions_skips_existing_rules(tmp_path: Path) -> None:
    """已有同 hash 的学习规则时，不重复生成建议。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "existing_rule_miner")
        session_id = "rule-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "有规则商户",
                "description": "有规则描述",
                "parser_id": "wechat",
                "payment_method": "微信支付",
            },
            annotation={"type": "支出", "category_id": 10},
        )

        composite_hash = db.build_composite_match_hash(
            parser_id="wechat",
            counterparty="有规则商户",
            description="有规则描述",
            payment_method="微信支付",
        )
        assert composite_hash is not None

        conn = await db._get_connection()
        await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, enabled, created_at, updated_at
            ) VALUES (?, 'composite', ?, ?, '支出', 1, datetime('now'), datetime('now'))
            """,
            (user_id, composite_hash, composite_hash),
        )
        await conn.commit()

        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["skipped_existing"] >= 1

        suggestions = await db.get_learning_suggestions(user_id=user_id, status="pending")
        assert len(suggestions) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_accept_learning_suggestion_creates_rule(tmp_path: Path) -> None:
    """accept_learning_suggestion 应将建议提升为长期学习规则。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "accept_tester")
        session_id = "accept-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "接受商户",
                "description": "接受描述",
                "parser_id": "wechat",
                "payment_method": "支付宝",
            },
            annotation={"type": "支出", "category_id": 5},
        )

        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["created"] >= 1

        suggestions = await db.get_learning_suggestions(user_id=user_id, status="pending")
        assert len(suggestions) >= 1
        suggestion_id = suggestions[0]["id"]

        result = await db.accept_learning_suggestion(suggestion_id, user_id=user_id)
        assert result is not None
        assert result["status"] == "accepted"
        assert result["rule_id"] is not None

        accepted = await db.get_learning_suggestions(user_id=user_id, status="accepted")
        assert len(accepted) == 1

        rules = await db.get_import_learning_rules(user_id=user_id, enabled_only=True, limit=100)
        rule_hashes = [r["normalized_match_value"] for r in rules]
        assert suggestions[0]["normalized_match_value"] in rule_hashes
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_update_composite_learning_rule_recomputes_runtime_match_fields(tmp_path: Path) -> None:
    """编辑 composite matchValue 后，应同步更新运行时匹配 hash 与 features。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "composite_update_tester")
        rule_id = await _create_composite_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="旧商户",
            description="旧描述",
            payment_method="微信支付",
            learned_type="收入",
        )

        updated = await db.update_import_learning_rule(
            rule_id,
            user_id=user_id,
            match_value="c=新商户|d=新描述|p=alipay|m=支付宝",
            learned_type="支出",
        )
        assert updated is not None
        assert updated["match_value"] == "c=新商户|d=新描述|p=alipay|m=支付宝"
        assert updated["normalized_match_value"] == "c=新商户|d=新描述|p=alipay|m=支付宝"
        assert updated["parser_id"] == "alipay"
        assert updated["composite_match_hash"] == "c=新商户|d=新描述|p=alipay|m=支付宝"
        assert updated["match_features_json"] == (
            '{"counterparty": "新商户", "description": "新描述", '
            '"parser_id": "alipay", "payment_method": "支付宝"}'
        )

        from bill_analyser.core.bills import BillService

        service = BillService(db)
        old_bill = {
            "_parser_id": "wechat",
            "counterparty": "旧商户",
            "description": "旧描述",
            "payment_method": "微信支付",
            "type": "收入",
        }
        new_bill = {
            "_parser_id": "alipay",
            "counterparty": "新商户",
            "description": "新描述",
            "payment_method": "支付宝",
            "type": "收入",
        }

        old_applied = await service._apply_import_learning_rules([old_bill], user_id=user_id)
        new_applied = await service._apply_import_learning_rules([new_bill], user_id=user_id)

        assert old_applied == 0
        assert old_bill["type"] == "收入"
        assert new_applied == 1
        assert new_bill["type"] == "支出"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_reject_learning_suggestion_marks_rejected(tmp_path: Path) -> None:
    """reject_learning_suggestion 应将建议标记为 rejected。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "reject_tester")
        session_id = "reject-session-001"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        await _create_preview_with_annotation(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_data={
                "counterparty": "拒绝商户",
                "description": "拒绝描述",
                "parser_id": "alipay",
                "payment_method": "支付宝",
            },
            annotation={"type": "收入"},
        )

        await db.mine_learning_suggestions(user_id=user_id)
        suggestions = await db.get_learning_suggestions(user_id=user_id, status="pending")
        assert len(suggestions) >= 1
        suggestion_id = suggestions[0]["id"]

        success = await db.reject_learning_suggestion(suggestion_id, user_id=user_id)
        assert success is True

        rejected = await db.get_learning_suggestions(user_id=user_id, status="rejected")
        assert len(rejected) == 1

        success_again = await db.reject_learning_suggestion(suggestion_id, user_id=user_id)
        assert success_again is False
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_accept_nonexistent_suggestion_returns_none(tmp_path: Path) -> None:
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "nonexist_tester")
        result = await db.accept_learning_suggestion(99999, user_id=user_id)
        assert result is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_mine_empty_annotations_returns_zero(tmp_path: Path) -> None:
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "empty_miner")
        stats = await db.mine_learning_suggestions(user_id=user_id)
        assert stats["total_annotations"] == 0
        assert stats["mined"] == 0
    finally:
        await db.close()
