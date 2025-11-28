"""
Alipay Parser - 支付宝账单解析器

解析支付宝账单导出的 CSV 文件。
"""

import csv
from typing import Dict, List, Any

from .base import ParserBase
from ..utils.logger import log_method


class AlipayParser(ParserBase):
    """支付宝账单解析器"""

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = ['.csv']

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为支付宝账单"""
        try:
            with open(file_path, 'r', encoding='gbk') as f:
                first_lines = ''.join([f.readline() for _ in range(5)])
                return 'alipay' in file_path.lower() or '支付宝' in first_lines
        except Exception:  # pylint: disable=broad-except
            return False

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析支付宝账单"""
        if not self.validate_file(file_path):
            return []

        bills = []

        # 支付宝账单通常是 GBK 编码
        encodings = ['gbk', 'utf-8']

        for encoding in encodings:
            try:
                with open(file_path, 'r', encoding=encoding) as f:
                    lines = f.readlines()

                    # 查找数据起始行
                    data_start = 0
                    for i, line in enumerate(lines):
                        if '交易时间' in line or '交易创建时间' in line:
                            data_start = i
                            break

                    if data_start == 0:
                        continue

                    # 解析CSV数据
                    reader = csv.DictReader(lines[data_start:])

                    for row in reader:
                        try:
                            # 跳过空行和页脚
                            time_field = row.get('交易时间') or row.get('交易创建时间') or ''
                            if not time_field or '共' in time_field:
                                continue

                            # 提取字段
                            bill = {
                                'date': time_field,
                                'type': row.get('收/支', ''),
                                'counterparty': row.get('交易对方', '') or row.get('对方', ''),
                                'description': row.get('商品说明', '') or row.get('商品名称', ''),
                                'amount': row.get('金额', '') or row.get('金额(元)', ''),
                                'channel': '支付宝'
                            }

                            bills.append(bill)

                        except Exception as e:  # pylint: disable=broad-except
                            self.logger.error("解析行数据失败: %s", e)

                    if bills:
                        break

            except Exception:  # pylint: disable=broad-except
                continue

        if not bills:
            self.logger.error("解析支付宝账单失败")
            return []

        self.logger.info("支付宝账单解析完成: %d 条", len(bills))
        return self.post_process(bills)
