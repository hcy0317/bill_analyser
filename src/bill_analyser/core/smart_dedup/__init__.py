"""Public facade for the smart deduplication engine.

The implementation is split by functional domain under this package, while the
public import path remains `bill_analyser.core.smart_dedup`.
"""
# pylint: disable=invalid-name

from typing import Any

from .engine import SmartDeduplicationEngine
from .models import DeduplicationResult, DeduplicationType, DuplicateGroup

DeduplicationType.__module__ = __name__
DuplicateGroup.__module__ = __name__
DeduplicationResult.__module__ = __name__
SmartDeduplicationEngine.__module__ = __name__

_engine: SmartDeduplicationEngine | None = None


def get_dedup_engine() -> SmartDeduplicationEngine:
    """获取去重引擎单例

    Returns:
        SmartDeduplicationEngine: 去重引擎实例
    """
    global _engine  # pylint: disable=global-statement
    if _engine is None:
        _engine = SmartDeduplicationEngine()
    return _engine


def smart_deduplicate(bills: list[dict[str, Any]]) -> DeduplicationResult:
    """智能去重

    Args:
        bills: 账单列表

    Returns:
        DeduplicationResult: 去重结果
    """
    return get_dedup_engine().process(bills)


__all__ = [
    "DeduplicationResult",
    "DeduplicationType",
    "DuplicateGroup",
    "SmartDeduplicationEngine",
    "get_dedup_engine",
    "smart_deduplicate",
]
