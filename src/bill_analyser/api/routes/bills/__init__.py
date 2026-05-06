"""Bills API route package split by functional domain."""
# pylint: disable=too-few-public-methods

from __future__ import annotations

import sys
from types import ModuleType

from . import support as _support
from .support import *  # noqa: F403
from . import import_detection as _import_detection
from .import_detection import *  # noqa: F403
from . import import_rows as _import_rows
from .import_rows import *  # noqa: F403
from . import import_mapping as _import_mapping
from .import_mapping import *  # noqa: F403
from . import import_review as _import_review
from .import_review import *  # noqa: F403
from . import crud_query as _crud_query
from .crud_query import *  # noqa: F403
from . import crud_prepare as _crud_prepare
from .crud_prepare import *  # noqa: F403
from . import crud_create_update as _crud_create_update
from .crud_create_update import *  # noqa: F403
from . import import_legacy as _import_legacy
from .import_legacy import *  # noqa: F403
from . import import_learning as _import_learning
from .import_learning import *  # noqa: F403
from . import import_config as _import_config
from .import_config import *  # noqa: F403
from . import category_actions as _category_actions
from .category_actions import *  # noqa: F403
from . import parse_import as _parse_import
from .parse_import import *  # noqa: F403
from . import reconciliation as _reconciliation
from .reconciliation import *  # noqa: F403
from . import v2_pipeline as _v2_pipeline
from .v2_pipeline import *  # noqa: F403
from . import v2_sessions as _v2_sessions
from .v2_sessions import *  # noqa: F403
from . import v2_preview_actions as _v2_preview_actions
from .v2_preview_actions import *  # noqa: F403

_COMPAT_MODULES = (
    _support,
    _import_detection,
    _import_rows,
    _import_mapping,
    _import_review,
    _crud_query,
    _crud_prepare,
    _crud_create_update,
    _import_legacy,
    _import_learning,
    _import_config,
    _category_actions,
    _parse_import,
    _reconciliation,
    _v2_pipeline,
    _v2_sessions,
    _v2_preview_actions,
)


class _BillsCompatModule(ModuleType):
    """Propagate legacy package-level monkeypatches into split route modules."""

    def __setattr__(self, name: str, value):
        super().__setattr__(name, value)
        for module in _COMPAT_MODULES:
            if hasattr(module, name):
                setattr(module, name, value)


sys.modules[__name__].__class__ = _BillsCompatModule

__all__ = [name for name in globals() if not name.startswith("__") and name != "ModuleType"]
