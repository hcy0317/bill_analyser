"""检查所有文件，找出可能是农业银行但被误判的文件"""
import pandas as pd
from pathlib import Path

bills_dir = Path('src/bills')
files = sorted(list(bills_dir.glob('*.xlsx')) + list(bills_dir.glob('*.xls')) + list(bills_dir.glob('*.csv')))

print("检查所有文件中的农业银行特征...\n")

abc_indicators = [
    "农业银行", "农业银⾏", "账⼾活期", "对⼿信息", "⽇志号", "交易附⾔"
]

for file_path in files:
    try:
        # 读取前10行
        if file_path.suffix == '.csv':
            df = pd.read_csv(file_path, encoding='gbk', nrows=10, header=None, on_bad_lines='skip')
        else:
            # 先尝试HTML
            try:
                tables = pd.read_html(str(file_path), encoding='utf-8')
                if tables:
                    df = tables[0].head(10)
                else:
                    df = pd.read_excel(file_path, header=None, nrows=10)
            except:
                df = pd.read_excel(file_path, header=None, nrows=10)
        
        # 检查内容
        content = ""
        for i in range(len(df)):
            for j in range(len(df.columns)):
                cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                content += cell_value + " "
        
        # 检查是否包含农业银行特征
        matches = [ind for ind in abc_indicators if ind in content]
        if matches:
            print(f"✓ {file_path.name}")
            print(f"  匹配特征: {matches}")
            print()
    except Exception as e:
        # 忽略无法读取的文件
        pass

print("检查完成")
