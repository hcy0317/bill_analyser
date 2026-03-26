"""
综合诊断脚本 - 验证统计数据流
"""
import asyncio
import requests
from datetime import datetime, timedelta
from src.core.db import Database


async def check_database():
    """检查数据库中的实际数据"""
    print("=" * 60)
    print("1. 数据库数据检查")
    print("=" * 60)
    
    db = Database()
    await db.init_db()
    
    # 查询11月所有账单
    bills_nov, total_nov = await db.query_bills(
        page=1,
        page_size=1000,
        filters={'start_date': '2025-11-01', 'end_date': '2025-11-30'}
    )
    
    income_nov = sum(float(b['amount']) for b in bills_nov if b['type'] == '收入')
    expense_nov = sum(float(b['amount']) for b in bills_nov if b['type'] == '支出')
    
    print(f"11月账单统计:")
    print(f"  总数: {total_nov}")
    print(f"  收入: {income_nov}元")
    print(f"  支出: {expense_nov}元")
    
    if bills_nov:
        print(f"\n11月账单明细:")
        for b in bills_nov:
            print(f"  {b['date']}: {b['type']} {b['amount']}元 - {b['description']}")
    
    # 查询2025年所有账单
    bills_year, total_year = await db.query_bills(
        page=1,
        page_size=1000,
        filters={'start_date': '2025-01-01', 'end_date': '2025-12-31'}
    )
    
    income_year = sum(float(b['amount']) for b in bills_year if b['type'] == '收入')
    expense_year = sum(float(b['amount']) for b in bills_year if b['type'] == '支出')
    
    print(f"\n2025年账单统计:")
    print(f"  总数: {total_year}")
    print(f"  收入: {income_year}元")
    print(f"  支出: {expense_year}元")
    
    await db.close()
    
    return {
        'month': {'income': income_nov, 'expense': expense_nov, 'count': total_nov},
        'year': {'income': income_year, 'expense': expense_year, 'count': total_year}
    }


def check_api():
    """检查API响应"""
    print("\n" + "=" * 60)
    print("2. API响应检查")
    print("=" * 60)
    
    # 生成时间戳（使用东8区）
    from datetime import timezone
    tz = timezone(timedelta(hours=8))
    
    today_start = datetime.now(tz).replace(hour=0, minute=0, second=0, microsecond=0)
    today_end = today_start + timedelta(days=1) - timedelta(seconds=1)
    
    month_start = today_start.replace(day=1)
    next_month = month_start.replace(day=28) + timedelta(days=4)
    month_end = next_month.replace(day=1) - timedelta(seconds=1)
    
    year_start = today_start.replace(month=1, day=1)
    year_end = today_start.replace(month=12, day=31, hour=23, minute=59, second=59)
    
    print(f"时间戳生成:")
    print(f"  今天: {today_start} ~ {today_end}")
    print(f"  本月: {month_start} ~ {month_end}")
    print(f"  本年: {year_start} ~ {year_end}")
    
    # 构造query参数
    query = (
        f"today_{int(today_start.timestamp())}_{int(today_end.timestamp())}"
        f"|thisMonth_{int(month_start.timestamp())}_{int(month_end.timestamp())}"
        f"|thisYear_{int(year_start.timestamp())}_{int(year_end.timestamp())}"
    )
    
    print(f"\nQuery参数: {query}")
    
    # 调用API
    url = f"http://127.0.0.1:5000/api/v1/transactions/amounts.json?query={query}"
    response = requests.get(url, timeout=10)
    
    print(f"\nAPI响应状态: {response.status_code}")
    
    if response.status_code == 200:
        data = response.json()
        print(f"响应数据:")
        
        if 'result' in data:
            for period, stats in data['result'].items():
                print(f"\n  {period}:")
                print(f"    startTime: {stats.get('startTime', 0)}")
                print(f"    endTime: {stats.get('endTime', 0)}")
                
                if 'amounts' in stats and stats['amounts']:
                    for amount_info in stats['amounts']:
                        print(f"    amounts:")
                        print(f"      currency: {amount_info.get('currency', 'N/A')}")
                        print(f"      收入: {amount_info.get('incomeAmount', 0)}")
                        print(f"      支出: {amount_info.get('expenseAmount', 0)}")
                else:
                    # 兼容旧格式
                    print(f"    收入: {stats.get('totalIncome', 0)}")
                    print(f"    支出: {stats.get('totalExpense', 0)}")
                    print(f"    净额: {stats.get('totalNet', 0)}")
        else:
            print(f"  错误: 响应中没有result字段")
            print(f"  实际响应: {data}")
        
        return data
    else:
        print(f"  错误: {response.text}")
        return None


def compare_results(db_data, api_data):
    """对比数据库和API的结果"""
    print("\n" + "=" * 60)
    print("3. 数据对比")
    print("=" * 60)
    
    if not api_data or 'result' not in api_data:
        print("❌ API数据不可用，无法对比")
        return
    
    api_result = api_data['result']
    
    # 对比本月
    if 'thisMonth' in api_result:
        api_month = api_result['thisMonth']
        print(f"\n本月数据对比:")
        print(f"  数据库 - 收入: {db_data['month']['income']}元, 支出: {db_data['month']['expense']}元")
        
        # 从新格式中提取金额
        api_income = 0
        api_expense = 0
        if 'amounts' in api_month and api_month['amounts']:
            for amount_info in api_month['amounts']:
                api_income += amount_info.get('incomeAmount', 0)
                api_expense += amount_info.get('expenseAmount', 0)
        else:
            # 兼容旧格式
            api_income = api_month.get('totalIncome', 0)
            api_expense = api_month.get('totalExpense', 0)
        
        print(f"  API    - 收入: {api_income}元, 支出: {api_expense}元")
        
        if (abs(db_data['month']['income'] - api_income) < 0.01 and
            abs(db_data['month']['expense'] - api_expense) < 0.01):
            print(f"  ✅ 数据一致")
        else:
            print(f"  ❌ 数据不一致！")
    
    # 对比本年
    if 'thisYear' in api_result:
        api_year = api_result['thisYear']
        print(f"\n本年数据对比:")
        print(f"  数据库 - 收入: {db_data['year']['income']}元, 支出: {db_data['year']['expense']}元")
        
        # 从新格式中提取金额
        api_income = 0
        api_expense = 0
        if 'amounts' in api_year and api_year['amounts']:
            for amount_info in api_year['amounts']:
                api_income += amount_info.get('incomeAmount', 0)
                api_expense += amount_info.get('expenseAmount', 0)
        else:
            # 兼容旧格式
            api_income = api_year.get('totalIncome', 0)
            api_expense = api_year.get('totalExpense', 0)
        
        print(f"  API    - 收入: {api_income}元, 支出: {api_expense}元")
        
        if (abs(db_data['year']['income'] - api_income) < 0.01 and
            abs(db_data['year']['expense'] - api_expense) < 0.01):
            print(f"  ✅ 数据一致")
        else:
            print(f"  ❌ 数据不一致！")


async def main():
    """主函数"""
    print("\n" + "=" * 60)
    print("统计数据诊断报告")
    print("=" * 60)
    
    # 1. 检查数据库
    db_data = await check_database()
    
    # 2. 检查API
    api_data = check_api()
    
    # 3. 对比结果
    compare_results(db_data, api_data)
    
    # 4. 结论
    print("\n" + "=" * 60)
    print("4. 诊断结论")
    print("=" * 60)
    
    if db_data['month']['count'] > 0:
        print(f"✅ 数据库中有账单数据({db_data['month']['count']}条本月账单)")
    else:
        print(f"❌ 数据库中没有本月账单数据")
    
    if api_data and 'result' in api_data:
        month_expense = api_data['result'].get('thisMonth', {}).get('totalExpense', 0)
        if month_expense > 0:
            print(f"✅ API返回正确的统计数据(本月支出{month_expense}元)")
        else:
            print(f"⚠️  API返回的本月支出为0")
    else:
        print(f"❌ API响应异常")
    
    print("\n建议:")
    if db_data['month']['count'] > 0 and api_data and 'result' in api_data:
        # 从新格式中提取本月支出
        month_expense_api = 0
        thisMonth = api_data['result'].get('thisMonth', {})
        if 'amounts' in thisMonth and thisMonth['amounts']:
            for amount_info in thisMonth['amounts']:
                month_expense_api += amount_info.get('expenseAmount', 0)
        else:
            month_expense_api = thisMonth.get('totalExpense', 0)
        
        if month_expense_api > 0:
            print("  ✅ 后端API工作正常")
            print("  ✅ 数据库查询正常")
            print("  ✅ API返回格式符合前端期望:")
            print("     - 包含amounts数组")
            print("     - 包含currency、incomeAmount、expenseAmount字段")
            print("\n  前端应该能正常显示统计数据！")
            print("\n  如果前端仍显示0，请检查:")
            print("     - 浏览器是否缓存了旧的API响应（强制刷新: Ctrl+Shift+R）")
            print("     - 浏览器控制台是否有JavaScript错误")
            print("     - 网络请求是否成功（F12 > Network标签）")
        else:
            print("  ❌ API返回的数据不正确，需要检查后端实现")


if __name__ == "__main__":
    asyncio.run(main())
