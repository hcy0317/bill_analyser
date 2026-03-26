from src.parsers.abc import ABCParser
from src.parsers.icbc import ICBCParser
from src.parsers.ccb import CCBParser
import os

files = [f for f in os.listdir('src/bills') if '农业' in f or 'abc' in f.lower()]
print('农业银行文件:', files)

abc = ABCParser()
icbc = ICBCParser()
ccb = CCBParser()

print('\n检查匹配:')
for f in files:
    fp = f'src/bills/{f}'
    print(f'\n{f}:')
    print(f'  ABC: {abc.can_parse(fp)}')
    print(f'  ICBC: {icbc.can_parse(fp)}')
    print(f'  CCB: {ccb.can_parse(fp)}')
