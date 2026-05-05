"""Backup API route package split by backup functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import files as _files
from .files import *  # noqa: F403
from . import cleanup as _cleanup
from .cleanup import *  # noqa: F403
from . import jobs as _jobs
from .jobs import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _files,
    _cleanup,
    _jobs,
)


class _BackupCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split backup route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _BackupCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
