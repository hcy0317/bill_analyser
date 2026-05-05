"""Matching API route package split by matching functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import queries as _queries
from .queries import *  # noqa: F403
from . import actions as _actions
from .actions import *  # noqa: F403
from . import pairs as _pairs
from .pairs import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _queries,
    _actions,
    _pairs,
)


class _MatchingCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split matching route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _MatchingCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
