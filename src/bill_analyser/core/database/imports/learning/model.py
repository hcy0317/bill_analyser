"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.import_learning.features import (
    FEATURE_SCHEMA_VERSION,
    build_label_confirmation_counts,
    prepare_training_samples,
)
from bill_analyser.core.import_learning.model import MODEL_KEY, train_dual_head_model
from bill_analyser.core.import_learning.policy import POLICY_VERSION

_UNSET: Any = object()


class ImportLearningModelMixin:
    """Manage active import-learning model snapshots and refreshes."""

    @log_method
    async def get_active_import_learning_model(self, user_id: int = 1) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            """
                SELECT *
                FROM import_learning_model_registry
                WHERE user_id = ? AND model_key = ? AND status = 'active'
                ORDER BY updated_at DESC, id DESC
                LIMIT 1
                """,
            (user_id, MODEL_KEY),
        ) as cursor:
            row = await cursor.fetchone()
        if not row:
            return None

        model_row = dict(row)
        try:
            metrics_payload = json.loads(model_row.get("metrics_json") or "{}")
        except TypeError, ValueError, json.JSONDecodeError:
            metrics_payload = {}
        model_row["metrics"] = metrics_payload if isinstance(metrics_payload, dict) else {}
        return model_row

    @log_method
    async def get_import_learning_model_suppressed_preview_ids(
        self,
        session_id: str,
        *,
        user_id: int = 1,
    ) -> set[int]:
        """Return preview ids where prior user feedback should block model auto-suggestions."""
        normalized_session_id = str(session_id or "").strip()
        if not normalized_session_id:
            return set()

        conn = await self._get_connection()
        async with conn.execute(
            """
                SELECT DISTINCT preview_id
                FROM import_learning_feedback_events
                WHERE user_id = ?
                  AND session_id = ?
                  AND preview_id IS NOT NULL
                  AND event_type IN ('model_preview_reject', 'preview_rollback')
                """,
            (user_id, normalized_session_id),
        ) as cursor:
            rows = await cursor.fetchall()
        suppressed_ids: set[int] = set()
        for row in rows:
            try:
                suppressed_ids.add(int(row["preview_id"]))
            except TypeError, ValueError:
                continue
        return suppressed_ids

    @log_method
    async def refresh_import_learning_model(self, user_id: int = 1) -> dict[str, Any]:
        """Create a versioned dataset snapshot and refresh the active trainable model."""
        conn = await self._get_connection()
        async with conn.execute(
            """
                SELECT *
                FROM import_learning_corpus_samples
                WHERE user_id = ?
                ORDER BY updated_at ASC, id ASC
                """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()

        corpus_rows = [dict(row) for row in rows]
        samples = prepare_training_samples(corpus_rows)
        confirmation_counts = build_label_confirmation_counts(samples)
        label_counts: dict[str, int] = {}
        for sample in samples:
            label_counts[sample.semantic_label] = label_counts.get(sample.semantic_label, 0) + 1

        now = utc_now_iso()
        snapshot_payload = {
            "feature_schema_version": FEATURE_SCHEMA_VERSION,
            "policy_version": POLICY_VERSION,
            "sample_ids": [sample.sample_id for sample in samples],
            "semantic_label_counts": label_counts,
            "joint_label_confirmation_counts": confirmation_counts,
        }
        training_result = train_dual_head_model(samples)
        snapshot_status = "ready" if training_result.trainable else "insufficient"
        cursor = await conn.execute(
            """
                INSERT INTO import_learning_dataset_snapshots (
                    user_id, name, corpus_sample_count, filters_json,
                    status, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
            (
                user_id,
                f"{MODEL_KEY}:{now}",
                len(samples),
                json.dumps(snapshot_payload, ensure_ascii=False, sort_keys=True),
                snapshot_status,
                now,
                now,
            ),
        )
        dataset_snapshot_id = int(cursor.lastrowid or 0)

        if not training_result.trainable or not training_result.parameters:
            await conn.commit()
            return {
                "trained": False,
                "reason": training_result.reason,
                "dataset_snapshot_id": dataset_snapshot_id,
                "sample_count": len(samples),
            }

        model_version = f"v{dataset_snapshot_id}"
        model_payload = {
            "feature_schema_version": FEATURE_SCHEMA_VERSION,
            "policy_version": POLICY_VERSION,
            "parameter_ref": "metrics_json.model_parameters",
            "model_parameters": training_result.parameters,
            "training_metrics": training_result.metrics,
            "joint_label_confirmation_counts": confirmation_counts,
        }
        await conn.execute(
            """
                UPDATE import_learning_model_registry
                SET status = 'archived', updated_at = ?
                WHERE user_id = ? AND model_key = ? AND status = 'active'
                """,
            (now, user_id, MODEL_KEY),
        )
        await conn.execute(
            """
                INSERT INTO import_learning_model_registry (
                    user_id, model_key, model_version, dataset_snapshot_id,
                    status, metrics_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, 'active', ?, ?, ?)
                """,
            (
                user_id,
                MODEL_KEY,
                model_version,
                dataset_snapshot_id,
                json.dumps(model_payload, ensure_ascii=False, sort_keys=True),
                now,
                now,
            ),
        )
        await conn.commit()
        return {
            "trained": True,
            "reason": training_result.reason,
            "dataset_snapshot_id": dataset_snapshot_id,
            "model_version": model_version,
            "sample_count": len(samples),
        }

    @log_method
    async def list_import_learning_suggestions_for_session(
        self,
        session_id: str,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        previews = await self.get_preview_by_session(session_id, user_id=user_id)
        preview_map = {int(preview["id"]): preview for preview in previews if preview.get("id")}
        samples = await self.get_import_annotation_samples(session_id, user_id=user_id)
        if not preview_map or not samples:
            return []

        existing_rules = await self.get_import_learning_rules(user_id=user_id, enabled_only=False, limit=None)
        existing_composite_hashes = {
            str(rule.get("normalized_match_value") or "")
            for rule in existing_rules
            if str(rule.get("match_type") or "") == "composite" and str(rule.get("normalized_match_value") or "")
        }

        pending_suggestions: dict[str, dict[str, Any]] = {}
        conflicted_hashes: set[str] = set()
        for sample in samples:
            preview_id = int(sample.get("preview_id", 0) or 0)
            preview = preview_map.get(preview_id)
            if not preview:
                continue

            composite_hash = self.build_composite_match_hash(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            match_features = self.build_composite_match_features(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            if not composite_hash or not match_features:
                continue
            if composite_hash in existing_composite_hashes or composite_hash in conflicted_hashes:
                continue

            suggestion_signature = self._build_import_learning_suggestion_signature(sample, preview)
            existing_suggestion = pending_suggestions.get(composite_hash)
            if existing_suggestion is None:
                pending_suggestions[composite_hash] = self._build_import_learning_suggestion_payload(
                    session_id=session_id,
                    sample=sample,
                    preview=preview,
                    composite_hash=composite_hash,
                    match_features=match_features,
                )
                continue

            if existing_suggestion["signature"] != suggestion_signature:
                conflicted_hashes.add(composite_hash)
                pending_suggestions.pop(composite_hash, None)
                continue

            if preview_id not in existing_suggestion["source_preview_ids"]:
                existing_suggestion["source_preview_ids"].append(preview_id)
                existing_suggestion["sample_count"] += 1
            sample_annotation_time = str(sample.get("updated_at") or sample.get("created_at") or "")
            if sample_annotation_time > str(existing_suggestion.get("last_annotation_at") or ""):
                existing_suggestion["last_annotation_at"] = sample_annotation_time

        suggestions = list(pending_suggestions.values())
        for suggestion in suggestions:
            suggestion["source_preview_ids"] = sorted(
                int(preview_id) for preview_id in suggestion.get("source_preview_ids") or []
            )
            suggestion.pop("signature", None)

        suggestions.sort(
            key=lambda suggestion: (
                int(suggestion.get("sample_count") or 0),
                str(suggestion.get("last_annotation_at") or ""),
                int((suggestion.get("source_preview_ids") or [0])[-1] or 0),
                str(suggestion.get("match_value") or ""),
            ),
            reverse=True,
        )
        return suggestions
