#!/usr/bin/env python3
"""
调试实际导入过程 - 检查 source_account_id 在各阶段的值
"""
import sys
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path
from src.parsers.factory import ParserFactory
from src.utils.validator import BillValidator
from src.core.smart_dedup import SmartDeduplicationEngine

def check_source_account_ids():
    """检查解析后的 source_account_id 值"""
    
    # 查找账单文件
    bills_dir = Path(project_root) / 'bills'
    if not bills_dir.exists():
        print("❌ bills 目录不存在")
        return
    
    # 列出可用的账单文件
    csv_files = list(bills_dir.glob('*.csv'))
    print(f"\n找到 {len(csv_files)} 个账单文件:")
    for i, f in enumerate(csv_files[:5]):
        print(f"  [{i}] {f.name}")
    if len(csv_files) > 5:
        print(f"  ... 还有 {len(csv_files) - 5} 个文件")
    
    # 解析一个微信账单和一个支付宝账单
    factory = ParserFactory()
    validator = BillValidator()
    dedup_engine = SmartDeduplicationEngine()
    
    all_bills = []
    
    # 选择测试文件
    test_files = []
    for f in csv_files:
        name = f.name.lower()
        if '微信' in name or 'wechat' in name:
            test_files.append(f)
        elif '支付宝' in name or 'alipay' in name:
            test_files.append(f)
        if len(test_files) >= 2:
            break
    
    if len(test_files) < 2:
        print("⚠️ 未找到足够的测试文件（需要微信和支付宝各一个）")
        test_files = csv_files[:2]
    
    print(f"\n测试文件: {[f.name for f in test_files]}")
    
    for file_path in test_files:
        print(f"\n{'='*60}")
        print(f"解析文件: {file_path.name}")
        print('='*60)
        
        # 1. 解析
        parser_info = factory.detect_parser(str(file_path))
        if not parser_info:
            print(f"  ❌ 无法检测解析器")
            continue
            
        print(f"  检测到解析器: {parser_info['id']} ({parser_info['name']})")
        
        bills = factory.parse(str(file_path))
        print(f"  解析得到 {len(bills)} 条账单")
        
        if not bills:
            continue
        
        # 检查 source_account_id
        source_ids = set(b.get('source_account_id') for b in bills)
        print(f"  source_account_id 值: {source_ids}")
        
        # 打印前3条账单的关键字段
        for i, bill in enumerate(bills[:3]):
            print(f"\n  账单 [{i}]:")
            print(f"    date: {bill.get('date')}")
            print(f"    amount: {bill.get('amount')}")
            print(f"    type: {bill.get('type')}")
            print(f"    source_account_id: {bill.get('source_account_id')}")
            print(f"    counterparty: {bill.get('counterparty', '')[:30]}")
        
        all_bills.extend(bills)
    
    if not all_bills:
        print("\n❌ 没有解析到任何账单")
        return
    
    print(f"\n{'='*60}")
    print(f"合并后共 {len(all_bills)} 条账单")
    print('='*60)
    
    # 2. 验证
    valid_bills, invalid_bills = validator.validate_bills(all_bills)
    print(f"\n验证结果: 有效 {len(valid_bills)}, 无效 {len(invalid_bills)}")
    
    # 检查验证后的 source_account_id
    source_ids_after_validation = set(b.get('source_account_id') for b in valid_bills)
    print(f"验证后 source_account_id 值: {source_ids_after_validation}")
    
    # 3. 去重（手动检查关键步骤）
    print(f"\n{'='*60}")
    print("开始去重前检查")
    print('='*60)
    
    # 分析平台和银行账单的分布
    platform_bills = []
    bank_bills = []
    other_bills = []
    
    PLATFORM_SOURCES = {'wechat', 'alipay', '微信', '支付宝'}
    BANK_SOURCES = {'icbc', 'cmbc', 'abc', 'ccb', '工商银行', '民生银行', '农业银行', '建设银行'}
    
    for bill in valid_bills:
        src = str(bill.get('source_account_id', '')).lower()
        if src in PLATFORM_SOURCES:
            platform_bills.append(bill)
        elif src in BANK_SOURCES:
            bank_bills.append(bill)
        else:
            other_bills.append(bill)
    
    print(f"\n账单来源分布:")
    print(f"  平台账单 (wechat/alipay): {len(platform_bills)}")
    print(f"  银行账单 (icbc/cmbc/abc/ccb): {len(bank_bills)}")
    print(f"  其他来源: {len(other_bills)}")
    
    if other_bills:
        other_sources = set(b.get('source_account_id') for b in other_bills)
        print(f"  其他来源的具体值: {other_sources}")
    
    # 4. 执行去重
    print(f"\n{'='*60}")
    print("执行智能去重")
    print('='*60)
    
    result = dedup_engine.process(valid_bills)
    
    print(f"\n去重结果:")
    print(f"  原始数量: {result.original_count}")
    print(f"  移除数量: {result.removed_count}")
    print(f"  保留数量: {len(result.kept_bills)}")
    print(f"  转账配对: {len(result.transfer_pairs)} 对")
    print(f"  分账组: {len(result.split_groups)} 组")
    print(f"  重复组: {len(result.duplicate_groups)} 组")
    
    # 检查诊断信息
    if hasattr(result, 'diagnostics'):
        print(f"\n诊断信息:")
        for key, value in result.diagnostics.items():
            print(f"  {key}: {value}")

if __name__ == '__main__':
    check_source_account_ids()
