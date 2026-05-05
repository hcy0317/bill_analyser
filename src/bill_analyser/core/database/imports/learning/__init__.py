"""Import-learning helpers for composite matches, annotations, and rules."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .base import ImportLearningBaseMixin
from .events import ImportLearningEventsMixin
from .corpus import ImportLearningCorpusMixin
from .model import ImportLearningModelMixin
from .rules import ImportLearningRulesMixin
from .suggestion_center import ImportLearningSuggestionCenterMixin


class DatabaseImportLearningMixin(  # pylint: disable=too-many-ancestors
    ImportLearningBaseMixin,
    ImportLearningEventsMixin,
    ImportLearningCorpusMixin,
    ImportLearningModelMixin,
    ImportLearningRulesMixin,
    ImportLearningSuggestionCenterMixin,
    DatabaseFacadeBase,
):
    """Import learning facade composed from persistence parts."""


__all__ = [
    "DatabaseImportLearningMixin",
]
