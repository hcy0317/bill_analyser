"""Feature encoding for trainable import-learning recommendations."""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any

import numpy as np

FEATURE_SCHEMA_VERSION = "import-learning-features-v1"
DEFAULT_FEATURE_DIMENSION = 96

_TOKEN_SPLIT_RE = re.compile(r"[\s|,，/、_\-:：;；()（）\[\]{}]+")


@dataclass(frozen=True)
class ImportLearningTrainingSample:
    """One normalized supervised sample for the dual-head learning model."""

    sample_id: int
    features: dict[str, Any]
    semantic_label: str
    route_label: str


def normalize_learning_text(raw_value: Any) -> str:
    if raw_value is None:
        return ""
    text = str(raw_value).strip().lower()
    if not text:
        return ""
    return " ".join(text.split())


def _normalize_optional_int(raw_value: Any) -> int:
    if raw_value in (None, "", 0, "0"):
        return 0
    try:
        return max(int(raw_value), 0)
    except (TypeError, ValueError):
        return 0


def _load_snapshot_payload(raw_payload: Any) -> dict[str, Any]:
    if isinstance(raw_payload, dict):
        return dict(raw_payload)
    if not raw_payload:
        return {}
    try:
        payload = json.loads(str(raw_payload))
    except (TypeError, ValueError, json.JSONDecodeError):
        return {}
    return dict(payload) if isinstance(payload, dict) else {}


def _amount_bucket(raw_amount: Any) -> str:
    try:
        amount = abs(float(raw_amount or 0))
    except (TypeError, ValueError):
        amount = 0.0
    if amount == 0:
        return "zero"
    if amount < 20:
        return "lt20"
    if amount < 100:
        return "lt100"
    if amount < 500:
        return "lt500"
    return "gte500"


def build_semantic_label(row: dict[str, Any]) -> str:
    learned_type = normalize_learning_text(row.get("annotated_type"))
    category_id = _normalize_optional_int(row.get("annotated_category_id"))
    return f"type={learned_type}|category={category_id}"


def build_route_label(row: dict[str, Any]) -> str:
    source_id = _normalize_optional_int(row.get("annotated_source_account_id"))
    destination_id = _normalize_optional_int(row.get("annotated_destination_account_id"))
    return f"source={source_id}|destination={destination_id}"


def parse_semantic_label(label: str) -> dict[str, Any]:
    result: dict[str, Any] = {"type": "", "category_id": None}
    for part in str(label or "").split("|"):
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        if key == "type":
            result["type"] = value
        elif key == "category":
            category_id = _normalize_optional_int(value)
            result["category_id"] = category_id or None
    return result


def parse_route_label(label: str) -> dict[str, Any]:
    result: dict[str, Any] = {"source_account_id": None, "destination_account_id": None}
    for part in str(label or "").split("|"):
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        normalized_id = _normalize_optional_int(value) or None
        if key == "source":
            result["source_account_id"] = normalized_id
        elif key == "destination":
            result["destination_account_id"] = normalized_id
    return result


def build_feature_payload(row: dict[str, Any]) -> dict[str, Any]:
    snapshot = _load_snapshot_payload(row.get("source_snapshot_json"))
    preview_type = normalize_learning_text(snapshot.get("preview_type") or row.get("annotated_type"))
    return {
        "parser_id": normalize_learning_text(row.get("parser_id")),
        "counterparty": normalize_learning_text(row.get("counterparty")),
        "description": normalize_learning_text(row.get("description")),
        "payment_method": normalize_learning_text(row.get("payment_method")),
        "amount_bucket": _amount_bucket(snapshot.get("preview_amount")),
        "preview_type": preview_type,
    }


def prepare_training_samples(rows: list[dict[str, Any]]) -> list[ImportLearningTrainingSample]:
    samples: list[ImportLearningTrainingSample] = []
    for row in rows:
        sample_id = _normalize_optional_int(row.get("id"))
        if sample_id <= 0:
            continue
        semantic_label = build_semantic_label(row)
        route_label = build_route_label(row)
        if semantic_label == "type=|category=0" and route_label == "source=0|destination=0":
            continue
        features = build_feature_payload(row)
        text_feature_count = sum(
            1
            for key in ("parser_id", "counterparty", "description", "payment_method")
            if features.get(key)
        )
        if text_feature_count < 2:
            continue
        samples.append(
            ImportLearningTrainingSample(
                sample_id=sample_id,
                features=features,
                semantic_label=semantic_label,
                route_label=route_label,
            )
        )
    return samples


def _iter_text_tokens(field: str, value: str) -> list[str]:
    tokens: list[str] = []
    normalized_value = normalize_learning_text(value)
    if not normalized_value:
        return tokens

    tokens.append(f"{field}={normalized_value}")
    for part in _TOKEN_SPLIT_RE.split(normalized_value):
        if not part:
            continue
        tokens.append(f"{field}:tok={part}")
        if len(part) >= 2:
            for index in range(len(part) - 1):
                tokens.append(f"{field}:bi={part[index:index + 2]}")
    return tokens


def iter_feature_tokens(features: dict[str, Any]) -> list[str]:
    tokens: list[str] = []
    parser_id = normalize_learning_text(features.get("parser_id"))
    if parser_id:
        tokens.append(f"parser_id={parser_id}")
    for field in ("counterparty", "description", "payment_method"):
        tokens.extend(_iter_text_tokens(field, str(features.get(field) or "")))
    amount_bucket = normalize_learning_text(features.get("amount_bucket"))
    if amount_bucket:
        tokens.append(f"amount_bucket={amount_bucket}")
    preview_type = normalize_learning_text(features.get("preview_type"))
    if preview_type:
        tokens.append(f"preview_type={preview_type}")
    return tokens


def _hash_token(token: str, dimension: int) -> int:
    digest = hashlib.blake2b(token.encode("utf-8"), digest_size=4).digest()
    return int.from_bytes(digest, "little") % dimension


def vectorize_features(features: dict[str, Any], dimension: int = DEFAULT_FEATURE_DIMENSION) -> np.ndarray:
    vector = np.zeros((dimension,), dtype=np.float64)
    for token in iter_feature_tokens(features):
        vector[_hash_token(token, dimension)] += 1.0
    norm = np.linalg.norm(vector)
    if norm > 0:
        vector = vector / norm
    return vector


def build_label_confirmation_counts(samples: list[ImportLearningTrainingSample]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for sample in samples:
        key = f"{sample.semantic_label}||{sample.route_label}"
        counts[key] = counts.get(key, 0) + 1
    return counts
