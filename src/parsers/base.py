"""
Parser Base Module - 账单解析器基类

定义账单解析器的抽象接口。
"""

from abc import ABC, abstractmethod
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Any

from ..utils.logger import get_logger, log_method


class ParserBase(ABC):
    """账单解析器抽象基类"""

    def __init__(self):
        """初始化解析器"""
        self.logger = get_logger(self.__class__.__name__)
        self.supported_extensions = ['.csv', '.xlsx', '.xls']

    @abstractmethod
    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """
        解析账单文件

        Args:
            file_path: 账单文件路径

        Returns:
            List[Dict]: 账单列表，每个账单包含以下字段:
                - trade_time: 交易时间 (str, YYYY-MM-DD HH:MM:SS格式)
                - type: 类型 (收入/支出/转账/退款)
                - main_category: 主分类 (str, 可为空)
                - sub_category: 子分类 (str, 可为空)
                - amount: 金额 (float)
                - account: 账户/来源 (str, 如"微信支付"、"支付宝"、"工商银行"等)
                - description: 描述/备注 (str)
                - counterparty: 交易对方 (str, 可为空)
                - payment_method: 支付方式 (str, 可为空)
        """
        raise NotImplementedError

    @abstractmethod
    def can_parse(self, file_path: str) -> bool:
        """
        判断是否能解析此文件

        Args:
            file_path: 文件路径

        Returns:
            bool: 是否能解析
        """
        raise NotImplementedError

    @log_method
    def validate_file(self, file_path: str) -> bool:
        """
        验证文件是否存在且格式正确

        Args:
            file_path: 文件路径

        Returns:
            bool: 是否有效
        """
        path = Path(file_path)

        if not path.exists():
            self.logger.error(f"文件不存在: {file_path}")
            return False

        if not path.is_file():
            self.logger.error(f"不是文件: {file_path}")
            return False

        if path.suffix.lower() not in self.supported_extensions:
            self.logger.warning(
                f"文件扩展名不支持: {path.suffix}, "
                f"支持的扩展名: {', '.join(self.supported_extensions)}"
            )
            return False

        return True

    def normalize_date(self, date_str: str) -> str:
        """
        规范化日期格式为 YYYY-MM-DD HH:MM:SS

        Args:
            date_str: 日期字符串

        Returns:
            str: 规范化后的日期字符串
        """
        # 常见日期格式
        formats = [
            '%Y-%m-%d %H:%M:%S',
            '%Y-%m-%d',
            '%Y/%m/%d %H:%M:%S',
            '%Y/%m/%d',
            '%Y年%m月%d日 %H:%M:%S',
            '%Y年%m月%d日',
            '%Y.%m.%d %H:%M:%S',
            '%Y.%m.%d',
        ]

        for fmt in formats:
            try:
                dt = datetime.strptime(date_str.strip(), fmt)
                return dt.strftime('%Y-%m-%d %H:%M:%S')
            except ValueError:
                continue

        # 如果都失败，返回原字符串
        self.logger.warning(f"无法解析日期格式: {date_str}")
        return date_str

    def normalize_amount(self, amount_str: str) -> float:
        """
        规范化金额格式

        Args:
            amount_str: 金额字符串

        Returns:
            float: 金额数值
        """
        try:
            # 移除常见符号
            cleaned = str(amount_str).replace('¥', '').replace('$', '') \
                                    .replace(',', '').replace('，', '') \
                                    .strip()

            return float(cleaned)
        except (ValueError, AttributeError) as e:
            self.logger.warning(f"无法解析金额: {amount_str} - {e}")
            return 0.0

    def normalize_type(self, type_str: str) -> str:
        """
        规范化交易类型

        Args:
            type_str: 类型字符串

        Returns:
            str: 规范化的类型（收入/支出/转账/退款）
        """
        type_map = {
            '收入': '收入',
            '收款': '收入',
            '入账': '收入',
            '支出': '支出',
            '支付': '支出',
            '消费': '支出',
            '付款': '支出',
            '转账': '转账',
            '转出': '转账',
            '转入': '转账',
            '退款': '退款',
            '退钱': '退款',
            '不计收支': '转账',
        }

        type_str = str(type_str).strip()

        for key, value in type_map.items():
            if key in type_str:
                return value

        # 默认根据金额符号判断
        self.logger.debug(f"未识别的交易类型: {type_str}")
        return '支出'

    @log_method
    def post_process(self, bills: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """
        后处理账单数据，统一字段格式

        Args:
            bills: 原始账单列表（支持旧格式字段）

        Returns:
            List[Dict]: 处理后的账单列表（统一为新格式字段）
        """
        processed_bills = []

        for bill in bills:
            try:
                # 创建新格式的账单对象
                processed_bill = {}

                # 1. 处理交易时间字段
                # 旧字段: date, 新字段: trade_time
                time_value = bill.get('trade_time') or bill.get('date', '')
                if time_value:
                    processed_bill['trade_time'] = self.normalize_date(time_value)
                else:
                    processed_bill['trade_time'] = ''

                # 2. 处理交易类型字段
                # 保持字段名: type
                type_value = bill.get('type', '')
                if type_value:
                    processed_bill['type'] = self.normalize_type(type_value)
                else:
                    processed_bill['type'] = '支出'  # 默认为支出

                # 3. 处理金额字段
                # 保持字段名: amount
                amount_value = bill.get('amount', 0)
                processed_bill['amount'] = self.normalize_amount(amount_value)

                # 4. 处理账户字段
                # 旧字段: channel, 新字段: account
                account_value = bill.get('account') or bill.get('channel', '')
                processed_bill['account'] = str(account_value) if account_value else ''

                # 5. 处理描述字段
                # 保持字段名: description
                processed_bill['description'] = str(bill.get('description', ''))

                # 6. 处理交易对方字段
                # 保持字段名: counterparty
                processed_bill['counterparty'] = str(bill.get('counterparty', ''))

                # 7. 处理分类字段（解析器不负责分类，后端会通过CategoryEngine处理）
                # 保持为空，由后端业务逻辑填充
                processed_bill['main_category'] = bill.get('main_category', '')
                processed_bill['sub_category'] = bill.get('sub_category', '')

                # 8. 处理支付方式字段
                # 新增字段: payment_method
                processed_bill['payment_method'] = bill.get('payment_method', '')

                # 验证必需字段
                required_fields = ['trade_time', 'type', 'amount', 'account']
                if all(field in processed_bill for field in required_fields):
                    # 确保所有字段都存在（即使为空）
                    for field in ['description', 'counterparty', 'main_category',
                                  'sub_category', 'payment_method']:
                        if field not in processed_bill:
                            processed_bill[field] = ''

                    processed_bills.append(processed_bill)
                else:
                    missing = [f for f in required_fields if f not in processed_bill]
                    self.logger.warning(f"账单缺少必需字段: {missing}")

            except Exception as e:  # pylint: disable=broad-except
                self.logger.error(f"处理账单时出错: {e}")

        self.logger.info(f"后处理完成: {len(processed_bills)}/{len(bills)} 条有效")
        return processed_bills
