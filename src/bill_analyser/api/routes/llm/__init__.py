"""LLM API route package split by LLM functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import analysis as _analysis
from .analysis import *  # noqa: F403
from . import candidates as _candidates
from .candidates import *  # noqa: F403
from . import configs as _configs
from .configs import *  # noqa: F403
from . import preview as _preview
from .preview import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _analysis,
    _candidates,
    _configs,
    _preview,
)


class _LlmCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split llm route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _LlmCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
