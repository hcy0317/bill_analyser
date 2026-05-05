"""Statistics API route package split by functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import basic as _basic
from .basic import *  # noqa: F403
from . import exchange_rates as _exchange_rates
from .exchange_rates import *  # noqa: F403
from . import category_analysis as _category_analysis
from .category_analysis import *  # noqa: F403
from . import trend_analysis as _trend_analysis
from .trend_analysis import *  # noqa: F403
from . import asset_trends as _asset_trends
from .asset_trends import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _basic,
    _exchange_rates,
    _category_analysis,
    _trend_analysis,
    _asset_trends,
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
