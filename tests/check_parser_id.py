#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
测试解析器的PARSER_ID设置
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.parsers.factory import ParserFactory

# 获取所有解析器信息
factory = ParserFactory()

print("=" * 60)
print("所有解析器的 PARSER_ID 设置")
print("=" * 60)

# 检查所有解析器的PARSER_ID
for parser in factory.parsers:
    print(f'{parser.__class__.__name__}: PARSER_ID = "{parser.PARSER_ID}"')
