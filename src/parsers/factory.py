"""
Parser Factory - 解析器工厂

根据文件特征自动识别并选择合适的解析器。
"""

from pathlib import Path
from typing import List, Dict, Any, Optional

from .base import ParserBase
from .wechat import WeChatParser
from .alipay import AlipayParser
from .icbc import ICBCParser
from .cmbc import CMBCParser
from .abc import ABCParser
from .ccb import CCBParser
from ..utils.logger import get_logger, log_method


class ParserFactory:
    """解析器工厂"""

    def __init__(self):
        """初始化工厂"""
        self.logger = get_logger('ParserFactory')

        # 注册所有解析器
        self.parsers: List[ParserBase] = [
            WeChatParser(),
            AlipayParser(),
            ICBCParser(),
            CMBCParser(),
            ABCParser(),
            CCBParser(),
        ]

        self.logger.info("解析器工厂初始化完成，注册了 %d 个解析器", len(self.parsers))

    @log_method
    def get_parser(self, file_path: str, parser_type: Optional[str] = None) -> Optional[ParserBase]:
        """
        根据文件特征或指定类型获取合适的解析器

        Args:
            file_path: 文件路径
            parser_type: 指定解析器类型（wechat/alipay/icbc/cmbc/abc/ccb），None表示自动检测

        Returns:
            Optional[ParserBase]: 解析器实例，如果找不到则返回 None
        """
        path = Path(file_path)

        if not path.exists():
            self.logger.error("文件不存在: %s", file_path)
            return None

        # 如果指定了解析器类型，直接返回对应解析器
        if parser_type:
            parser_map = {
                'wechat': WeChatParser,
                'alipay': AlipayParser,
                'icbc': ICBCParser,
                'cmbc': CMBCParser,
                'abc': ABCParser,
                'ccb': CCBParser
            }

            parser_class = parser_map.get(parser_type.lower())
            if parser_class:
                self.logger.info(
                    "使用指定解析器 %s 处理文件: %s",
                    parser_type,
                    path.name
                )
                return parser_class()

            self.logger.warning("未知的解析器类型: %s，将自动检测", parser_type)

        # 尝试所有解析器自动检测
        for parser in self.parsers:
            try:
                if parser.can_parse(file_path):
                    self.logger.info(
                        "为文件 %s 选择解析器: %s",
                        path.name,
                        parser.__class__.__name__
                    )
                    return parser
            except Exception as e:  # pylint: disable=broad-except
                self.logger.debug(
                    "解析器 %s 无法处理文件: %s",
                    parser.__class__.__name__,
                    e
                )

        self.logger.warning("未找到合适的解析器: %s", file_path)
        return None

    @log_method
    def parse(self, file_path: str, parser_type: Optional[str] = None) -> List[Dict[str, Any]]:
        """
        自动解析账单文件

        Args:
            file_path: 文件路径
            parser_type: 指定解析器类型（可选）

        Returns:
            List[Dict]: 账单列表
        """
        parser = self.get_parser(file_path, parser_type)

        if parser is None:
            self.logger.error("无法解析文件: %s", file_path)
            return []

        try:
            bills = parser.parse(file_path)
            self.logger.info(
                "文件解析完成: %s，共 %d 条账单",
                Path(file_path).name,
                len(bills)
            )
            return bills

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析文件失败 %s: %s", file_path, e)
            return []

    @log_method
    def parse_multiple(self, file_paths: List[str]) -> List[Dict[str, Any]]:
        """
        批量解析多个账单文件

        Args:
            file_paths: 文件路径列表

        Returns:
            List[Dict]: 合并后的账单列表
        """
        all_bills = []

        self.logger.info("开始批量解析 %d 个文件", len(file_paths))

        for file_path in file_paths:
            bills = self.parse(file_path)
            all_bills.extend(bills)

        self.logger.info(
            "批量解析完成: %d 个文件，共 %d 条账单",
            len(file_paths),
            len(all_bills)
        )

        return all_bills

    def get_supported_formats(self) -> List[str]:
        """
        获取所有支持的文件格式

        Returns:
            List[str]: 文件扩展名列表
        """
        formats = set()
        for parser in self.parsers:
            formats.update(parser.supported_extensions)
        return sorted(list(formats))

    def get_parser_info(self) -> List[Dict[str, Any]]:
        """
        获取所有解析器的信息

        Returns:
            List[Dict]: 解析器信息列表
        """
        return [
            {
                'name': parser.__class__.__name__,
                'supported_formats': parser.supported_extensions
            }
            for parser in self.parsers
        ]


# 全局工厂实例
_factory = ParserFactory()


def get_parser(file_path: str) -> Optional[ParserBase]:
    """
    获取文件解析器

    Args:
        file_path: 文件路径

    Returns:
        Optional[ParserBase]: 解析器实例
    """
    return _factory.get_parser(file_path)


def parse_file(file_path: str) -> List[Dict[str, Any]]:
    """
    解析账单文件

    Args:
        file_path: 文件路径

    Returns:
        List[Dict]: 账单列表
    """
    return _factory.parse(file_path)


def parse_multiple_files(file_paths: List[str]) -> List[Dict[str, Any]]:
    """
    批量解析文件

    Args:
        file_paths: 文件路径列表

    Returns:
        List[Dict]: 账单列表
    """
    return _factory.parse_multiple(file_paths)


def get_supported_formats() -> List[str]:
    """获取支持的文件格式"""
    return _factory.get_supported_formats()
