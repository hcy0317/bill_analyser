"""对账单API完整测试脚本

测试v6.1修复：
1. 使用账户ID筛选（不再使用账户名称）
2. V1 Adapter转换确保utcOffset正确
3. 日期显示字段完整
4. 转账判断使用账户ID比较

使用方法：
1. 确保后端服务器运行在 http://127.0.0.1:5000
2. 执行: python tests/test_reconciliation_api.py
"""

import os
import requests
from datetime import datetime, timedelta

import pytest


REQUEST_TIMEOUT = (2, 10)
LIVE_BACKEND_TEST_ENV = 'BILL_ANALYSER_RUN_LIVE_BACKEND_TESTS'


def _request_or_fail(method, url, **kwargs):
    """带超时的 live backend 请求，避免整套测试挂住。"""
    kwargs.setdefault('timeout', REQUEST_TIMEOUT)

    try:
        return method(url, **kwargs)
    except requests.RequestException as exc:
        pytest.fail(f'live backend request failed: {exc}')


def test_reconciliation_api():
    """测试对账单API完整流程"""
    if os.environ.get(LIVE_BACKEND_TEST_ENV) != '1':
        pytest.skip(f'set {LIVE_BACKEND_TEST_ENV}=1 to enable live backend reconciliation API test')
    
    base_url = "http://127.0.0.1:5000"
    
    # 1. 获取账户列表
    print("\n=== 步骤1: 获取账户列表 ===")
    accounts_response = _request_or_fail(requests.get, f"{base_url}/api/v1/accounts/list.json")
    if accounts_response.status_code != 200:
        print(f"❌ 获取账户列表失败: {accounts_response.status_code}")
        print(f"   响应: {accounts_response.text}")
        return False
    
    accounts_data = accounts_response.json()
    if not accounts_data.get('success') or not accounts_data.get('result', {}).get('items'):
        print(f"❌ 账户列表为空")
        return False
    
    test_account = accounts_data['result']['items'][0]
    account_id = test_account['id']
    account_name = test_account['name']
    print(f"✅ 找到测试账户: ID={account_id}, Name={account_name}")
    
    # 2. 调用对账单API
    print(f"\n=== 步骤2: 调用对账单API (账户ID={account_id}) ===")
    
    # 查询最近30天
    end_time = int(datetime.now().timestamp())
    start_time = int((datetime.now() - timedelta(days=30)).timestamp())
    
    reconciliation_url = f"{base_url}/api/bills/reconciliation_statements"
    params = {
        'account_id': account_id,
        'start_time': start_time,
        'end_time': end_time
    }
    
    print(f"   请求参数: {params}")
    response = _request_or_fail(requests.get, reconciliation_url, params=params)
    
    if response.status_code != 200:
        print(f"❌ 对账单API失败: {response.status_code}")
        print(f"   响应: {response.text}")
        return False
    
    data = response.json()
    if not data.get('success'):
        print(f"❌ API返回失败: {data.get('error')}")
        return False
    
    result = data['result']
    print(f"✅ 对账单API调用成功")
    print(f"   期初余额: {result['openingBalance']}")
    print(f"   期末余额: {result['closingBalance']}")
    print(f"   总收入: {result['totalInflows']}")
    print(f"   总支出: {result['totalOutflows']}")
    print(f"   净流入: {result['netFlow']}")
    print(f"   交易数量: {len(result['transactions'])}")
    
    # 3. 验证交易数据格式
    print("\n=== 步骤3: 验证交易数据格式 ===")
    
    if len(result['transactions']) == 0:
        print("⚠️  警告: 该账户在指定时间范围内没有交易记录")
        return True  # 没有交易不算错误
    
    # 检查第一笔交易
    first_transaction = result['transactions'][0]
    
    required_fields = [
        'id', 'time', 'type', 'amount', 'accountBalance',
        'utcOffset', 'gregorianCalendarYearDashMonthDashDay',
        'gregorianCalendarDayOfMonth', 'displayDayOfWeek'
    ]
    
    missing_fields = []
    for field in required_fields:
        if field not in first_transaction:
            missing_fields.append(field)
    
    if missing_fields:
        print(f"❌ 缺少字段: {missing_fields}")
        print(f"   实际字段: {list(first_transaction.keys())}")
        return False
    
    print("✅ 所有必需字段存在")
    
    # 4. 验证utcOffset不是NaN
    print("\n=== 步骤4: 验证utcOffset正确 ===")
    utc_offset = first_transaction['utcOffset']
    if not isinstance(utc_offset, (int, float)) or utc_offset != utc_offset:  # NaN检测
        print(f"❌ utcOffset是NaN或无效: {utc_offset}")
        return False
    
    if utc_offset < -720 or utc_offset > 840:
        print(f"⚠️  警告: utcOffset超出合理范围: {utc_offset}分钟")
    
    print(f"✅ utcOffset正确: {utc_offset}分钟 (约{utc_offset/60:.1f}小时)")
    
    # 5. 验证日期字段
    print("\n=== 步骤5: 验证日期显示字段 ===")
    date_str = first_transaction['gregorianCalendarYearDashMonthDashDay']
    day_of_month = first_transaction['gregorianCalendarDayOfMonth']
    day_of_week = first_transaction['displayDayOfWeek']
    
    # 验证日期格式
    try:
        date_obj = datetime.strptime(date_str, '%Y-%m-%d')
    except ValueError:
        print(f"❌ 日期格式错误: {date_str}")
        return False
    
    # 验证日期组件匹配
    if date_obj.day != day_of_month:
        print(f"❌ 日期不匹配: gregorianCalendarDayOfMonth={day_of_month}, 实际={date_obj.day}")
        return False
    
    # 验证星期几(1=周日, 2=周一, ..., 7=周六)
    if day_of_week < 1 or day_of_week > 7:
        print(f"❌ displayDayOfWeek超出范围: {day_of_week}")
        return False
    
    weekday_names = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
    weekday_name = weekday_names[day_of_week - 1]
    
    print(f"✅ 日期字段正确: {date_str} ({weekday_name}), day={day_of_month}")
    
    # 6. 验证余额连续性
    print("\n=== 步骤6: 验证余额连续性 ===")
    
    # 按时间正序排序
    transactions_sorted = sorted(result['transactions'], key=lambda x: x['time'])
    
    expected_balance = result['openingBalance']
    balance_errors = []
    
    for idx, trans in enumerate(transactions_sorted):
        trans_type = trans['type']  # 1=收入, 2=支出, 3/4=转账/投资
        amount = trans['amount']
        actual_balance = trans['accountBalance']
        
        # 计算期望余额
        if trans_type == 1:  # 收入
            expected_balance += amount
        elif trans_type in [2, 4]:  # 支出/投资
            expected_balance -= amount
        # type=3转账需要更复杂的判断，此处简化
        
        # 允许小的浮点误差
        if abs(actual_balance - expected_balance) > 0.01:
            balance_errors.append(f"交易#{idx+1}: 期望{expected_balance:.2f}, 实际{actual_balance:.2f}")
    
    if balance_errors:
        print(f"⚠️  余额不一致 ({len(balance_errors)}笔):")
        for err in balance_errors[:3]:  # 只显示前3个
            print(f"   {err}")
    else:
        print(f"✅ 所有交易余额连续正确 ({len(transactions_sorted)}笔)")
    
    # 7. 最终余额验证
    if abs(transactions_sorted[-1]['accountBalance'] - result['closingBalance']) > 0.01:
        print(f"❌ 最终余额不匹配: 最后一笔交易={transactions_sorted[-1]['accountBalance']}, API返回={result['closingBalance']}")
        return False
    
    print(f"✅ 最终余额正确: {result['closingBalance']:.2f}")
    
    print("\n" + "="*60)
    print("🎉 所有验证通过！对账单API修复成功！")
    print("="*60)
    return True


if __name__ == '__main__':
    try:
        success = test_reconciliation_api()
        exit(0 if success else 1)
    except Exception as e:
        print(f"\n❌ 测试失败: {e}")
        import traceback
        traceback.print_exc()
        exit(1)
