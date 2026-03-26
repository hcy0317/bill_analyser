"""
CSV文件解析器

支持通用CSV格式账单导入
"""

import csv
from pathlib import Path
from typing import Any

import chardet

from .base_parser import BaseParser, ParserFactory


class CSVParser(BaseParser):
    """CSV文件解析器"""

    async def parse(self, file_path: str, config: dict[str, Any] | None = None) -> list[dict[str, Any]]:
        """解析CSV文件"""
        config = config or {}

        encoding = config.get("encoding") or await self.detect_encoding(file_path)
        delimiter = config.get("delimiter", ",")
        skip_rows = config.get("skip_rows", 0)
        has_header = config.get("has_header", True)
        field_mapping = config.get("field_mappings", {})

        bills = []

        with open(file_path, encoding=encoding) as f:
            # 跳过指定行数
            for _ in range(skip_rows):
                next(f)

            reader = csv.DictReader(f, delimiter=delimiter) if has_header else csv.reader(f, delimiter=delimiter)

            for row_idx, row in enumerate(reader):
                try:
                    if has_header:
                        raw_data = dict(row)
                    else:
                        # 如果没有header，使用列索引
                        raw_data = {str(i): val for i, val in enumerate(row)}

                    # 应用字段映射
                    if field_mapping:
                        bill = self.standardize_bill(raw_data, field_mapping)
                        bills.append(bill)

                except Exception as e:
                    self.logger.warning(f"解析第{row_idx + 1}行失败: {e}")
                    continue

        self.logger.info(f"成功解析CSV文件，共{len(bills)}条记录")
        return bills

    async def validate(self, file_path: str) -> bool:
        """验证是否为有效的CSV文件"""
        try:
            path = Path(file_path)
            if not path.exists():
                return False

            if path.suffix.lower() not in [".csv", ".txt"]:
                return False

            # 尝试读取前几行
            encoding = await self.detect_encoding(file_path)
            with open(file_path, encoding=encoding) as f:
                csv.Sniffer().sniff(f.read(1024))
            return True

        except Exception as e:
            self.logger.debug(f"CSV验证失败: {e}")
            return False

    async def detect_encoding(self, file_path: str) -> str:
        """检测文件编码"""
        with open(file_path, "rb") as f:
            raw_data = f.read(10000)
            result = chardet.detect(raw_data)
            return result["encoding"] or "utf-8"

    async def preview(self, file_path: str, rows: int = 10) -> dict[str, Any]:
        """预览CSV内容"""
        encoding = await self.detect_encoding(file_path)

        with open(file_path, encoding=encoding) as f:
            # 尝试检测分隔符
            sample = f.read(1024)
            f.seek(0)

            try:
                dialect = csv.Sniffer().sniff(sample)
                delimiter = dialect.delimiter
            except:
                delimiter = ","

            reader = csv.reader(f, delimiter=delimiter)

            data = []
            headers = None

            for idx, row in enumerate(reader):
                if idx == 0:
                    headers = row
                elif idx <= rows:
                    data.append(row)
                else:
                    break

            return {
                "headers": headers,
                "sample_data": data,
                "delimiter": delimiter,
                "encoding": encoding,
                "total_rows": sum(1 for _ in open(file_path, encoding=encoding)) - 1,
            }


# 注册解析器
ParserFactory.register("csv", CSVParser)
