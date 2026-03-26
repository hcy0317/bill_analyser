"""最终验证测试 - 验证所有3个问题已修复"""
import requests


def test_reconciliation_api():
    """测试对账单API"""
    url = "http://127.0.0.1:5000/api/bills/reconciliation_statements"
    params = {
        'account_id': 3,
        'start_time': 1759248000,
        'end_time': 1764518400
    }
    
    print("=" * 60)
    print("最终验证测试：对账单API")
    print("=" * 60)
    
    response = requests.get(url, params=params, timeout=10)
    assert response.status_code == 200, f"API返回错误状态码: {response.status_code}"
    
    data = response.json()
    result = data.get('result', {})
    transactions = result.get('transactions', [])
    
    print(f"\n✓ API调用成功 (状态码200)")
    print(f"✓ 返回{len(transactions)}笔交易")
    
    # 问题1: 日期显示星期几
    print("\n" + "=" * 60)
    print("问题1: 日期分组显示星期几")
    print("=" * 60)
    for txn in transactions:
        date_str = txn.get('gregorianCalendarYearDashMonthDashDay')
        day_of_week = txn.get('displayDayOfWeek')
        assert date_str, f"交易#{txn['id']} 缺少日期字段"
        assert day_of_week is not None, f"交易#{txn['id']} 缺少星期字段"
        
        weekday_names = ['', '周日', '周一', '周二', '周三', '周四', '周五', '周六']
        weekday_name = weekday_names[day_of_week]
        
        print(f"✓ 交易#{txn['id']}: {date_str} {weekday_name}")
    
    # 问题2: 对账单显示分类
    print("\n" + "=" * 60)
    print("问题2: 对账单显示分类")
    print("=" * 60)
    for txn in transactions:
        category_name = txn.get('categoryName')
        assert category_name, f"交易#{txn['id']} 缺少分类字段"
        print(f"✓ 交易#{txn['id']}: 分类={category_name}")
    
    # 问题3: 账户余额趋势逻辑正确
    print("\n" + "=" * 60)
    print("问题3: 账户余额趋势逻辑")
    print("=" * 60)
    
    # 预期值：
    # 10月15日: -333元 = -33300分
    # 11月21日: -444元 = -44400分 (累计支出333+111)
    
    txn_222 = next((t for t in transactions if t['id'] == '222'), None)
    txn_223 = next((t for t in transactions if t['id'] == '223'), None)
    
    assert txn_222, "未找到交易#222 (11月21日, 111元)"
    assert txn_223, "未找到交易#223 (10月15日, 333元)"
    
    balance_222 = txn_222.get('accountClosingBalance')
    balance_223 = txn_223.get('accountClosingBalance')
    
    assert balance_222 == -44400, f"交易#222余额错误: 期望-44400, 实际{balance_222}"
    assert balance_223 == -33300, f"交易#223余额错误: 期望-33300, 实际{balance_223}"
    
    print(f"✓ 交易#223 (10月15日 支出333元): 账户余额 = {balance_223/100:.2f}元")
    print(f"✓ 交易#222 (11月21日 支出111元): 账户余额 = {balance_222/100:.2f}元")
    print(f"✓ 余额累计计算正确: 0 - 333 = -333, -333 - 111 = -444")
    
    # 验证汇总数据
    print("\n" + "=" * 60)
    print("汇总数据验证")
    print("=" * 60)
    opening = result.get('openingBalance')
    closing = result.get('closingBalance')
    inflows = result.get('totalInflows')
    outflows = result.get('totalOutflows')
    net = result.get('netFlow')
    
    assert opening == 0, f"期初余额错误: 期望0, 实际{opening}"
    assert closing == -44400, f"期末余额错误: 期望-44400, 实际{closing}"
    assert inflows == 0, f"总流入错误: 期望0, 实际{inflows}"
    assert outflows == 44400, f"总流出错误: 期望44400, 实际{outflows}"
    assert net == -44400, f"净流入错误: 期望-44400, 实际{net}"
    
    print(f"✓ 期初余额: {opening/100:.2f}元")
    print(f"✓ 期末余额: {closing/100:.2f}元")
    print(f"✓ 总流入: {inflows/100:.2f}元")
    print(f"✓ 总流出: {outflows/100:.2f}元")
    print(f"✓ 净流入: {net/100:.2f}元")
    
    print("\n" + "=" * 60)
    print("所有测试通过！")
    print("=" * 60)
    print("\n✅ 问题1: 日期显示星期几 - 已修复")
    print("✅ 问题2: 对账单显示分类 - 已修复")
    print("✅ 问题3: 账户余额趋势逻辑 - 已修复")
    print("\n修复摘要:")
    print("- gregorianCalendarYearDashMonthDashDay 字段正确返回")
    print("- displayDayOfWeek 字段正确返回 (1=周日, 2=周一, ..., 7=周六)")
    print("- categoryName 字段正确返回")
    print("- accountClosingBalance 按时间正序计算，逻辑正确")
    

if __name__ == '__main__':
    try:
        test_reconciliation_api()
    except AssertionError as e:
        print(f"\n❌ 测试失败: {e}")
        exit(1)
    except Exception as e:
        print(f"\n❌ 测试异常: {e}")
        exit(1)
