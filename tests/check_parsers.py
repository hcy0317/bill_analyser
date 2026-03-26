"""
检查各解析器是否正确匹配对应的账单文件
"""
import sys
from pathlib import Path
from src.parsers.factory import ParserFactory
from src.parsers.wechat import WeChatParser
from src.parsers.alipay import AlipayParser
from src.parsers.icbc import ICBCParser
from src.parsers.cmbc import CMBCParser

# 设置stdout编码为utf-8
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

def check_parser_matching():
    """检查解析器匹配情况"""
    bills_dir = Path('src/bills')
    if not bills_dir.exists():
        print(f"❌ 目录不存在: {bills_dir}")
        return
    
    # 获取所有账单文件
    files = []
    for pattern in ['*.xlsx', '*.xls', '*.csv']:
        files.extend(bills_dir.glob(pattern))
    
    if not files:
        print("❌ 未找到任何账单文件")
        return
    
    print(f"找到 {len(files)} 个账单文件\n")
    print("="*80)
    
    # 初始化所有解析器
    parsers = {
        '微信': WeChatParser(),
        '支付宝': AlipayParser(),
        '工商银行': ICBCParser(),
        '民生银行': CMBCParser()
    }
    
    factory = ParserFactory()
    
    # 统计
    conflict_files = []  # 多个解析器匹配的文件
    no_parser_files = []  # 没有解析器匹配的文件
    parser_counts = {name: 0 for name in parsers.keys()}
    
    for file_path in sorted(files):
        file_name = file_path.name
        matched_parsers = []
        
        # 检查每个解析器
        for parser_name, parser in parsers.items():
            if parser.can_parse(str(file_path)):
                matched_parsers.append(parser_name)
                parser_counts[parser_name] += 1
        
        # 检查factory选择的解析器
        factory_parser = factory.get_parser(str(file_path))
        factory_name = factory_parser.__class__.__name__.replace('Parser', '') if factory_parser else None
        
        # 判断状态
        status = ""
        if len(matched_parsers) == 0:
            status = "❌ 无匹配"
            no_parser_files.append(file_name)
        elif len(matched_parsers) > 1:
            status = f"⚠️  多匹配 ({', '.join(matched_parsers)})"
            conflict_files.append((file_name, matched_parsers))
        else:
            status = f"✅ {matched_parsers[0]}"
        
        # 显示结果
        print(f"{status:25s} | {file_name}")
        
        # 如果有冲突，显示factory选择
        if len(matched_parsers) > 1 and factory_name:
            factory_display = {
                'WeChat': '微信',
                'Alipay': '支付宝',
                'ICBC': '工商银行',
                'CMBC': '民生银行'
            }.get(factory_name, factory_name)
            print(f"{'':25s}   Factory选择: {factory_display}")
    
    # 显示统计
    print("\n" + "="*80)
    print("📊 统计结果:")
    print(f"{'='*80}")
    print(f"总文件数: {len(files)}")
    print(f"\n各解析器匹配数:")
    for parser_name, count in parser_counts.items():
        print(f"  {parser_name:10s}: {count:3d} 个文件")
    
    print(f"\n问题文件:")
    print(f"  多解析器匹配: {len(conflict_files)} 个")
    print(f"  无解析器匹配: {len(no_parser_files)} 个")
    
    # 详细显示冲突
    if conflict_files:
        print(f"\n⚠️  多解析器匹配的文件 ({len(conflict_files)}个):")
        for file_name, matched in conflict_files:
            print(f"  - {file_name}")
            print(f"    匹配: {', '.join(matched)}")
    
    # 详细显示无匹配
    if no_parser_files:
        print(f"\n❌ 无解析器匹配的文件 ({len(no_parser_files)}个):")
        for file_name in no_parser_files:
            print(f"  - {file_name}")
    
    print("="*80)

if __name__ == '__main__':
    check_parser_matching()
