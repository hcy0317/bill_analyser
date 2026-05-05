from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.bills import BillService
from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path, name: str) -> Database:
    db = Database(str(tmp_path / f"{name}.db"))
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


async def _create_category(db: Database, *, user_id: int, main: str, sub: str) -> int:
    category_id = await db.create_category(
        {
            "type": 3,
            "main_category": main,
            "sub_category": sub,
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


async def _insert_preview(
    db: Database,
    *,
    session_id: str,
    user_id: int,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    amount: float = 28.0,
) -> int:
    return int(
        await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-04-30 12:00:00",
                "preview_type": "支出",
                "preview_amount": amount,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": counterparty,
                "preview_payment_method": payment_method,
                "preview_description": description,
                "preview_parser_id": parser_id,
            },
            user_id=user_id,
        )
    )


async def _train_learning_samples(
    db: Database,
    *,
    user_id: int,
    session_id: str,
    rows: list[dict[str, Any]],
) -> list[int]:
    await db.create_import_session(session_id, user_id=user_id, file_count=1)
    preview_ids: list[int] = []
    updates: list[dict[str, Any]] = []
    for row in rows:
        preview_id = await _insert_preview(
            db,
            session_id=session_id,
            user_id=user_id,
            parser_id=str(row.get("parser_id") or "alipay"),
            counterparty=str(row.get("counterparty") or ""),
            description=str(row.get("description") or ""),
            payment_method=str(row.get("payment_method") or "支付宝"),
            amount=float(row.get("amount") or 28.0),
        )
        preview_ids.append(preview_id)
        updates.append(
            {
                "id": preview_id,
                "preview_type": row.get("preview_type", "支出"),
                "category_id": row.get("category_id"),
                "preview_source_account_id": row.get("source_account_id"),
                "preview_destination_account_id": row.get("destination_account_id"),
            }
        )
    saved = await db.save_import_annotation_samples(session_id, updates, user_id=user_id)
    assert saved == len(rows)
    return preview_ids


async def _setup_learning_domain(tmp_path: Path, name: str) -> tuple[Database, BillService, int, int, int, int]:
    db = await _create_database(tmp_path, name)
    user_id = await _create_user(db, f"{name}_user")
    coffee_category_id = await _create_category(db, user_id=user_id, main="餐饮", sub="咖啡")
    tea_category_id = await _create_category(db, user_id=user_id, main="餐饮", sub="茶饮")
    source_account_id = await _create_account(db, user_id=user_id, name="支付宝")
    service = BillService(db=db)
    return db, service, user_id, coffee_category_id, tea_category_id, source_account_id


@pytest.mark.asyncio
async def test_import_learning_corpus_trains_active_model_and_green_preview_signal(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_green_model",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-green",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "喜茶", "description": "芝士茶饮", "category_id": tea_category_id,
                 "source_account_id": source_account_id},
            ],
        )
        active_model = await db.get_active_import_learning_model(user_id=user_id)
        assert active_model is not None
        assert active_model["dataset_snapshot_id"] is not None
        assert active_model["metrics"]["parameter_ref"] == "metrics_json.model_parameters"

        target_session_id = "target-green"
        await db.create_import_session(target_session_id, user_id=user_id, file_count=1)
        target_id = await _insert_preview(
            db,
            session_id=target_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )

        previews = await service.get_import_preview(target_session_id, user_id=user_id)
        assert previews[0]["id"] == target_id
        assert previews[0]["matching"]["learning"]["source"] == "model"
        assert previews[0]["matching"]["learning"]["mode"] == "green"
        assert previews[0]["matching"]["learning"]["review_status"] == "pending"
        assert previews[0]["category_id"] in (None, "", 0, "0")
        assert await db.get_import_learning_rules(user_id=user_id, enabled_only=False, limit=None) == []
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_green_accept_and_reject_correction_write_feedback_and_refresh_model(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_feedback_loop",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-feedback",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "喜茶", "description": "芝士茶饮", "category_id": tea_category_id,
                 "source_account_id": source_account_id},
            ],
        )
        active_model = await db.get_active_import_learning_model(user_id=user_id)
        assert active_model is not None
        model_version = str(active_model["model_version"])

        accept_session_id = "target-green-accept"
        await db.create_import_session(accept_session_id, user_id=user_id, file_count=1)
        accept_id = await _insert_preview(
            db,
            session_id=accept_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )
        accept_result = await service.apply_preview_learning_decision(
            accept_id,
            "accept",
            expected_state={
                "sessionId": accept_session_id,
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
                "sourceAccountId": None,
                "destinationAccountId": None,
            },
            response_mode="preview-item",
            model_version=model_version,
            user_id=user_id,
        )
        assert accept_result["success"] is True
        assert accept_result["preview_item"]["category_id"] == coffee_category_id

        reject_session_id = "target-green-reject"
        await db.create_import_session(reject_session_id, user_id=user_id, file_count=1)
        reject_id = await _insert_preview(
            db,
            session_id=reject_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克茶饮",
            description="茶饮消费",
            payment_method="支付宝",
        )
        conn = await db._get_connection()  # pylint: disable=protected-access
        await conn.execute(
            """
            UPDATE bills_preview
            SET preview_main_category = '餐饮', preview_sub_category = '茶饮',
                preview_source_account_id = ?
            WHERE id = ? AND user_id = ?
            """,
            (source_account_id, reject_id, user_id),
        )
        await conn.commit()
        reject_active_model = await db.get_active_import_learning_model(user_id=user_id)
        assert reject_active_model is not None
        reject_model_version = str(reject_active_model["model_version"])
        reject_result = await service.apply_preview_learning_decision(
            reject_id,
            "reject",
            expected_state={
                "sessionId": reject_session_id,
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": tea_category_id,
                "recurringId": None,
                "sourceAccountId": source_account_id,
                "destinationAccountId": None,
            },
            response_mode="preview-item",
            model_version=reject_model_version,
            user_id=user_id,
        )
        assert reject_result["success"] is True

        corpus = await db.get_import_learning_corpus_samples(user_id=user_id, limit=None)
        corrective = [row for row in corpus if int(row["preview_id"]) == reject_id][0]
        assert int(corrective["annotated_category_id"]) == tea_category_id
        active_model = await db.get_active_import_learning_model(user_id=user_id)
        assert active_model is not None
        async with conn.execute(
            "SELECT event_type FROM import_learning_feedback_events WHERE user_id = ? ORDER BY id",
            (user_id,),
        ) as cursor:
            event_types = [row["event_type"] for row in await cursor.fetchall()]
        assert "model_preview_accept" in event_types
        assert "model_preview_reject" in event_types
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_generic_model_accept_rejects_stale_model_version(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_stale_model_accept",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-stale-v1",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "喜茶", "description": "芝士茶饮", "category_id": tea_category_id,
                 "source_account_id": source_account_id},
            ],
        )

        target_session_id = "target-stale-model"
        await db.create_import_session(target_session_id, user_id=user_id, file_count=1)
        target_id = await _insert_preview(
            db,
            session_id=target_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )

        seen_previews = await service.get_import_preview(target_session_id, user_id=user_id)
        seen_learning = seen_previews[0]["matching"]["learning"]
        seen_model_version = str(seen_learning["model_version"])
        assert seen_learning["source"] == "model"

        refreshed = await db.refresh_import_learning_model(user_id=user_id)
        assert refreshed["trained"] is True
        assert refreshed["model_version"] != seen_model_version

        result = await service._accept_matching_candidate(  # pylint: disable=protected-access
            f"preview:{target_id}:learning",
            {
                "expectedState": {
                    "sessionId": target_session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": None,
                    "destinationAccountId": None,
                },
                "responseMode": "preview-item",
                "modelVersion": seen_model_version,
            },
            user_id=user_id,
        )

        assert result == {
            "success": False,
            "error": "Learning candidate changed, please refresh",
            "status_code": 409,
        }
        preview_after_conflict = await db.get_preview_bill_by_id(target_id, user_id=user_id)
        assert preview_after_conflict is not None
        assert preview_after_conflict["preview_matching_feedback"] == {}
        assert await db.get_import_learning_corpus_samples(user_id=user_id, limit=None)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_blue_auto_apply_requires_more_than_two_confirmations_and_can_rollback(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, _tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_blue_model",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-blue",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克烘焙", "description": "咖啡早餐", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
            ],
        )
        target_session_id = "target-blue"
        await db.create_import_session(target_session_id, user_id=user_id, file_count=1)
        target_id = await _insert_preview(
            db,
            session_id=target_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )

        previews = await service.get_import_preview(target_session_id, user_id=user_id)
        learning = previews[0]["matching"]["learning"]
        assert learning["mode"] == "blue"
        assert learning["confirmations"] > 2
        assert learning["review_status"] == "accepted"
        assert previews[0]["category_id"] == coffee_category_id
        assert previews[0]["preview_source_account_id"] == source_account_id

        clear_result = await service.apply_preview_learning_decision(
            target_id,
            "clear",
            expected_state={
                "sessionId": target_session_id,
                "reviewStatus": "accepted",
                "previewType": "支出",
                "categoryId": coffee_category_id,
                "recurringId": None,
                "sourceAccountId": source_account_id,
                "destinationAccountId": None,
            },
            response_mode="preview-item",
            user_id=user_id,
        )
        assert clear_result["success"] is True
        assert clear_result["preview_item"]["category_id"] in (None, "", 0, "0")

        conn = await db._get_connection()  # pylint: disable=protected-access
        async with conn.execute(
            """
            SELECT event_type FROM import_learning_feedback_events
            WHERE user_id = ? ORDER BY id
            """,
            (user_id,),
        ) as cursor:
            event_types = [row["event_type"] for row in await cursor.fetchall()]
        assert "model_blue_auto_apply" in event_types
        assert "preview_rollback" in event_types
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_exact_learning_rule_suppresses_model_preview_signal(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, _tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_exact_priority",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-exact-priority",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克烘焙", "description": "咖啡早餐", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
            ],
        )

        exact_hash = db.build_composite_match_hash(
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )
        conn = await db._get_connection()  # pylint: disable=protected-access
        await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id, enabled, parser_id,
                composite_match_hash, match_features_json, created_at, updated_at
            ) VALUES (?, 'composite', ?, ?, '支出', ?, 1, 'alipay', ?, ?, ?, ?)
            """,
            (
                user_id,
                exact_hash,
                exact_hash,
                coffee_category_id,
                exact_hash,
                '{"parser_id": "alipay", "counterparty": "星巴克门店", "description": "咖啡拿铁", "payment_method": "支付宝"}',
                "2026-04-30T12:00:00",
                "2026-04-30T12:00:00",
            ),
        )
        await conn.commit()

        target_session_id = "target-exact-priority"
        await db.create_import_session(target_session_id, user_id=user_id, file_count=1)
        await _insert_preview(
            db,
            session_id=target_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )
        previews = await service.get_import_preview(target_session_id, user_id=user_id)
        assert previews[0]["matching"]["learning"].get("source", "") == ""
        assert previews[0]["matching"]["learning"]["review_status"] == ""
        assert previews[0]["category_id"] in (None, "", 0, "0")
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_import_learning_disabled_suppresses_active_model_signal(tmp_path: Path) -> None:
    db, service, user_id, coffee_category_id, _tea_category_id, source_account_id = await _setup_learning_domain(
        tmp_path,
        "learning_disabled_model",
    )
    try:
        await _train_learning_samples(
            db,
            user_id=user_id,
            session_id="train-disabled",
            rows=[
                {"counterparty": "星巴克咖啡", "description": "门店咖啡", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克臻选", "description": "咖啡消费", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
                {"counterparty": "星巴克烘焙", "description": "咖啡早餐", "category_id": coffee_category_id,
                 "source_account_id": source_account_id},
            ],
        )
        assert await db.get_active_import_learning_model(user_id=user_id) is not None
        await db.update_user(user_id, {"import_learning_enabled": 0})

        target_session_id = "target-disabled"
        await db.create_import_session(target_session_id, user_id=user_id, file_count=1)
        await _insert_preview(
            db,
            session_id=target_session_id,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克门店",
            description="咖啡拿铁",
            payment_method="支付宝",
        )
        previews = await service.get_import_preview(target_session_id, user_id=user_id)
        assert previews[0]["learning_recommendation_score"] == 0.0
        assert previews[0]["matching"]["learning"]["review_status"] == ""
        assert previews[0]["category_id"] in (None, "", 0, "0")
    finally:
        await db.close()
