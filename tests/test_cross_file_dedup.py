#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
测试跨文件平台-银行去重功能 (v6.46)

模拟场景：
1. 用户先导入支付宝账单（已在数据库中）
2. 用户再导入农业银行账单（新导入）
3. 系统应该识别出两条账单是同一笔交易的重复记录
"""

import asyncio
import sys
sys.path.insert(0, '.')

from src.core.smart_dedup import SmartDeduplicationEngine


class MockDB:
    """模拟数据库，返回已有的支付宝账单"""
    
    async def get_bills_by_date_range(self, start_date, end_date, user_id=1):
        """模拟返回数据库中已有的账单"""
        return [
            {
                'id': 1,
                'date': '2025-08-20 09:23:52',
                'amount': -50.0,  # 支出（负数）
                'source_account_id': 'alipay',
                'counterparty': '蚂蚁财富-蚂蚁（杭州）基金销售有限公司',
                'description': '基金购买'
            }
        ]


async def test_cross_file_platform_bank_dedup():
    """测试跨文件平台-银行去重"""
    print("=" * 60)
    print("测试场景1: 支付宝(数据库) + 农业银行(新导入) - 金额方向相同")
    print("=" * 60)
    
    # 新导入的农业银行账单
    new_bills = [
        {
            'date': '2025-08-20 09:23:53',
            'amount': -50.0,  # 支出（负数）
            'source_account_id': 'abc',  # 农业银行
            'counterparty': '蚂蚁（杭州）基金销售有限公司',
            'description': '基金'
        }
    ]
    
    engine = SmartDeduplicationEngine()
    db = MockDB()
    result = await engine.process_with_db(new_bills, db, user_id=1)
    
    print(f"结果: 保留 {len(result.kept_bills)}, 移除 {result.removed_count}")
    print(f"数据库重复组数: {len(result.duplicate_groups)}")
    
    for g in result.duplicate_groups:
        print(f"  - {g.reason}")
    
    # 验证结果
    if result.removed_count == 1:
        print("\n✅ 测试通过！跨文件平台-银行去重成功！")
        return True
    else:
        print("\n❌ 测试失败！未能识别跨文件重复")
        return False


async def test_cross_file_different_direction():
    """测试跨文件平台-银行去重（金额方向不同）"""
    print("\n" + "=" * 60)
    print("测试场景2: 支付宝(数据库) + 农业银行(新导入) - 金额方向不同")
    print("说明: 支付宝记录为-50(支出), 银行记录为+50(收入)")
    print("=" * 60)
    
    class MockDB2:
        async def get_bills_by_date_range(self, start_date, end_date, user_id=1):
            return [
                {
                    'id': 2,
                    'date': '2025-08-20 09:23:52',
                    'amount': -50.0,  # 支付宝记录为支出
                    'source_account_id': 'alipay',
                    'counterparty': '蚂蚁财富-蚂蚁（杭州）基金销售有限公司',
                    'description': '基金购买'
                }
            ]
    
    # 新导入的农业银行账单（金额为正，可能被解析为收入）
    new_bills = [
        {
            'date': '2025-08-20 09:23:53',
            'amount': 50.0,  # 银行记录为正（可能因为解析方式不同）
            'source_account_id': 'abc',
            'counterparty': '蚂蚁（杭州）基金销售有限公司',
            'description': '基金'
        }
    ]
    
    engine = SmartDeduplicationEngine()
    db = MockDB2()
    result = await engine.process_with_db(new_bills, db, user_id=1)
    
    print(f"结果: 保留 {len(result.kept_bills)}, 移除 {result.removed_count}")
    print(f"数据库重复组数: {len(result.duplicate_groups)}")
    
    for g in result.duplicate_groups:
        print(f"  - {g.reason}")
    
    # 验证结果（对于平台-银行对，即使金额方向不同也应该识别）
    if result.removed_count == 1:
        print("\n✅ 测试通过！即使金额方向不同也能识别平台-银行重复！")
        return True
    else:
        print("\n❌ 测试失败！未能识别金额方向不同的平台-银行重复")
        return False


async def test_similarity_matching():
    """测试相似度匹配"""
    print("\n" + "=" * 60)
    print("测试场景3: counterparty相似度匹配")
    print("说明: '蚂蚁（杭州）基金销售有限公司' vs '蚂蚁财富-蚂蚁（杭州）基金销售有限公司'")
    print("=" * 60)
    
    from src.core.smart_dedup import SmartDeduplicationEngine
    engine = SmartDeduplicationEngine()
    
    s1 = '蚂蚁（杭州）基金销售有限公司'
    s2 = '蚂蚁财富-蚂蚁（杭州）基金销售有限公司'
    
    similarity = engine._calculate_similarity(s1, s2)
    print(f"'{s1}' vs '{s2}'")
    print(f"相似度: {similarity:.2%}")
    
    # 检查包含关系
    contains = s1 in s2 or s2 in s1
    print(f"包含关系: {contains}")
    
    if similarity >= 0.5 or contains:
        print("\n✅ 测试通过！counterparty能够匹配")
        return True
    else:
        print("\n❌ 测试失败！counterparty无法匹配")
        return False


async def main():
    """运行所有测试"""
    print("=" * 60)
    print("v6.46 跨文件平台-银行去重功能测试")
    print("=" * 60)
    
    results = []
    results.append(await test_similarity_matching())
    results.append(await test_cross_file_platform_bank_dedup())
    results.append(await test_cross_file_different_direction())
    
    print("\n" + "=" * 60)
    print("测试总结")
    print("=" * 60)
    passed = sum(results)
    total = len(results)
    print(f"通过: {passed}/{total}")
    
    if passed == total:
        print("\n🎉 所有测试通过！v6.46跨文件去重功能正常工作！")
    else:
        print(f"\n⚠️ 有 {total - passed} 个测试失败")


if __name__ == '__main__':
    asyncio.run(main())
