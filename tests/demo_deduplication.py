"""
快速测试去重功能

创建模拟账单数据，测试去重引擎的各项功能
"""

import sys
from pathlib import Path

# 添加项目根目录到路径
sys.path.insert(0, str(Path(__file__).parent.parent))

# pylint: disable=wrong-import-position
from src.utils.deduplication import DeduplicationEngine, DeduplicationMode


def create_test_bills():
    """创建测试账单数据"""
    return [
        # 1. 支付宝账单（与银行重复）
        {
            'id': 1,
            'source': 'alipay',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'description': '淘宝购物',
            'transaction_no': ''
        },
        # 2. 银行账单（与支付宝对应）
        {
            'id': 2,
            'source': 'icbc',
            'date': '2025-01-15 10:05:00',
            'amount': -100.00,
            'description': '支付宝消费-淘宝',
            'transaction_no': ''
        },
        # 3. 微信账单（与银行重复）
        {
            'id': 3,
            'source': 'wechat',
            'date': '2025-01-16 14:30:00',
            'amount': -50.00,
            'description': '餐饮美食',
            'transaction_no': ''
        },
        # 4. 银行账单（与微信对应）
        {
            'id': 4,
            'source': 'cmbc',
            'date': '2025-01-16 14:35:00',
            'amount': -50.00,
            'description': '微信支付-餐饮',
            'transaction_no': ''
        },
        # 5. 转账：工商 -> 支付宝
        {
            'id': 5,
            'source': 'icbc',
            'date': '2025-01-17 09:00:00',
            'amount': -200.00,
            'description': '转账到支付宝',
            'transaction_no': ''
        },
        # 6. 转账：支付宝收入
        {
            'id': 6,
            'source': 'alipay',
            'date': '2025-01-17 09:05:00',
            'amount': 200.00,
            'description': '银行卡充值',
            'transaction_no': ''
        },
        # 7. 转账：支付宝 -> 微信
        {
            'id': 7,
            'source': 'alipay',
            'date': '2025-01-18 10:00:00',
            'amount': -150.00,
            'description': '提现到微信',
            'transaction_no': ''
        },
        # 8. 转账：微信收入
        {
            'id': 8,
            'source': 'wechat',
            'date': '2025-01-18 10:10:00',
            'amount': 150.00,
            'description': '收款',
            'transaction_no': ''
        },
        # 9. 真实消费：支付宝
        {
            'id': 9,
            'source': 'alipay',
            'date': '2025-01-19 12:00:00',
            'amount': -30.00,
            'description': '外卖订单',
            'transaction_no': ''
        },
        # 10. 真实消费：微信
        {
            'id': 10,
            'source': 'wechat',
            'date': '2025-01-20 15:00:00',
            'amount': -45.00,
            'description': '打车费用',
            'transaction_no': ''
        },
        # 11. 真实消费：银行
        {
            'id': 11,
            'source': 'icbc',
            'date': '2025-01-21 18:00:00',
            'amount': -500.00,
            'description': 'POS消费-商场',
            'transaction_no': ''
        },
    ]


def test_simple_mode():
    """测试Simple模式"""
    print("\n" + "=" * 70)
    print("测试 Simple 模式（仅删除完全重复）")
    print("=" * 70)

    engine = DeduplicationEngine(DeduplicationMode.SIMPLE)
    bills = create_test_bills()

    print(f"\n原始账单数: {len(bills)}")

    result_bills, stats = engine.deduplicate_all(bills)

    print(f"去重后账单数: {len(result_bills)}")
    print(f"支付工具-银行重复: {stats['payment_bank_duplicates']}")
    print(f"账户间转账重复: {stats['transfer_duplicates']}")

    print("\n保留的账单:")
    for bill in result_bills:
        print(f"  [{bill['id']}] {bill['source']:8} {bill['amount']:8.2f} | {bill['description']}")


def test_advanced_mode():
    """测试Advanced模式"""
    print("\n" + "=" * 70)
    print("测试 Advanced 模式（推荐模式）")
    print("=" * 70)

    engine = DeduplicationEngine(DeduplicationMode.ADVANCED)
    bills = create_test_bills()

    print(f"\n原始账单数: {len(bills)}")

    result_bills, stats = engine.deduplicate_all(bills)

    print(f"去重后账单数: {len(result_bills)}")
    print(f"支付工具-银行重复: {stats['payment_bank_duplicates']}")
    print(f"账户间转账重复: {stats['transfer_duplicates']}")

    print("\n保留的账单:")
    for bill in result_bills:
        print(f"  [{bill['id']}] {bill['source']:8} {bill['amount']:8.2f} | {bill['description']}")

    print("\n" + engine.get_deduplication_report(stats))


def test_aggressive_mode():
    """测试Aggressive模式"""
    print("\n" + "=" * 70)
    print("测试 Aggressive 模式（最激进）")
    print("=" * 70)

    engine = DeduplicationEngine(DeduplicationMode.AGGRESSIVE)
    bills = create_test_bills()

    print(f"\n原始账单数: {len(bills)}")

    result_bills, stats = engine.deduplicate_all(bills)

    print(f"去重后账单数: {len(result_bills)}")
    print(f"支付工具-银行重复: {stats['payment_bank_duplicates']}")
    print(f"账户间转账重复: {stats['transfer_duplicates']}")

    print("\n保留的账单:")
    for bill in result_bills:
        print(f"  [{bill['id']}] {bill['source']:8} {bill['amount']:8.2f} | {bill['description']}")


def main():
    """主函数"""
    print("\n" + "=" * 70)
    print("账单去重功能快速测试")
    print("=" * 70)

    print("\n测试数据说明:")
    print("  - 账单1 (支付宝) + 账单2 (银行): 重复交易")
    print("  - 账单3 (微信) + 账单4 (银行): 重复交易")
    print("  - 账单5 + 账单6: 转账对 (工商→支付宝)")
    print("  - 账单7 + 账单8: 转账对 (支付宝→微信)")
    print("  - 账单9, 10, 11: 真实消费")

    # 测试三种模式
    test_simple_mode()
    test_advanced_mode()
    test_aggressive_mode()

    print("\n" + "=" * 70)
    print("测试完成")
    print("=" * 70)


if __name__ == '__main__':
    main()
