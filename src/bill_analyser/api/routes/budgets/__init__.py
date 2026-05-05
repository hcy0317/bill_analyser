"""Budgets API route package split by budget functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import crud as _crud
from .crud import *  # noqa: F403
from . import execution as _execution
from .execution import *  # noqa: F403
from . import history as _history
from .history import *  # noqa: F403
from . import io as _io
from .io import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _crud,
    _execution,
    _history,
    _io,
)


class _BudgetsCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split budgets route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _BudgetsCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
