"""Categories API route package split by category functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import collection as _collection
from .collection import *  # noqa: F403
from . import mutations as _mutations
from .mutations import *  # noqa: F403
from . import lists as _lists
from .lists import *  # noqa: F403
from . import rules as _rules
from .rules import *  # noqa: F403
from . import analytics as _analytics
from .analytics import *  # noqa: F403
from . import io as _io
from .io import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _collection,
    _mutations,
    _lists,
    _rules,
    _analytics,
    _io,
)


class _CategoriesCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split categories route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _CategoriesCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
