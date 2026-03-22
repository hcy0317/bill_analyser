"""Legacy compatibility wrapper for account adapter.

保留旧模块名，内部转发到中性实现模块 `account_adapter.py`。
"""

from src.api.adapters.account_adapter import AccountAdapter as _AccountAdapter

AccountAdapter = _AccountAdapter
V1AccountAdapter = _AccountAdapter

__all__ = ['AccountAdapter', 'V1AccountAdapter']
