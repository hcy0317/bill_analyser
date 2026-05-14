# pylint: disable=too-few-public-methods,wildcard-import,unused-wildcard-import
"""Statistics API route package split by functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import basic as _basic
from .basic import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _basic,
)


class _StatisticsCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _StatisticsCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
