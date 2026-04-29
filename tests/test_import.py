"""
Import Tests - 账单导入测试
"""

import tempfile
from datetime import datetime
from pathlib import Path

import pytest
from src.core.bill_service import BillService


@pytest.fixture
async def service():
    """测试服务fixture"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"

        from src.core.db import Database
        db = Database(str(db_path))

        svc = BillService(db)
        await svc.initialize()

        yield svc

        await svc.close()


async def _create_investment_category_rule(
    service: BillService,
    *,
    user_id: int,
    rule_expression: str,
    sub_category: str = "基金",
) -> int:
    parent_id = await service.db.create_category({
        "type": 5,
        "main_category": "投资",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": "",
    }, user_id=user_id)
    assert parent_id is not None

    category_id = await service.db.create_category({
        "type": 5,
        "main_category": "投资",
        "sub_category": sub_category,
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": "",
    }, user_id=user_id)
    assert category_id is not None

    rule_id = await service.db.create_category_rule({
        "category_id": category_id,
        "name": f"investment-{sub_category}",
        "priority": 1,
        "rule_expression": rule_expression,
        "regex_enabled": False,
        "enabled": True,
    }, user_id=user_id)
    assert rule_id is not None

    return category_id


def test_parser_factory():
    """测试解析器工厂"""
    from src.parsers.factory import ParserFactory

    factory = ParserFactory()
    assert len(factory.parsers) > 0

    formats = factory.get_supported_formats()
    assert ".csv" in formats


@pytest.mark.asyncio
async def test_import_invalid_file(service):
    """测试导入不存在的文件"""
    result = await service.import_bills("nonexistent.csv")

    # 应该处理异常而不是崩溃
    assert not result["success"]


@pytest.mark.asyncio
async def test_clean_invalid(service):
    """测试清理无效账单"""
    count = await service.clean_invalid()
    assert count >= 0


@pytest.mark.asyncio
async def test_update_categories(service):
    """测试更新分类"""
    result = await service.update_categories()
    assert "total" in result
    assert "updated" in result


@pytest.mark.asyncio
async def test_reclassify_preview_replays_session_annotations(service):
    """测试重新分类会回放当前会话的人工标注结果。"""
    session_id = "session-reclassify-annotation"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    parent_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert parent_id is not None

    category_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "游戏",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert category_id is not None

    source_account_id = await service.db.create_account({
        "name": "支付宝",
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
        "aliases": '["支付宝"]'
    }, user_id=1)
    destination_account_id = await service.db.create_account({
        "name": "现金",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["现金"]'
    }, user_id=1)

    inserted = await service.db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-07 10:00:00",
                "preview_type": "支出",
                "preview_amount": 88.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "测试商户",
                "preview_payment_method": "支付宝",
                "preview_description": "待人工修正"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=1)
    assert inserted == 1

    previews = await service.db.get_preview_by_session(session_id)
    assert len(previews) == 1
    preview_id = previews[0]["id"]

    result = await service.reclassify_preview_bills(
        session_id,
        preview_updates=[{
            "id": preview_id,
            "preview_type": "转账",
            "category_id": category_id,
            "preview_source_account_id": source_account_id,
            "preview_destination_account_id": destination_account_id,
        }],
        user_id=1
    )

    assert result["success"] is True
    assert result["session_samples_saved"] == 1
    assert result["annotation_applied"] == 1

    updated_previews = await service.get_import_preview(session_id)
    assert len(updated_previews) == 1
    assert updated_previews[0]["preview_type"] == "转账"
    assert updated_previews[0]["preview_main_category"] == "餐饮"
    assert updated_previews[0]["preview_sub_category"] == "游戏"
    assert updated_previews[0]["preview_source_account_id"] == source_account_id
    assert updated_previews[0]["preview_destination_account_id"] == destination_account_id


@pytest.mark.asyncio
async def test_reclassify_preview_applies_session_annotation_learning_to_similar_bills(service):
    """测试当前会话人工标注会作为临时学习规则应用到相似账单。"""
    session_id = "session-annotation-learning"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    parent_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert parent_id is not None

    category_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "早餐",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert category_id is not None

    source_account_id = await service.db.create_account({
        "name": "招商银行",
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
        "aliases": '["招商银行"]'
    }, user_id=1)
    destination_account_id = await service.db.create_account({
        "name": "现金",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["现金"]'
    }, user_id=1)

    inserted = await service.db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-07 08:00:00",
                "preview_type": "支出",
                "preview_amount": 18.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "测试早餐铺",
                "preview_payment_method": "招商银行",
                "preview_description": "早餐消费"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        },
        {
            "preview_data": {
                "preview_date": "2026-03-08 08:10:00",
                "preview_type": "支出",
                "preview_amount": 20.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "测试早餐铺",
                "preview_payment_method": "",
                "preview_description": "另一笔早餐"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=1)
    assert inserted == 2

    previews = await service.db.get_preview_by_session(session_id)
    assert len(previews) == 2
    annotated_preview_id = previews[0]["id"]
    similar_preview_id = previews[1]["id"]

    result = await service.reclassify_preview_bills(
        session_id,
        preview_updates=[{
            "id": annotated_preview_id,
            "preview_type": "转账",
            "category_id": category_id,
            "preview_source_account_id": source_account_id,
            "preview_destination_account_id": destination_account_id,
        }],
        user_id=1
    )

    assert result["success"] is True
    assert result["session_samples_saved"] == 1
    assert result["session_suggestion_applied"] >= 1

    updated_previews = await service.get_import_preview(session_id)
    assert len(updated_previews) == 2
    preview_map = {item["id"]: item for item in updated_previews}

    assert preview_map[annotated_preview_id]["preview_type"] == "转账"
    assert preview_map[similar_preview_id]["preview_type"] == "转账"
    assert preview_map[similar_preview_id]["preview_main_category"] == "餐饮"
    assert preview_map[similar_preview_id]["preview_sub_category"] == "早餐"
    assert preview_map[similar_preview_id]["preview_source_account_id"] == source_account_id
    assert preview_map[similar_preview_id]["preview_destination_account_id"] == destination_account_id


@pytest.mark.asyncio
async def test_reclassify_preview_applies_long_term_learning_rules(service):
    """测试重新分类会应用长期导入学习规则。"""
    user_id = await service.db.create_user({
        "username": "import_learning_user",
        "email": "import_learning_user@example.com",
        "password_hash": "hash",
        "nickname": "import_learning_user"
    })

    learning_session_id = "session-long-learning-source"
    await service.db.create_import_session(learning_session_id, user_id=user_id, file_count=1)

    parent_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert parent_id is not None

    category_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "夜宵",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert category_id is not None

    source_account_id = await service.db.create_account({
        "name": "支付宝",
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
        "aliases": '["支付宝"]'
    }, user_id=user_id)
    destination_account_id = await service.db.create_account({
        "name": "现金",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["现金"]'
    }, user_id=user_id)

    inserted = await service.db.insert_preview_bills_batch(learning_session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-07 22:10:00",
                "preview_type": "支出",
                "preview_amount": 28.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "夜宵商户",
                "preview_payment_method": "支付宝",
                "preview_description": "深夜小吃"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    previews = await service.db.get_preview_by_session(learning_session_id)
    preview_id = previews[0]["id"]

    promote_result = await service.promote_session_annotations_to_learning(
        learning_session_id,
        preview_updates=[{
            "id": preview_id,
            "preview_type": "转账",
            "category_id": category_id,
            "preview_source_account_id": source_account_id,
            "preview_destination_account_id": destination_account_id,
        }],
        user_id=user_id
    )
    assert promote_result["success"] is True
    assert promote_result["rules_total"] == 1

    target_session_id = "session-long-learning-target"
    await service.db.create_import_session(target_session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(target_session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-08 22:30:00",
                "preview_type": "支出",
                "preview_amount": 32.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "夜宵商户",
                "preview_payment_method": "支付宝",
                "preview_description": "深夜小吃"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    reclassify_result = await service.reclassify_preview_bills(target_session_id, user_id=user_id)
    assert reclassify_result["success"] is True

    updated_previews = await service.get_import_preview(target_session_id)
    assert len(updated_previews) == 1
    assert updated_previews[0]["preview_type"] == "转账"
    assert updated_previews[0]["preview_main_category"] == "餐饮"
    assert updated_previews[0]["preview_sub_category"] == "夜宵"
    assert updated_previews[0]["preview_source_account_id"] == source_account_id
    assert updated_previews[0]["preview_destination_account_id"] == destination_account_id


@pytest.mark.asyncio
async def test_reclassify_preview_skips_long_term_learning_when_disabled(service):
    """测试关闭长期导入学习后不会回放学习规则。"""
    user_id = await service.db.create_user({
        "username": "import_learning_disabled_user",
        "email": "import_learning_disabled_user@example.com",
        "password_hash": "hash",
        "nickname": "import_learning_disabled_user"
    })

    learning_session_id = "session-long-learning-disabled-source"
    await service.db.create_import_session(learning_session_id, user_id=user_id, file_count=1)

    parent_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert parent_id is not None

    category_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "午餐",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert category_id is not None

    inserted = await service.db.insert_preview_bills_batch(learning_session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-07 12:00:00",
                "preview_type": "支出",
                "preview_amount": 20.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "午餐商户",
                "preview_payment_method": "支付宝",
                "preview_description": "学习午餐"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    preview_id = (await service.db.get_preview_by_session(learning_session_id))[0]["id"]
    promote_result = await service.promote_session_annotations_to_learning(
        learning_session_id,
        preview_updates=[{
            "id": preview_id,
            "preview_type": "转账",
            "category_id": category_id,
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
        }],
        user_id=user_id
    )
    assert promote_result["success"] is True

    updated = await service.db.update_user(user_id, {"import_learning_enabled": 0})
    assert updated is True

    target_session_id = "session-long-learning-disabled-target"
    await service.db.create_import_session(target_session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(target_session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-08 12:00:00",
                "preview_type": "支出",
                "preview_amount": 22.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "午餐商户",
                "preview_payment_method": "",
                "preview_description": "未匹配午餐"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    reclassify_result = await service.reclassify_preview_bills(target_session_id, user_id=user_id)
    assert reclassify_result["success"] is True

    updated_previews = await service.get_import_preview(target_session_id)
    assert len(updated_previews) == 1
    assert updated_previews[0]["preview_type"] == "支出"
    assert not updated_previews[0]["preview_main_category"]
    assert updated_previews[0]["preview_source_account_id"] is None


def test_apply_session_annotation_learning_rules_prefers_composite_match():
    """测试会话临时学习规则会优先使用复合匹配而不是单字段匹配。"""
    bill = {
        "id": 200,
        "_parser_id": "wechat",
        "counterparty": "早餐铺",
        "description": "共同描述",
        "payment_method": "微信支付",
        "type": "支出",
        "main_category": "",
        "sub_category": "",
        "source_account_id": None,
        "destination_account_id": None,
    }
    rule_lookup = {
        ("description", "共同描述"): {
            "preview_id": 11,
            "annotated_type": "支出",
            "annotated_main_category": "单字段分类",
            "annotated_sub_category": "描述命中",
            "annotated_source_account_id": 101,
            "annotated_destination_account_id": None,
        },
        ("composite", "c=早餐铺|d=共同描述|p=wechat|m=微信支付"): {
            "preview_id": 22,
            "annotated_type": "转账",
            "annotated_main_category": "复合分类",
            "annotated_sub_category": "优先命中",
            "annotated_source_account_id": 202,
            "annotated_destination_account_id": 303,
        },
    }

    apply_rules = getattr(BillService, "_apply_session_annotation_learning_rules")
    applied_count = apply_rules(
        [bill],
        rule_lookup,
        annotation_map={},
        type_only=False,
    )

    assert applied_count == 1
    assert bill["_session_annotation_match_type"] == "composite"
    assert bill["_session_annotation_source_preview_id"] == 22
    assert bill["type"] == "转账"
    assert bill["main_category"] == "复合分类"
    assert bill["sub_category"] == "优先命中"
    assert bill["source_account_id"] == 202
    assert bill["destination_account_id"] == 303


def test_apply_session_annotation_learning_rules_falls_back_to_single_field_when_parser_differs():
    """测试 parser 不同时不会误命中 composite，而会回退到单字段规则。"""
    bill = {
        "id": 201,
        "_parser_id": "alipay",
        "counterparty": "早餐铺",
        "description": "共同描述",
        "payment_method": "微信支付",
        "type": "支出",
        "main_category": "",
        "sub_category": "",
        "source_account_id": None,
        "destination_account_id": None,
    }
    rule_lookup = {
        ("description", "共同描述"): {
            "preview_id": 11,
            "annotated_type": "支出",
            "annotated_main_category": "单字段分类",
            "annotated_sub_category": "描述命中",
            "annotated_source_account_id": 101,
            "annotated_destination_account_id": None,
        },
        ("composite", "c=早餐铺|d=共同描述|p=wechat|m=微信支付"): {
            "preview_id": 22,
            "annotated_type": "转账",
            "annotated_main_category": "复合分类",
            "annotated_sub_category": "优先命中",
            "annotated_source_account_id": 202,
            "annotated_destination_account_id": 303,
        },
    }

    apply_rules = getattr(BillService, "_apply_session_annotation_learning_rules")
    applied_count = apply_rules(
        [bill],
        rule_lookup,
        annotation_map={},
        type_only=False,
    )

    assert applied_count == 1
    assert bill["_session_annotation_match_type"] == "description"
    assert bill["_session_annotation_source_preview_id"] == 11
    assert bill["type"] == "支出"
    assert bill["main_category"] == "单字段分类"
    assert bill["sub_category"] == "描述命中"
    assert bill["source_account_id"] == 101
    assert bill["destination_account_id"] is None


@pytest.mark.asyncio
async def test_get_import_preview_returns_parser_source_and_manual_annotation_flag(service):
    """测试三阶段预览接口会返回解析器来源、结构化 parser tags 和人工标注标记。"""
    session_id = "session-preview-parser-and-annotation"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    inserted = await service.db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-09 10:00:00",
                "preview_type": "支出",
                "preview_amount": 18.8,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "解析器早餐铺",
                "preview_payment_method": "微信支付",
                "preview_description": "豆浆油条",
                "preview_parser_id": "wechat",
                "preview_parser_tags": ["parser:wechat", "channel:wallet"],
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=1)
    assert inserted == 1

    preview_id = (await service.db.get_preview_by_session(session_id))[0]["id"]
    saved_count = await service.db.save_import_annotation_samples(
        session_id,
        [{
            "id": preview_id,
            "preview_type": "支出",
            "category_id": None,
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
        }],
        user_id=1,
    )
    assert saved_count == 1

    preview_items = await service.get_import_preview(session_id)
    assert len(preview_items) == 1
    assert preview_items[0]["preview_parser_id"] == "wechat"
    assert preview_items[0]["preview_parser_tags"] == ["parser:wechat", "channel:wallet"]
    assert preview_items[0]["preview_is_manually_annotated"] is True


@pytest.mark.asyncio
async def test_import_stage2_dedup_preserves_parser_tags_in_preview(service):
    """测试阶段2去重生成的预览数据会保留结构化 parser tags。"""
    session_id = "session-preview-parser-tags-stage2"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    inserted = await service.db.insert_parser_templates(
        session_id,
        [
            {
                "date": "2026-03-10 08:30:00",
                "amount": -18.8,
                "type": "支出",
                "description": "阶段2 parser tags",
                "counterparty": "测试商户",
                "payment_method": "微信支付",
                "parser_tags": ["parser:wechat", "channel:wallet"],
            }
        ],
        parser_id="wechat",
        user_id=1,
    )
    assert inserted == 1

    result = await service.import_stage2_dedup(session_id, user_id=1)
    assert result["success"] is True

    preview_items = await service.get_import_preview(session_id, user_id=1)
    assert len(preview_items) == 1
    assert preview_items[0]["preview_parser_id"] == "wechat"
    assert preview_items[0]["preview_parser_tags"] == ["parser:wechat", "channel:wallet"]


@pytest.mark.asyncio
async def test_get_import_preview_matching_normalizes_dedup_source_ids_after_db_round_trip(service):
    """matching.dedup.source_ids 应在真实 preview 表回读后保持数组形状。"""
    session_id = "session-preview-matching-dedup-source-ids"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    inserted = await service.db.insert_preview_bills_batch(
        session_id,
        [
            {
                "preview_data": {
                    "preview_date": "2026-03-10 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 66.0,
                    "preview_destination_amount": 0.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "测试商户",
                    "preview_payment_method": "银行卡",
                    "preview_description": "真实 round trip dedup ids",
                    "preview_parser_id": "generic",
                },
                "dedup_type": "transfer",
                "dedup_source_ids": [301, 302],
            }
        ],
        user_id=1,
    )
    assert inserted == 1

    preview_items = await service.get_import_preview(session_id, user_id=1)
    assert len(preview_items) == 1
    assert preview_items[0]["matching"]["dedup"] == {
        "type": "transfer",
        "source_ids": [301, 302],
        "source_count": 2,
        "source_labels": [],
        "sources": [],
    }


@pytest.mark.asyncio
async def test_get_import_preview_returns_structured_platform_duplicate_source_metadata(service):
    """真实 preview 表回读后应返回平台重复来源标签与计数。"""
    session_id = "session-preview-platform-duplicate-metadata"
    await service.db.create_import_session(session_id, user_id=1, file_count=1)

    inserted = await service.db.insert_preview_bills_batch(
        session_id,
        [
            {
                "preview_data": {
                    "preview_date": "2026-03-10 09:30:00",
                    "preview_type": "支出",
                    "preview_amount": 88.0,
                    "preview_destination_amount": 0.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "测试商户",
                    "preview_payment_method": "支付宝",
                    "preview_description": "平台重复来源元数据",
                    "preview_parser_id": "alipay",
                    "preview_parser_tags": ["parser:alipay", "channel:wallet", "parser:cmbc", "channel:bank"],
                },
                "dedup_type": "platform_bank",
                "dedup_source_ids": [401, 402],
            }
        ],
        user_id=1,
    )
    assert inserted == 1

    preview_items = await service.get_import_preview(session_id, user_id=1)
    assert len(preview_items) == 1
    assert preview_items[0]["matching"]["dedup"] == {
        "type": "platform_bank",
        "source_ids": [401, 402],
        "source_count": 2,
        "source_labels": ["支付宝", "民生银行"],
        "sources": [
            {
                "position": 0,
                "role": "kept",
                "parser_id": "alipay",
                "parser_label": "支付宝",
                "label": "支付宝",
                "channel": "wallet",
                "tags": ["parser:alipay", "channel:wallet"],
                "account_id": None,
            },
            {
                "position": 1,
                "role": "duplicate",
                "parser_id": "cmbc",
                "parser_label": "民生银行",
                "label": "民生银行",
                "channel": "bank",
                "tags": ["parser:cmbc", "channel:bank"],
                "account_id": None,
            },
        ],
    }
    assert preview_items[0]["matching"]["parser"]["source_chain"] == preview_items[0]["matching"]["dedup"]["sources"]


@pytest.mark.asyncio
async def test_reclassify_preview_still_applies_legacy_single_field_learning_rules(service):
    """测试历史单字段长期学习规则在升级后仍可回放，避免老用户规则失效。"""
    user_id = await service.db.create_user({
        "username": "legacy_import_learning_user",
        "email": "legacy_import_learning_user@example.com",
        "password_hash": "hash",
        "nickname": "legacy_import_learning_user"
    })

    conn = await getattr(service.db, "_get_connection")()
    now = datetime.now().isoformat()
    await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, enabled, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 1, ?, ?)
        """,
        (user_id, "description", "旧规则描述", "旧规则描述", "转账", now, now)
    )
    await conn.commit()

    session_id = "legacy-single-field-learning-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-10 08:00:00",
                "preview_type": "支出",
                "preview_amount": 66.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "陌生商户",
                "preview_payment_method": "",
                "preview_description": "旧规则描述"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    reclassify_result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert reclassify_result["success"] is True

    preview_items = await service.get_import_preview(session_id)
    assert len(preview_items) == 1
    assert preview_items[0]["preview_type"] == "转账"


@pytest.mark.asyncio
async def test_reclassify_preview_uses_historical_source_account_memory(service):
    """测试重新分类会使用历史确认账单记忆匹配源账户。"""
    user_id = await service.db.create_user({
        "username": "history_source_user",
        "email": "history_source_user@example.com",
        "password_hash": "hash",
        "nickname": "history_source_user"
    })

    source_account_id = await service.db.create_account({
        "name": "备用借记卡",
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
        "aliases": "[]"
    }, user_id=user_id)

    historical_bill_id = await service.db.create_bill({
        "date": "2026-03-01 08:30:00",
        "type": "支出",
        "amount": 15.0,
        "counterparty": "历史早餐铺",
        "description": "早餐固定消费",
        "payment_method": "",
        "main_category": "",
        "sub_category": "",
        "source_account_id": source_account_id,
        "destination_account_id": 0,
        "destination_amount": 0.0,
        "created_at": datetime.now().isoformat(),
        "updated_at": datetime.now().isoformat(),
    }, user_id=user_id)
    assert historical_bill_id is not None

    session_id = "history-source-memory-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 08:35:00",
            "preview_type": "支出",
            "preview_amount": 18.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "历史早餐铺",
            "preview_payment_method": "",
            "preview_description": "新的一天早餐"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_source_account_id"] == source_account_id


@pytest.mark.asyncio
async def test_reclassify_preview_uses_historical_destination_account_memory(service):
    """测试投资/双账户场景会使用历史确认账单记忆匹配目标账户。"""
    user_id = await service.db.create_user({
        "username": "history_destination_user",
        "email": "history_destination_user@example.com",
        "password_hash": "hash",
        "nickname": "history_destination_user"
    })

    source_account_id = await service.db.create_account({
        "name": "农业银行储蓄卡",
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
        "aliases": '["农业银行"]'
    }, user_id=user_id)
    destination_account_id = await service.db.create_account({
        "name": "稳健理财账户",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": "[]"
    }, user_id=user_id)

    historical_bill_id = await service.db.create_bill({
        "date": "2026-03-02 10:00:00",
        "type": "投资",
        "amount": 500.0,
        "counterparty": "余额宝",
        "description": "自动转入余额宝",
        "payment_method": "农业银行",
        "main_category": "",
        "sub_category": "",
        "source_account_id": source_account_id,
        "destination_account_id": destination_account_id,
        "destination_amount": 500.0,
        "created_at": datetime.now().isoformat(),
        "updated_at": datetime.now().isoformat(),
    }, user_id=user_id)
    assert historical_bill_id is not None

    session_id = "history-destination-memory-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 10:10:00",
            "preview_type": "投资",
            "preview_amount": 600.0,
            "preview_destination_amount": 600.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "余额宝",
            "preview_payment_method": "农业银行",
            "preview_description": "本周继续转入"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_source_account_id"] == source_account_id
    assert previews[0]["preview_destination_account_id"] == destination_account_id


@pytest.mark.asyncio
async def test_get_import_preview_returns_transfer_suggestion_for_suspicious_non_transfer(service):
    """测试预览接口会为疑似转账的普通收支账单返回转账推荐。"""
    user_id = await service.db.create_user({
        "username": "transfer_preview_user",
        "email": "transfer_preview_user@example.com",
        "password_hash": "hash",
        "nickname": "transfer_preview_user"
    })

    session_id = "transfer-suggestion-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 10:10:00",
            "preview_type": "支出",
            "preview_amount": 600.0,
            "preview_destination_amount": 600.0,
            "preview_main_category": "转账",
            "preview_sub_category": "现金存取",
            "preview_source_account_id": 11,
            "preview_destination_account_id": 22,
            "preview_counterparty": "转入余额宝",
            "preview_payment_method": "农业银行",
            "preview_description": "转账到余额宝"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["suggested_preview_type"] == "转账"
    assert previews[0]["transfer_suggestion_level"] in {"high", "medium"}
    assert previews[0]["transfer_suggestion_score"] >= 0.55


@pytest.mark.asyncio
async def test_get_import_preview_skips_transfer_suggestion_for_existing_transfer(service):
    """测试已经是转账类型的预览账单不会重复返回转账推荐。"""
    user_id = await service.db.create_user({
        "username": "transfer_existing_user",
        "email": "transfer_existing_user@example.com",
        "password_hash": "hash",
        "nickname": "transfer_existing_user"
    })

    session_id = "existing-transfer-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 10:10:00",
            "preview_type": "转账",
            "preview_amount": 600.0,
            "preview_destination_amount": 600.0,
            "preview_main_category": "转账",
            "preview_sub_category": "账户互转",
            "preview_source_account_id": 11,
            "preview_destination_account_id": 22,
            "preview_counterparty": "内部转账",
            "preview_payment_method": "农业银行",
            "preview_description": "转账"
        },
        "dedup_type": "transfer",
        "dedup_source_ids": [1, 2]
    }], user_id=user_id)
    assert inserted == 1

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["suggested_preview_type"] == ""
    assert previews[0]["transfer_suggestion_level"] == ""
    assert previews[0]["transfer_suggestion_score"] == 0.0


@pytest.mark.asyncio
async def test_get_import_preview_returns_learning_model_recommendation(service):
    """测试预览接口会消费 active learning model 返回推荐信号。"""
    user_id = await service.db.create_user({
        "username": "learning_similarity_user",
        "email": "learning_similarity_user@example.com",
        "password_hash": "hash",
        "nickname": "learning_similarity_user"
    })

    category_parent_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert category_parent_id is not None

    category_id = await service.db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "咖啡",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=user_id)
    assert category_id is not None

    training_session_id = "learning-model-training-session"
    await service.db.create_import_session(training_session_id, user_id=user_id, file_count=1)
    training_rows = [
        ("星巴克咖啡", "门店消费"),
        ("星巴克臻选", "咖啡消费"),
        ("星巴克烘焙", "咖啡早餐"),
    ]
    inserted = await service.db.insert_preview_bills_batch(
        training_session_id,
        [
            {
                "preview_data": {
                    "preview_date": f"2026-03-0{index} 08:10:00",
                    "preview_type": "支出",
                    "preview_amount": 38.0 + index,
                    "preview_destination_amount": 0.0,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "咖啡",
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                    "preview_counterparty": counterparty,
                    "preview_payment_method": "支付宝",
                    "preview_description": description,
                    "preview_parser_id": "alipay",
                },
                "dedup_type": "remaining",
                "dedup_source_ids": [],
            }
            for index, (counterparty, description) in enumerate(training_rows, start=1)
        ],
        user_id=user_id,
    )
    assert inserted == 3

    conn = await getattr(service.db, "_get_connection")()
    async with conn.execute(
        "SELECT id FROM bills_preview WHERE session_id = ? AND user_id = ? ORDER BY id",
        (training_session_id, user_id),
    ) as cursor:
        training_preview_ids = [int(row["id"]) for row in await cursor.fetchall()]
    assert len(training_preview_ids) == 3
    saved_count = await service.db.save_import_annotation_samples(
        training_session_id,
        [
            {
                "id": preview_id,
                "preview_type": "支出",
                "category_id": category_id,
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
            }
            for preview_id in training_preview_ids
        ],
        user_id=user_id,
    )
    assert saved_count == 3
    active_model = await service.db.get_active_import_learning_model(user_id=user_id)
    assert active_model is not None

    session_id = "learning-model-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 12:10:00",
            "preview_type": "支出",
            "preview_amount": 38.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "星巴克",
            "preview_payment_method": "支付宝",
            "preview_description": "咖啡消费",
            "preview_parser_id": "alipay"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["learning_recommendation_level"] in {"high", "medium", "low"}
    assert previews[0]["learning_recommendation_score"] >= 0.70
    assert previews[0]["learning_recommendation_type"] == "支出"
    assert previews[0]["learning_recommendation_summary"] == "支出 | 餐饮/咖啡"
    assert previews[0]["learning_recommendation_source"] == "model"
    assert previews[0]["learning_recommendation_mode"] == "blue"
    assert previews[0]["matching"]["learning"]["review_status"] == "accepted"
    assert "model:dual_head" in previews[0]["learning_recommendation_reason"]


@pytest.mark.asyncio
async def test_get_import_preview_skips_learning_similarity_when_exact_rule_matches(service):
    """测试已存在精确 composite 命中时不会重复返回相似度推荐。"""
    user_id = await service.db.create_user({
        "username": "learning_exact_match_user",
        "email": "learning_exact_match_user@example.com",
        "password_hash": "hash",
        "nickname": "learning_exact_match_user"
    })

    conn = await getattr(service.db, "_get_connection")()
    now = datetime.now().isoformat()
    rule_hash = service.db.build_composite_match_hash(
        parser_id="wechat",
        counterparty="便利店",
        description="早餐",
        payment_method="微信支付",
    )
    assert rule_hash is not None
    await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, enabled, parser_id, composite_match_hash,
            match_features_json, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            "composite",
            rule_hash,
            rule_hash,
            "支出",
            "wechat",
            rule_hash,
            '{"counterparty": "便利店", "description": "早餐", "parser_id": "wechat", "payment_method": "微信支付"}',
            now,
            now,
        )
    )
    await conn.commit()

    session_id = "learning-exact-match-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 07:30:00",
            "preview_type": "支出",
            "preview_amount": 12.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "便利店",
            "preview_payment_method": "微信支付",
            "preview_description": "早餐",
            "preview_parser_id": "wechat"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["learning_recommendation_level"] == ""
    assert previews[0]["learning_recommendation_score"] == 0.0
    assert previews[0]["learning_recommendation_summary"] == ""


@pytest.mark.asyncio
async def test_reclassify_preview_detects_investment_by_platform_and_product(service):
    """测试预览重新分类会通过 canonical category rule 识别投资并匹配目标账户。"""
    user_id = await service.db.create_user({
        "username": "investment_detect_user",
        "email": "investment_detect_user@example.com",
        "password_hash": "hash",
        "nickname": "investment_detect_user"
    })
    await _create_investment_category_rule(
        service,
        user_id=user_id,
        rule_expression="OR={蚂蚁财富,余额宝,基金买入}",
    )

    source_account_id = await service.db.create_account({
        "name": "农业银行储蓄卡",
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
        "aliases": '["农业银行"]'
    }, user_id=user_id)
    destination_account_id = await service.db.create_account({
        "name": "蚂蚁财富账户",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["蚂蚁财富","余额宝","基金"]'
    }, user_id=user_id)

    session_id = "investment-detect-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 11:10:00",
            "preview_type": "支出",
            "preview_amount": 600.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "蚂蚁（杭州）基金销售有限公司",
            "preview_payment_method": "农业银行",
            "preview_description": "蚂蚁财富余额宝基金买入"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_type"] == "投资"
    assert previews[0]["preview_source_account_id"] == source_account_id
    assert previews[0]["preview_destination_account_id"] == destination_account_id


@pytest.mark.asyncio
async def test_reclassify_preview_does_not_misclassify_repayment_as_investment(service):
    """测试没有 investment category rule 命中时，不会靠旧关键词旁路误判成投资。"""
    user_id = await service.db.create_user({
        "username": "investment_exclude_user",
        "email": "investment_exclude_user@example.com",
        "password_hash": "hash",
        "nickname": "investment_exclude_user"
    })

    source_account_id = await service.db.create_account({
        "name": "农业银行储蓄卡",
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
        "aliases": '["农业银行"]'
    }, user_id=user_id)

    session_id = "investment-exclude-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 11:10:00",
            "preview_type": "支出",
            "preview_amount": 600.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "支付宝信贷业务待还款账户",
            "preview_payment_method": "农业银行",
            "preview_description": "蚂蚁财富自动还款"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_type"] == "支出"
    assert previews[0]["preview_source_account_id"] == source_account_id
    assert previews[0]["preview_destination_account_id"] in (None, "", 0)


@pytest.mark.asyncio
async def test_reclassify_preview_uses_canonical_investment_category_rule(service):
    """测试投资识别通过 canonical category rule 生效，而不是用户关键词设置。"""
    user_id = await service.db.create_user({
        "username": "investment_custom_keyword_user",
        "email": "investment_custom_keyword_user@example.com",
        "password_hash": "hash",
        "nickname": "investment_custom_keyword_user"
    })
    await _create_investment_category_rule(
        service,
        user_id=user_id,
        rule_expression="OR={星河财富,量化组合}",
        sub_category="组合投资",
    )

    source_account_id = await service.db.create_account({
        "name": "招商银行储蓄卡",
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
        "aliases": '["招商银行"]'
    }, user_id=user_id)
    destination_account_id = await service.db.create_account({
        "name": "星河财富账户",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["星河财富","量化组合"]'
    }, user_id=user_id)

    session_id = "investment-custom-keyword-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 11:20:00",
            "preview_type": "支出",
            "preview_amount": 399.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "星河财富科技有限公司",
            "preview_payment_method": "招商银行",
            "preview_description": "星河财富量化组合买入"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_type"] == "投资"
    assert previews[0]["preview_source_account_id"] == source_account_id
    assert previews[0]["preview_destination_account_id"] == destination_account_id
    assert previews[0]["investment_signal_score"] == 0.0
    assert previews[0]["investment_platform"] == ""
    assert previews[0]["investment_product"] == ""
    assert previews[0]["matching"]["investment"] == {
        "score": 0.0,
        "level": "",
        "reason": "",
        "platform": "",
        "product": "",
        "review_status": "",
        "suppressed": False,
    }


@pytest.mark.asyncio
async def test_reclassify_preview_detects_investment_via_canonical_reits_rule(service):
    """测试 investment category rule 表达式可覆盖 REITs 场景。"""
    user_id = await service.db.create_user({
        "username": "investment_reits_user",
        "email": "investment_reits_user@example.com",
        "password_hash": "hash",
        "nickname": "investment_reits_user"
    })
    await _create_investment_category_rule(
        service,
        user_id=user_id,
        rule_expression="OR={京东金融,肯特瑞,REIT,REITs,华夏华润商业REIT}",
        sub_category="REITs",
    )

    source_account_id = await service.db.create_account({
        "name": "建设银行储蓄卡",
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
        "aliases": '["建设银行"]'
    }, user_id=user_id)
    destination_account_id = await service.db.create_account({
        "name": "京东金融账户",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["京东金融","肯特瑞"]'
    }, user_id=user_id)

    session_id = "investment-reits-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 12:20:00",
            "preview_type": "支出",
            "preview_amount": 699.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "京东肯特瑞基金销售有限公司",
            "preview_payment_method": "建设银行",
            "preview_description": "京东金融 华夏华润商业REIT买入"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    result = await service.reclassify_preview_bills(session_id, user_id=user_id)
    assert result["success"] is True

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["preview_type"] == "投资"
    assert previews[0]["preview_source_account_id"] == source_account_id
    assert previews[0]["preview_destination_account_id"] == destination_account_id
    assert previews[0]["investment_signal_score"] == 0.0
    assert previews[0]["investment_platform"] == ""
    assert previews[0]["investment_product"] == ""
    assert previews[0]["matching"]["investment"] == {
        "score": 0.0,
        "level": "",
        "reason": "",
        "platform": "",
        "product": "",
        "review_status": "",
        "suppressed": False,
    }


@pytest.mark.asyncio
async def test_get_import_preview_does_not_emit_separate_investment_signal(service):
    """测试投资类型预览账单不再返回独立 investment signal 字段。"""
    user_id = await service.db.create_user({
        "username": "investment_signal_user",
        "email": "investment_signal_user@example.com",
        "password_hash": "hash",
        "nickname": "investment_signal_user"
    })

    session_id = "investment-signal-preview-session"
    await service.db.create_import_session(session_id, user_id=user_id, file_count=1)
    inserted = await service.db.insert_preview_bills_batch(session_id, [{
        "preview_data": {
            "preview_date": "2026-03-08 12:10:00",
            "preview_type": "投资",
            "preview_amount": 888.0,
            "preview_destination_amount": 888.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": 10,
            "preview_destination_account_id": 20,
            "preview_counterparty": "蚂蚁（杭州）基金销售有限公司",
            "preview_payment_method": "农业银行",
            "preview_description": "蚂蚁财富指数基金买入"
        },
        "dedup_type": "remaining",
        "dedup_source_ids": []
    }], user_id=user_id)
    assert inserted == 1

    previews = await service.get_import_preview(session_id)
    assert len(previews) == 1
    assert previews[0]["investment_signal_score"] == 0.0
    assert previews[0]["investment_signal_level"] == ""
    assert previews[0]["investment_signal_reason"] == ""
    assert previews[0]["investment_platform"] == ""
    assert previews[0]["investment_product"] == ""
    assert previews[0]["matching"]["investment"] == {
        "score": 0.0,
        "level": "",
        "reason": "",
        "platform": "",
        "product": "",
        "review_status": "",
        "suppressed": False,
    }
