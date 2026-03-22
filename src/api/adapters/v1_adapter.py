"""Legacy compatibility wrapper for transaction adapter.

保留旧模块名，内部转发到中性实现模块 `transaction_adapter.py`。
"""

from src.api.adapters.transaction_adapter import (
	ResponseBuilder as _ResponseBuilder,
	TransactionAdapter as _TransactionAdapter,
)

TransactionAdapter = _TransactionAdapter
V1TransactionAdapter = _TransactionAdapter
ResponseBuilder = _ResponseBuilder
V1ResponseBuilder = _ResponseBuilder

__all__ = ['TransactionAdapter', 'V1TransactionAdapter', 'ResponseBuilder', 'V1ResponseBuilder']
