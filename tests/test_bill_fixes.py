"""
测试投资账单创建和分类修复

功能：
1. 测试投资账单自动设置destination_account_id
2. 测试自动分类功能
3. 测试防止幽灵账单
"""
import requests
import json
import time

BASE_URL = "http://127.0.0.1:5000"

def test_create_investment():
    """测试创建投资账单"""
    print("\n" + "="*80)
    print("测试1: 创建投资账单（验证自动目标账户）")
    print("="*80)
    
    # 模拟前端发送的投资数据
    investment_data = {
        "type": 5,  # 投资
        "categoryId": "",  # 空分类，测试自动分类
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "88",  # 现金账户
        "destinationAccountId": "0",  # 目标账户为0，应该自动设置
        "sourceAmount": 50000,  # 500元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "pictureIds": [],
        "comment": "测试投资-自动目标账户",
        "clientSessionId": "test-investment-001"
    }
    
    try:
        response = requests.post(
            f"{BASE_URL}/api/bills",
            json=investment_data,
            headers={"Content-Type": "application/json"}
        )
        
        print(f"\n状态码: {response.status_code}")
        result = response.json()
        
        if result.get('success'):
            bill = result.get('result', {})
            print(f"✓ 账单创建成功！")
            print(f"  账单ID: {bill.get('id')}")
            print(f"  源账户ID: {bill.get('sourceAccountId')}")
            print(f"  目标账户ID: {bill.get('destinationAccountId')}")
            print(f"  分类ID: {bill.get('categoryId')}")
            print(f"  分类名称: {bill.get('categoryName')}")
            
            # 验证结果
            if bill.get('destinationAccountId') != '0':
                print(f"\n✓ 目标账户自动设置成功: {bill.get('destinationAccountId')}")
            else:
                print(f"\n✗ 目标账户仍为0，自动设置失败！")
            
            if bill.get('categoryName'):
                print(f"✓ 自动分类成功: {bill.get('categoryName')}")
            else:
                print(f"✗ 分类为空，自动分类失败！")
            
            return bill.get('id')
        else:
            print(f"✗ 创建失败: {result.get('error')}")
            return None
            
    except Exception as e:
        print(f"✗ 请求失败: {e}")
        return None


def test_create_expense():
    """测试创建支出账单（验证自动分类）"""
    print("\n" + "="*80)
    print("测试2: 创建支出账单（验证自动分类）")
    print("="*80)
    
    expense_data = {
        "type": 3,  # 支出
        "categoryId": "",  # 空分类
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "114",  # 花呗
        "destinationAccountId": "0",
        "sourceAmount": 8800,  # 88元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "pictureIds": [],
        "comment": "测试支出-自动分类",
        "clientSessionId": "test-expense-001"
    }
    
    try:
        response = requests.post(
            f"{BASE_URL}/api/bills",
            json=expense_data,
            headers={"Content-Type": "application/json"}
        )
        
        result = response.json()
        
        if result.get('success'):
            bill = result.get('result', {})
            print(f"✓ 账单创建成功！")
            print(f"  账单ID: {bill.get('id')}")
            print(f"  分类名称: {bill.get('categoryName')}")
            print(f"  子分类: {bill.get('subCategoryName')}")
            
            if bill.get('categoryName'):
                print(f"\n✓ 自动分类成功: {bill.get('categoryName')}")
            else:
                print(f"\n✗ 分类为空，自动分类失败！")
            
            return bill.get('id')
        else:
            print(f"✗ 创建失败: {result.get('error')}")
            return None
            
    except Exception as e:
        print(f"✗ 请求失败: {e}")
        return None


def test_create_without_account():
    """测试创建账单时source_account_id为空（验证防幽灵账单）"""
    print("\n" + "="*80)
    print("测试3: 创建账单不指定账户（验证防幽灵账单）")
    print("="*80)
    
    ghost_data = {
        "type": 3,  # 支出
        "categoryId": "",
        "time": int(time.time()),
        "utcOffset": 480,
        "sourceAccountId": "",  # 空账户ID
        "destinationAccountId": "0",
        "sourceAmount": 12300,  # 123元
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "pictureIds": [],
        "comment": "测试-空账户ID",
        "clientSessionId": "test-ghost-001"
    }
    
    try:
        response = requests.post(
            f"{BASE_URL}/api/bills",
            json=ghost_data,
            headers={"Content-Type": "application/json"}
        )
        
        result = response.json()
        
        if result.get('success'):
            bill = result.get('result', {})
            print(f"✓ 账单创建成功（应该使用默认账户）")
            print(f"  账单ID: {bill.get('id')}")
            print(f"  账户ID: {bill.get('sourceAccountId')}")
            print(f"  账户名称: {bill.get('accountName')}")
            
            if bill.get('sourceAccountId') != '0':
                print(f"\n✓ 防幽灵账单成功: 使用默认账户ID={bill.get('sourceAccountId')}")
            else:
                print(f"\n✗ 创建了幽灵账单，source_account_id=0！")
            
            return bill.get('id')
        else:
            print(f"✗ 创建失败: {result.get('error')}")
            return None
            
    except Exception as e:
        print(f"✗ 请求失败: {e}")
        return None


def verify_bill(bill_id):
    """查询并验证账单"""
    if not bill_id:
        return
    
    print(f"\n验证账单详情: ID={bill_id}")
    
    try:
        response = requests.get(
            f"{BASE_URL}/api/v1/transactions/get.json?id={bill_id}",
            headers={"X-Timezone-Offset": "480"}
        )
        
        result = response.json()
        
        if result.get('success'):
            bill = result.get('result', {})
            print(f"  类型: {bill.get('type')}")
            print(f"  源账户: {bill.get('sourceAccountId')} - {bill.get('accountName')}")
            print(f"  目标账户: {bill.get('destinationAccountId')}")
            print(f"  分类: {bill.get('categoryId')} - {bill.get('categoryName')}")
        else:
            print(f"  查询失败: {result.get('error')}")
            
    except Exception as e:
        print(f"  查询失败: {e}")


if __name__ == "__main__":
    print("\n" + "="*80)
    print("Bill Analyser - 修复功能测试")
    print("="*80)
    print("测试目标：")
    print("1. 投资账单自动设置destination_account_id")
    print("2. 所有账单自动分类（main_category不为None）")
    print("3. 防止创建幽灵账单（source_account_id不为0）")
    print("="*80)
    
    # 等待后端启动
    print("\n检查后端服务器...")
    try:
        requests.get(f"{BASE_URL}/api/bills?page=1&page_size=1", timeout=2)
        print("[OK] 后端服务器运行中")
    except:
        print("[ERROR] 后端服务器未启动，请先运行: .\\启动后端.ps1")
        exit(1)
    
    # 执行测试
    investment_id = test_create_investment()
    time.sleep(0.5)
    
    expense_id = test_create_expense()
    time.sleep(0.5)
    
    ghost_id = test_create_without_account()
    time.sleep(0.5)
    
    # 验证账单
    print("\n" + "="*80)
    print("验证创建的账单")
    print("="*80)
    
    if investment_id:
        verify_bill(investment_id)
    
    if expense_id:
        verify_bill(expense_id)
    
    if ghost_id:
        verify_bill(ghost_id)
    
    print("\n" + "="*80)
    print("测试完成！")
    print("="*80)
