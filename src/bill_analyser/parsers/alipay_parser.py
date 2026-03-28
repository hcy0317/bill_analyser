"""Legacy Alipay parser compatibility shim."""

from __future__ import annotations

import warnings

from .alipay import AlipayParser

warnings.warn(
    "bill_analyser.parsers.alipay_parser 已弃用；请改用 bill_analyser.parsers.alipay。",
    DeprecationWarning,
    stacklevel=2,
)

__all__ = ["AlipayParser"]
