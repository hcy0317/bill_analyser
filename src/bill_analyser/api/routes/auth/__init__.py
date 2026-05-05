"""Auth API route package split by security functional domain."""

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import cloud_settings as _cloud_settings
from .cloud_settings import *  # noqa: F403
from . import profile as _profile
from .profile import *  # noqa: F403
from . import external_auth as _external_auth
from .external_auth import *  # noqa: F403
from . import session as _session
from .session import *  # noqa: F403
from . import registration as _registration
from .registration import *  # noqa: F403
from . import account_recovery as _account_recovery
from .account_recovery import *  # noqa: F403
from . import tokens as _tokens
from .tokens import *  # noqa: F403
from . import step_up as _step_up
from .step_up import *  # noqa: F403
from . import two_factor_support as _two_factor_support
from .two_factor_support import *  # noqa: F403
from . import two_factor as _two_factor
from .two_factor import *  # noqa: F403
from . import two_factor_login as _two_factor_login
from .two_factor_login import *  # noqa: F403
from . import user_data as _user_data
from .user_data import *  # noqa: F403
from . import system as _system
from .system import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _cloud_settings,
    _profile,
    _external_auth,
    _session,
    _registration,
    _account_recovery,
    _tokens,
    _step_up,
    _two_factor_support,
    _two_factor,
    _two_factor_login,
    _user_data,
    _system,
)


class _AuthCompatModule(ModuleType):
    """Propagate package-level monkeypatches into split auth route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _AuthCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
