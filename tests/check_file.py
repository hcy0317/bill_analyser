import pandas as pd

# 检查工商银行历史明细文件
wb = pd.read_excel('src/bills/工商银行历史明细（申请单号：25082520010041279887）.xlsx', header=None)
print('文件: 工商银行历史明细.xlsx')
print('Shape:', wb.shape)
for i in range(min(10, len(wb))):
    values = [str(wb.iloc[i, j]) if not pd.isna(wb.iloc[i, j]) else '' for j in range(min(10, len(wb.columns)))]
    print(f'Row {i}: {values}')

# 检查是否有特殊编码的农业银行字符
content = ""
for i in range(min(10, len(wb))):
    for j in range(len(wb.columns)):
        cell_value = str(wb.iloc[i, j]) if not pd.isna(wb.iloc[i, j]) else ""
        content += cell_value + " "

print("\n特征检查:")
print(f"包含'农业银行': {'农业银行' in content}")
print(f"包含'农业银⾏': {'农业银⾏' in content}")
print(f"包含'工商银行': {'工商银行' in content}")
print(f"包含'账⼾活期': {'账⼾活期' in content}")
print(f"包含'储种': {'储种' in content}")
