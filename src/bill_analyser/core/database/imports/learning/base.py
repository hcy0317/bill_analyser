"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    pass


_UNSET: Any = object()


class ImportLearningBaseMixin:
    """Shared import-learning normalization and signature helpers."""

    @staticmethod
    def _normalize_import_learning_suggestion_id(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized_value = int(raw_value)
        except TypeError, ValueError:
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
