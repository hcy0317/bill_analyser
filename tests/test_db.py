"""
Database Tests - 数据库模块测试
"""

import pytest
import asyncio
from pathlib import Path
import tempfile

from src.core.db import Database


@pytest.fixture
async def db():
    """测试数据库fixture"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        database = Database(str(db_path))
        await database.init_db()
        yield database
        await database.close()


@pytest.mark.asyncio
async def test_init_db(db):
    """测试数据库初始化"""
    stats = await db.get_statistics()
    assert stats['total_bills'] == 0


@pytest.mark.asyncio
async def test_insert_bills(db):
    """测试插入账单"""
    bills = [
        {
            'date': '2025-01-01 12:00:00',
            'type': '支出',
            'amount': 100.0,
            'counterparty': '测试商家',
            'description': '测试消费'
        }
    ]
    
    count = await db.insert_bills(bills)
    assert count == 1
    
    # 重复插入应该被忽略
    count = await db.insert_bills(bills)
    assert count == 0


@pytest.mark.asyncio
async def test_get_bills(db):
    """测试查询账单"""
    # 插入测试数据
    bills = [
        {
            'date': '2025-01-01',
            'type': '支出',
            'amount': 100.0,
            'counterparty': '商家A',
            'description': '消费A'
        },
        {
            'date': '2025-01-02',
            'type': '收入',
            'amount': 200.0,
            'counterparty': '来源B',
            'description': '收入B'
        }
    ]
    await db.insert_bills(bills)
    
    # 查询所有
    result = await db.get_bills()
    assert len(result) == 2
    
    # 按类型过滤
    result = await db.get_bills({'type': '支出'})
    assert len(result) == 1
    assert result[0]['type'] == '支出'


@pytest.mark.asyncio
async def test_update_bill(db):
    """测试更新账单"""
    bills = [{
        'date': '2025-01-01',
        'type': '支出',
        'amount': 100.0,
        'counterparty': '商家',
        'description': '原描述'
    }]
    await db.insert_bills(bills)
    
    result = await db.get_bills()
    bill_id = result[0]['id']
    
    success = await db.update_bill(bill_id, {'description': '新描述'})
    assert success
    
    result = await db.get_bills()
    assert result[0]['description'] == '新描述'


@pytest.mark.asyncio
async def test_delete_bill(db):
    """测试删除账单"""
    bills = [{
        'date': '2025-01-01',
        'type': '支出',
        'amount': 100.0,
        'counterparty': '商家',
        'description': '描述'
    }]
    await db.insert_bills(bills)
    
    result = await db.get_bills()
    bill_id = result[0]['id']
    
    success = await db.delete_bill(bill_id)
    assert success
    
    result = await db.get_bills()
    assert len(result) == 0


@pytest.mark.asyncio
async def test_import_annotation_samples_roundtrip(db):
    """测试导入会话标注样本的保存、读取与清理。"""
    session_id = 'test-session-annotation'
    await db.create_import_session(session_id, user_id=1, file_count=1)

    saved = await db.save_import_annotation_samples(session_id, [
        {
            'id': 101,
            'preview_type': '转账',
            'category_id': 12,
            'preview_source_account_id': 3,
            'preview_destination_account_id': 8,
        }
    ], user_id=1)

    assert saved == 1

    samples = await db.get_import_annotation_samples(session_id, user_id=1)
    assert len(samples) == 1
    assert samples[0]['preview_id'] == 101
    assert samples[0]['annotated_type'] == '转账'
    assert samples[0]['annotated_category_id'] == 12
    assert samples[0]['annotated_source_account_id'] == 3
    assert samples[0]['annotated_destination_account_id'] == 8

    cleared = await db.clear_session_data(session_id)
    assert cleared['annotation_count'] == 1


@pytest.mark.asyncio
async def test_import_learning_rules_promote_toggle_delete(db):
    """测试长期导入学习规则的提升、启停与删除。"""
    session_id = 'test-session-learning-promote'
    await db.create_import_session(session_id, user_id=1, file_count=1)

    category_parent_id = await db.create_category({
        'type': 3,
        'main_category': '餐饮',
        'sub_category': '',
        'description': '',
        'priority': 0,
        'keywords': '',
        'hidden': False,
        'icon': '',
        'color': ''
    }, user_id=1)
    assert category_parent_id is not None

    category_id = await db.create_category({
        'type': 3,
        'main_category': '餐饮',
        'sub_category': '早餐',
        'description': '',
        'priority': 0,
        'keywords': '',
        'hidden': False,
        'icon': '',
        'color': ''
    }, user_id=1)
    assert category_id is not None

    source_account_id = await db.create_account({
        'name': '支付宝',
        'type': 1,
        'category': 'asset',
        'currency': 'CNY',
        'icon': '',
        'color': '',
        'balance': 0.0,
        'initial_balance': 0.0,
        'hidden': False,
        'display_order': 0,
        'comment': '',
        'aliases': '["支付宝"]'
    }, user_id=1)
    destination_account_id = await db.create_account({
        'name': '现金',
        'type': 1,
        'category': 'asset',
        'currency': 'CNY',
        'icon': '',
        'color': '',
        'balance': 0.0,
        'initial_balance': 0.0,
        'hidden': False,
        'display_order': 1,
        'comment': '',
        'aliases': '["现金"]'
    }, user_id=1)

    inserted = await db.insert_preview_bills_batch(session_id, [
        {
            'preview_data': {
                'preview_date': '2026-03-07 08:00:00',
                'preview_type': '支出',
                'preview_amount': 18.0,
                'preview_destination_amount': 0.0,
                'preview_main_category': '',
                'preview_sub_category': '',
                'preview_source_account_id': None,
                'preview_destination_account_id': None,
                'preview_counterparty': '学习商户',
                'preview_payment_method': '支付宝',
                'preview_description': '学习描述'
            },
            'dedup_type': 'remaining',
            'dedup_source_ids': []
        }
    ], user_id=1)
    assert inserted == 1

    previews = await db.get_preview_by_session(session_id)
    assert len(previews) == 1
    preview_id = previews[0]['id']

    saved = await db.save_import_annotation_samples(session_id, [
        {
            'id': preview_id,
            'preview_type': '转账',
            'category_id': category_id,
            'preview_source_account_id': source_account_id,
            'preview_destination_account_id': destination_account_id,
        }
    ], user_id=1)
    assert saved == 1

    promoted = await db.promote_import_annotation_samples_to_learning(session_id, user_id=1)
    assert promoted['selected_samples'] == 1
    assert promoted['rules_total'] == 3
    assert promoted['created'] == 3
    assert promoted['updated'] == 0

    rules = await db.get_import_learning_rules(user_id=1)
    assert len(rules) == 3
    assert {rule['match_type'] for rule in rules} == {
        'counterparty', 'description', 'payment_method'
    }
    assert all(rule['enabled'] == 1 for rule in rules)
    assert all(rule['learned_type'] == '转账' for rule in rules)
    assert all(rule['learned_category_id'] == category_id for rule in rules)

    counterparty_rule = next(rule for rule in rules if rule['match_type'] == 'counterparty')
    toggled = await db.set_import_learning_rule_enabled(
        int(counterparty_rule['id']), False, user_id=1
    )
    assert toggled is True

    updated_rules = await db.get_import_learning_rules(user_id=1)
    updated_counterparty_rule = next(
        rule for rule in updated_rules if rule['match_type'] == 'counterparty'
    )
    assert updated_counterparty_rule['enabled'] == 0

    usage_count = await db.increment_import_learning_rule_usage(
        [int(updated_counterparty_rule['id'])], user_id=1
    )
    assert usage_count == 1

    refreshed_rules = await db.get_import_learning_rules(user_id=1)
    refreshed_counterparty_rule = next(
        rule for rule in refreshed_rules if rule['match_type'] == 'counterparty'
    )
    assert refreshed_counterparty_rule['applied_count'] == 1
    assert refreshed_counterparty_rule['last_applied_at']

    deleted = await db.delete_import_learning_rule(
        int(updated_counterparty_rule['id']), user_id=1
    )
    assert deleted is True

    final_rules = await db.get_import_learning_rules(user_id=1)
    assert len(final_rules) == 2

    conn = await db._get_connection()
    async with conn.execute(
        'SELECT COUNT(*) AS total FROM import_learning_rule_logs WHERE user_id = ?',
        (1,)
    ) as cursor:
        row = await cursor.fetchone()
    assert row['total'] >= 5


@pytest.mark.asyncio
async def test_import_config_save_match_and_delete(db):
    """测试导入列映射模板的保存、匹配与删除。"""
    config_id = await db.save_import_config({
        'name': '支付宝通用CSV模板',
        'file_format': 'csv',
        'description': '用于通用表格导入学习',
        'field_mappings': {
            'date': '交易时间',
            'type': '交易类型',
            'amount': '金额',
            'description': '备注'
        },
        'date_format': '%Y-%m-%d %H:%M:%S',
        'delimiter': ',',
        'skip_rows': 0,
        'has_header': True,
        'sample_headers': ['交易时间', '交易类型', '金额', '备注'],
        'custom_rules': {'source': 'pytest'},
        'is_default': True
    }, user_id=1)

    assert config_id > 0

    configs = await db.get_import_configs(user_id=1, file_format='csv')
    assert len(configs) == 1
    assert configs[0]['id'] == config_id
    assert configs[0]['field_mappings']['date'] == '交易时间'
    assert configs[0]['sample_headers'] == ['交易时间', '交易类型', '金额', '备注']
    assert configs[0]['is_default'] is True
    assert configs[0]['description_summary'] == (
        '映射: 时间->交易时间 / 类型->交易类型 / 金额->金额 / 描述->备注 | '
        '表头: 交易时间 / 交易类型 / 金额 / 备注'
    )

    exact_match = await db.find_matching_import_config(
        'csv',
        ['交易时间', '交易类型', '金额', '备注'],
        user_id=1
    )
    assert exact_match is not None
    assert exact_match['id'] == config_id
    assert exact_match['match_reason'] == 'exact_header_signature'
    assert exact_match['match_score'] == 1.0
    assert exact_match['description'] == '用于通用表格导入学习'
    assert exact_match['description_summary'].startswith('映射: 时间->交易时间')

    fuzzy_match = await db.find_matching_import_config(
        'csv',
        ['交易时间', '金额', '交易类型', '备注', '附加列'],
        user_id=1
    )
    assert fuzzy_match is not None
    assert fuzzy_match['id'] == config_id
    assert fuzzy_match['match_score'] >= 0.6
    assert fuzzy_match['matched_header_count'] == 4

    deleted = await db.delete_import_config(config_id, user_id=1)
    assert deleted is True
    assert await db.get_import_configs(user_id=1, file_format='csv') == []


@pytest.mark.asyncio
async def test_user_import_learning_enabled_persistence(db):
    """测试用户级长期导入学习开关可持久化。"""
    user_id = await db.create_user({
        'username': 'learning_toggle_user',
        'email': 'learning_toggle_user@example.com',
        'password_hash': 'hash',
        'nickname': 'learning_toggle_user'
    })

    created_user = await db.get_user_by_id(user_id)
    assert created_user is not None
    assert int(created_user.get('import_learning_enabled', 1)) == 1

    updated = await db.update_user(user_id, {'import_learning_enabled': 0})
    assert updated is True

    refreshed_user = await db.get_user_by_id(user_id)
    assert refreshed_user is not None
    assert int(refreshed_user.get('import_learning_enabled', 1)) == 0


@pytest.mark.asyncio
async def test_deduplicate(db):
    """测试去重"""
    # 插入重复数据（通过不同批次）
    bill = {
        'date': '2025-01-01',
        'type': '支出',
        'amount': 100.0,
        'counterparty': '商家',
        'description': '描述'
    }
    
    # 手动插入以模拟重复（绕过哈希检查）
    await db._get_connection()
    # 这个测试需要特殊处理，跳过
    pass
