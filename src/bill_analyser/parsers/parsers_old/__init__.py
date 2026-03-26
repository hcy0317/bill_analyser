#!/usr/bin/env python3

"""
银行解析器工厂和管理器
"""

import logging
from typing import List, Optional

from .abc import ABCParser
from .base import BaseBankParser
from .cmbc import CMBCParser
from .icbc import ICBCParser

logger = logging.getLogger(__name__)


class BankParserFactory:
    """银行解析器工厂"""

    def __init__(self):
        # 注册所有银行解析器
        self.parsers = [
            ICBCParser(),
            ABCParser(),
            CMBCParser(),
        ]

    def get_parser(self, file_path: str) -> BaseBankParser | None:
        """根据文件获取对应的银行解析器"""
        try:
            for parser in self.parsers:
                if parser.can_parse(file_path):
                    logger.info("选择解析器: %s - %s", parser.get_bank_name(), file_path)
                    return parser

            logger.warning("未找到合适的银行解析器: %s", file_path)
            return None

        except Exception as e:
            logger.error("获取银行解析器失败: %s, 错误: %s", file_path, e)
            return None

    def get_supported_banks(self) -> list[str]:
        """获取支持的银行列表"""
        return [parser.get_bank_name() for parser in self.parsers]

    def parse_file(self, file_path: str):
        """直接解析文件"""
        parser = self.get_parser(file_path)
        if parser:
            return parser.parse(file_path)
        import pandas as pd

        return pd.DataFrame()


# 全局工厂实例
bank_factory = BankParserFactory()
