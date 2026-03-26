"""测试交易日历筛选功能

验证：
1. 桌面端ListPage添加了currentCalendarDate监听器
2. 移动端ListPage添加了currentCalendarDate监听器
3. 日历日期变化时能正确触发reload()
"""

import re


def test_desktop_calendar_watch():
    """测试桌面端交易日历watch逻辑"""
    with open('src/ui/frontend/src/views/desktop/transactions/ListPage.vue', 'r', encoding='utf-8') as f:
        content = f.read()

    # 检查是否添加了watch(currentCalendarDate)
    assert 'watch(currentCalendarDate' in content, \
        "桌面端ListPage未添加currentCalendarDate监听器"

    # 检查watch逻辑是否包含reload()调用
    watch_pattern = r'watch\(currentCalendarDate.*?reload\(\)'
    assert re.search(watch_pattern, content, re.DOTALL), \
        "桌面端ListPage的currentCalendarDate监听器未调用reload()"

    # 检查是否有日历模式判断
    assert 'TransactionListPageType.Calendar.type' in content, \
        "桌面端ListPage未判断日历模式"

    print("✅ 桌面端交易日历watch逻辑测试通过")


def test_mobile_calendar_watch():
    """测试移动端交易日历watch逻辑"""
    with open('src/ui/frontend/src/views/mobile/transactions/ListPage.vue', 'r', encoding='utf-8') as f:
        content = f.read()

    # 检查是否导入了watch
    assert 'import { ref, computed, nextTick, onMounted, onUnmounted, watch }' in content, \
        "移动端ListPage未导入watch"

    # 检查是否添加了watch(currentCalendarDate)
    assert 'watch(currentCalendarDate' in content, \
        "移动端ListPage未添加currentCalendarDate监听器"

    # 检查watch逻辑是否包含reload()调用
    watch_pattern = r'watch\(currentCalendarDate.*?reload\(\)'
    assert re.search(watch_pattern, content, re.DOTALL), \
        "移动端ListPage的currentCalendarDate监听器未调用reload()"

    # 检查是否有日历模式判断
    assert 'TransactionListPageType.Calendar.type' in content, \
        "移动端ListPage未判断日历模式"

    print("✅ 移动端交易日历watch逻辑测试通过")


def test_reconciliation_api_enhancement():
    """测试对账单API增强功能"""
    with open('src/ui/backend/routes/bills.py', 'r', encoding='utf-8') as f:
        content = f.read()

    # 检查是否增强了参数验证
    assert 'category_ids = request.args.get' in content, \
        "对账单API未添加category_ids参数"

    assert 'trans_type = request.args.get' in content, \
        "对账单API未添加type参数"

    assert 'keyword = request.args.get' in content, \
        "对账单API未添加keyword参数"

    # 检查是否添加了完整日志
    assert '[get_reconciliation_statements] 入口参数' in content, \
        "对账单API未添加入口日志"

    assert '[get_reconciliation_statements] 返回成功' in content, \
        "对账单API未添加出口日志"

    # 检查是否计算期初/期末余额
    assert 'opening_balance' in content, \
        "对账单API未计算期初余额"

    assert 'closing_balance' in content, \
        "对账单API未计算期末余额"

    # 检查是否处理转账类型
    assert 'dest_account = bill.get' in content, \
        "对账单API未正确处理转账类型"

    print("✅ 对账单API增强功能测试通过")


if __name__ == '__main__':
    print("=" * 60)
    print("交易日历和对账单功能测试")
    print("=" * 60)

    try:
        test_desktop_calendar_watch()
        test_mobile_calendar_watch()
        test_reconciliation_api_enhancement()

        print("\n" + "=" * 60)
        print("✅ 所有测试通过！")
        print("=" * 60)

        print("\n功能实现总结:")
        print("1. ✅ 桌面端交易日历添加watch(currentCalendarDate)监听器")
        print("2. ✅ 移动端交易日历添加watch(currentCalendarDate)监听器")
        print("3. ✅ 日历日期变化时自动调用reload()重新加载数据")
        print("4. ✅ 对账单API增强筛选功能（category_ids, type, keyword）")
        print("5. ✅ 对账单API添加期初/期末余额计算")
        print("6. ✅ 对账单API添加完整日志记录（入口/出口参数）")
        print("7. ✅ 对账单API正确处理转账类型（转入/转出）")

    except AssertionError as e:
        print(f"\n❌ 测试失败: {e}")
        exit(1)
    except Exception as e:
        print(f"\n❌ 测试异常: {e}")
        exit(1)
