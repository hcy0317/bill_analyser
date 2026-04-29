"""Small numpy dual-head classifier for import-learning inference."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import numpy as np

from .features import (
    DEFAULT_FEATURE_DIMENSION,
    ImportLearningTrainingSample,
    vectorize_features,
)

MODEL_KEY = "import-learning-dual-head"
MODEL_FAMILY = "shared-hidden-dual-softmax-v1"
MIN_TRAINING_SAMPLES = 3
HIDDEN_DIMENSION = 16


@dataclass(frozen=True)
class ImportLearningPrediction:
    semantic_label: str
    route_label: str
    semantic_confidence: float
    route_confidence: float
    semantic_margin: float
    route_margin: float


@dataclass(frozen=True)
class ImportLearningTrainingResult:
    trainable: bool
    reason: str
    parameters: dict[str, Any] | None
    metrics: dict[str, Any]


def _softmax(logits: np.ndarray) -> np.ndarray:
    shifted = logits - np.max(logits, axis=1, keepdims=True)
    exp = np.exp(shifted)
    return exp / np.sum(exp, axis=1, keepdims=True)


def _top_confidence_and_margin(probs: np.ndarray) -> tuple[int, float, float]:
    if probs.size == 0:
        return 0, 0.0, 0.0
    sorted_indices = np.argsort(probs)[::-1]
    best_index = int(sorted_indices[0])
    best_score = float(probs[best_index])
    second_score = float(probs[sorted_indices[1]]) if len(sorted_indices) > 1 else 0.0
    return best_index, best_score, best_score - second_score


def _label_maps(labels: list[str]) -> tuple[list[str], dict[str, int]]:
    classes = sorted(set(labels))
    return classes, {label: index for index, label in enumerate(classes)}


def train_dual_head_model(
    samples: list[ImportLearningTrainingSample],
    *,
    feature_dimension: int = DEFAULT_FEATURE_DIMENSION,
    hidden_dimension: int = HIDDEN_DIMENSION,
    epochs: int = 220,
    learning_rate: float = 0.35,
) -> ImportLearningTrainingResult:
    if len(samples) < MIN_TRAINING_SAMPLES:
        return ImportLearningTrainingResult(
            trainable=False,
            reason="insufficient_samples",
            parameters=None,
            metrics={"sample_count": len(samples), "minimum_samples": MIN_TRAINING_SAMPLES},
        )

    semantic_classes, semantic_index = _label_maps([sample.semantic_label for sample in samples])
    route_classes, route_index = _label_maps([sample.route_label for sample in samples])
    if not semantic_classes or not route_classes:
        return ImportLearningTrainingResult(
            trainable=False,
            reason="missing_labels",
            parameters=None,
            metrics={"sample_count": len(samples)},
        )

    x = np.vstack([vectorize_features(sample.features, feature_dimension) for sample in samples])
    y_semantic = np.array([semantic_index[sample.semantic_label] for sample in samples], dtype=np.int64)
    y_route = np.array([route_index[sample.route_label] for sample in samples], dtype=np.int64)
    sample_count = x.shape[0]

    rng = np.random.default_rng(20260429)
    w_hidden = rng.normal(0.0, 0.08, size=(feature_dimension, hidden_dimension))
    b_hidden = np.zeros((hidden_dimension,), dtype=np.float64)
    w_semantic = rng.normal(0.0, 0.08, size=(hidden_dimension, len(semantic_classes)))
    b_semantic = np.zeros((len(semantic_classes),), dtype=np.float64)
    w_route = rng.normal(0.0, 0.08, size=(hidden_dimension, len(route_classes)))
    b_route = np.zeros((len(route_classes),), dtype=np.float64)

    loss = 0.0
    for _epoch in range(epochs):
        hidden_raw = x @ w_hidden + b_hidden
        hidden = np.maximum(hidden_raw, 0.0)
        semantic_probs = _softmax(hidden @ w_semantic + b_semantic)
        route_probs = _softmax(hidden @ w_route + b_route)

        loss_semantic = -np.log(semantic_probs[np.arange(sample_count), y_semantic] + 1e-12).mean()
        loss_route = -np.log(route_probs[np.arange(sample_count), y_route] + 1e-12).mean()
        loss = float((loss_semantic + loss_route) / 2.0)

        grad_semantic_logits = semantic_probs
        grad_semantic_logits[np.arange(sample_count), y_semantic] -= 1.0
        grad_semantic_logits /= sample_count

        grad_route_logits = route_probs
        grad_route_logits[np.arange(sample_count), y_route] -= 1.0
        grad_route_logits /= sample_count

        grad_w_semantic = hidden.T @ grad_semantic_logits
        grad_b_semantic = grad_semantic_logits.sum(axis=0)
        grad_w_route = hidden.T @ grad_route_logits
        grad_b_route = grad_route_logits.sum(axis=0)
        grad_hidden = grad_semantic_logits @ w_semantic.T + grad_route_logits @ w_route.T
        grad_hidden[hidden_raw <= 0] = 0.0
        grad_w_hidden = x.T @ grad_hidden
        grad_b_hidden = grad_hidden.sum(axis=0)

        w_hidden -= learning_rate * grad_w_hidden
        b_hidden -= learning_rate * grad_b_hidden
        w_semantic -= learning_rate * grad_w_semantic
        b_semantic -= learning_rate * grad_b_semantic
        w_route -= learning_rate * grad_w_route
        b_route -= learning_rate * grad_b_route

    parameters = {
        "model_family": MODEL_FAMILY,
        "feature_dimension": feature_dimension,
        "hidden_dimension": hidden_dimension,
        "semantic_classes": semantic_classes,
        "route_classes": route_classes,
        "weights": {
            "hidden": w_hidden.round(8).tolist(),
            "hidden_bias": b_hidden.round(8).tolist(),
            "semantic": w_semantic.round(8).tolist(),
            "semantic_bias": b_semantic.round(8).tolist(),
            "route": w_route.round(8).tolist(),
            "route_bias": b_route.round(8).tolist(),
        },
    }
    metrics = {
        "sample_count": len(samples),
        "semantic_class_count": len(semantic_classes),
        "route_class_count": len(route_classes),
        "training_loss": round(loss, 6),
        "epochs": epochs,
    }
    return ImportLearningTrainingResult(
        trainable=True,
        reason="trained",
        parameters=parameters,
        metrics=metrics,
    )


def predict_dual_head(parameters: dict[str, Any], features: dict[str, Any]) -> ImportLearningPrediction | None:
    try:
        feature_dimension = int(parameters.get("feature_dimension") or DEFAULT_FEATURE_DIMENSION)
        semantic_classes = [str(item) for item in list(parameters.get("semantic_classes") or [])]
        route_classes = [str(item) for item in list(parameters.get("route_classes") or [])]
        weights = dict(parameters.get("weights") or {})
        w_hidden = np.asarray(weights.get("hidden"), dtype=np.float64)
        b_hidden = np.asarray(weights.get("hidden_bias"), dtype=np.float64)
        w_semantic = np.asarray(weights.get("semantic"), dtype=np.float64)
        b_semantic = np.asarray(weights.get("semantic_bias"), dtype=np.float64)
        w_route = np.asarray(weights.get("route"), dtype=np.float64)
        b_route = np.asarray(weights.get("route_bias"), dtype=np.float64)
    except (TypeError, ValueError):
        return None

    if not semantic_classes or not route_classes:
        return None

    x = vectorize_features(features, feature_dimension).reshape(1, -1)
    hidden = np.maximum(x @ w_hidden + b_hidden, 0.0)
    semantic_probs = _softmax(hidden @ w_semantic + b_semantic)[0]
    route_probs = _softmax(hidden @ w_route + b_route)[0]
    semantic_index, semantic_confidence, semantic_margin = _top_confidence_and_margin(semantic_probs)
    route_index, route_confidence, route_margin = _top_confidence_and_margin(route_probs)
    return ImportLearningPrediction(
        semantic_label=semantic_classes[semantic_index],
        route_label=route_classes[route_index],
        semantic_confidence=round(semantic_confidence, 4),
        route_confidence=round(route_confidence, 4),
        semantic_margin=round(semantic_margin, 4),
        route_margin=round(route_margin, 4),
    )
