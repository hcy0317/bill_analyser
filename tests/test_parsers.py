"""测试民生银行和工商银行解析器"""
import sys
sys.path.insert(0, '.')

from src.parsers.cmbc import CMBCParser
from src.parsers.icbc import ICBCParser
import os

def main():
    print('=== 测试所有民生银行文件 ===')
    cmbc = CMBCParser()
    cmbc_files = [f for f in os.listdir('bills') if '账号6226192003866332' in f]
    cmbc_total = 0
    for cmbc_file in cmbc_files:
        file_path = f'bills/{cmbc_file}'
        if cmbc.can_parse(file_path):
            bills = cmbc.parse(file_path)
            cmbc_total += len(bills)
            print(f'  {cmbc_file}: {len(bills)} 条')
        else:
            print(f'  {cmbc_file}: can_parse=False')

    print(f'民生银行总计: {cmbc_total} 条')

    print('\n=== 测试工商银行文件 ===')
    icbc = ICBCParser()
    icbc_file = 'bills/工商银行历史明细（申请单号：25082520010041279887）.xlsx'
    if icbc.can_parse(icbc_file):
        bills = icbc.parse(icbc_file)
        print(f'工商银行总计: {len(bills)} 条')
    else:
        print('工商银行: can_parse=False')

    print('\n=== 解析器隔离性检查 ===')
    # 确保CMBC不会误识别ICBC文件
    result1 = cmbc.can_parse(icbc_file)
    print(f'CMBC解析器 识别 ICBC文件: {result1} (应为False)')
    
    # 确保ICBC不会误识别CMBC文件
    result2 = icbc.can_parse(f'bills/{cmbc_files[0]}')
    print(f'ICBC解析器 识别 CMBC文件: {result2} (应为False)')
    
    if not result1 and not result2:
        print('\n✅ 解析器隔离性正常！')
    else:
        print('\n❌ 解析器隔离性有问题！')

if __name__ == '__main__':
    main()
