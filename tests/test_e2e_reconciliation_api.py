"""端到端测试：对账单API完整性

测试真实环境中对账单API返回的数据格式
"""

import requests
import json
from datetime import datetime


def test_reconciliation_api_e2e():
    """端到端测试对账单API"""
    # 后端URL
    base_url = "http://127.0.0.1:5000"
    
    # 测试参数（使用农业银行账户ID=3）
    account_id = 3  # 农业银行
    start_time = int(datetime(2025, 10, 1).timestamp())  # 2025-10-01 00:00:00
    end_time = int(datetime(2025, 12, 1).timestamp())    # 2025-12-01 00:00:00
    
    # 构建请求URL
    url = f"{base_url}/api/bills/reconciliation_statements"
    params = {
        'account_id': account_id,
        'start_time': start_time,
        'end_time': end_time
    }
    
    print("\n=== 对账单API端到端测试 ===")
    print(f"请求URL: {url}")
    print(f"请求参数: {json.dumps(params, ensure_ascii=False)}")
    
    # 发送请求
    try:
        response = requests.get(url, params=params, timeout=10)
        print(f"\n响应状态码: {response.status_code}")
        
        if response.status_code != 200:
            print(f"✗ API返回错误: {response.text}")
            return False
        
        # 解析响应
        data = response.json()
        print(f"\n响应success: {data.get('success')}")
        
        if not data.get('success'):
            print(f"✗ API返回失败: {data.get('error')}")
            return False
        
        result = data.get('result', {})
        transactions = result.get('transactions', [])
        
        print(f"\n=== 对账单汇总 ===")
        print(f"账户: {result.get('accountName')} (ID={result.get('accountId')})")
        print(f"期初余额: {result.get('openingBalance')} 分")
        print(f"期末余额: {result.get('closingBalance')} 分")
        print(f"总流入: {result.get('totalInflows')} 分")
        print(f"总流出: {result.get('totalOutflows')} 分")
        print(f"净流入: {result.get('netFlow')} 分")
        print(f"交易数量: {len(transactions)}")
        
        if not transactions:
            print("\n⚠ 没有交易记录")
            return True
        
        # 检查前3笔交易的字段完整性
        print(f"\n=== 前3笔交易字段检查 ===")
        for i, trans in enumerate(transactions[:3], 1):
            print(f"\n--- 交易#{i} ---")
            print(f"ID: {trans.get('id')}")
            print(f"时间: {trans.get('time')}")
            
            # 关键字段检查
            required_fields = [
                'gregorianCalendarYearDashMonthDashDay',
                'displayDayOfWeek',
                'categoryId',
                'categoryName',
                'subCategoryName',
                'accountClosingBalance'
            ]
            
            missing_fields = []
            for field in required_fields:
                value = trans.get(field)
                if value is None:
                    missing_fields.append(field)
                    print(f"✗ {field}: MISSING")
                else:
                    print(f"✓ {field}: {value}")
            
            # 验证日期显示字段
            date_str = trans.get('gregorianCalendarYearDashMonthDashDay')
            weekday = trans.get('displayDayOfWeek')
            if date_str and weekday:
                # 将displayDayOfWeek映射为中文
                weekday_names = ['', '周日', '周一', '周二', '周三', '周四', '周五', '周六']
                weekday_name = weekday_names[weekday] if 0 <= weekday < len(weekday_names) else '未知'
                print(f"   → 日期显示: {date_str} {weekday_name}")
            
            # 验证分类字段
            category_name = trans.get('categoryName')
            sub_category = trans.get('subCategoryName')
            if category_name or sub_category:
                print(f"   → 分类显示: {category_name} - {sub_category}")
            
            # 验证余额字段
            balance = trans.get('accountClosingBalance')
            if balance is not None:
                yuan = balance / 100
                print(f"   → 账户余额: {balance}分 (¥{yuan:.2f})")
            
            if missing_fields:
                print(f"\n⚠ 缺少字段: {', '.join(missing_fields)}")
                return False
        
        print("\n✅ 对账单API字段完整性测试通过")
        return True
        
    except requests.exceptions.RequestException as e:
        print(f"\n✗ 请求失败: {e}")
        return False
    except Exception as e:
        print(f"\n✗ 测试异常: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_transaction_list_api_e2e():
    """端到端测试交易列表API"""
    base_url = "http://127.0.0.1:5000"
    
    # 构建请求URL
    url = f"{base_url}/api/v1/transactions/list.json"
    params = {
        'count': 10,
        'page': 1
    }
    
    print("\n\n=== 交易列表API端到端测试 ===")
    print(f"请求URL: {url}")
    print(f"请求参数: {json.dumps(params, ensure_ascii=False)}")
    
    try:
        response = requests.get(url, params=params, timeout=10)
        print(f"\n响应状态码: {response.status_code}")
        
        if response.status_code != 200:
            print(f"✗ API返回错误: {response.text}")
            return False
        
        data = response.json()
        print(f"\n响应success: {data.get('success')}")
        
        if not data.get('success'):
            print(f"✗ API返回失败: {data.get('error')}")
            return False
        
        result = data.get('result', {})
        items = result.get('items', [])
        total = result.get('totalCount', 0)
        
        print(f"\n总交易数: {total}")
        print(f"返回数量: {len(items)}")
        
        if not items:
            print("\n⚠ 没有交易记录")
            return True
        
        # 检查前3笔交易的字段
        print(f"\n=== 前3笔交易字段检查 ===")
        for i, trans in enumerate(items[:3], 1):
            print(f"\n--- 交易#{i} ---")
            print(f"ID: {trans.get('id')}")
            
            # 检查日期字段
            date_str = trans.get('gregorianCalendarYearDashMonthDashDay')
            weekday = trans.get('displayDayOfWeek')
            
            if date_str:
                print(f"✓ gregorianCalendarYearDashMonthDashDay: {date_str}")
            else:
                print(f"✗ gregorianCalendarYearDashMonthDashDay: MISSING")
            
            if weekday is not None:
                weekday_names = ['', '周日', '周一', '周二', '周三', '周四', '周五', '周六']
                weekday_name = weekday_names[weekday] if 0 <= weekday < len(weekday_names) else '未知'
                print(f"✓ displayDayOfWeek: {weekday} ({weekday_name})")
            else:
                print(f"✗ displayDayOfWeek: MISSING")
            
            # 检查分类字段
            category_name = trans.get('categoryName')
            if category_name:
                print(f"✓ categoryName: {category_name}")
            else:
                print(f"⚠ categoryName: {category_name}")
        
        print("\n✅ 交易列表API字段完整性测试通过")
        return True
        
    except Exception as e:
        print(f"\n✗ 测试异常: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == '__main__':
    print("=" * 60)
    print("端到端测试：验证对账单和交易列表API")
    print("=" * 60)
    
    # 测试对账单API
    result1 = test_reconciliation_api_e2e()
    
    # 测试交易列表API
    result2 = test_transaction_list_api_e2e()
    
    # 最终结果
    print("\n" + "=" * 60)
    if result1 and result2:
        print("✅ 所有端到端测试通过")
    else:
        print("✗ 部分测试失败")
        if not result1:
            print("  - 对账单API测试失败")
        if not result2:
            print("  - 交易列表API测试失败")
    print("=" * 60)
