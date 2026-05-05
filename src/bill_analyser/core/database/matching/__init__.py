"""Historical bill matching persistence helpers."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .base import MatchingBaseMixin
from .settings import MatchingSettingsMixin
from .feedback import MatchingFeedbackMixin
from .suppressions import MatchingSuppressionsMixin
from .candidates import MatchingCandidatesMixin
from .learning import MatchingLearningMixin
from .manual_pairs import MatchingManualPairsMixin


class DatabaseMatchingMixin(  # pylint: disable=too-many-ancestors
    MatchingBaseMixin,
    MatchingSettingsMixin,
    MatchingFeedbackMixin,
    MatchingSuppressionsMixin,
    MatchingCandidatesMixin,
    MatchingLearningMixin,
    MatchingManualPairsMixin,
    DatabaseFacadeBase,
):
    """Compatibility facade composed from db_matching persistence parts."""


__all__ = [
    "DatabaseMatchingMixin",
]
