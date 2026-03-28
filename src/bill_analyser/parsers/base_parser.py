"""Legacy compatibility shims for the removed parser factory stack."""

from __future__ import annotations

import warnings

from .base import ParserBase as BaseParser
from .factory import ParserFactory

warnings.warn(
    "bill_analyser.parsers.base_parser 已弃用；请改用 bill_analyser.parsers.base / factory。",
    DeprecationWarning,
    stacklevel=2,
)

__all__ = ["BaseParser", "ParserFactory"]
