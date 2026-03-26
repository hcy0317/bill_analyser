#!/usr/bin/env python3
"""
快速检查解析器匹配情况
"""

from src.parsers.wechat import WeChatParser
from src.parsers.alipay import AlipayParser
from src.parsers.icbc import ICBCParser
from src.parsers.cmbc import CMBCParser
from src.parsers.abc import ABCParser
from src.parsers.ccb import CCBParser
from pathlib import Path

def main():
    # 初始化所有解析器
    parsers = {
        'WeChat': WeChatParser(),
        'Alipay': AlipayParser(),
        'ICBC': ICBCParser(),
        'CMBC': CMBCParser(),
        'ABC': ABCParser(),
        'CCB': CCBParser()
    }
    
    # 获取所有账单文件
    bills_dir = Path('src/bills')
    files = list(bills_dir.glob('*.csv')) + list(bills_dir.glob('*.xlsx')) + list(bills_dir.glob('*.xls'))
    
    # 统计信息
    matched = {name: [] for name in parsers.keys()}
    unmatched = []
    conflicts = []
    
    # 检查每个文件
    for file in files:
        matched_parsers = []
        for name, parser in parsers.items():
            if parser.can_parse(str(file)):
                matched_parsers.append(name)
        
        if len(matched_parsers) == 0:
            unmatched.append(file.name)
        elif len(matched_parsers) == 1:
            matched[matched_parsers[0]].append(file.name)
        else:
            conflicts.append((file.name, matched_parsers))
    
    # 输出统计
    print("\n" + "="*60)
    print("解析器匹配统计")
    print("="*60)
    
    total_matched = 0
    for name, files_list in matched.items():
        count = len(files_list)
        total_matched += count
        print(f"{name}: {count} 个文件")
    
    print(f"\n无匹配: {len(unmatched)} 个文件")
    print(f"冲突: {len(conflicts)} 个文件")
    print(f"总计: {len(files)} 个文件")
    
    # 检查结果
    if len(unmatched) == 0 and len(conflicts) == 0:
        print(f"\n✅ 所有 {len(files)} 个文件都已正确匹配！")
    else:
        print(f"\n❌ 存在问题：")
        if unmatched:
            print(f"  - 未匹配文件: {unmatched}")
        if conflicts:
            print(f"  - 冲突文件: {conflicts}")

if __name__ == '__main__':
    main()
