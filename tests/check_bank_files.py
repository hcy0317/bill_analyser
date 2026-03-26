#!/usr/bin/env python3
"""
调试银行账单解析 - 检查银行账单文件是否被正确识别
"""
import sys
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path
from src.parsers.factory import ParserFactory

def check_bank_files():
    """检查银行账单文件"""
    
    bills_dir = Path(project_root) / 'bills'
    factory = ParserFactory()
    
    # 列出所有非微信非支付宝的文件
    all_files = list(bills_dir.iterdir())
    
    bank_like_files = []
    for f in all_files:
        if f.is_file():
            name = f.name.lower()
            if '微信' not in name and 'wechat' not in name and \
               '支付宝' not in name and 'alipay' not in name:
                bank_like_files.append(f)
    
    print(f"\n找到 {len(bank_like_files)} 个可能的银行账单文件:")
    for f in bank_like_files:
        print(f"  - {f.name}")
    
    print("\n" + "="*60)
    print("检测每个文件的解析器类型")
    print("="*60)
    
    for file_path in bank_like_files:
        print(f"\n文件: {file_path.name}")
        
        try:
            parser_info = factory.detect_parser(str(file_path))
            if parser_info:
                print(f"  ✅ 检测到解析器: {parser_info['id']} ({parser_info['name']})")
                
                # 尝试解析
                bills = factory.parse(str(file_path))
                if bills:
                    print(f"  ✅ 成功解析 {len(bills)} 条账单")
                    # 检查 source_account_id
                    sources = set(b.get('source_account_id') for b in bills)
                    print(f"  source_account_id: {sources}")
                    
                    # 打印第一条账单
                    bill = bills[0]
                    print(f"  第一条账单:")
                    print(f"    date: {bill.get('date')}")
                    print(f"    amount: {bill.get('amount')}")
                    print(f"    type: {bill.get('type')}")
                else:
                    print(f"  ⚠️ 解析返回空列表")
            else:
                print(f"  ❌ 无法检测解析器")
        except Exception as e:
            print(f"  ❌ 错误: {e}")

if __name__ == '__main__':
    check_bank_files()
