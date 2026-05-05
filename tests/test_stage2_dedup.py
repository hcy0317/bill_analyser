#!/usr/bin/env python
"""
测试三阶段导入的平台-银行去重

完整测试 import_stage2_dedup 方法中的平台-银行去重是否正确工作
"""

# pylint: disable=trailing-whitespace

import asyncio
import sqlite3

from bill_analyser.core.bills import BillService
from bill_analyser.core.db import Database
from tests.runtime_paths import get_test_db_path, remove_test_database_family


def setup_test_data(db_path, session_id, user_id):
    """使用同步方式插入测试数据"""
    conn = sqlite3.connect(db_path)
    c = conn.cursor()

    # 清空之前的测试数据
    c.execute("DELETE FROM import_sessions WHERE session_id = ?", (session_id,))
    c.execute("DELETE FROM bills_parser_template WHERE session_id = ?", (session_id,))
    c.execute("DELETE FROM bills_preview WHERE session_id = ?", (session_id,))

    # 创建导入会话
    c.execute("""
        INSERT INTO import_sessions (session_id, user_id, status, file_count, 
        total_parsed, total_preview, total_confirmed, created_at, updated_at)
        VALUES (?, ?, 'parsing', 2, 0, 0, 0, datetime('now'), datetime('now'))
    """, (session_id, user_id))

    # 插入模拟的解析器模板数据
    templates = [
        {
            "parser_date": "2025-08-25 09:30:13",
            "parser_amount": -50.0,
            "parser_type": "支出",
            "parser_description": "基金购买",
            "parser_id": "abc",  # 农业银行
            "parser_counterparty": "蚂蚁（杭州）基金销售有限公司",
            "parser_payment_method": "中国农业银行",
            "parser_original_type": "消费",
            "parser_original_category": "",
            "parser_account_id": 1,
        },
        {
            "parser_date": "2025-08-25 09:30:12",
            "parser_amount": -50.0,
            "parser_type": "支出",
            "parser_description": "理财购买",
            "parser_id": "alipay",  # 支付宝
            "parser_counterparty": "国泰黄金-蚂蚁（杭州）基金销售有限公司",
            "parser_payment_method": "支付宝",
            "parser_original_type": "消费",
            "parser_original_category": "",
            "parser_account_id": 2,
        },
        # 添加一些不相关的账单，验证不会误伤
        {
            "parser_date": "2025-08-25 10:00:00",
            "parser_amount": -100.0,
            "parser_type": "支出",
            "parser_description": "午餐",
            "parser_id": "wechat",
            "parser_counterparty": "餐厅",
            "parser_payment_method": "微信支付",
            "parser_original_type": "消费",
            "parser_original_category": "",
            "parser_account_id": 3,
        },
    ]

    for t in templates:
        c.execute("""
            INSERT INTO bills_parser_template 
            (session_id, user_id, parser_date, parser_amount, parser_type, 
             parser_description, parser_id, parser_counterparty, parser_payment_method,
             parser_original_type, parser_original_category, parser_account_id,
             parser_is_processed, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
        """, (session_id, user_id, t["parser_date"], t["parser_amount"], t["parser_type"],
              t["parser_description"], t["parser_id"], t["parser_counterparty"],
              t["parser_payment_method"], t["parser_original_type"],
              t["parser_original_category"], t["parser_account_id"]))

    conn.commit()
    conn.close()
    return len(templates)


def check_results(db_path, session_id):
    """检查结果"""
    conn = sqlite3.connect(db_path)
    c = conn.cursor()

    print("\n检查预览表数据:")
    c.execute("""
        SELECT id, preview_date, preview_amount, dedup_type, dedup_source_ids
        FROM bills_preview WHERE session_id = ?
        ORDER BY id
    """, (session_id,))
    rows = c.fetchall()

    for row in rows:
        print(f"  ID={row[0]}: date={row[1]}, amount={row[2]}, "
              f"dedup_type={row[3]}, source_ids={row[4]}")

    # 验证结果
    print("\n验证结果:")

    # 应该有2条预览记录（alipay保留合并了abc，wechat单独保留）
    if len(rows) == 2:
        print("✅ 预览记录数正确: 2条")
    else:
        print(f"❌ 预览记录数错误: 应为2条，实际为{len(rows)}条")

    # 检查具体的去重类型
    c.execute("""
        SELECT dedup_type, dedup_source_ids FROM bills_preview 
        WHERE session_id = ? AND dedup_type = 'platform_bank'
    """, (session_id,))
    platform_bank_rows = c.fetchall()

    if platform_bank_rows:
        print("✅ 存在 platform_bank 类型的去重记录")
        for row in platform_bank_rows:
            print(f"   source_ids: {row[1]}")
    else:
        print("❌ 没有找到 platform_bank 类型的去重记录")

    conn.close()


async def test_stage2_dedup():
    """测试阶段2的平台-银行去重"""
    db_path = get_test_db_path("test_stage2_dedup.db")
    user_id = 1
    session_id = "test-session-002"

    print("=" * 60)
    print("测试阶段2去重 - 平台-银行去重")
    print("=" * 60)

    remove_test_database_family(db_path)

    db = Database(str(db_path))
    await db.init_db()
    try:
        # 1. 使用同步方式插入测试数据
        template_count = setup_test_data(db_path, session_id, user_id)
        print(f"创建会话: {session_id}")
        print(f"插入了 {template_count} 条解析模板")

        # 2. 使用异步方式执行阶段2
        bill_service = BillService(db)

        print("\n执行阶段2...")
        result = await bill_service.import_stage2_dedup(session_id, user_id)

        print("\n阶段2结果:")
        print(f"  - success: {result.get('success')}")
        print(f"  - preview_count: {result.get('preview_count')}")
        print(f"  - dedup_stats: {result.get('dedup_stats')}")
        print(f"  - match_stats: {result.get('match_stats')}")

        # 3. 使用同步方式检查结果
        check_results(db_path, session_id)
    finally:
        await db.close()
        remove_test_database_family(db_path)


if __name__ == "__main__":
    asyncio.run(test_stage2_dedup())
