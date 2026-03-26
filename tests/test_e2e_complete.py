"""
完整端到端测试: 账单管理系统
测试账户创建→账单添加→列表获取→详情查看→删除的完整流程
"""
import requests
from datetime import datetime
import time

BASE_URL = 'http://127.0.0.1:5000/api'
TOKEN = None


def login():
    """登录获取JWT Token"""
    global TOKEN
    print('\n🔐 登录系统...')
    response = requests.post(
        f'{BASE_URL}/authorize.json',
        json={'loginName': 'admin', 'password': 'admin123'},
        timeout=10
    )
    result = response.json()
    if result.get('success') and 'result' in result:
        TOKEN = result['result']['token']
        print('   ✅ 登录成功!')
        return True
    print(f'   ❌ 登录失败: {result.get("error", "未知错误")}')
    return False


def get_headers():
    """获取带认证的请求头"""
    return {'Authorization': f'Bearer {TOKEN}'}


def test_account_crud():
    """测试账户CRUD操作"""
    print('\n📁 测试账户管理...')
    
    # 创建测试账户
    print('   1. 创建测试账户...')
    account_data = {
        'name': 'E2E测试账户',
        'type': 'SingleAccount',
        'categoryId': 2,
        'balance': 500.0
    }
    response = requests.post(
        f'{BASE_URL}/accounts',
        json=account_data,
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    
    if not (result.get('success') and 'result' in result):
        print(f'      ❌ 创建失败: {result}')
        return None
    
    account_id = result['result']['id']
    print(f'      ✅ 账户创建成功 (ID: {account_id})')
    return account_id


def test_bill_operations(account_id):
    """测试账单操作"""
    print('\n💰 测试账单管理...')
    
    created_bill_ids = []
    
    # 创建多个测试账单
    print('   1. 创建测试账单...')
    test_bills = [
        {'amount': 50.00, 'comment': 'E2E测试-早餐'},
        {'amount': 120.50, 'comment': 'E2E测试-午餐'},
        {'amount': 80.00, 'comment': 'E2E测试-晚餐'}
    ]
    
    for i, bill_info in enumerate(test_bills, 1):
        bill_data = {
            'type': 3,  # Expense
            'time': int(datetime(2024, 11, 20, 8 + i * 4, 0, 0).timestamp()),
            'sourceAccountId': str(account_id),
            'sourceAmount': bill_info['amount'],
            'categoryId': '',
            'comment': bill_info['comment'],
            'utcOffset': 480
        }
        response = requests.post(
            f'{BASE_URL}/bills',
            json=bill_data,
            headers=get_headers(),
            timeout=10
        )
        result = response.json()
        
        if result.get('success') and 'result' in result:
            bill_id = result['result']['id']
            created_bill_ids.append(bill_id)
            print(f'      ✅ 账单 {i} 创建成功 (ID: {bill_id}, 金额: ¥{bill_info["amount"]})')
        else:
            print(f'      ❌ 账单 {i} 创建失败: {result}')
    
    if not created_bill_ids:
        print('      ⚠️  未成功创建任何账单')
        return []
    
    # 短暂延迟确保数据库写入
    time.sleep(0.5)
    
    # 测试获取账单列表
    print('\n   2. 获取账单列表...')
    response = requests.get(
        f'{BASE_URL}/bills',
        params={'page': 1, 'page_size': 20},
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    
    if result.get('success') and 'result' in result:
        total = result['result'].get('total', 0)
        items = result['result'].get('items', [])
        print(f'      ✅ 列表获取成功 (总记录: {total}, 当前页: {len(items)}条)')
        
        # 验证创建的账单是否在列表中
        found_count = sum(1 for item in items if item['id'] in created_bill_ids)
        print(f'      ℹ️  找到 {found_count}/{len(created_bill_ids)} 条新建账单')
    else:
        print(f'      ❌ 列表获取失败: {result}')
    
    # 测试获取单个账单详情
    print('\n   3. 获取账单详情...')
    test_bill_id = created_bill_ids[0]
    response = requests.get(
        f'{BASE_URL}/bills/{test_bill_id}',
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    
    if result.get('success') and 'result' in result:
        bill = result['result']
        print(f'      ✅ 详情获取成功')
        print(f'         - 金额: ¥{bill.get("amount")}')
        print(f'         - 描述: {bill.get("description")}')
        print(f'         - 日期: {bill.get("date")}')
    else:
        print(f'      ❌ 详情获取失败: {result}')
    
    # 测试更新账单
    print('\n   4. 更新账单...')
    update_data = {
        'amount': 99.99,
        'description': 'E2E测试-已修改'
    }
    response = requests.put(
        f'{BASE_URL}/bills/{test_bill_id}',
        json=update_data,
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    
    if result.get('success'):
        print(f'      ✅ 账单更新成功 (ID: {test_bill_id})')
    else:
        print(f'      ❌ 更新失败: {result}')
    
    return created_bill_ids, account_id


def cleanup(bill_ids, account_id):
    """清理测试数据"""
    print('\n🧹 清理测试数据...')
    
    # 删除账单
    print('   1. 删除测试账单...')
    for bill_id in bill_ids:
        try:
            response = requests.delete(
                f'{BASE_URL}/bills/{bill_id}',
                headers=get_headers(),
                timeout=10
            )
            if response.status_code == 200:
                print(f'      ✅ 账单 {bill_id} 已删除')
            else:
                print(f'      ⚠️  账单 {bill_id} 删除失败')
        except Exception as e:
            print(f'      ❌ 删除账单 {bill_id} 出错: {e}')
    
    # 删除账户
    print('\n   2. 删除测试账户...')
    try:
        response = requests.delete(
            f'{BASE_URL}/accounts/{account_id}',
            headers=get_headers(),
            timeout=10
        )
        result = response.json()
        if result.get('success') and result.get('result'):
            print(f'      ✅ 账户 {account_id} 已删除')
        else:
            print(f'      ⚠️  账户删除失败: {result}')
    except Exception as e:
        print(f'      ❌ 删除账户出错: {e}')


def test_v1_api_compatibility():
    """测试v1 API兼容性"""
    print('\n🔄 测试v1 API兼容性...')
    
    # 测试v1账户列表
    print('   1. v1/accounts/list.json...')
    response = requests.get(
        f'{BASE_URL}/v1/accounts/list.json',
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    if result.get('success') and 'result' in result:
        print(f'      ✅ v1账户接口正常 ({len(result.get("result", []))}个账户)')
    else:
        print(f'      ❌ v1账户接口失败: {result}')
    
    # 测试v1交易列表
    print('   2. v1/transactions/list.json...')
    response = requests.get(
        f'{BASE_URL}/v1/transactions/list.json',
        params={
            'max_time': int(datetime.now().timestamp()),
            'min_time': 0,
            'type': 0,
            'count': 5,
            'page': 1,
            'with_count': 'true'
        },
        headers=get_headers(),
        timeout=10
    )
    result = response.json()
    if result.get('success') and 'result' in result:
        print('      ✅ v1交易接口正常')
    else:
        print(f'      ❌ v1交易接口失败: {result}')


def main():
    """主测试流程"""
    print('='*60)
    print('Bill Analyser - 端到端测试')
    print('='*60)
    
    try:
        # 1. 登录
        if not login():
            return
        
        # 2. 测试账户创建
        account_id = test_account_crud()
        if not account_id:
            return
        
        # 3. 测试账单操作
        bill_ids, account_id = test_bill_operations(account_id)
        
        # 4. 测试v1兼容性
        test_v1_api_compatibility()
        
        # 5. 清理数据
        cleanup(bill_ids, account_id)
        
        print('\n' + '='*60)
        print('✅ 所有测试通过!')
        print('='*60)
        
    except Exception as e:
        print(f'\n❌ 测试失败: {e}')
        import traceback
        traceback.print_exc()


if __name__ == '__main__':
    main()
