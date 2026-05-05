"""Import-preview editing, confirmation, and temporary staging helpers."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .base import ImportPreviewBaseMixin
from .inserts import ImportPreviewInsertsMixin
from .reads import ImportPreviewReadsMixin
from .decisions import ImportPreviewDecisionsMixin
from .updates import ImportPreviewUpdatesMixin
from .confirmation import ImportPreviewConfirmationMixin


class DatabaseImportPreviewMixin(  # pylint: disable=too-many-ancestors
    ImportPreviewBaseMixin,
    ImportPreviewInsertsMixin,
    ImportPreviewReadsMixin,
    ImportPreviewDecisionsMixin,
    ImportPreviewUpdatesMixin,
    ImportPreviewConfirmationMixin,
    DatabaseFacadeBase,
):
    """Import preview facade composed from persistence parts."""


__all__ = [
    "DatabaseImportPreviewMixin",
]
