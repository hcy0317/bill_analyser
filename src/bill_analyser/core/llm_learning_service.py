"""LLM Learning Service — orchestrates LLM-driven classification and rule induction."""

from __future__ import annotations

import json
import time
from typing import Any

from ..utils.logger import get_logger
from .llm_prompts import SYSTEM_PROMPT, build_classification_prompt, build_rule_induction_prompt
from .llm_provider import LLMProvider, LLMResponse

logger = get_logger("LLMLearningService")

# Simple in-memory rate limiter
_RATE_LIMIT_WINDOW = 60  # seconds
_RATE_LIMIT_MAX_CALLS = 10


class LLMLearningService:
    """Orchestrates LLM analysis of transactions and rule induction."""

    def __init__(self, db: Any, provider: LLMProvider) -> None:
        self._db = db
        self._provider = provider
        self._call_timestamps: list[float] = []

    def _check_rate_limit(self) -> None:
        """Simple in-memory sliding-window rate limiter."""
        now = time.time()
        self._call_timestamps = [
            ts for ts in self._call_timestamps if now - ts < _RATE_LIMIT_WINDOW
        ]
        if len(self._call_timestamps) >= _RATE_LIMIT_MAX_CALLS:
            raise RuntimeError(
                f"Rate limit exceeded: max {_RATE_LIMIT_MAX_CALLS} calls per {_RATE_LIMIT_WINDOW}s"
            )
        self._call_timestamps.append(now)

    async def analyze_transactions(
        self,
        user_id: int,
        bill_ids: list[int] | None = None,
        limit: int = 20,
    ) -> list[dict[str, Any]]:
        """Fetch uncategorized bills, send to LLM, create candidate records."""
        self._check_rate_limit()

        # Fetch uncategorized transactions
        if bill_ids:
            transactions = []
            for bid in bill_ids[:limit]:
                bill = await self._db.get_bill_by_id(bid, user_id=user_id)
                if bill:
                    transactions.append(bill)
        else:
            conn = await self._db._get_connection()
            import aiosqlite

            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? "
                "AND (main_category IS NULL OR main_category = '' OR main_category = '未分类') "
                "ORDER BY date DESC LIMIT ?",
                (user_id, limit),
            ) as cursor:
                transactions = [dict(row) for row in await cursor.fetchall()]

        if not transactions:
            logger.info("No uncategorized transactions found for user %s", user_id)
            return []

        prompt = build_classification_prompt(transactions)

        try:
            response: LLMResponse = await self._provider.generate(
                prompt=prompt,
                system_prompt=SYSTEM_PROMPT,
                temperature=0.3,
                max_tokens=4096,
            )
        except Exception as exc:
            logger.error("LLM classification request failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        suggestions = self._parse_classification_response(response.content)
        candidates: list[dict[str, Any]] = []

        for suggestion in suggestions:
            bill_id = suggestion.get("bill_id")
            main_cat = suggestion.get("suggested_main_category", "")
            sub_cat = suggestion.get("suggested_sub_category", "")
            confidence = float(suggestion.get("confidence", 0.0))

            source_ids = [bill_id] if bill_id else []

            candidate_id = await self._db.create_llm_candidate(
                user_id=user_id,
                type="classification",
                source_bill_ids=source_ids,
                suggested_main_category=main_cat,
                suggested_sub_category=sub_cat,
                suggested_rule_expression=None,
                confidence=confidence,
                llm_provider=response.provider,
                llm_model=response.model,
                llm_response_raw=json.dumps(response.raw_response, ensure_ascii=False),
            )
            candidates.append({
                "id": candidate_id,
                "bill_id": bill_id,
                "suggested_main_category": main_cat,
                "suggested_sub_category": sub_cat,
                "confidence": confidence,
            })

        logger.info(
            "LLM classification produced %d candidates for user %s",
            len(candidates),
            user_id,
        )
        return candidates

    async def induce_rules(
        self,
        user_id: int,
        category_id: int,
        sample_count: int = 10,
    ) -> list[dict[str, Any]]:
        """Fetch categorized samples, ask LLM to induce keyword patterns."""
        self._check_rate_limit()

        # Get category info
        category = await self._db.get_category_by_id(category_id, user_id=user_id)
        if not category:
            raise ValueError(f"Category {category_id} not found")

        category_name = f"{category.get('main_category', '')}/{category.get('sub_category', '')}"

        # Fetch sample transactions for this category
        conn = await self._db._get_connection()
        import aiosqlite

        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM bills WHERE user_id = ? "
            "AND main_category = ? AND sub_category = ? "
            "ORDER BY date DESC LIMIT ?",
            (user_id, category.get("main_category", ""), category.get("sub_category", ""), sample_count),
        ) as cursor:
            transactions = [dict(row) for row in await cursor.fetchall()]

        if not transactions:
            logger.info("No samples found for category %s (user %s)", category_name, user_id)
            return []

        prompt = build_rule_induction_prompt(category_name, transactions)

        try:
            response: LLMResponse = await self._provider.generate(
                prompt=prompt,
                system_prompt=SYSTEM_PROMPT,
                temperature=0.3,
                max_tokens=4096,
            )
        except Exception as exc:
            logger.error("LLM rule induction request failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        rules = self._parse_rule_induction_response(response.content)
        candidates: list[dict[str, Any]] = []

        source_bill_ids = [t.get("id") for t in transactions if t.get("id")]

        for rule in rules:
            rule_expression = rule.get("rule_expression", "")
            confidence = float(rule.get("confidence", 0.0))
            rule_name = rule.get("rule_name", "")

            candidate_id = await self._db.create_llm_candidate(
                user_id=user_id,
                type="rule_induction",
                source_bill_ids=source_bill_ids,
                suggested_main_category=category.get("main_category"),
                suggested_sub_category=category.get("sub_category"),
                suggested_rule_expression=rule_expression,
                confidence=confidence,
                llm_provider=response.provider,
                llm_model=response.model,
                llm_response_raw=json.dumps(response.raw_response, ensure_ascii=False),
            )
            candidates.append({
                "id": candidate_id,
                "rule_name": rule_name,
                "rule_expression": rule_expression,
                "confidence": confidence,
                "explanation": rule.get("explanation", ""),
            })

        logger.info(
            "LLM rule induction produced %d candidates for category '%s' (user %s)",
            len(candidates),
            category_name,
            user_id,
        )
        return candidates

    async def accept_candidate(self, candidate_id: int) -> dict[str, Any]:
        """Mark candidate as accepted. Optionally create a category_rule if it's a rule induction."""
        candidate = await self._db.get_llm_candidate_by_id(candidate_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")

        await self._db.update_llm_candidate_status(candidate_id, "accepted")

        result: dict[str, Any] = {"candidate_id": candidate_id, "status": "accepted"}

        # If rule induction, create category_rule automatically
        if candidate.get("type") == "rule_induction" and candidate.get("suggested_rule_expression"):
            user_id = candidate["user_id"]
            main_cat = candidate.get("suggested_main_category", "")
            sub_cat = candidate.get("suggested_sub_category", "")

            # Find matching category
            conn = await self._db._get_connection()
            import aiosqlite

            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT id FROM categories WHERE user_id = ? AND main_category = ? AND sub_category = ?",
                (user_id, main_cat, sub_cat),
            ) as cursor:
                cat_row = await cursor.fetchone()

            if cat_row:
                rule_data = {
                    "category_id": cat_row["id"],
                    "name": f"LLM induced: {main_cat}/{sub_cat}",
                    "priority": 50,
                    "rule_expression": candidate["suggested_rule_expression"],
                    "regex_enabled": False,
                    "enabled": True,
                }
                rule_id = await self._db.create_category_rule(rule_data, user_id=user_id)
                result["created_rule_id"] = rule_id

        return result

    async def reject_candidate(self, candidate_id: int) -> bool:
        """Mark candidate as rejected."""
        candidate = await self._db.get_llm_candidate_by_id(candidate_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")
        return await self._db.update_llm_candidate_status(candidate_id, "rejected")

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
