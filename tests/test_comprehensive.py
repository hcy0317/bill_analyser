# -*- coding: utf-8 -*-
"""
综合功能测试

验证：
1. 创建投资账单 - destination_account_id自动设置
2. 创建支出账单 - 自动分类
3. 创建收入账单 - 自动分类
4. 账户余额同步
5. 删除账单后余额回滚
6. 防止幽灵账单
"""
import requests
import json
import time

BASE_URL = "http://127.0.0.1:5000"

def get_account_balance(account_id):
    """查询账户余额"""
    try:
        response = requests.get(f"{BASE_URL}/api/accounts/{account_id}")
        result = response.json()
        if result.get('success'):
            return result['result'].get('balance', 0) / 100  # 转换为元
        return None
    except:
        return None

def test_comprehensive():
    """综合测试"""
    print("\n" + "="*80)
    print("Bill Analyser - 综合功能测试")
    print("="*80)
    
    # 检查后端
    try:
        requests.get(f"{BASE_URL}/api/bills?page=1&page_size=1", timeout=2)
        print("[OK] 后端服务器运行中\n")
    except:
        print("[ERROR] 后端服务器未启动")
        return
    
    test_results = {
        '投资自动目标账户': False,
        '投资自动分类': False,
        '支出自动分类': False,
        '收入自动分类': False,
        '防幽灵账单': False,
        '余额同步-支出': False,
        '余额回滚-删除': False
    }
    
    # 测试1: 投资账单
    print("="*80)
    print("测试1: 创建投资账单")
    print("="*80)
    
    initial_balance = get_account_balance(88)  # 现金账户
    print(f"现金账户初始余额: {initial_balance}元")
    
    investment_data = {
        "type": 5,
        "categoryId": "",
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "88",
        "destinationAccountId": "0",
        "sourceAmount": 100000,  # 1000元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": "测试投资1000元",
        "clientSessionId": "test-inv-001"
    }
    
    response = requests.post(f"{BASE_URL}/api/bills", json=investment_data)
    result = response.json()
    
    if result.get('success'):
        bill = result['result']
        print(f"[OK] 投资账单创建成功: ID={bill['id']}")
        print(f"  源账户: {bill['sourceAccountId']}")
        print(f"  目标账户: {bill['destinationAccountId']}")
        print(f"  分类: {bill['categoryName']}")
        
        if bill['destinationAccountId'] != '0':
            test_results['投资自动目标账户'] = True
            print("[PASS] 投资自动目标账户: PASS")
        else:
            print("[FAIL] 投资自动目标账户: FAIL")
        
        if bill['categoryName']:
            test_results['投资自动分类'] = True
            print("[PASS] 投资自动分类: PASS")
        else:
            print("[FAIL] 投资自动分类: FAIL")
        
        investment_id = bill['id']
    else:
        print(f"[FAIL] 创建失败: {result.get('error')}")
        investment_id = None
    
    time.sleep(0.5)
    
    # 验证余额
    new_balance = get_account_balance(88)
    if new_balance and initial_balance:
        expected_balance = initial_balance + 1000
        print(f"\n现金账户新余额: {new_balance}元 (预期: {expected_balance}元)")
        if abs(new_balance - expected_balance) < 0.01:
            print("[PASS] 余额同步-投资: PASS")
        else:
            print(f"[FAIL] 余额同步-投资: FAIL (差异: {new_balance - expected_balance}元)")
    
    # 测试2: 支出账单
    print("\n" + "="*80)
    print("测试2: 创建支出账单")
    print("="*80)
    
    expense_data = {
        "type": 3,
        "categoryId": "",
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "114",  # 花呗
        "destinationAccountId": "0",
        "sourceAmount": 25000,  # 250元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": "测试支出250元",
        "clientSessionId": "test-exp-001"
    }
    
    initial_balance_huabei = get_account_balance(114)
    print(f"花呗初始余额: {initial_balance_huabei}元")
    
    response = requests.post(f"{BASE_URL}/api/bills", json=expense_data)
    result = response.json()
    
    if result.get('success'):
        bill = result['result']
        print(f"[OK] 支出账单创建成功: ID={bill['id']}")
        print(f"  分类: {bill['categoryName']}")
        
        if bill['categoryName']:
            test_results['支出自动分类'] = True
            print("[PASS] 支出自动分类: PASS")
        else:
            print("[FAIL] 支出自动分类: FAIL")
        
        expense_id = bill['id']
    else:
        print(f"[FAIL] 创建失败: {result.get('error')}")
        expense_id = None
    
    time.sleep(0.5)
    
    # 验证余额
    new_balance_huabei = get_account_balance(114)
    if new_balance_huabei and initial_balance_huabei:
        expected_balance = initial_balance_huabei - 250
        print(f"\n花呗新余额: {new_balance_huabei}元 (预期: {expected_balance}元)")
        if abs(new_balance_huabei - expected_balance) < 0.01:
            test_results['余额同步-支出'] = True
            print("[PASS] 余额同步-支出: PASS")
        else:
            print(f"[FAIL] 余额同步-支出: FAIL (差异: {new_balance_huabei - expected_balance}元)")
    
    # 测试3: 收入账单
    print("\n" + "="*80)
    print("测试3: 创建收入账单")
    print("="*80)
    
    income_data = {
        "type": 2,
        "categoryId": "",
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "88",
        "destinationAccountId": "0",
        "sourceAmount": 500000,  # 5000元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": "测试收入5000元",
        "clientSessionId": "test-inc-001"
    }
    
    response = requests.post(f"{BASE_URL}/api/bills", json=income_data)
    result = response.json()
    
    if result.get('success'):
        bill = result['result']
        print(f"[OK] 收入账单创建成功: ID={bill['id']}")
        print(f"  分类: {bill['categoryName']}")
        
        if bill['categoryName']:
            test_results['收入自动分类'] = True
            print("[PASS] 收入自动分类: PASS")
        else:
            print("[FAIL] 收入自动分类: FAIL")
        
        income_id = bill['id']
    else:
        print(f"[FAIL] 创建失败: {result.get('error')}")
        income_id = None
    
    time.sleep(0.5)
    
    # 测试4: 防幽灵账单
    print("\n" + "="*80)
    print("测试4: 防幽灵账单（空账户ID）")
    print("="*80)
    
    ghost_data = {
        "type": 3,
        "categoryId": "",
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "",  # 空账户
        "destinationAccountId": "0",
        "sourceAmount": 10000,
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": "测试空账户",
        "clientSessionId": "test-ghost-001"
    }
    
    response = requests.post(f"{BASE_URL}/api/bills", json=ghost_data)
    result = response.json()
    
    if result.get('success'):
        bill = result['result']
        print(f"[OK] 账单创建成功: ID={bill['id']}")
        print(f"  账户ID: {bill['sourceAccountId']}")
        print(f"  账户名: {bill['accountName']}")
        
        if bill['sourceAccountId'] != '0':
            test_results['防幽灵账单'] = True
            print("[PASS] 防幽灵账单: PASS (使用默认账户)")
        else:
            print("[FAIL] 防幽灵账单: FAIL (创建了幽灵账单)")
        
        ghost_id = bill['id']
    else:
        print(f"[FAIL] 创建失败: {result.get('error')}")
        ghost_id = None
    
    time.sleep(0.5)
    
    # 测试5: 删除账单余额回滚
    if expense_id:
        print("\n" + "="*80)
        print("测试5: 删除账单后余额回滚")
        print("="*80)
        
        before_delete = get_account_balance(114)
        print(f"删除前花呗余额: {before_delete}元")
        
        response = requests.delete(f"{BASE_URL}/api/bills/{expense_id}")
        result = response.json()
        
        if result.get('success'):
            print(f"[OK] 账单删除成功: ID={expense_id}")
            time.sleep(0.5)
            
            after_delete = get_account_balance(114)
            print(f"删除后花呗余额: {after_delete}元")
            
            if after_delete and before_delete:
                expected = before_delete + 250  # 支出删除应该增加余额
                if abs(after_delete - expected) < 0.01:
                    test_results['余额回滚-删除'] = True
                    print("[PASS] 余额回滚-删除: PASS")
                else:
                    print(f"[FAIL] 余额回滚-删除: FAIL (预期: {expected}元, 实际: {after_delete}元)")
        else:
            print(f"[FAIL] 删除失败: {result.get('error')}")
    
    # 清理测试数据
    print("\n" + "="*80)
    print("清理测试数据")
    print("="*80)
    
    test_ids = [investment_id, income_id, ghost_id]
    for bill_id in test_ids:
        if bill_id:
            try:
                requests.delete(f"{BASE_URL}/api/bills/{bill_id}")
                print(f"[OK] 已删除测试账单: ID={bill_id}")
            except:
                pass
    
    # 输出测试结果
    print("\n" + "="*80)
    print("测试结果汇总")
    print("="*80)
    
    passed = sum(1 for v in test_results.values() if v)
    total = len(test_results)
    
    for test_name, result in test_results.items():
        status = "[PASS]" if result else "[FAIL]"
        print(f"{status} {test_name}")
    
    print("\n" + "="*80)
    print(f"通过率: {passed}/{total} ({passed*100//total}%)")
    print("="*80)
    
    if passed == total:
        print("\n[SUCCESS] 所有测试通过！")
    else:
        print(f"\n[WARNING] {total - passed}个测试失败")

if __name__ == "__main__":
    test_comprehensive()
