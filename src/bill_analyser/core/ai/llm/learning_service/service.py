"""Composable LLMLearningService facade class."""

from __future__ import annotations

from typing import Any

from ..provider import LLMProvider
from .generation import PromptGenerationMixin
from .normalization import PreviewInputNormalizationMixin
from .preview_recommendations import PreviewRecommendationsMixin
from .rate_limit import RateLimitMixin
from .rule_synthesis import RuleSynthesisMixin
from .session_analysis import SessionAnalysisMixin


class LLMLearningService(
    SessionAnalysisMixin,
    RuleSynthesisMixin,
    PreviewRecommendationsMixin,
    PromptGenerationMixin,
    PreviewInputNormalizationMixin,
    RateLimitMixin,
):
    """Orchestrates LLM analysis of transactions and rule induction."""

    def __init__(
        self,
        db: Any,
        provider: LLMProvider | None,
        advanced_settings: dict[str, Any] | None = None,
    ) -> None:
        self._db = db
        self._provider = provider
        self._advanced_settings = advanced_settings or {}
