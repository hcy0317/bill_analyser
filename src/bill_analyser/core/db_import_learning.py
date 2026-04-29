"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso
from .import_learning.features import (
    FEATURE_SCHEMA_VERSION,
    build_label_confirmation_counts,
    prepare_training_samples,
)
from .import_learning.model import MODEL_KEY, train_dual_head_model
from .import_learning.policy import POLICY_VERSION


_UNSET: Any = object()


class DatabaseImportLearningMixin(DatabaseFacadeBase):
    """Import annotation, long-term learning rule, and composite-match helpers."""

    @staticmethod
    def _normalize_import_learning_suggestion_id(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized_value = int(raw_value)
        except (TypeError, ValueError):
            return None
        return normalized_value if normalized_value > 0 else None

    @classmethod
    def _build_import_learning_suggestion_signature(
        cls,
        sample: dict[str, Any],
        preview: dict[str, Any],
    ) -> tuple[str, int | None, int | None, int | None]:
        return (
            str(sample.get("annotated_type") or preview.get("preview_type") or "").strip(),
            cls._normalize_import_learning_suggestion_id(sample.get("annotated_category_id")),
            cls._normalize_import_learning_suggestion_id(sample.get("annotated_source_account_id")),
            cls._normalize_import_learning_suggestion_id(sample.get("annotated_destination_account_id")),
        )

    @classmethod
    def _build_import_learning_suggestion_payload(
        cls,
        *,
        session_id: str,
        sample: dict[str, Any],
        preview: dict[str, Any],
        composite_hash: str,
        match_features: dict[str, str],
    ) -> dict[str, Any]:
        preview_id = int(sample.get("preview_id", 0) or 0)
        signature = cls._build_import_learning_suggestion_signature(sample, preview)
        last_annotation_at = str(sample.get("updated_at") or sample.get("created_at") or "")
        return {
            "match_type": "composite",
            "match_value": composite_hash,
            "normalized_match_value": composite_hash,
            "learned_type": signature[0],
            "learned_category_id": signature[1],
            "learned_source_account_id": signature[2],
            "learned_destination_account_id": signature[3],
            "source_session_id": session_id,
            "source_preview_ids": [preview_id],
            "sample_count": 1,
            "parser_id": str(preview.get("preview_parser_id") or "").strip(),
            "composite_match_hash": composite_hash,
            "match_features": dict(match_features),
            "signature": signature,
            "last_annotation_at": last_annotation_at,
        }

    @staticmethod
    def _normalize_import_learning_text(raw_value: Any) -> str:
        if raw_value is None:
            return ""
        text = str(raw_value).strip().lower()
        if not text:
            return ""
        parts = [part.strip() for part in text.split("|") if part.strip()]
        if parts:
            text = " | ".join(parts)
        return " ".join(text.split())

    @classmethod
    def build_composite_match_hash(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> str | None:
        features = cls.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        if not features:
            return None

        key_aliases = {
            "parser_id": "p",
            "counterparty": "c",
            "description": "d",
            "payment_method": "m",
        }
        return "|".join(f"{key_aliases[key]}={value}" for key, value in sorted(features.items()))

    @classmethod
    def build_composite_match_features(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> dict[str, str] | None:
        norm = cls._normalize_import_learning_text
        features = {
            "parser_id": norm(parser_id),
            "counterparty": norm(counterparty),
            "description": norm(description),
            "payment_method": norm(payment_method),
        }
        non_empty = {key: value for key, value in features.items() if value}
        if len(non_empty) < 2:
            return None
        return non_empty

    @classmethod
    def parse_composite_match_value(cls, raw_value: Any) -> dict[str, str] | None:
        """Parse editable composite matchValue text back into runtime match features."""
        alias_to_key = {
            "p": "parser_id",
            "parser": "parser_id",
            "parser_id": "parser_id",
            "c": "counterparty",
            "counterparty": "counterparty",
            "d": "description",
            "description": "description",
            "m": "payment_method",
            "payment": "payment_method",
            "payment_method": "payment_method",
        }
        features: dict[str, str] = {}
        for part in str(raw_value or "").split("|"):
            if "=" not in part:
                continue
            raw_key, raw_feature_value = part.split("=", 1)
            key = alias_to_key.get(cls._normalize_import_learning_text(raw_key))
            value = cls._normalize_import_learning_text(raw_feature_value)
            if key and value:
                features[key] = value

        if len(features) < 2:
            return None
        return features

    async def _record_import_learning_rule_log(
        self,
        conn: aiosqlite.Connection,
        *,
        rule_id: int | None,
        user_id: int,
        action: str,
        match_type: str,
        match_value: str,
        normalized_match_value: str,
        session_id: str | None = None,
        preview_id: int | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        await conn.execute(
            """
            INSERT INTO import_learning_rule_logs (
                rule_id, user_id, action, match_type, match_value,
                normalized_match_value, session_id, preview_id,
                payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                rule_id,
                user_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                json.dumps(payload or {}, ensure_ascii=False, sort_keys=True),
                utc_now_iso(),
            ),
        )

    @staticmethod
    def _build_import_learning_concept_key(
        *,
        rule_id: int | None = None,
        suggestion_id: int | None = None,
        candidate_id: str | None = None,
        fallback: str = "",
    ) -> tuple[str, str]:
        if rule_id:
            return (f"rule:{int(rule_id)}", "rule")
        if suggestion_id:
            return (f"suggestion:{int(suggestion_id)}", "suggestion")
        if candidate_id:
            return (f"candidate:{candidate_id}", "candidate")
        return (fallback or "global", "global")

    async def record_import_learning_feedback_event(
        self,
        event_type: str,
        *,
        user_id: int = 1,
        rule_id: int | None = None,
        suggestion_id: int | None = None,
        session_id: str | None = None,
        preview_id: int | None = None,
        bill_id: int | None = None,
        candidate_id: str | None = None,
        payload: dict[str, Any] | None = None,
        conn: aiosqlite.Connection | None = None,
    ) -> None:
        normalized_event_type = str(event_type or "").strip()
        if not normalized_event_type:
            return

        active_conn = conn or await self._get_connection()
        now = utc_now_iso()
        payload = dict(payload or {})
        await active_conn.execute(
            """
            INSERT INTO import_learning_feedback_events (
                user_id, event_type, rule_id, suggestion_id, session_id,
                preview_id, bill_id, candidate_id, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                normalized_event_type,
                rule_id,
                suggestion_id,
                session_id,
                preview_id,
                bill_id,
                candidate_id,
                json.dumps(payload, ensure_ascii=False, sort_keys=True),
                now,
            ),
        )

        concept_key, concept_type = self._build_import_learning_concept_key(
            rule_id=rule_id,
            suggestion_id=suggestion_id,
            candidate_id=candidate_id,
            fallback=normalized_event_type,
        )
        accepted_delta = 1 if "accept" in normalized_event_type else 0
        rejected_delta = 1 if "reject" in normalized_event_type else 0
        auto_applied_delta = int(payload.get("applied_count") or 1) if "auto_apply" in normalized_event_type else 0
        rollback_delta = 1 if "rollback" in normalized_event_type or bool(payload.get("rollback")) else 0
        await active_conn.execute(
            """
            INSERT INTO import_learning_concept_stats (
                user_id, concept_key, concept_type,
                accepted_count, rejected_count, auto_applied_count, rollback_count, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, concept_key, concept_type) DO UPDATE SET
                accepted_count = accepted_count + excluded.accepted_count,
                rejected_count = rejected_count + excluded.rejected_count,
                auto_applied_count = auto_applied_count + excluded.auto_applied_count,
                rollback_count = rollback_count + excluded.rollback_count,
                updated_at = excluded.updated_at
            """,
            (
                user_id,
                concept_key,
                concept_type,
                accepted_delta,
                rejected_delta,
                auto_applied_delta,
                rollback_delta,
                now,
            ),
        )

        if conn is None:
            await active_conn.commit()

    async def _get_import_learning_corpus_preview(
        self,
        conn: aiosqlite.Connection,
        *,
        session_id: str,
        preview_id: int,
        user_id: int,
    ) -> dict[str, Any] | None:
        async with conn.execute(
            """
            SELECT id, session_id, user_id, preview_parser_id,
                   preview_counterparty, preview_description,
                   preview_payment_method, preview_type,
                   preview_main_category, preview_sub_category,
                   preview_source_account_id, preview_destination_account_id
            FROM bills_preview
            WHERE id = ? AND session_id = ? AND user_id = ?
            LIMIT 1
            """,
            (preview_id, session_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _upsert_import_learning_corpus_sample(
        self,
        conn: aiosqlite.Connection,
        *,
        session_id: str,
        user_id: int,
        preview_id: int,
        sample: dict[str, Any],
        now: str,
    ) -> bool:
        preview = await self._get_import_learning_corpus_preview(
            conn,
            session_id=session_id,
            preview_id=preview_id,
            user_id=user_id,
        )
        if not preview:
            return False

        parser_id = str(preview.get("preview_parser_id") or "")
        counterparty = str(preview.get("preview_counterparty") or "")
        description = str(preview.get("preview_description") or "")
        payment_method = str(preview.get("preview_payment_method") or "")
        composite_hash = self.build_composite_match_hash(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        match_features = self.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        await conn.execute(
            """
            INSERT INTO import_learning_corpus_samples (
                user_id, session_id, preview_id,
                parser_id, counterparty, description, payment_method,
                composite_match_hash, match_features_json,
                annotated_type, annotated_category_id,
                annotated_source_account_id, annotated_destination_account_id,
                source_snapshot_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, session_id, preview_id) DO UPDATE SET
                parser_id = excluded.parser_id,
                counterparty = excluded.counterparty,
                description = excluded.description,
                payment_method = excluded.payment_method,
                composite_match_hash = excluded.composite_match_hash,
                match_features_json = excluded.match_features_json,
                annotated_type = excluded.annotated_type,
                annotated_category_id = excluded.annotated_category_id,
                annotated_source_account_id = excluded.annotated_source_account_id,
                annotated_destination_account_id = excluded.annotated_destination_account_id,
                source_snapshot_json = excluded.source_snapshot_json,
                updated_at = excluded.updated_at
            """,
            (
                user_id,
                session_id,
                preview_id,
                parser_id,
                counterparty,
                description,
                payment_method,
                composite_hash,
                json.dumps(match_features or {}, ensure_ascii=False, sort_keys=True),
                sample.get("preview_type") or sample.get("annotated_type"),
                sample.get("category_id") or sample.get("annotated_category_id"),
                sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                json.dumps(preview, ensure_ascii=False, sort_keys=True),
                now,
                now,
            ),
        )
        return True

    @log_method
    async def save_import_annotation_samples(
        self,
        session_id: str,
        samples: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        if not samples:
            return 0

        conn = await self._get_connection()
        now = utc_now_iso()
        saved_count = 0
        for sample in samples:
            preview_id = sample.get("preview_id") or sample.get("id")
            if not preview_id:
                continue
            normalized_preview_id = int(preview_id)
            await conn.execute(
                """
                INSERT INTO import_annotation_samples (
                    session_id, user_id, preview_id,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id, preview_id) DO UPDATE SET
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    updated_at = excluded.updated_at
                """,
                (
                    session_id,
                    user_id,
                    normalized_preview_id,
                    sample.get("preview_type") or sample.get("annotated_type"),
                    sample.get("category_id") or sample.get("annotated_category_id"),
                    sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                    sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                    now,
                    now,
                ),
            )
            await self._upsert_import_learning_corpus_sample(
                conn,
                session_id=session_id,
                user_id=user_id,
                preview_id=normalized_preview_id,
                sample=sample,
                now=now,
            )
            saved_count += 1

        await conn.commit()
        if saved_count > 0:
            await self.refresh_import_learning_model(user_id=user_id)
        return saved_count

    @log_method
    async def get_import_annotation_samples(self, session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM import_annotation_samples
            WHERE session_id = ? AND user_id = ?
            ORDER BY updated_at ASC, id ASC
            """,
            (session_id, user_id),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_import_learning_corpus_samples(
        self,
        user_id: int = 1,
        *,
        limit: int | None = None,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_corpus_samples WHERE user_id = ? ORDER BY updated_at DESC, id DESC"
        params: list[Any] = [user_id]
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])
        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def count_import_learning_corpus_samples(self, user_id: int = 1) -> int:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT COUNT(*) AS total_count FROM import_learning_corpus_samples WHERE user_id = ?",
            (user_id,),
        ) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)

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
        except (TypeError, ValueError, json.JSONDecodeError):
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
            except (TypeError, ValueError):
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

    @log_method
    async def promote_import_annotation_samples_to_learning(
        self,
        session_id: str,
        preview_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, int]:
        previews = await self.get_preview_by_session(session_id, user_id=user_id)
        preview_map = {int(preview["id"]): preview for preview in previews if preview.get("id")}
        samples = await self.get_import_annotation_samples(session_id, user_id=user_id)

        selection_requested = preview_ids is not None
        selected_preview_ids = {int(pid) for pid in (preview_ids or []) if pid}
        if selection_requested and not selected_preview_ids:
            return {"selected_samples": 0, "rules_total": 0, "created": 0, "updated": 0}
        if selected_preview_ids:
            samples = [sample for sample in samples if int(sample.get("preview_id", 0) or 0) in selected_preview_ids]
        if not samples:
            return {"selected_samples": 0, "rules_total": 0, "created": 0, "updated": 0}

        pending_rules: dict[tuple[str, str], dict[str, Any]] = {}
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

            pending_rules[("composite", composite_hash)] = {
                "match_type": "composite",
                "match_value": composite_hash,
                "normalized_match_value": composite_hash,
                "learned_type": sample.get("annotated_type") or preview.get("preview_type"),
                "learned_category_id": sample.get("annotated_category_id"),
                "learned_source_account_id": sample.get("annotated_source_account_id"),
                "learned_destination_account_id": sample.get("annotated_destination_account_id"),
                "source_session_id": session_id,
                "source_preview_id": preview_id,
                "parser_id": preview.get("preview_parser_id", ""),
                "composite_match_hash": composite_hash,
                "match_features_json": json.dumps(match_features, ensure_ascii=False, sort_keys=True),
            }

        if not pending_rules:
            return {"selected_samples": len(samples), "rules_total": 0, "created": 0, "updated": 0}

        conn = await self._get_connection()
        now = utc_now_iso()
        created_count = 0
        updated_count = 0

        for rule_data in pending_rules.values():
            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (user_id, rule_data["match_type"], rule_data["normalized_match_value"]),
            ) as cursor:
                existing = await cursor.fetchone()

            await conn.execute(
                """
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id,
                    learned_source_account_id, learned_destination_account_id,
                    enabled, source_session_id, source_preview_id,
                    parser_id, composite_match_hash, match_features_json,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    match_value = excluded.match_value,
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    enabled = 1,
                    source_session_id = excluded.source_session_id,
                    source_preview_id = excluded.source_preview_id,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                """,
                (
                    user_id,
                    rule_data["match_type"],
                    rule_data["match_value"],
                    rule_data["normalized_match_value"],
                    rule_data["learned_type"],
                    rule_data["learned_category_id"],
                    rule_data["learned_source_account_id"],
                    rule_data["learned_destination_account_id"],
                    rule_data["source_session_id"],
                    rule_data["source_preview_id"],
                    rule_data.get("parser_id"),
                    rule_data.get("composite_match_hash"),
                    rule_data.get("match_features_json"),
                    now,
                    now,
                ),
            )

            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (user_id, rule_data["match_type"], rule_data["normalized_match_value"]),
            ) as cursor:
                saved_row = await cursor.fetchone()

            rule_id = int(saved_row["id"]) if saved_row else None
            action = "updated" if existing else "created"
            if existing:
                updated_count += 1
            else:
                created_count += 1

            await self._record_import_learning_rule_log(
                conn,
                rule_id=rule_id,
                user_id=user_id,
                action=action,
                match_type=rule_data["match_type"],
                match_value=rule_data["match_value"],
                normalized_match_value=rule_data["normalized_match_value"],
                session_id=session_id,
                preview_id=rule_data["source_preview_id"],
                payload=rule_data,
            )

        await conn.commit()
        return {
            "selected_samples": len(samples),
            "rules_total": len(pending_rules),
            "created": created_count,
            "updated": updated_count,
        }

    @log_method
    async def get_import_learning_rules(
        self,
        user_id: int = 1,
        enabled_only: bool = False,
        limit: int | None = 200,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]
        if enabled_only:
            query += " AND enabled = 1"
        query += " ORDER BY updated_at DESC, id DESC"
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_import_learning_rule_by_id(
        self,
        rule_id: int,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    @log_method
    async def count_import_learning_rules(self, user_id: int = 1, enabled_only: bool = False) -> int:
        conn = await self._get_connection()
        query = "SELECT COUNT(*) AS total_count FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]
        if enabled_only:
            query += " AND enabled = 1"
        async with conn.execute(query, tuple(params)) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)

    @log_method
    async def set_import_learning_rule_enabled(self, rule_id: int, enabled: bool, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        enabled_value = 1 if enabled else 0
        now = utc_now_iso()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return False

        await conn.execute(
            "UPDATE import_learning_rules SET enabled = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (enabled_value, now, rule_id, user_id),
        )
        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="enabled" if enabled else "disabled",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload={"enabled": enabled_value},
        )
        await conn.commit()
        return True

    @log_method
    async def update_import_learning_rule(
        self,
        rule_id: int,
        user_id: int = 1,
        *,
        match_value: str | None = None,
        learned_type: str | None = None,
        learned_category_id: Any = _UNSET,
        enabled: bool | None = None,
    ) -> dict[str, Any] | None:
        """Update editable fields of a learning rule. Returns updated row or None if not found."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return None

        now = utc_now_iso()
        updates: list[str] = ["updated_at = ?"]
        params: list[Any] = [now]
        updated_match_value = str(existing["match_value"] or "")
        updated_normalized_match_value = str(existing["normalized_match_value"] or "")
        updated_match_features: dict[str, str] | None = None
        updated_composite_hash = str(existing["composite_match_hash"] or "")

        if match_value is not None:
            existing_match_type = str(existing["match_type"] or "")
            if existing_match_type == "composite":
                updated_match_features = self.parse_composite_match_value(match_value)
                if not updated_match_features:
                    raise ValueError("composite matchValue must contain at least two keyed features")
                updated_composite_hash = self.build_composite_match_hash(
                    parser_id=updated_match_features.get("parser_id", ""),
                    counterparty=updated_match_features.get("counterparty", ""),
                    description=updated_match_features.get("description", ""),
                    payment_method=updated_match_features.get("payment_method", ""),
                ) or ""
                updated_match_value = updated_composite_hash
                updated_normalized_match_value = updated_composite_hash
                updates.append("match_value = ?")
                params.append(updated_match_value)
                updates.append("normalized_match_value = ?")
                params.append(updated_normalized_match_value)
                updates.append("parser_id = ?")
                params.append(updated_match_features.get("parser_id", ""))
                updates.append("composite_match_hash = ?")
                params.append(updated_composite_hash)
                updates.append("match_features_json = ?")
                params.append(json.dumps(updated_match_features, ensure_ascii=False, sort_keys=True))
            else:
                updated_match_value = match_value
                updated_normalized_match_value = self._normalize_import_learning_text(match_value)
                if not updated_normalized_match_value:
                    raise ValueError("matchValue cannot be empty")
                updates.append("match_value = ?")
                params.append(updated_match_value)
                updates.append("normalized_match_value = ?")
                params.append(updated_normalized_match_value)
                updates.append("composite_match_hash = ?")
                params.append(None)
                updates.append("match_features_json = ?")
                params.append(None)
                updates.append("parser_id = ?")
                params.append(None)

        if learned_type is not None:
            updates.append("learned_type = ?")
            params.append(learned_type)

        if learned_category_id is not _UNSET:
            updates.append("learned_category_id = ?")
            params.append(learned_category_id)

        if enabled is not None:
            updates.append("enabled = ?")
            params.append(1 if enabled else 0)

        params.extend([rule_id, user_id])
        await conn.execute(
            f"UPDATE import_learning_rules SET {', '.join(updates)} WHERE id = ? AND user_id = ?",
            tuple(params),
        )
        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="updated",
            match_type=existing["match_type"],
            match_value=updated_match_value,
            normalized_match_value=updated_normalized_match_value,
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload={
                "match_value": match_value,
                "normalized_match_value": updated_normalized_match_value,
                "composite_match_hash": updated_composite_hash,
                "match_features": updated_match_features,
                "learned_type": learned_type,
                "learned_category_id": None if learned_category_id is _UNSET else learned_category_id,
                "enabled": enabled,
            },
        )
        await conn.commit()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    @log_method
    async def delete_import_learning_rule(self, rule_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            (rule_id, user_id),
        ) as cursor:
            existing = await cursor.fetchone()
        if not existing:
            return False

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="deleted",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload=dict(existing),
        )
        await conn.execute("DELETE FROM import_learning_rules WHERE id = ? AND user_id = ?", (rule_id, user_id))
        await conn.commit()
        return True

    @log_method
    async def increment_import_learning_rule_usage(self, rule_ids: list[int], user_id: int = 1) -> int:
        unique_ids = [int(rule_id) for rule_id in sorted(set(rule_ids)) if rule_id]
        if not unique_ids:
            return 0
        conn = await self._get_connection()
        now = utc_now_iso()
        placeholders = ",".join(["?" for _ in unique_ids])
        cursor = await conn.execute(
            f"""
            UPDATE import_learning_rules
            SET applied_count = applied_count + 1,
                last_applied_at = ?,
                updated_at = ?
            WHERE user_id = ? AND id IN ({placeholders})
            """,
            (now, now, user_id, *unique_ids),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def batch_update_preview_classification(self, updates: list[dict[str, Any]]) -> int:
        if not updates:
            return 0
        conn = await self._get_connection()
        updated_count = 0

        for update_item in updates:
            try:
                preview_id = update_item.get("id")
                if not preview_id:
                    continue
                await conn.execute(
                    """
                    UPDATE bills_preview SET
                        preview_type = ?,
                        preview_main_category = ?,
                        preview_sub_category = ?,
                        preview_source_account_id = ?,
                        preview_destination_account_id = ?
                    WHERE id = ?
                    """,
                    (
                        update_item.get("preview_type", ""),
                        update_item.get("preview_main_category", ""),
                        update_item.get("preview_sub_category", ""),
                        update_item.get("preview_source_account_id"),
                        update_item.get("preview_destination_account_id"),
                        preview_id,
                    ),
                )
                updated_count += 1
            except Exception as exc:  # pragma: no cover - defensive logging branch
                self.logger.error("[重新分类-更新失败] id=%s, error=%s", update_item.get("id"), exc)

        await conn.commit()
        return updated_count

    # ------------------------------------------------------------------
    # Independent learning suggestion center (cross-session)
    # ------------------------------------------------------------------

    @log_method
    async def mine_learning_suggestions(self, user_id: int = 1) -> dict[str, int]:
        """从 durable corpus 中挖掘学习建议，写入 import_learning_suggestions。

        Returns:
            dict with keys: total_annotations, mined, created, updated, skipped_conflict, skipped_existing
        """
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT *
            FROM import_learning_corpus_samples
            WHERE user_id = ?
            ORDER BY session_id, updated_at ASC, id ASC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        all_annotations = [dict(row) for row in rows]
        if not all_annotations:
            return {"total_annotations": 0, "mined": 0, "created": 0, "updated": 0,
                    "skipped_conflict": 0, "skipped_existing": 0}

        existing_rules = await self.get_import_learning_rules(user_id=user_id, enabled_only=False, limit=None)
        existing_rule_hashes: dict[str, int] = {}
        for rule in existing_rules:
            if str(rule.get("match_type") or "") == "composite" and rule.get("normalized_match_value"):
                existing_rule_hashes[str(rule["normalized_match_value"])] = int(rule["id"])

        candidates: dict[str, dict[str, Any]] = {}
        conflicted_hashes: set[str] = set()

        for annotation in all_annotations:
            composite_hash = str(annotation.get("composite_match_hash") or "")
            match_features: dict[str, str] | None = None
            try:
                parsed_features = json.loads(annotation.get("match_features_json") or "{}")
                if isinstance(parsed_features, dict):
                    match_features = {
                        str(key): self._normalize_import_learning_text(value)
                        for key, value in parsed_features.items()
                        if self._normalize_import_learning_text(value)
                    }
            except (TypeError, ValueError, json.JSONDecodeError):
                match_features = None
            if not composite_hash:
                composite_hash = self.build_composite_match_hash(
                    parser_id=annotation.get("parser_id", ""),
                    counterparty=annotation.get("counterparty", ""),
                    description=annotation.get("description", ""),
                    payment_method=annotation.get("payment_method", ""),
                ) or ""
            if not match_features:
                match_features = self.build_composite_match_features(
                    parser_id=annotation.get("parser_id", ""),
                    counterparty=annotation.get("counterparty", ""),
                    description=annotation.get("description", ""),
                    payment_method=annotation.get("payment_method", ""),
                )
            if not composite_hash or not match_features:
                continue
            if composite_hash in conflicted_hashes:
                continue

            sig = self._build_import_learning_suggestion_signature(annotation, annotation)
            existing_candidate = candidates.get(composite_hash)
            if existing_candidate is None:
                candidates[composite_hash] = {
                    "match_type": "composite",
                    "match_value": composite_hash,
                    "normalized_match_value": composite_hash,
                    "composite_match_hash": composite_hash,
                    "match_features": match_features,
                    "suggested_type": sig[0],
                    "suggested_category_id": sig[1],
                    "suggested_source_account_id": sig[2],
                    "suggested_destination_account_id": sig[3],
                    "sample_count": 1,
                    "source_session_ids": {str(annotation.get("session_id", ""))},
                    "source_preview_ids": {int(annotation.get("preview_id", 0))},
                    "signature": sig,
                    "existing_rule_id": existing_rule_hashes.get(composite_hash),
                }
                continue

            if existing_candidate["signature"] != sig:
                conflicted_hashes.add(composite_hash)
                candidates.pop(composite_hash, None)
                continue

            existing_candidate["sample_count"] += 1
            existing_candidate["source_session_ids"].add(str(annotation.get("session_id", "")))
            existing_candidate["source_preview_ids"].add(int(annotation.get("preview_id", 0)))

        now = utc_now_iso()
        created_count = 0
        updated_count = 0
        skipped_existing = 0

        for candidate in candidates.values():
            if candidate.get("existing_rule_id"):
                skipped_existing += 1
                continue

            session_ids = sorted(candidate.pop("source_session_ids", set()))
            preview_ids = sorted(candidate.pop("source_preview_ids", set()))
            candidate.pop("signature", None)

            features_summary_parts = []
            for key, label in [("counterparty", "交易方"), ("description", "描述"), ("payment_method", "支付方式")]:
                value = candidate.get("match_features", {}).get(key, "")
                if value:
                    features_summary_parts.append(f"{label}: {value}")
            summary = " | ".join(features_summary_parts) if features_summary_parts else candidate["match_value"]

            await conn.execute(
                """
                INSERT INTO import_learning_suggestions (
                    user_id, match_type, match_value, normalized_match_value,
                    composite_match_hash, match_features_json,
                    suggested_type, suggested_category_id,
                    suggested_source_account_id, suggested_destination_account_id,
                    sample_count, source_session_ids_json, source_preview_ids_json,
                    status, existing_rule_id, summary,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    suggested_type = excluded.suggested_type,
                    suggested_category_id = excluded.suggested_category_id,
                    suggested_source_account_id = excluded.suggested_source_account_id,
                    suggested_destination_account_id = excluded.suggested_destination_account_id,
                    sample_count = excluded.sample_count,
                    source_session_ids_json = excluded.source_session_ids_json,
                    source_preview_ids_json = excluded.source_preview_ids_json,
                    summary = excluded.summary,
                    updated_at = excluded.updated_at
                """,
                (
                    user_id,
                    candidate["match_type"],
                    candidate["match_value"],
                    candidate["normalized_match_value"],
                    candidate["composite_match_hash"],
                    json.dumps(candidate.get("match_features") or {}, ensure_ascii=False, sort_keys=True),
                    candidate.get("suggested_type"),
                    candidate.get("suggested_category_id"),
                    candidate.get("suggested_source_account_id"),
                    candidate.get("suggested_destination_account_id"),
                    candidate["sample_count"],
                    json.dumps(session_ids, ensure_ascii=False),
                    json.dumps(preview_ids, ensure_ascii=False),
                    candidate.get("existing_rule_id"),
                    summary,
                    now,
                    now,
                ),
            )

            async with conn.execute("SELECT changes()") as cur:
                row = await cur.fetchone()
            changed = int(row[0]) if row else 0
            if changed > 0:
                async with conn.execute(
                    """
                    SELECT id FROM import_learning_suggestions
                    WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                    LIMIT 1
                    """,
                    (user_id, candidate["match_type"], candidate["normalized_match_value"]),
                ) as cur:
                    saved = await cur.fetchone()
                if saved:
                    async with conn.execute(
                        "SELECT created_at FROM import_learning_suggestions WHERE id = ?",
                        (saved["id"],),
                    ) as cur:
                        check = await cur.fetchone()
                    if check and str(check["created_at"]) == now:
                        created_count += 1
                    else:
                        updated_count += 1

        await conn.commit()
        return {
            "total_annotations": len(all_annotations),
            "mined": len(candidates),
            "created": created_count,
            "updated": updated_count,
            "skipped_conflict": len(conflicted_hashes),
            "skipped_existing": skipped_existing,
        }

    @log_method
    async def get_learning_suggestions(
        self,
        user_id: int = 1,
        status: str | None = None,
        limit: int | None = 200,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_suggestions WHERE user_id = ?"
        params: list[Any] = [user_id]
        if status:
            query += " AND status = ?"
            params.append(status)
        query += " ORDER BY sample_count DESC, updated_at DESC, id DESC"
        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])
        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def count_learning_suggestions(self, user_id: int = 1, status: str | None = None) -> int:
        conn = await self._get_connection()
        query = "SELECT COUNT(*) AS total_count FROM import_learning_suggestions WHERE user_id = ?"
        params: list[Any] = [user_id]
        if status:
            query += " AND status = ?"
            params.append(status)
        async with conn.execute(query, tuple(params)) as cursor:
            row = await cursor.fetchone()
        return int(row["total_count"] if row else 0)

    @log_method
    async def accept_learning_suggestion(self, suggestion_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """接受建议：将建议提升为 import_learning_rules 并标记 accepted。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_learning_suggestions WHERE id = ? AND user_id = ? LIMIT 1",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        if not row:
            return None
        suggestion = dict(row)
        if suggestion["status"] != "pending":
            return {"error": "suggestion_not_pending", "current_status": suggestion["status"]}

        now = utc_now_iso()
        features = {}
        try:
            parsed = json.loads(suggestion.get("match_features_json") or "{}")
            if isinstance(parsed, dict):
                features = parsed
        except (json.JSONDecodeError, TypeError):
            pass

        await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id,
                learned_source_account_id, learned_destination_account_id,
                enabled, parser_id, composite_match_hash, match_features_json,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                learned_type = excluded.learned_type,
                learned_category_id = excluded.learned_category_id,
                learned_source_account_id = excluded.learned_source_account_id,
                learned_destination_account_id = excluded.learned_destination_account_id,
                enabled = 1,
                parser_id = excluded.parser_id,
                composite_match_hash = excluded.composite_match_hash,
                match_features_json = excluded.match_features_json,
                updated_at = excluded.updated_at
            """,
            (
                user_id,
                suggestion["match_type"],
                suggestion["match_value"],
                suggestion["normalized_match_value"],
                suggestion.get("suggested_type"),
                suggestion.get("suggested_category_id"),
                suggestion.get("suggested_source_account_id"),
                suggestion.get("suggested_destination_account_id"),
                features.get("parser_id", ""),
                suggestion.get("composite_match_hash"),
                suggestion.get("match_features_json"),
                now,
                now,
            ),
        )

        async with conn.execute(
            "SELECT id FROM import_learning_rules WHERE user_id = ? AND match_type = ? AND normalized_match_value = ? LIMIT 1",
            (user_id, suggestion["match_type"], suggestion["normalized_match_value"]),
        ) as cursor:
            rule_row = await cursor.fetchone()
        rule_id = int(rule_row["id"]) if rule_row else None

        await conn.execute(
            "UPDATE import_learning_suggestions SET status = 'accepted', existing_rule_id = ?, updated_at = ? WHERE id = ?",
            (rule_id, now, suggestion_id),
        )

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="created_from_suggestion",
            match_type=suggestion["match_type"],
            match_value=suggestion["match_value"],
            normalized_match_value=suggestion["normalized_match_value"],
            payload={"suggestion_id": suggestion_id},
        )
        await self.record_import_learning_feedback_event(
            "suggestion_accept",
            user_id=user_id,
            rule_id=rule_id,
            suggestion_id=suggestion_id,
            payload={"status": "accepted"},
            conn=conn,
        )

        await conn.commit()
        return {"suggestion_id": suggestion_id, "rule_id": rule_id, "status": "accepted"}

    @log_method
    async def reject_learning_suggestion(self, suggestion_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT id, status FROM import_learning_suggestions WHERE id = ? AND user_id = ? LIMIT 1",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        if not row:
            return False
        if row["status"] != "pending":
            return False

        now = utc_now_iso()
        await conn.execute(
            "UPDATE import_learning_suggestions SET status = 'rejected', updated_at = ? WHERE id = ?",
            (now, suggestion_id),
        )
        await self.record_import_learning_feedback_event(
            "suggestion_reject",
            user_id=user_id,
            suggestion_id=suggestion_id,
            payload={"status": "rejected"},
            conn=conn,
        )
        await conn.commit()
        return True
