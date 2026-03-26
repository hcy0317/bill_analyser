"""
支付宝账单解析器

支持支付宝导出的CSV格式账单
"""

import csv
import chardet
from pathlib import Path
from typing import Any, Dict, List, Optional

from .base_parser import BaseParser, ParserFactory


class AlipayParser(BaseParser):
    """支付宝账单解析器"""

    # 支付宝标准字段映射
    ALIPAY_FIELD_MAPPING = {
        "date": "交易时间",
        "type": "收/支",
        "amount": "金额",
        "description": "商品说明",
        "counterparty": "交易对方",
        "category": "分类",
        "account": "资金渠道",
        "comment": "备注",
    }

    async def parse(self, file_path: str, config: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """解析支付宝账单文件"""
        config = config or {}

        encoding = config.get("encoding") or await self.detect_encoding(file_path)
        field_mapping = config.get("field_mappings") or self.ALIPAY_FIELD_MAPPING

        bills = []

        with open(file_path, "r", encoding=encoding) as f:
            # 跳过支付宝CSV文件的前几行说明
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

                    # 应用字段映射
                    bill = self.standardize_bill(row, field_mapping)

                    # 支付宝特殊处理：识别退款
                    if "退款" in str(row.get("商品说明", "")) or "退款" in str(row.get("备注", "")):
                        bill["comment"] = f"{bill.get('comment', '')} [退款]".strip()

                    bills.append(bill)

                except Exception as e:
                    self.logger.warning(f"解析第{row_idx + 1}行失败: {e}")
                    continue

        self.logger.info(f"成功解析支付宝账单，共{len(bills)}条记录")
        return bills

    async def validate(self, file_path: str) -> bool:
        """验证是否为支付宝账单文件"""
        try:
            path = Path(file_path)
            if not path.exists():
                return False

            encoding = await self.detect_encoding(file_path)

            with open(file_path, "r", encoding=encoding) as f:
                content = f.read(1000)

                # 检查是否包含支付宝特征字段
                alipay_indicators = ["支付宝", "交易时间", "收/支", "交易对方"]
                return any(indicator in content for indicator in alipay_indicators)

        except Exception as e:
            self.logger.debug(f"支付宝账单验证失败: {e}")
            return False

    async def detect_encoding(self, file_path: str) -> str:
        """检测文件编码"""
        with open(file_path, "rb") as f:
            raw_data = f.read(10000)
            result = chardet.detect(raw_data)

            # 支付宝通常使用GBK或UTF-8
            encoding = result["encoding"] or "gbk"

            # 优先尝试GBK
            if "gb" in encoding.lower() or "gbk" in encoding.lower():
                return "gbk"

            return encoding

    async def preview(self, file_path: str, rows: int = 10) -> Dict[str, Any]:
        """预览支付宝账单内容"""
        encoding = await self.detect_encoding(file_path)

        with open(file_path, "r", encoding=encoding) as f:
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
                "format": "alipay",
                "field_mapping": self.ALIPAY_FIELD_MAPPING,
            }

    def _parse_type(self, type_str: Any) -> str:
        """解析支付宝的收支类型"""
        if not type_str:
            return "expense"

        type_str = str(type_str).strip()

        if "收入" in type_str or type_str == "收":
            return "income"
        if "支出" in type_str or type_str == "支":
            return "expense"

        return super()._parse_type(type_str)


# 注册解析器
ParserFactory.register("alipay", AlipayParser)
