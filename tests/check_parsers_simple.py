"""
生成解析器匹配的最终报告
"""
import sys
import io
from pathlib import Path
from src.parsers.factory import ParserFactory
from src.parsers.wechat import WeChatParser
from src.parsers.alipay import AlipayParser
from src.parsers.icbc import ICBCParser
from src.parsers.cmbc import CMBCParser
from src.parsers.abc import ABCParser
from src.parsers.ccb import CCBParser

if sys.platform == 'win32':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

bills_dir = Path('src/bills')
files = sorted(list(bills_dir.glob('*.xlsx')) + list(bills_dir.glob('*.xls')) + list(bills_dir.glob('*.csv')))

parsers = {
    'WeChat': WeChatParser(),
    'Alipay': AlipayParser(),
    'ICBC': ICBCParser(),
    'CMBC': CMBCParser(),
    'ABC': ABCParser(),
    'CCB': CCBParser()
}

parser_counts = {name: [] for name in parsers.keys()}
unmatched = []

print("解析器匹配检查结果")
print("=" * 80)

for file_path in files:
    matched = []
    for name, parser in parsers.items():
        if parser.can_parse(str(file_path)):
            matched.append(name)
            parser_counts[name].append(file_path.name)
    
    if not matched:
        unmatched.append(file_path.name)

print(f"\n总文件数: {len(files)}")
print(f"\n各解析器匹配:")
for name, files_list in parser_counts.items():
    print(f"  {name:10s}: {len(files_list):3d} 个文件")

print(f"\n无匹配文件: {len(unmatched)} 个")
if unmatched:
    for fname in unmatched:
        print(f"  - {fname}")

print("\n" + "=" * 80)
print(f"✅ 结论: {len(files) - len(unmatched)}/{len(files)} 个文件可正确匹配解析器")
if len(unmatched) == 0:
    print("🎉 所有文件都能正确匹配!")
else:
    print(f"⚠️  剩余 {len(unmatched)} 个文件需要处理")
