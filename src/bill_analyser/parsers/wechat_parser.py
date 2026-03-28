"""Legacy WeChat parser compatibility shim."""

from __future__ import annotations

import warnings

from .wechat import WeChatParser

warnings.warn(
    "bill_analyser.parsers.wechat_parser 已弃用；请改用 bill_analyser.parsers.wechat。",
    DeprecationWarning,
    stacklevel=2,
)

__all__ = ["WeChatParser"]
