"""
微信账单解析器

支持微信导出的CSV格式账单
"""

import csv
from pathlib import Path
from typing import Any

import chardet

from .base_parser import BaseParser, ParserFactory


class WeChatParser(BaseParser):
    """微信账单解析器"""

    # 微信标准字段映射
    WECHAT_FIELD_MAPPING = {
        "date": "交易时间",
        "type": "收/支",
        "amount": "金额(元)",
        "description": "商品",
        "counterparty": "交易对方",
        "category": "交易类型",
        "account": "支付方式",
        "comment": "备注",
    }

    async def parse(self, file_path: str, config: dict[str, Any] | None = None) -> list[dict[str, Any]]:
        """解析微信账单文件"""
        config = config or {}

        encoding = config.get("encoding") or await self.detect_encoding(file_path)
        field_mapping = config.get("field_mappings") or self.WECHAT_FIELD_MAPPING

        bills = []

        with open(file_path, encoding=encoding) as f:
            lines = f.readlines()

            # 找到表头行（通常包含"交易时间"）
            header_idx = 0
            for idx, line in enumerate(lines):
                if "交易时间" in line:
                    header_idx = idx
                    break

            # 从表头行开始读取
            content = "".join(lines[header_idx:])

            reader = csv.DictReader(content.splitlines())

            for row_idx, row in enumerate(reader):
                try:
                    # 过滤掉空行和汇总行
                    if not row.get("交易时间") or "共" in str(row.get("交易时间", "")):
                        continue

                    # 微信特殊处理：状态过滤
                    status = row.get("当前状态", "")
                    if status and "已全额退款" in status:
                        # 跳过已退款的交易
                        continue

                    # 应用字段映射
                    bill = self.standardize_bill(row, field_mapping)

                    # 微信特殊处理：添加状态信息
                    if status and status != "支付成功" and status != "已收钱":
                        bill["comment"] = f"{bill.get('comment', '')} [{status}]".strip()

                    bills.append(bill)

                except Exception as e:
                    self.logger.warning(f"解析第{row_idx + 1}行失败: {e}")
                    continue

        self.logger.info(f"成功解析微信账单，共{len(bills)}条记录")
        return bills

    async def validate(self, file_path: str) -> bool:
        """验证是否为微信账单文件"""
        try:
            path = Path(file_path)
            if not path.exists():
                return False

            encoding = await self.detect_encoding(file_path)

            with open(file_path, encoding=encoding) as f:
                content = f.read(1000)

                # 检查是否包含微信特征字段
                wechat_indicators = ["微信支付", "交易时间", "支付方式", "当前状态"]
                return any(indicator in content for indicator in wechat_indicators)

        except Exception as e:
            self.logger.debug(f"微信账单验证失败: {e}")
            return False

    async def detect_encoding(self, file_path: str) -> str:
        """检测文件编码"""
        with open(file_path, "rb") as f:
            raw_data = f.read(10000)
            result = chardet.detect(raw_data)

            # 微信通常使用UTF-8
            encoding = result["encoding"] or "utf-8"

            if "utf" in encoding.lower():
                return "utf-8"
            elif "gb" in encoding.lower():
                return "gbk"

            return encoding

    async def preview(self, file_path: str, rows: int = 10) -> dict[str, Any]:
        """预览微信账单内容"""
        encoding = await self.detect_encoding(file_path)

        with open(file_path, encoding=encoding) as f:
            lines = f.readlines()

            # 找到表头行
            header_idx = 0
            for idx, line in enumerate(lines):
                if "交易时间" in line:
                    header_idx = idx
                    break

            # 从表头行开始读取
            content = "".join(lines[header_idx:])

            reader = csv.DictReader(content.splitlines())

            headers = reader.fieldnames
            sample_data = []

            for idx, row in enumerate(reader):
                if idx >= rows:
                    break
                # 过滤汇总行
                if not row.get("交易时间") or "共" in str(row.get("交易时间", "")):
                    continue
                sample_data.append(list(row.values()))

            return {
                "headers": headers,
                "sample_data": sample_data,
                "encoding": encoding,
                "format": "wechat",
                "field_mapping": self.WECHAT_FIELD_MAPPING,
            }

    def _parse_type(self, type_str: Any) -> str:
        """解析微信的收支类型"""
        if not type_str:
            return "expense"

        type_str = str(type_str).strip()

        if "收入" in type_str or type_str == "收":
            return "income"
        elif "支出" in type_str or type_str == "支":
            return "expense"

        return super()._parse_type(type_str)


# 注册解析器
ParserFactory.register("wechat", WeChatParser)
