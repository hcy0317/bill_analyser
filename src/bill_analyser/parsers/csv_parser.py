"""Legacy generic CSV parser compatibility shim."""

from __future__ import annotations

import warnings
from pathlib import Path
from typing import Any

from .base import ParserBase
from .factory import ParserFactory


class CSVParser(ParserBase):
    """兼容旧导入路径的通用 CSV 解析 shim。"""

    PARSER_ID = "csv-compat"
    PARSER_NAME = "兼容CSV解析器"

    def __init__(self):
        super().__init__()
        self.supported_extensions = [".csv", ".txt"]
        warnings.warn(
            "bill_analyser.parsers.csv_parser.CSVParser 已弃用；请改用 ParserFactory 自动选择当前解析器。",
            DeprecationWarning,
            stacklevel=2,
        )

    def can_parse(self, file_path: str) -> bool:
        return Path(file_path).suffix.lower() in self.supported_extensions

    def parse(self, file_path: str) -> list[dict[str, Any]]:
        return ParserFactory().parse(file_path)


__all__ = ["CSVParser"]
