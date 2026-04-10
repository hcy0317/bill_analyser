"""DB integration coverage for import-preview editing, guards, and confirm edge paths."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

# pylint: disable=protected-access,too-many-locals

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_import_preview_paths.db"))
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


async def _create_category(
    db: Database,
    *,
    user_id: int,
    main_category: str,
    sub_category: str = "",
) -> int:
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


def _build_recurring_template_payload(
    *,
    name: str,
    category_id: int,
    source_account_id: int,
    destination_account_id: int,
    start_date: str,
) -> dict[str, object]:
    return {
        "templateType": 2,
        "name": name,
        "type": 3,
        "categoryId": str(category_id),
        "sourceAccountId": str(source_account_id),
        "destinationAccountId": str(destination_account_id),
        "sourceAmount": 1200,
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": f"{name} 备注",
        "hidden": False,
        "utcOffset": 0,
        "scheduledFrequencyType": 1,
        "scheduledFrequency": "1",
        "scheduledStartDate": start_date,
        "scheduledEndDate": "",
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
async def test_preview_edit_confirm_and_cleanup_paths_cover_alias_fields_and_counts(
    tmp_path: Path,
) -> None:
    """导入预览链路应覆盖插入、批量编辑、确认入账和清理计数。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_path_user")
        session_id = "preview-path-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        breakfast_category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="餐饮",
            sub_category="早餐",
        )
        transit_category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="交通",
            sub_category="地铁",
        )

        first_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-04-01 08:00:00",
                "preview_type": "支出",
                "preview_amount": 18.5,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_counterparty": "测试早餐铺",
                "preview_payment_method": "支付宝",
                "preview_description": "豆浆油条",
            },
            user_id=user_id,
            dedup_type="remaining",
            dedup_source_ids=[11, 22],
        )
        assert first_preview_id > 0

        inserted_count = await db.insert_preview_bills_batch(
            session_id,
            [
                {
                    "preview_data": {
                        "preview_date": "2026-04-01 09:00:00",
                        "preview_type": "支出",
                        "preview_amount": 4.0,
                        "preview_main_category": "交通",
                        "preview_sub_category": "地铁",
                        "preview_counterparty": "地铁站",
                        "preview_payment_method": "微信支付",
                        "preview_description": "早高峰通勤",
                        "preview_parser_id": "wechat",
                    },
                    "dedup_type": "platform_bank",
                    "dedup_source_ids": [33, 44],
                },
                {
                    "preview_data": {
                        "preview_date": "2026-04-01 10:30:00",
                        "preview_type": "收入",
                        "preview_amount": 100.0,
                        "preview_counterparty": "报销入账",
                        "preview_payment_method": "银行卡",
                        "preview_description": "交通报销",
                    },
                    "dedup_type": "remaining",
                    "dedup_source_ids": [],
                },
            ],
            user_id=user_id,
        )
        assert inserted_count == 2

        previews = await db.get_preview_by_session(session_id, user_id=user_id)
        assert len(previews) == 3
        previews_by_id = {int(item["id"]): item for item in previews}
        assert previews_by_id[first_preview_id]["category_id"] == breakfast_category_id
        second_preview_id = next(
            preview_id
            for preview_id, preview in previews_by_id.items()
            if preview_id != first_preview_id and preview.get("preview_parser_id") == "wechat"
        )
        assert previews_by_id[second_preview_id]["category_id"] == transit_category_id

        updated = await db.update_preview_bill(
            first_preview_id,
            {
                "category_id": transit_category_id,
                "is_selected": 0,
                "preview_description": "早餐改成通勤餐",
            },
            user_id=user_id,
        )
        assert updated is True

        updated_count = await db.update_preview_bills_batch(
            session_id,
            [
                {
                    "id": first_preview_id,
                    "date": "2026-04-01 08:05:00",
                    "amount": 20.0,
                    "category_id": breakfast_category_id,
                    "isSelected": True,
                },
                {
                    "id": second_preview_id,
                    "mainCategory": "交通",
                    "subCategory": "地铁",
                    "selected": False,
                    "description": "已取消选择的预览",
                },
            ],
            user_id=user_id,
        )
        assert updated_count == 2

        selected_preview_ids = {
            int(item["id"])
            for item in await db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=True,
            )
        }
        assert first_preview_id in selected_preview_ids
        assert second_preview_id not in selected_preview_ids
        assert len(selected_preview_ids) == 2

        reset_count = await db.reset_session_preview_selection(session_id, user_id=user_id)
        assert reset_count == 3
        assert (
            await db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=True,
            )
            == []
        )

        assert (
            await db.update_preview_selection(
                [first_preview_id, second_preview_id],
                True,
                user_id=user_id,
            )
            == 2
        )

        conn = await db._get_connection()
        await conn.execute(
            """
            INSERT INTO bills_parser_template (
                session_id, user_id, parser_date, parser_amount, parser_type,
                parser_description, parser_id, parser_counterparty, parser_payment_method,
                parser_original_type, parser_original_category, parser_account_id,
                parser_is_processed, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
            """,
            (
                session_id,
                user_id,
                "2026-04-01 07:59:59",
                20.0,
                "支出",
                "原始解析记录",
                "wechat",
                "原始对手方",
                "微信支付",
                "消费",
                "",
                1,
            ),
        )
        await conn.commit()

        saved_annotations = await db.save_import_annotation_samples(
            session_id,
            [
                {
                    "id": first_preview_id,
                    "preview_type": "支出",
                    "category_id": breakfast_category_id,
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                }
            ],
            user_id=user_id,
        )
        assert saved_annotations == 1

        confirm_result = await db.confirm_preview_to_bills(session_id, user_id=user_id)
        assert confirm_result["confirmed_count"] == 2
        assert confirm_result["duplicate_count"] == 0
        assert confirm_result["errors"] == []

        confirmed_bills = await db.get_bills(user_id=user_id)
        bill_descriptions = {bill["description"] for bill in confirmed_bills}
        assert "早餐改成通勤餐" in bill_descriptions
        assert "已取消选择的预览" in bill_descriptions

        cleanup_result = await db.clear_session_data(session_id, user_id=user_id)
        assert cleanup_result == {
            "parser_count": 1,
            "preview_count": 3,
            "annotation_count": 1,
        }
        assert await db.get_preview_by_session(session_id, user_id=user_id) == []
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_guard_paths_return_empty_or_false_without_side_effects(
    tmp_path: Path,
) -> None:
    """预览链路的空输入/无效输入 guard 分支应返回安全默认值。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_guard_user")
        session_id = "preview-guard-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-01 08:00:00",
                "preview_type": "支出",
                "preview_amount": 12.5,
                "preview_counterparty": "安全默认值商户",
                "preview_description": "安全默认值备注",
            },
            user_id=user_id,
        )
        assert preview_id > 0

        assert await db.insert_preview_bills_batch(session_id, [], user_id=user_id) == 0
        assert await db.update_preview_selection([], True, user_id=user_id) == 0
        assert await db.update_preview_bill(preview_id, {}, user_id=user_id) is False
        assert (
            await db.update_preview_bill(
                preview_id,
                {"category_id": 999999},
                user_id=user_id,
            )
            is False
        )
        assert await db.update_preview_bills_batch(session_id, [], user_id=user_id) == 0
        assert await db.update_preview_bills_batch(
            session_id,
            [{"preview_type": "收入"}],
            user_id=user_id,
        ) == 0
        assert await db.get_recurring_candidates_for_preview(999999, user_id=user_id) == {
            "preview": None,
            "linked_recurring_id": None,
            "candidates": [],
        }

        preview = await db.get_preview_bill_by_id(preview_id, user_id=user_id)
        assert preview is not None
        assert preview["preview_type"] == "支出"
        assert preview["preview_description"] == "安全默认值备注"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_transfer_decision_accept_reject_and_clear_restore_preview_fields(
    tmp_path: Path,
) -> None:
    """转账建议决策应支持接受、拒绝和清除，并在需要时恢复原始预览字段。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_transfer_decision_user")
        session_id = "preview-transfer-decision-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-06-01 10:30:00",
                "preview_type": "支出",
                "preview_amount": 50.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_counterparty": "测试早餐店",
                "preview_payment_method": "招商银行",
                "preview_description": "跨卡转账早餐卡",
                "preview_recurring_id": 88,
                "preview_recurring_name": "早餐模板",
                "preview_recurring_candidate_count": 2,
                "preview_recurring_match_score": 0.91,
                "preview_recurring_match_reasons": "amount|date",
                "preview_recurring_matched_date": "2026-06-01",
            },
            user_id=user_id,
            dedup_type="transfer",
            dedup_source_ids=[10, 11],
        )
        assert preview_id > 0

        accepted_preview = await db.update_preview_transfer_decision(preview_id, "accept", user_id=user_id)
        assert accepted_preview is not None
        assert accepted_preview["preview_type"] == "转账"
        assert accepted_preview["preview_main_category"] == ""
        assert accepted_preview["preview_sub_category"] == ""
        assert accepted_preview["preview_recurring_id"] is None
        assert accepted_preview["preview_recurring_name"] == ""
        transfer_feedback = accepted_preview["preview_matching_feedback"]["transfer"]
        assert transfer_feedback["review_status"] == "accepted"
        assert transfer_feedback["reviewed_type"] == "转账"
        assert transfer_feedback["suppressed"] is False
        assert transfer_feedback["previous_preview"]["preview_type"] == "支出"
        assert transfer_feedback["previous_preview"]["preview_main_category"] == "餐饮"

        rejected_preview = await db.update_preview_transfer_decision(preview_id, "reject", user_id=user_id)
        assert rejected_preview is not None
        assert rejected_preview["preview_type"] == "支出"
        assert rejected_preview["preview_main_category"] == "餐饮"
        assert rejected_preview["preview_sub_category"] == "早餐"
        assert rejected_preview["preview_recurring_id"] == 88
        rejected_feedback = rejected_preview["preview_matching_feedback"]["transfer"]
        assert rejected_feedback == {
            "review_status": "rejected",
            "reviewed_type": "",
            "suppressed": True,
        }

        cleared_preview = await db.update_preview_transfer_decision(preview_id, "clear", user_id=user_id)
        assert cleared_preview is not None
        assert cleared_preview["preview_matching_feedback"] == {}
        assert cleared_preview["preview_type"] == "支出"
        assert cleared_preview["preview_main_category"] == "餐饮"
        assert cleared_preview["preview_sub_category"] == "早餐"
        assert cleared_preview["preview_recurring_id"] == 88
        assert await db.update_preview_transfer_decision(999999, "accept", user_id=user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_batch_sync_can_clear_transfer_decision_feedback_without_overwriting_manual_fields(
    tmp_path: Path,
) -> None:
    """批量同步预览草稿时应能清理 transfer 决策反馈，并保留用户当前手工字段。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_transfer_manual_sync_user")
        session_id = "preview-transfer-manual-sync-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-06-02 12:00:00",
                "preview_type": "支出",
                "preview_amount": 66.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "午餐",
                "preview_counterparty": "测试午餐店",
                "preview_payment_method": "银行卡",
                "preview_description": "午餐卡转账",
                "preview_recurring_id": 18,
                "preview_recurring_name": "午餐模板",
                "preview_recurring_candidate_count": 1,
                "preview_recurring_match_score": 0.8,
                "preview_recurring_match_reasons": "amount",
                "preview_recurring_matched_date": "2026-06-02",
            },
            user_id=user_id,
            dedup_type="transfer",
            dedup_source_ids=[101, 102],
        )
        assert preview_id > 0

        accepted_preview = await db.update_preview_transfer_decision(preview_id, "accept", user_id=user_id)
        assert accepted_preview is not None
        assert accepted_preview["preview_matching_feedback"]["transfer"]["review_status"] == "accepted"

        updated_count = await db.update_preview_bills_batch(
            session_id,
            [
                {
                    "id": preview_id,
                    "preview_type": "支出",
                    "preview_main_category": "交通",
                    "preview_sub_category": "地铁",
                    "preview_recurring_id": 28,
                    "preview_recurring_name": "通勤模板",
                    "preview_recurring_candidate_count": 3,
                    "preview_recurring_match_score": 0.67,
                    "preview_recurring_match_reasons": "manual",
                    "preview_recurring_matched_date": "2026-06-03",
                    "clear_transfer_decision": True,
                }
            ],
            user_id=user_id,
        )
        assert updated_count == 1

        synced_preview = await db.get_preview_bill_by_id(preview_id, user_id=user_id)
        assert synced_preview is not None
        assert synced_preview["preview_matching_feedback"] == {}
        assert synced_preview["preview_type"] == "支出"
        assert synced_preview["preview_main_category"] == "交通"
        assert synced_preview["preview_sub_category"] == "地铁"
        assert synced_preview["preview_recurring_id"] == 28
        assert synced_preview["preview_recurring_name"] == "通勤模板"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_transfer_decision_rejects_stale_snapshot_inside_db_transaction(
    tmp_path: Path,
) -> None:
    """转账决策更新应在 DB 事务内拒绝基于旧快照的重放写入。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_transfer_db_cas_user")
        session_id = "preview-transfer-db-cas-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-06-04 08:00:00",
                "preview_type": "支出",
                "preview_amount": 30.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_counterparty": "测试早餐店",
                "preview_payment_method": "银行卡",
                "preview_description": "CAS transfer test",
            },
            user_id=user_id,
            dedup_type="transfer",
            dedup_source_ids=[301, 302],
        )
        assert preview_id > 0

        preview_before_accept = await db.get_preview_bill_by_id(preview_id, user_id=user_id)
        assert preview_before_accept is not None
        expected_snapshot = {
            "session_id": str(preview_before_accept.get("session_id") or ""),
            "preview_type": str(preview_before_accept.get("preview_type") or ""),
            "preview_main_category": str(preview_before_accept.get("preview_main_category") or ""),
            "preview_sub_category": str(preview_before_accept.get("preview_sub_category") or ""),
            "preview_recurring_id": preview_before_accept.get("preview_recurring_id"),
            "preview_matching_feedback_json": str(preview_before_accept.get("preview_matching_feedback_json") or ""),
        }

        accepted_preview = await db.update_preview_transfer_decision(
            preview_id,
            "accept",
            user_id=user_id,
            expected_state=expected_snapshot,
        )
        assert accepted_preview is not None
        assert accepted_preview["preview_matching_feedback"]["transfer"]["review_status"] == "accepted"

        stale_reject_result = await db.update_preview_transfer_decision(
            preview_id,
            "reject",
            user_id=user_id,
            expected_state=expected_snapshot,
        )
        assert stale_reject_result == {"_state_conflict": True}
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_recurring_match_persists_clear_and_keeps_template_schedule_unchanged(
    tmp_path: Path,
) -> None:
    """预览 recurring 匹配应支持持久化/清除，并且 preview 阶段不能推进模板 next_date。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_recurring_match_user")
        source_account_id = await _create_account(db, user_id=user_id, name="预览扣款账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="预览收款账户")
        category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="模板主类",
            sub_category="模板子类",
        )
        recurring_a_id = await db.create_template(
            _build_recurring_template_payload(
                name="早餐模板 A",
                category_id=category_id,
                source_account_id=source_account_id,
                destination_account_id=destination_account_id,
                start_date="2026-03-08",
            ),
            user_id=user_id,
        )
        recurring_b_id = await db.create_template(
            _build_recurring_template_payload(
                name="早餐模板 B",
                category_id=category_id,
                source_account_id=source_account_id,
                destination_account_id=destination_account_id,
                start_date="2026-03-09",
            ),
            user_id=user_id,
        )
        session_id = "preview-recurring-match-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-03-09 10:30:00",
                "preview_type": "支出",
                "preview_amount": 12.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_counterparty": "测试早餐店",
                "preview_payment_method": "测试银行卡",
                "preview_description": "preview recurring match",
                "preview_source_account_id": source_account_id,
                "preview_destination_account_id": destination_account_id,
            },
            user_id=user_id,
        )
        assert preview_id > 0

        preview_before = await db.get_preview_bill_by_id(preview_id, user_id=user_id)
        assert preview_before is not None
        candidates_result = await db.get_recurring_candidates_for_preview(preview_id, user_id=user_id, tolerance_days=3)
        candidates = candidates_result["candidates"]
        assert len(candidates) >= 2
        target_candidate = next(
            candidate for candidate in candidates if int(candidate["id"]) == int(recurring_b_id)
        )

        next_date_before = await _get_raw_next_date(db, recurring_id=recurring_b_id, user_id=user_id)
        expected_snapshot = {
            "session_id": str(preview_before.get("session_id") or ""),
            "preview_type": str(preview_before.get("preview_type") or ""),
            "preview_main_category": str(preview_before.get("preview_main_category") or ""),
            "preview_sub_category": str(preview_before.get("preview_sub_category") or ""),
            "preview_recurring_id": preview_before.get("preview_recurring_id"),
            "preview_matching_feedback_json": str(preview_before.get("preview_matching_feedback_json") or ""),
        }

        matched_preview = await db.update_preview_recurring_match(
            preview_id,
            recurring_b_id,
            user_id=user_id,
            expected_state=expected_snapshot,
        )
        assert matched_preview is not None
        assert matched_preview["preview_recurring_id"] == recurring_b_id
        assert matched_preview["preview_recurring_name"] == target_candidate["name"]
        assert matched_preview["preview_recurring_candidate_count"] == len(candidates)
        assert matched_preview["preview_recurring_match_score"] == pytest.approx(
            float(target_candidate["matchScore"])
        )
        assert matched_preview["preview_recurring_matched_date"] == target_candidate["matchedOccurrenceDate"]
        assert await _get_raw_next_date(db, recurring_id=recurring_b_id, user_id=user_id) == next_date_before

        stale_clear_result = await db.update_preview_recurring_match(
            preview_id,
            None,
            user_id=user_id,
            expected_state=expected_snapshot,
        )
        assert stale_clear_result == {"_state_conflict": True}

        clear_snapshot = {
            "session_id": str(matched_preview.get("session_id") or ""),
            "preview_type": str(matched_preview.get("preview_type") or ""),
            "preview_main_category": str(matched_preview.get("preview_main_category") or ""),
            "preview_sub_category": str(matched_preview.get("preview_sub_category") or ""),
            "preview_recurring_id": matched_preview.get("preview_recurring_id"),
            "preview_matching_feedback_json": str(matched_preview.get("preview_matching_feedback_json") or ""),
        }
        cleared_preview = await db.update_preview_recurring_match(
            preview_id,
            None,
            user_id=user_id,
            expected_state=clear_snapshot,
        )
        assert cleared_preview is not None
        assert cleared_preview["preview_recurring_id"] is None
        assert cleared_preview["preview_recurring_name"] == ""
        assert cleared_preview["preview_recurring_candidate_count"] == len(candidates)
        assert cleared_preview["preview_recurring_match_score"] == 0
        assert cleared_preview["preview_recurring_match_reasons"] == ""
        assert cleared_preview["preview_recurring_matched_date"] == ""
        assert await _get_raw_next_date(db, recurring_id=recurring_b_id, user_id=user_id) == next_date_before

        cleared_snapshot = {
            "session_id": str(cleared_preview.get("session_id") or ""),
            "preview_type": str(cleared_preview.get("preview_type") or ""),
            "preview_main_category": str(cleared_preview.get("preview_main_category") or ""),
            "preview_sub_category": str(cleared_preview.get("preview_sub_category") or ""),
            "preview_recurring_id": cleared_preview.get("preview_recurring_id"),
            "preview_matching_feedback_json": str(cleared_preview.get("preview_matching_feedback_json") or ""),
        }

        invalid_match_result = await db.update_preview_recurring_match(
            preview_id,
            999999,
            user_id=user_id,
            expected_state=cleared_snapshot,
        )
        assert invalid_match_result == {"_invalid_recurring_id": True}
        assert await _get_raw_next_date(db, recurring_id=recurring_a_id, user_id=user_id) == "2026-03-08"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_preview_confirm_handles_income_duplicates_missing_recurring_and_dedup_query_window(
    tmp_path: Path,
) -> None:
    """确认预览应覆盖收入正数化、重复 hash、缺失 recurring 和 dedup 时间窗口查询。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_duplicate_user")
        session_id = "preview-duplicate-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=3)

        duplicate_preview = {
            "preview_date": "2026-05-01 09:00:00",
            "preview_type": "收入",
            "preview_amount": 88.0,
            "preview_counterparty": "工资账户",
            "preview_payment_method": "银行卡",
            "preview_description": "五月奖金",
        }
        first_preview_id = await db.insert_preview_bill(
            session_id,
            duplicate_preview,
            user_id=user_id,
        )
        second_preview_id = await db.insert_preview_bill(
            session_id,
            duplicate_preview,
            user_id=user_id,
        )
        recurring_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-31 18:00:00",
                "preview_type": "transfer",
                "preview_amount": 66.0,
                "preview_counterparty": "月底调拨",
                "preview_payment_method": "余额转账",
                "preview_description": "月底转账收入",
                "preview_recurring_id": 999999,
                "preview_recurring_name": "不存在的 recurring",
            },
            user_id=user_id,
        )
        assert first_preview_id > 0
        assert second_preview_id > 0
        assert recurring_preview_id > 0

        confirm_result = await db.confirm_preview_to_bills(session_id, user_id=user_id)
        assert confirm_result == {
            "confirmed_count": 2,
            "skipped_count": 0,
            "duplicate_count": 1,
            "errors": [],
        }

        session = await db.get_import_session(session_id, user_id=user_id)
        assert session is not None
        assert session["status"] == "completed"
        assert int(session["total_confirmed"]) == 2

        existing_bills = await db.get_existing_bills_for_dedup(
            user_id,
            "2026-05-01",
            "2026-05-31",
        )
        assert [(bill["date"], bill["type"], bill["amount"]) for bill in existing_bills] == [
            ("2026-05-01 09:00:00", "收入", 88.0),
            ("2026-05-31 18:00:00", "transfer", 66.0),
        ]
    finally:
        await db.close()
