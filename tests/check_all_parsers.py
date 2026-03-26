from src.parsers.wechat import WeChatParser
from src.parsers.alipay import AlipayParser
from src.parsers.icbc import ICBCParser
from src.parsers.cmbc import CMBCParser
from src.parsers.abc import ABCParser
from src.parsers.ccb import CCBParser
from pathlib import Path

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

print("检查每个文件的解析器匹配情况\n")

# 按解析器分组
for name, parser in parsers.items():
    print(f"\n{name} 解析器匹配的文件:")
    matched = []
    for file_path in files:
        if parser.can_parse(str(file_path)):
            matched.append(file_path.name)
    
    if matched:
        for fname in matched:
            print(f"  - {fname}")
    else:
        print("  (无)")
