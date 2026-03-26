"""Quick API test script"""
import requests

url = "http://127.0.0.1:5000/api/bills/reconciliation_statements"
params = {
    'account_id': 3,
    'start_time': 1759248000,
    'end_time': 1764518400
}

print("Requesting:", url)
print("Params:", params)

response = requests.get(url, params=params, timeout=10)
print("\nStatus:", response.status_code)

data = response.json()
result = data.get('result', {})

print("\n=== 汇总信息 ===")
print(f"期初余额: {result.get('openingBalance')} 分")
print(f"期末余额: {result.get('closingBalance')} 分")
print(f"总流入: {result.get('totalInflows')} 分")
print(f"总流出: {result.get('totalOutflows')} 分")
print(f"净流入: {result.get('netFlow')} 分")
print(f"交易数量: {result.get('itemCount')}")

print("\n=== 交易详情 ===")
transactions = result.get('transactions', [])
for txn in transactions:
    print(f"\n交易#{txn['id']}:")
    print(f"  日期: {txn.get('gregorianCalendarYearDashMonthDashDay')}")
    print(f"  星期: {txn.get('displayDayOfWeek')} (1=周日, 2=周一, ..., 7=周六)")
    print(f"  金额: {txn.get('amount')} 分")
    print(f"  分类: {txn.get('categoryName')}")
    print(f"  账户余额: {txn.get('accountClosingBalance')} 分")
