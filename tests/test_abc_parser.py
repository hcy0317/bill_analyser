"""
测试农业银行解析器的识别能力
"""
from src.parsers.abc import ABCParser
from src.parsers.icbc import ICBCParser
from src.parsers.ccb import CCBParser
import pandas as pd
import tempfile
import os

# 创建模拟的农业银行Excel数据
def create_mock_abc_file():
    # 模拟农业银行账单格式（使用特殊编码字符）
    data = {
        0: ['中国农业银⾏', '', '', '', ''],
        1: ['账⼾活期交易明细清单', '', '', '', ''],
        2: ['账⼾名称', '何辰延', '账⼾', '6228480012345678', ''],
        3: ['交易⽇期', '交易⾦额', '本次余额', '对⼿信息', '交易附⾔'],
        4: ['2024-01-01', '100.00', '1000.00', '张三', '转账'],
        5: ['2024-01-02', '-50.00', '950.00', '李四', '消费']
    }
    
    df = pd.DataFrame.from_dict(data, orient='index')
    
    # 创建临时文件
    temp_file = tempfile.NamedTemporaryFile(mode='w', suffix='.xlsx', delete=False, encoding='utf-8')
    temp_path = temp_file.name
    temp_file.close()
    
    df.to_excel(temp_path, index=False, header=False)
    return temp_path

# 测试
print("创建模拟农业银行文件...")
test_file = create_mock_abc_file()
print(f"文件路径: {test_file}")

abc = ABCParser()
icbc = ICBCParser()
ccb = CCBParser()

print("\n测试解析器识别:")
print(f"农业银行解析器: {abc.can_parse(test_file)}")
print(f"工商银行解析器: {icbc.can_parse(test_file)}")
print(f"建设银行解析器: {ccb.can_parse(test_file)}")

# 清理临时文件
os.unlink(test_file)

print("\n✅ 测试完成：农业银行解析器应该识别为True，其他应该为False")
