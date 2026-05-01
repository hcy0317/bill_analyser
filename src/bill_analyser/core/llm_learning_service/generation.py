"""Prompt generation, provider calls, and LLM response parsing."""
# pylint: disable=too-few-public-methods

from __future__ import annotations

import json
from typing import Any

from ..llm_prompts import (
    SYSTEM_PROMPT,
    build_classification_prompt,
    build_rule_induction_prompt,
    build_rule_expression_synthesis_prompt,
    render_prompt_template,
)
from ..llm_provider import LLMResponse
from .common import logger
from .limits import _DEFAULT_MAX_TOKENS, _DEFAULT_TEMPERATURE


class PromptGenerationMixin:
    """Build prompts from instance-scoped advanced settings and call the provider."""

    def _system_prompt(self) -> str:
        return str(self._advanced_settings.get("system_prompt") or SYSTEM_PROMPT)

    def _temperature(self) -> float:
        return float(self._advanced_settings.get("temperature", _DEFAULT_TEMPERATURE))

    def _max_tokens(self) -> int:
        return int(self._advanced_settings.get("max_tokens", _DEFAULT_MAX_TOKENS))

    def _reasoning_depth(self) -> str:
        return str(self._advanced_settings.get("reasoning_depth") or "")

    def _build_classification_prompt(self, transactions: list[dict[str, Any]]) -> str:
        default_prompt = build_classification_prompt(transactions)
        return render_prompt_template(
            str(self._advanced_settings.get("classification_prompt_template") or ""),
            default_prompt=default_prompt,
            transactions=transactions,
        )

    def _build_rule_induction_prompt(
        self,
        category_name: str,
        transactions: list[dict[str, Any]],
    ) -> str:
        default_prompt = build_rule_induction_prompt(category_name, transactions)
        return render_prompt_template(
            str(self._advanced_settings.get("rule_prompt_template") or ""),
            default_prompt=default_prompt,
            transactions=transactions,
            category_name=category_name,
        )

    def _build_rule_expression_synthesis_prompt(
        self,
        knowledge_summary_pack: dict[str, Any],
        *,
        max_candidates: int,
    ) -> str:
        return build_rule_expression_synthesis_prompt(
            knowledge_summary_pack,
            max_candidates=max_candidates,
        )

    async def _generate(self, prompt: str) -> LLMResponse:
        if self._provider is None:
            raise RuntimeError("LLM provider is not available")
        return await self._provider.generate(
            prompt=prompt,
            system_prompt=self._system_prompt(),
            temperature=self._temperature(),
            max_tokens=self._max_tokens(),
            reasoning_depth=self._reasoning_depth(),
        )

    @staticmethod
    def _parse_classification_response(content: str) -> list[dict[str, Any]]:
        """Parse LLM JSON response for classification suggestions."""
        try:
            # Strip possible markdown code fences
            cleaned = content.strip()
            if cleaned.startswith("```"):
                lines = cleaned.split("\n")
                lines = [l for l in lines if not l.strip().startswith("```")]
                cleaned = "\n".join(lines)
            return json.loads(cleaned)
        except (json.JSONDecodeError, TypeError) as exc:
            logger.warning("Failed to parse LLM classification response: %s", exc)
            return []

    @staticmethod
    def _parse_rule_induction_response(content: str) -> list[dict[str, Any]]:
        """Parse LLM JSON response for rule induction suggestions."""
        try:
            cleaned = content.strip()
            if cleaned.startswith("```"):
                lines = cleaned.split("\n")
                lines = [l for l in lines if not l.strip().startswith("```")]
                cleaned = "\n".join(lines)
            return json.loads(cleaned)
        except (json.JSONDecodeError, TypeError) as exc:
            logger.warning("Failed to parse LLM rule induction response: %s", exc)
            return []
