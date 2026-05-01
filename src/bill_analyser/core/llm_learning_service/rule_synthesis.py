"""Rule induction, rule synthesis, and candidate review behavior."""
# pylint: disable=duplicate-code,protected-access

from __future__ import annotations

import json
from typing import Any

import aiosqlite

from ..category_engine import CategoryEngine
from ..import_learning.model import MODEL_KEY
from ..llm_provider import LLMResponse
from .common import logger
from .limits import _MAX_RULE_SYNTHESIS_EVIDENCE, _MAX_RULE_SYNTHESIS_GROUPS


class RuleSynthesisMixin:
    """Generate category-rule candidates and apply candidate decisions."""

    # pylint: disable=too-many-locals
    async def induce_rules(
        self,
        user_id: int,
        category_id: int,
        sample_count: int = 10,
    ) -> list[dict[str, Any]]:
        """Fetch categorized samples, ask LLM to induce keyword patterns."""
        self._reserve_rate_limit_slots(user_id, 1)

        # Get category info
        category = await self._db.get_category_by_id(category_id, user_id=user_id)
        if not category:
            raise ValueError(f"Category {category_id} not found")

        category_name = f"{category.get('main_category', '')}/{category.get('sub_category', '')}"

        # Fetch sample transactions for this category
        conn = await self._db._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM bills WHERE user_id = ? "
            "AND main_category = ? AND sub_category = ? "
            "ORDER BY date DESC LIMIT ?",
            (
                user_id,
                category.get("main_category", ""),
                category.get("sub_category", ""),
                sample_count,
            ),
        ) as cursor:
            transactions = [dict(row) for row in await cursor.fetchall()]

        if not transactions:
            logger.info("No samples found for category %s (user %s)", category_name, user_id)
            return []

        prompt = self._build_rule_induction_prompt(category_name, transactions)

        try:
            response: LLMResponse = await self._generate(prompt)
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

    @staticmethod
    def _safe_json_object(raw_value: Any) -> dict[str, Any]:
        if isinstance(raw_value, dict):
            return dict(raw_value)
        if raw_value in (None, ""):
            return {}
        try:
            parsed = json.loads(str(raw_value))
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}
        return dict(parsed) if isinstance(parsed, dict) else {}

    @staticmethod
    def _build_category_path(main_category: str, sub_category: str) -> str:
        return "/".join(
            part.strip()
            for part in (str(main_category or ""), str(sub_category or ""))
            if part and str(part).strip()
        )

    async def _load_category_path_map(self, user_id: int) -> dict[int, str]:
        conn = await self._db._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            (
                "SELECT id, main_category, sub_category FROM categories "
                "WHERE user_id = ? ORDER BY main_category, sub_category, id"
            ),
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()

        category_paths: dict[int, str] = {}
        for row in rows:
            category_path = self._build_category_path(
                str(row["main_category"] or ""),
                str(row["sub_category"] or ""),
            )
            if category_path:
                category_paths[int(row["id"])] = category_path
        return category_paths

    async def _load_existing_category_rule_signatures(
        self,
        user_id: int,
    ) -> set[tuple[str, str]]:
        conn = await self._db._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            """
            SELECT c.main_category, c.sub_category, cr.rule_expression
            FROM category_rules cr
            JOIN categories c ON c.id = cr.category_id
            WHERE cr.user_id = ? AND c.user_id = ? AND cr.rule_expression IS NOT NULL
            """,
            (user_id, user_id),
        ) as cursor:
            rows = await cursor.fetchall()

        signatures: set[tuple[str, str]] = set()
        for row in rows:
            category_path = self._build_category_path(
                str(row["main_category"] or ""),
                str(row["sub_category"] or ""),
            )
            rule_expression = str(row["rule_expression"] or "").strip()
            if category_path and rule_expression:
                signatures.add((category_path, rule_expression))
        return signatures

    async def _load_pending_llm_candidate_signatures(
        self,
        user_id: int,
        *,
        candidate_type: str,
    ) -> set[tuple[str, str]]:
        candidates = await self._db.get_llm_candidates(
            user_id=user_id,
            status="pending",
            type=candidate_type,
            limit=500,
            offset=0,
        )
        signatures: set[tuple[str, str]] = set()
        for candidate in candidates:
            category_path = self._build_category_path(
                str(candidate.get("suggested_main_category") or ""),
                str(candidate.get("suggested_sub_category") or ""),
            )
            rule_expression = str(candidate.get("suggested_rule_expression") or "").strip()
            if category_path and rule_expression:
                signatures.add((category_path, rule_expression))
        return signatures

    def _is_valid_category_rule_expression(self, rule_expression: str) -> bool:
        compiled = CategoryEngine().keyword_matcher.compile_rule_expression(rule_expression)
        return not compiled.is_empty

    async def _load_rule_synthesis_knowledge_summary_pack(
        self,
        user_id: int,
    ) -> dict[str, Any]:
        category_paths = await self._load_category_path_map(user_id)
        learning_rules = await self._db.get_import_learning_rules(
            user_id=user_id,
            enabled_only=False,
            limit=200,
            offset=0,
        )
        learning_suggestions = await self._db.get_learning_suggestions(
            user_id=user_id,
            limit=200,
            offset=0,
        )

        conn = await self._db._get_connection()
        conn.row_factory = aiosqlite.Row

        concept_stats_map: dict[str, dict[str, int]] = {}
        async with conn.execute(
            """
            SELECT concept_key, accepted_count, rejected_count,
                   auto_applied_count, rollback_count
            FROM import_learning_concept_stats
            WHERE user_id = ?
            ORDER BY updated_at DESC, concept_key ASC
            """,
            (user_id,),
        ) as cursor:
            for row in await cursor.fetchall():
                concept_stats_map[str(row["concept_key"])] = {
                    "accepted_count": int(row["accepted_count"] or 0),
                    "rejected_count": int(row["rejected_count"] or 0),
                    "auto_applied_count": int(row["auto_applied_count"] or 0),
                    "rollback_count": int(row["rollback_count"] or 0),
                }

        active_model: dict[str, Any] | None = None
        async with conn.execute(
            """
            SELECT model_version, dataset_snapshot_id, metrics_json, updated_at
            FROM import_learning_model_registry
            WHERE user_id = ? AND model_key = ? AND status = 'active'
            ORDER BY updated_at DESC, id DESC
            LIMIT 1
            """,
            (user_id, MODEL_KEY),
        ) as cursor:
            active_model_row = await cursor.fetchone()
        if active_model_row:
            metrics = self._safe_json_object(active_model_row["metrics_json"])
            active_model = {
                "model_version": str(active_model_row["model_version"] or ""),
                "dataset_snapshot_id": int(active_model_row["dataset_snapshot_id"] or 0),
                "feature_schema_version": str(metrics.get("feature_schema_version") or ""),
                "policy_version": str(metrics.get("policy_version") or ""),
                "sample_count": int(metrics.get("sample_count") or 0),
                "updated_at": str(active_model_row["updated_at"] or ""),
            }

        category_bundles: dict[str, dict[str, Any]] = {}

        def _ensure_bundle(category_path: str, learned_type: str) -> dict[str, Any]:
            bundle = category_bundles.get(category_path)
            if bundle is None:
                bundle = {
                    "category_name": category_path,
                    "learned_type": learned_type,
                    "evidence": [],
                    "feedback_summary": {
                        "accepted_count": 0,
                        "rejected_count": 0,
                        "auto_applied_count": 0,
                        "rollback_count": 0,
                    },
                }
                category_bundles[category_path] = bundle
            elif learned_type and not bundle.get("learned_type"):
                bundle["learned_type"] = learned_type
            return bundle

        for rule in learning_rules:
            category_path = category_paths.get(int(rule.get("learned_category_id") or 0))
            if not category_path:
                continue
            bundle = _ensure_bundle(category_path, str(rule.get("learned_type") or ""))
            concept_key = f"rule:{int(rule.get('id') or 0)}"
            feedback = concept_stats_map.get(concept_key, {})
            bundle["evidence"].append({
                "source": "learning_rule",
                "source_id": concept_key,
                "match_type": str(rule.get("match_type") or ""),
                "match_value": str(rule.get("match_value") or ""),
                "match_features": self._safe_json_object(rule.get("match_features_json")),
                "sample_count": int(rule.get("applied_count") or 0),
                "enabled": bool(rule.get("enabled")),
                "feedback": feedback,
            })
            for feedback_key, value in feedback.items():
                bundle["feedback_summary"][feedback_key] += int(value or 0)

        for suggestion in learning_suggestions:
            if str(suggestion.get("status") or "") == "rejected":
                continue
            category_path = category_paths.get(int(suggestion.get("suggested_category_id") or 0))
            if not category_path:
                continue
            bundle = _ensure_bundle(category_path, str(suggestion.get("suggested_type") or ""))
            concept_key = f"suggestion:{int(suggestion.get('id') or 0)}"
            feedback = concept_stats_map.get(concept_key, {})
            bundle["evidence"].append({
                "source": "learning_suggestion",
                "source_id": concept_key,
                "match_type": str(suggestion.get("match_type") or ""),
                "match_value": str(suggestion.get("match_value") or ""),
                "match_features": self._safe_json_object(suggestion.get("match_features_json")),
                "sample_count": int(suggestion.get("sample_count") or 0),
                "status": str(suggestion.get("status") or ""),
                "feedback": feedback,
            })
            for feedback_key, value in feedback.items():
                bundle["feedback_summary"][feedback_key] += int(value or 0)

        ranked_categories = sorted(
            category_bundles.values(),
            key=lambda bundle: (
                int(bundle["feedback_summary"]["accepted_count"])
                + int(bundle["feedback_summary"]["auto_applied_count"]) * 2,
                len(bundle["evidence"]),
                bundle["category_name"],
            ),
            reverse=True,
        )[:_MAX_RULE_SYNTHESIS_GROUPS]

        for bundle in ranked_categories:
            bundle["evidence"].sort(
                key=lambda evidence: (
                    int((evidence.get("feedback") or {}).get("accepted_count") or 0)
                    + int((evidence.get("feedback") or {}).get("auto_applied_count") or 0) * 2,
                    int(evidence.get("sample_count") or 0),
                    str(evidence.get("match_value") or ""),
                ),
                reverse=True,
            )
            bundle["evidence"] = bundle["evidence"][:_MAX_RULE_SYNTHESIS_EVIDENCE]

        return {
            "knowledge_summary_version": "a6-rule-synthesis-v1",
            "existing_categories": sorted(category_paths.values()),
            "active_model": active_model,
            "recent_llm_feedback": await self._load_memory_context(user_id),
            "categories": ranked_categories,
        }

    async def synthesize_rule_candidates(
        self,
        user_id: int,
        limit: int = 8,
    ) -> dict[str, Any]:
        """Synthesize rule candidates from durable learning and memory signals."""
        normalized_limit = self._normalize_limit(limit)
        knowledge_summary_pack = await self._load_rule_synthesis_knowledge_summary_pack(user_id)
        if not knowledge_summary_pack["categories"]:
            return {
                "mode": "rule_synthesis",
                "knowledge_summary_pack": knowledge_summary_pack,
                "candidates_created": 0,
                "candidates": [],
            }

        self._reserve_rate_limit_slots(user_id, 1)
        prompt = self._build_rule_expression_synthesis_prompt(
            knowledge_summary_pack,
            max_candidates=min(normalized_limit, _MAX_RULE_SYNTHESIS_GROUPS),
        )

        try:
            response: LLMResponse = await self._generate(prompt)
        except Exception as exc:
            logger.error("LLM rule synthesis request failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        raw_candidates = self._parse_rule_induction_response(response.content)
        if not raw_candidates:
            return {
                "mode": "rule_synthesis",
                "knowledge_summary_pack": knowledge_summary_pack,
                "candidates_created": 0,
                "candidates": [],
            }

        valid_category_paths = set(knowledge_summary_pack["existing_categories"])
        existing_signatures = await self._load_existing_category_rule_signatures(user_id)
        existing_signatures.update(
            await self._load_pending_llm_candidate_signatures(
                user_id,
                candidate_type="rule_synthesis",
            )
        )

        created_candidates: list[dict[str, Any]] = []
        created_signatures: set[tuple[str, str]] = set()

        for candidate in raw_candidates:
            main_category = str(
                candidate.get("suggested_main_category")
                or candidate.get("main_category")
                or ""
            ).strip()
            sub_category = str(
                candidate.get("suggested_sub_category")
                or candidate.get("sub_category")
                or ""
            ).strip()
            rule_expression = str(candidate.get("rule_expression") or "").strip()
            if not main_category or not rule_expression:
                continue

            category_path = self._build_category_path(main_category, sub_category)
            if not category_path or category_path not in valid_category_paths:
                continue
            if not self._is_valid_category_rule_expression(rule_expression):
                continue

            signature = (category_path, rule_expression)
            if signature in existing_signatures or signature in created_signatures:
                continue

            reason = str(
                candidate.get("reason")
                or candidate.get("explanation")
                or ""
            ).strip()
            rule_name = str(candidate.get("rule_name") or "").strip()
            llm_response_payload = {
                "rule_name": rule_name,
                "reason": reason,
                "raw_response": response.raw_response,
                "knowledge_summary_version": knowledge_summary_pack["knowledge_summary_version"],
            }
            candidate_id = await self._db.create_llm_candidate(
                user_id=user_id,
                type="rule_synthesis",
                source_bill_ids=[],
                suggested_main_category=main_category,
                suggested_sub_category=sub_category or None,
                suggested_rule_expression=rule_expression,
                confidence=float(candidate.get("confidence", 0.0) or 0.0),
                llm_provider=response.provider,
                llm_model=response.model,
                llm_response_raw=json.dumps(llm_response_payload, ensure_ascii=False),
            )
            created_candidates.append({
                "id": candidate_id,
                "type": "rule_synthesis",
                "rule_name": rule_name,
                "rule_expression": rule_expression,
                "confidence": float(candidate.get("confidence", 0.0) or 0.0),
                "category_name": category_path,
                "reason": reason,
                "status": "pending",
            })
            created_signatures.add(signature)
            if len(created_candidates) >= normalized_limit:
                break

        logger.info(
            "LLM rule synthesis produced %d candidates for user %s",
            len(created_candidates),
            user_id,
        )
        return {
            "mode": "rule_synthesis",
            "knowledge_summary_pack": knowledge_summary_pack,
            "candidates_created": len(created_candidates),
            "candidates": created_candidates,
        }

    async def accept_candidate(self, candidate_id: int, user_id: int) -> dict[str, Any]:
        """Mark candidate as accepted.

        Optionally create a category_rule if it's a rule induction.
        """
        candidate = await self._db.get_llm_candidate_by_id(candidate_id, user_id=user_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")

        await self._db.update_llm_candidate_status(
            candidate_id,
            "accepted",
            user_id=user_id,
        )

        result: dict[str, Any] = {"candidate_id": candidate_id, "status": "accepted"}

        # If rule induction/synthesis, create category_rule automatically
        if (
            str(candidate.get("type") or "").startswith("rule_")
            and candidate.get("suggested_rule_expression")
        ):
            main_cat = candidate.get("suggested_main_category", "")
            sub_cat = candidate.get("suggested_sub_category", "")

            # Find matching category
            conn = await self._db._get_connection()
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                (
                    "SELECT id FROM categories "
                    "WHERE user_id = ? AND main_category = ? AND sub_category = ?"
                ),
                (user_id, main_cat, sub_cat),
            ) as cursor:
                cat_row = await cursor.fetchone()

            if cat_row:
                rule_data = {
                    "category_id": cat_row["id"],
                    "name": (
                        f"LLM synthesized: {main_cat}/{sub_cat}"
                        if candidate.get("type") == "rule_synthesis"
                        else f"LLM induced: {main_cat}/{sub_cat}"
                    ),
                    "priority": 50,
                    "rule_expression": candidate["suggested_rule_expression"],
                    "regex_enabled": False,
                    "enabled": True,
                }
                rule_id = await self._db.create_category_rule(rule_data, user_id=user_id)
                result["created_rule_id"] = rule_id

        return result

    async def reject_candidate(self, candidate_id: int, user_id: int) -> bool:
        """Mark candidate as rejected."""
        candidate = await self._db.get_llm_candidate_by_id(candidate_id, user_id=user_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")
        return await self._db.update_llm_candidate_status(
            candidate_id,
            "rejected",
            user_id=user_id,
        )
