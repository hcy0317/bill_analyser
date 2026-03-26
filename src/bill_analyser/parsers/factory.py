"""
Parser Factory - 解析器工厂

根据文件特征自动识别并选择合适的解析器。

工厂模式：按顺序尝试所有注册的解析器的 can_parse() 方法，
返回第一个能够解析该文件的解析器。

支持的解析器:
- WeChatParser (wechat): 微信支付
- AlipayParser (alipay): 支付宝
- ICBCParser (icbc): 工商银行
- CMBCParser (cmbc): 民生银行
- ABCParser (abc): 农业银行
- CCBParser (ccb): 建设银行
"""

from pathlib import Path
from typing import Any

from ..utils.logger import get_logger, log_method
from .abc import ABCParser
from .alipay import AlipayParser
from .base import ParserBase
from .ccb import CCBParser
from .cmbc import CMBCParser
from .icbc import ICBCParser
from .wechat import WeChatParser


class ParserFactory:
    """解析器工厂

    自动识别账单类型并返回对应的解析器。
    解析器按优先级顺序尝试：微信 > 支付宝 > 工商银行 > 民生银行 > 农业银行 > 建设银行
    """

    def __init__(self):
        """初始化工厂"""
        self.logger = get_logger("ParserFactory")

        # 注册所有解析器（按优先级顺序）
        # 支付平台在前（信息更丰富），银行在后
        self.parsers: list[ParserBase] = [
            WeChatParser(),
            AlipayParser(),
            ICBCParser(),
            CMBCParser(),
            ABCParser(),
            CCBParser(),
        ]

        # 解析器ID到类的映射
        self._parser_map = {
            "wechat": WeChatParser,
            "alipay": AlipayParser,
            "icbc": ICBCParser,
            "cmbc": CMBCParser,
            "abc": ABCParser,
            "ccb": CCBParser,
        }

        self.logger.info(
            "[解析器工厂] 初始化完成，注册了 %d 个解析器: %s",
            len(self.parsers),
            ", ".join(p.PARSER_ID for p in self.parsers),
        )

    def get_parser_by_id(self, parser_id: str) -> ParserBase | None:
        """根据解析器ID获取解析器实例

        Args:
            parser_id: 解析器标识符 (wechat/alipay/icbc/cmbc/abc/ccb)

        Returns:
            解析器实例或None
        """
        parser_class = self._parser_map.get(parser_id.lower())
        if parser_class:
            return parser_class()
        return None

    @log_method
    def detect_parser(self, file_path: str) -> dict[str, Any] | None:
        """检测文件应该使用哪个解析器

        Args:
            file_path: 文件路径

        Returns:
            解析器信息字典 {id, name, class_name} 或 None
        """
        path = Path(file_path)

        if not path.exists():
            self.logger.error("[检测解析器] 文件不存在: %s", file_path)
            return None

        for parser in self.parsers:
            try:
                if parser.can_parse(file_path):
                    self.logger.info(
                        "[检测解析器] 文件 %s -> 使用 %s (%s)", path.name, parser.PARSER_NAME, parser.PARSER_ID
                    )
                    return {"id": parser.PARSER_ID, "name": parser.PARSER_NAME, "class_name": parser.__class__.__name__}
            except Exception as e:  # pylint: disable=broad-except
                self.logger.debug("[检测解析器] %s 检测失败: %s", parser.__class__.__name__, e)

        self.logger.warning("[检测解析器] 无法识别文件类型: %s", file_path)
        return None

    @log_method
    def get_parser(self, file_path: str, parser_type: str | None = None) -> ParserBase | None:
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
            self.logger.error("[获取解析器] 文件不存在: %s", file_path)
            return None

        # 如果指定了解析器类型，直接返回对应解析器
        if parser_type:
            parser = self.get_parser_by_id(parser_type)
            if parser:
                self.logger.info("[获取解析器] 使用指定解析器 %s 处理: %s", parser.PARSER_ID, path.name)
                return parser
            self.logger.warning("[获取解析器] 未知解析器类型: %s，将自动检测", parser_type)

        # 尝试所有解析器自动检测
        for parser in self.parsers:
            try:
                if parser.can_parse(file_path):
                    self.logger.info(
                        "[获取解析器] 自动选择 %s (%s) 处理: %s", parser.PARSER_NAME, parser.PARSER_ID, path.name
                    )
                    return parser
            except Exception as e:  # pylint: disable=broad-except
                self.logger.debug("[获取解析器] %s 无法处理: %s", parser.__class__.__name__, e)

        self.logger.warning("[获取解析器] 未找到合适的解析器: %s", file_path)
        return None

    @log_method
    def parse(self, file_path: str, parser_type: str | None = None) -> list[dict[str, Any]]:
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
            self.logger.info("文件解析完成: %s，共 %d 条账单", Path(file_path).name, len(bills))
            return bills

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析文件失败 %s: %s", file_path, e)
            return []

    @log_method
    def parse_multiple(self, file_paths: list[str]) -> list[dict[str, Any]]:
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

        self.logger.info("批量解析完成: %d 个文件，共 %d 条账单", len(file_paths), len(all_bills))

        return all_bills

    def get_supported_formats(self) -> list[str]:
        """
        获取所有支持的文件格式

        Returns:
            List[str]: 文件扩展名列表
        """
        formats = set()
        for parser in self.parsers:
            formats.update(parser.supported_extensions)
        return sorted(list(formats))

    def get_parser_info(self) -> list[dict[str, Any]]:
        """
        获取所有解析器的信息

        Returns:
            List[Dict]: 解析器信息列表
        """
        return [
            {"name": parser.__class__.__name__, "supported_formats": parser.supported_extensions}
            for parser in self.parsers
        ]


# 全局工厂实例
_factory = ParserFactory()


def get_parser(file_path: str) -> ParserBase | None:
    """
    获取文件解析器

    Args:
        file_path: 文件路径

    Returns:
        Optional[ParserBase]: 解析器实例
    """
    return _factory.get_parser(file_path)


def parse_file(file_path: str) -> list[dict[str, Any]]:
    """
    解析账单文件

    Args:
        file_path: 文件路径

    Returns:
        List[Dict]: 账单列表
    """
    return _factory.parse(file_path)


def parse_multiple_files(file_paths: list[str]) -> list[dict[str, Any]]:
    """
    批量解析文件

    Args:
        file_paths: 文件路径列表

    Returns:
        List[Dict]: 账单列表
    """
    return _factory.parse_multiple(file_paths)


def get_supported_formats() -> list[str]:
    """获取支持的文件格式"""
    return _factory.get_supported_formats()
