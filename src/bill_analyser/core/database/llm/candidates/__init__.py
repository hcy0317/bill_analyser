"""LLM candidates persistence helpers for the split database facade."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .crud import LLMCandidateCrudMixin
from .feedback import LLMCandidateFeedbackMixin
from .memory import LLMCandidateMemoryEventsMixin
from .preview_apply import LLMCandidatePreviewApplyMixin
from .preview_review import LLMCandidatePreviewReviewMixin


class DatabaseLLMCandidatesMixin(
    LLMCandidatePreviewApplyMixin,
    LLMCandidatePreviewReviewMixin,
    LLMCandidateMemoryEventsMixin,
    LLMCandidateCrudMixin,
    LLMCandidateFeedbackMixin,
    DatabaseFacadeBase,
):
    """LLM candidate CRUD helpers for classification and rule induction suggestions."""


__all__ = ["DatabaseLLMCandidatesMixin"]
