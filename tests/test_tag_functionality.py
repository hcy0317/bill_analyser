"""标签功能集成测试脚本"""

import requests
import sys
from datetime import datetime

BASE_URL = "http://127.0.0.1:5000/api"

def log_test(test_name, success, message=""):
    """打印测试结果"""
    status = "✅ PASS" if success else "❌ FAIL"
    print(f"{status} | {test_name}: {message}")
    return success

def test_create_tag():
    """测试创建标签"""
    print("\n[测试1] 创建标签")
    result = (False, None)
    try:
        response = requests.post(
            f"{BASE_URL}/v1/transaction/tags/add.json",
            json={
                "name": "测试标签_集成测试",
                "color": "#FF5733",
                "icon": "tag"
            }
        )
        data = response.json()
        if response.status_code == 200 and data.get('success'):
            tag_id = data['result']['id']
            result = (log_test("创建标签", True, f"标签ID: {tag_id}"), tag_id)
        else:
            result = (log_test("创建标签", False, f"响应: {data}"), None)
    except Exception as exc:
        result = (log_test("创建标签", False, str(exc)), None)
    return result

def test_modify_tag(tag_id):
    """测试修改标签"""
    print("\n[测试2] 修改标签")
    try:
        response = requests.post(
            f"{BASE_URL}/v1/transaction/tags/modify.json",
            json={
                "id": str(tag_id),
                "name": "测试标签_已修改",
                "color": "#00FF00"
            }
        )
        data = response.json()
        return log_test("修改标签", response.status_code == 200 and data.get('success'), 
                       f"响应: {data}")
    except Exception as e:
        return log_test("修改标签", False, str(e))

def test_hide_tag(tag_id):
    """测试隐藏标签"""
    print("\n[测试3] 隐藏标签")
    try:
        response = requests.post(
            f"{BASE_URL}/v1/transaction/tags/hide.json",
            json={"id": str(tag_id)}
        )
        data = response.json()
        return log_test("隐藏标签", response.status_code == 200 and data.get('success'),
                       f"响应: {data}")
    except Exception as e:
        return log_test("隐藏标签", False, str(e))

def test_show_tag(tag_id):
    """测试显示标签"""
    print("\n[测试4] 显示标签")
    try:
        response = requests.post(
            f"{BASE_URL}/v1/transaction/tags/show.json",
            json={"id": str(tag_id)}
        )
        data = response.json()
        return log_test("显示标签", response.status_code == 200 and data.get('success'),
                       f"响应: {data}")
    except Exception as e:
        return log_test("显示标签", False, str(e))

def test_list_tags():
    """测试获取标签列表"""
    print("\n[测试5] 获取标签列表")
    try:
        response = requests.post(f"{BASE_URL}/v1/transaction/tags/list.json", json={})
        data = response.json()
        if response.status_code == 200 and data.get('success'):
            tags = data.get('result', [])
            return log_test("获取标签列表", True, f"共{len(tags)}个标签")
        return log_test("获取标签列表", False, f"响应: {data}")
    except Exception as e:
        return log_test("获取标签列表", False, str(e))

def test_create_bill_with_tags(tag_id):
    """测试创建带标签的账单（验证adapter转换）"""
    print("\n[测试6] 创建带标签的账单")
    try:
        # 先获取账户列表
        acc_response = requests.get(f"{BASE_URL}/accounts/")
        if acc_response.status_code != 200:
            return log_test("创建带标签账单", False, "无法获取账户列表")
        
        accounts = acc_response.json().get('data', [])
        if not accounts:
            return log_test("创建带标签账单", False, "数据库中无账户")
        
        account_id = str(accounts[0]['id'])
        
        # 创建账单（使用v1格式）
        bill_data = {
            "type": 3,  # 支出
            "categoryId": "1",
            "time": int(datetime.now().timestamp() * 1000),
            "amount": 10050,  # 100.50元（前端格式：分）
            "relatedAccountId": account_id,
            "tagIds": [str(tag_id), "0"],  # 测试字符串数组和过滤"0"
            "comment": "测试标签关联_集成测试"
        }
        
        response = requests.post(f"{BASE_URL}/bills", json=bill_data)
        data = response.json()
        
        if response.status_code == 200 and data.get('success'):
            bill_id = data['result']['id']
            return log_test("创建带标签账单", True, 
                          f"账单ID: {bill_id}, 标签ID: {tag_id}"), bill_id
        return log_test("创建带标签账单", False, f"响应: {data}"), None
    except Exception as e:
        return log_test("创建带标签账单", False, str(e)), None

def test_query_bill_tags(bill_id):
    """测试查询账单标签（验证adapter批量查询）"""
    print("\n[测试7] 查询账单标签")
    try:
        response = requests.get(f"{BASE_URL}/v1/transactions/list.json?maxTime=9999999999999")
        data = response.json()
        
        if response.status_code == 200 and data.get('success'):
            transactions = data.get('result', {}).get('items', [])
            test_bill = next((t for t in transactions if t['id'] == str(bill_id)), None)
            
            if test_bill and 'tags' in test_bill:
                tags = test_bill['tags']
                return log_test("查询账单标签", True,
                              f"账单{bill_id}关联{len(tags)}个标签")
            return log_test("查询账单标签", False, "未找到测试账单或标签数据")
        return log_test("查询账单标签", False, f"响应: {data}")
    except Exception as ex:
        return log_test("查询账单标签", False, str(ex))

def test_delete_bill(bill_id):
    """测试删除测试账单"""
    print("\n[测试8] 删除测试账单")
    try:
        response = requests.delete(f"{BASE_URL}/bills/{bill_id}")
        data = response.json()
        return log_test("删除测试账单", response.status_code == 200 and data.get('success'),
                       f"账单ID: {bill_id}")
    except Exception as e:
        return log_test("删除测试账单", False, str(e))

def test_delete_tag(tag_id):
    """测试删除标签"""
    print("\n[测试9] 删除标签")
    try:
        response = requests.post(
            f"{BASE_URL}/v1/transaction/tags/delete.json",
            json={"id": str(tag_id)}
        )
        data = response.json()
        return log_test("删除标签", response.status_code == 200 and data.get('success'),
                       f"标签ID: {tag_id}")
    except Exception as e:
        return log_test("删除标签", False, str(e))

def main():
    """主测试流程"""
    print("=" * 60)
    print("交易标签系统集成测试")
    print("=" * 60)
    
    results = []
    tag_id = None
    bill_id = None
    
    # 测试1: 创建标签
    success, tag_id = test_create_tag()
    results.append(success)
    if not tag_id:
        print("\n❌ 创建标签失败，中止测试")
        return 1
    
    # 测试2: 修改标签
    results.append(test_modify_tag(tag_id))
    
    # 测试3: 隐藏标签
    results.append(test_hide_tag(tag_id))
    
    # 测试4: 显示标签
    results.append(test_show_tag(tag_id))
    
    # 测试5: 获取标签列表
    results.append(test_list_tags())
    
    # 测试6: 创建带标签的账单
    success, bill_id = test_create_bill_with_tags(tag_id)
    results.append(success)
    
    # 测试7: 查询账单标签
    if bill_id:
        results.append(test_query_bill_tags(bill_id))
    
    # 测试8: 清理测试数据 - 删除账单
    if bill_id:
        results.append(test_delete_bill(bill_id))
    
    # 测试9: 清理测试数据 - 删除标签
    results.append(test_delete_tag(tag_id))
    
    # 汇总结果
    print("\n" + "=" * 60)
    print(f"测试完成: {sum(results)}/{len(results)} 通过")
    print("=" * 60)
    
    return 0 if all(results) else 1

if __name__ == "__main__":
    sys.exit(main())
