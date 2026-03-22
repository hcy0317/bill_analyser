"""Legacy compatibility wrapper for category adapter.

保留旧模块名，内部转发到中性实现模块 `category_adapter.py`。
"""

from src.api.adapters.category_adapter import CategoryAdapter as _CategoryAdapter

CategoryAdapter = _CategoryAdapter
V1CategoryAdapter = _CategoryAdapter

__all__ = ['CategoryAdapter', 'V1CategoryAdapter']
