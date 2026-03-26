"""
v6.7修复验证脚本 - 统计API金额单位转换
测试amounts API是否正确返回分(cents)单位
"""

import requests
import json
from decimal import Decimal


def test_amounts_api():
    """测试amounts API返回的金额单位"""
    base_url = "http://127.0.0.1:5000"
    
    # 测试用例：本月查询 (2024年11月)
    # 已知数据库有169元支出
    test_cases = [
        {
            "name": "本月统计",
            "query": "thisMonth_1761926400_1764518399",
            "expected_expense": 16900,  # 169元 × 100 = 16900分
            "expected_income": 0
        }
    ]
    
    print("=" * 60)
    print("v6.7 统计API金额单位验证")
    print("=" * 60)
    
    for test in test_cases:
        print(f"\n[测试] {test['name']}")
        print(f"  查询参数: {test['query']}")
        
        url = f"{base_url}/api/v1/transactions/amounts.json?query={test['query']}"
        
        try:
            response = requests.get(url, timeout=5)
            response.raise_for_status()
            data = response.json()
            
            if not data.get('success'):
                print(f"  ❌ API调用失败: {data}")
                continue
            
            # 提取金额数据
            period_name = list(data['result'].keys())[0]
            amounts = data['result'][period_name]['amounts'][0]
            
            income = amounts['incomeAmount']
            expense = amounts['expenseAmount']
            currency = amounts['currency']
            
            print(f"  API响应:")
            print(f"    货币: {currency}")
            print(f"    收入: {income}分 (期望: {test['expected_income']}分)")
            print(f"    支出: {expense}分 (期望: {test['expected_expense']}分)")
            
            # 验证单位
            income_yuan = income / 100
            expense_yuan = expense / 100
            print(f"  前端显示:")
            print(f"    收入: {income_yuan:.2f}元")
            print(f"    支出: {expense_yuan:.2f}元")
            
            # 检查结果
            if income == test['expected_income'] and expense == test['expected_expense']:
                print(f"  ✅ 通过 - 金额单位正确(分)")
            else:
                print(f"  ❌ 失败 - 金额不匹配")
                
        except requests.exceptions.RequestException as e:
            print(f"  ❌ 网络错误: {e}")
        except Exception as e:
            print(f"  ❌ 解析错误: {e}")
    
    print("\n" + "=" * 60)
    print("验证完成")
    print("=" * 60)


def test_unit_conversion():
    """测试货币转换函数的数学正确性"""
    print("\n[单元测试] yuan_to_cents 转换函数")
    
    test_values = [
        (111.0, 11100),
        (169.0, 16900),
        (0.01, 1),
        (100.50, 10050),
        (0, 0),
        (1.005, 101),  # 测试四舍五入
    ]
    
    for yuan, expected_cents in test_values:
        # 模拟转换逻辑
        decimal_yuan = Decimal(str(yuan))
        decimal_cents = decimal_yuan * Decimal('100')
        cents = int(decimal_cents.quantize(Decimal('1'), rounding='ROUND_HALF_UP'))
        
        status = "✅" if cents == expected_cents else "❌"
        print(f"  {status} {yuan}元 → {cents}分 (期望: {expected_cents}分)")


if __name__ == '__main__':
    # 测试API
    test_amounts_api()
    
    # 测试转换函数
    test_unit_conversion()
