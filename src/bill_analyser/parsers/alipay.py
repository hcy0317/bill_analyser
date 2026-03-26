"""
Alipay Parser - 支付宝账单解析器

解析支付宝账单导出的 CSV 文件。

支付宝账单特征:
- 编码: GBK
- 列名: 交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收/付款方式,交易状态,交易订单号,商家订单号,备注

输出标准格式:
- date: 交易时间 (YYYY-MM-DD HH:MM:SS)
- amount: 金额 (支出为负, 收入为正)
- type: 类型 (收入/支出/转账/投资/退款)
- description: 聚合描述
- source_account_id: 'alipay'
"""

import csv
from typing import Any

from ..utils.logger import log_method
from .base import ParserBase


class AlipayParser(ParserBase):
    """支付宝账单解析器"""

    # 解析器标识符
    PARSER_ID = "alipay"
    PARSER_NAME = "支付宝"

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = [".csv"]
        self.logger.info("支付宝账单解析器已初始化 [ID=%s]", self.PARSER_ID)

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为支付宝账单"""
        try:
            # 支付宝账单通常是 GBK 编码
            with open(file_path, encoding="gbk") as f:
                first_lines = "".join([f.readline() for _ in range(15)])
                # 支付宝特征
                alipay_indicators = ["支付宝", "alipay", "支付宝账户", "支付宝（中国）网络技术有限公司"]
                return any(ind.lower() in first_lines.lower() for ind in alipay_indicators)
        except Exception:  # pylint: disable=broad-except
            return False

    @log_method
    def parse(self, file_path: str) -> list[dict[str, Any]]:
        """解析支付宝账单"""
        self.logger.info("[解析开始] 文件: %s", file_path)

        if not self.validate_file(file_path):
            return []

        bills = []

        # 支付宝账单通常是 GBK 编码
        encodings = ["gbk", "utf-8", "gb18030"]

        for encoding in encodings:
            try:
                bills = self._parse_with_encoding(file_path, encoding)
                if bills:
                    self.logger.info("[解析完成] 使用编码 %s 成功解析 %d 条账单", encoding, len(bills))
                    break
            except Exception as e:  # pylint: disable=broad-except
                self.logger.debug("使用编码 %s 解析失败: %s", encoding, e)
                continue

        if not bills:
            self.logger.error("解析支付宝账单失败: 所有编码尝试均失败")
            return []

        return self.post_process(bills)

    def _parse_with_encoding(self, file_path: str, encoding: str) -> list[dict[str, Any]]:
        """使用指定编码解析文件"""
        bills = []

        with open(file_path, encoding=encoding) as f:
            lines = f.readlines()

            # 查找数据起始行（包含"交易时间"的行）
            data_start = 0
            for i, line in enumerate(lines):
                if "交易时间" in line and "交易" in line:
                    data_start = i
                    break

            if data_start == 0:
                self.logger.warning("未找到数据起始行")
                return []

            self.logger.debug("数据起始行: %d", data_start)

            # 解析CSV数据
            reader = csv.DictReader(lines[data_start:])

            for row in reader:
                try:
                    bill = self._extract_bill_from_row(row)
                    if bill:
                        bills.append(bill)

                except Exception as e:  # pylint: disable=broad-except
                    self.logger.error("解析行数据失败: %s", e)

        return bills

    def _extract_bill_from_row(self, row: dict[str, str]) -> dict[str, Any] | None:
        """从行数据提取账单信息"""
        # 跳过空行和页脚
        time_field = row.get("交易时间") or row.get("交易创建时间") or ""
        time_field = time_field.strip()

        if not time_field or "共" in time_field or time_field == "":
            return None

        # 获取金额并清理
        amount_str = row.get("金额", "") or row.get("金额(元)", "") or "0"
        amount_str = amount_str.strip()

        # 获取交易类型
        trans_type = (row.get("收/支", "") or "").strip()

        # 获取原始分类（支付宝有自己的分类，如"投资理财"）
        original_category = (row.get("交易分类", "") or "").strip()

        # 提取所有可能有用的字段
        return {
            "date": time_field,
            "type": trans_type,
            "counterparty": (row.get("交易对方", "") or row.get("对方", "") or "").strip(),
            "opponent_account": (row.get("对方账号", "") or "").strip(),
            "description": (row.get("商品说明", "") or row.get("商品名称", "") or "").strip(),
            "goods": (row.get("商品说明", "") or row.get("商品名称", "") or "").strip(),
            "amount": amount_str,
            "payment_method": (row.get("收/付款方式", "") or "").strip(),
            "status": (row.get("交易状态", "") or "").strip(),
            "transaction_id": (row.get("交易订单号", "") or "").strip(),
            "merchant_id": (row.get("商家订单号", "") or "").strip(),
            "remark": (row.get("备注", "") or "").strip(),
            "original_category": original_category,
        }
