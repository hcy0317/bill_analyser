"""Legacy generic Excel parser compatibility shim."""

from __future__ import annotations

import warnings
from pathlib import Path
from typing import Any

from .base import ParserBase
from .factory import ParserFactory


class ExcelParser(ParserBase):
    """兼容旧导入路径的通用 Excel 解析 shim。"""

    PARSER_ID = "excel-compat"
    PARSER_NAME = "兼容Excel解析器"

    def __init__(self):
        super().__init__()
        self.supported_extensions = [".xlsx", ".xls"]
        warnings.warn(
            "bill_analyser.parsers.excel_parser.ExcelParser 已弃用；请改用 ParserFactory 自动选择当前解析器。",
            DeprecationWarning,
            stacklevel=2,
        )

    def can_parse(self, file_path: str) -> bool:
        return Path(file_path).suffix.lower() in self.supported_extensions

    def parse(self, file_path: str) -> list[dict[str, Any]]:
        return ParserFactory().parse(file_path)


__all__ = ["ExcelParser"]
