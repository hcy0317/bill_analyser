"""
文件格式解析器基类和工厂

定义统一的解析器接口，支持多种导入格式
"""

from abc import ABC, abstractmethod
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from ..utils.logger import get_logger


class BaseParser(ABC):
    """文件解析器基类"""

    def __init__(self):
        self.logger = get_logger(self.__class__.__name__)

    @abstractmethod
    async def parse(self, file_path: str, config: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """
        解析文件并返回标准化的账单数据

        Args:
            file_path: 文件路径
            config: 解析配置（字段映射、编码等）

        Returns:
            List[Dict]: 标准化的账单列表
        """
        pass

    @abstractmethod
    async def validate(self, file_path: str) -> bool:
        """
        验证文件格式是否正确

        Args:
            file_path: 文件路径

        Returns:
            bool: 是否为有效文件
        """
        pass

    @abstractmethod
    async def detect_encoding(self, file_path: str) -> str:
        """
        检测文件编码

        Args:
            file_path: 文件路径

        Returns:
            str: 编码名称
        """
        pass

    @abstractmethod
    async def preview(self, file_path: str, rows: int = 10) -> Dict[str, Any]:
        """
        预览文件内容

        Args:
            file_path: 文件路径
            rows: 预览行数

        Returns:
            Dict: 预览数据，包含headers和sample_data
        """
        pass

    def standardize_bill(self, raw_data: Dict[str, Any], field_mapping: Dict[str, str]) -> Dict[str, Any]:
        """
        将原始数据转换为标准账单格式

        Args:
            raw_data: 原始数据
            field_mapping: 字段映射配置

        Returns:
            Dict: 标准化的账单数据
        """
        bill = {}

        # 必填字段
        required_fields = ['date', 'type', 'amount', 'description']

        for std_field, raw_field in field_mapping.items():
            if raw_field and raw_field in raw_data:
                value = raw_data[raw_field]

                # 类型转换和验证
                if std_field == 'date':
                    bill['date'] = self._parse_date(value)
                elif std_field == 'type':
                    bill['type'] = self._parse_type(value)
                elif std_field == 'amount':
                    bill['amount'] = self._parse_amount(value)
                elif std_field in ['category', 'account', 'counterparty', 'description', 'comment']:
                    bill[std_field] = str(value).strip() if value else ''
                else:
                    bill[std_field] = value

        # 验证必填字段
        for field in required_fields:
            if field not in bill or not bill[field]:
                raise ValueError(f"缺少必填字段: {field}")

        return bill

    def _parse_date(self, date_str: Any) -> str:
        """解析日期字符串"""
        if isinstance(date_str, datetime):
            return date_str.strftime('%Y-%m-%d')

        if not date_str:
            raise ValueError("日期不能为空")

        date_str = str(date_str).strip()

        # 尝试多种日期格式
        date_formats = [
            '%Y-%m-%d',
            '%Y/%m/%d',
            '%Y.%m.%d',
            '%Y%m%d',
            '%d/%m/%Y',
            '%d-%m-%Y',
            '%m/%d/%Y',
            '%Y-%m-%d %H:%M:%S',
        ]

        for fmt in date_formats:
            try:
                dt = datetime.strptime(date_str, fmt)
                return dt.strftime('%Y-%m-%d')
            except ValueError:
                continue

        raise ValueError(f"无法解析日期: {date_str}")

    def _parse_type(self, type_str: Any) -> str:
        """解析交易类型"""
        if not type_str:
            return 'expense'

        type_str = str(type_str).strip().lower()

        # 收入关键词
        income_keywords = ['收入', 'income', '进账', '入账', '存入', 'credit', '+']
        # 支出关键词
        expense_keywords = ['支出', 'expense', '出账', '支取', 'debit', '-']

        for keyword in income_keywords:
            if keyword in type_str:
                return 'income'

        for keyword in expense_keywords:
            if keyword in type_str:
                return 'expense'

        # 默认为支出
        return 'expense'

    def _parse_amount(self, amount: Any) -> float:
        """解析金额"""
        if not amount:
            raise ValueError("金额不能为空")

        # 如果已经是数字
        if isinstance(amount, (int, float)):
            return abs(float(amount))

        # 字符串处理
        amount_str = str(amount).strip()

        # 移除货币符号和千位分隔符
        amount_str = amount_str.replace('¥', '').replace('$', '').replace('€', '')
        amount_str = amount_str.replace(',', '').replace(' ', '')

        # 处理括号表示负数的情况
        if amount_str.startswith('(') and amount_str.endswith(')'):
            amount_str = '-' + amount_str[1:-1]

        try:
            return abs(float(amount_str))
        except ValueError:
            raise ValueError(f"无法解析金额: {amount}")


class ParserFactory:
    """解析器工厂"""

    _parsers: Dict[str, type] = {}

    @classmethod
    def register(cls, format_name: str, parser_class: type):
        """
        注册解析器

        Args:
            format_name: 格式名称
            parser_class: 解析器类
        """
        cls._parsers[format_name.lower()] = parser_class

    @classmethod
    def create(cls, format_name: str) -> BaseParser:
        """
        创建解析器实例

        Args:
            format_name: 格式名称

        Returns:
            BaseParser: 解析器实例
        """
        format_name = format_name.lower()
        if format_name not in cls._parsers:
            raise ValueError(f"不支持的文件格式: {format_name}")

        return cls._parsers[format_name]()

    @classmethod
    def get_supported_formats(cls) -> List[str]:
        """获取支持的格式列表"""
        return list(cls._parsers.keys())
