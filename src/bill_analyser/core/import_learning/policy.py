"""Green/blue policy gates for trainable import-learning recommendations."""

from __future__ import annotations

from dataclasses import dataclass

from .model import ImportLearningPrediction

POLICY_VERSION = "learning-green-blue-policy-v1"
GREEN_CONFIDENCE_THRESHOLD = 0.52
GREEN_MARGIN_THRESHOLD = 0.02
BLUE_CONFIDENCE_THRESHOLD = 0.70
BLUE_MARGIN_THRESHOLD = 0.05
BLUE_ACCEPT_CONFIRMATION_THRESHOLD = 2


@dataclass(frozen=True)
class LearningPolicyDecision:
    mode: str
    score: float
    confidence: float
    margin: float
    auto_apply: bool
    level: str
    rejection_reasons: list[str]


def evaluate_learning_policy(
    prediction: ImportLearningPrediction,
    *,
    confirmation_count: int,
    conflict_reasons: list[str] | None = None,
) -> LearningPolicyDecision:
    conflicts = [reason for reason in list(conflict_reasons or []) if reason]
    confidence = min(prediction.semantic_confidence, prediction.route_confidence)
    margin = min(prediction.semantic_margin, prediction.route_margin)
    score = round((prediction.semantic_confidence + prediction.route_confidence) / 2.0, 4)
    rejection_reasons: list[str] = []

    if confidence < GREEN_CONFIDENCE_THRESHOLD:
        rejection_reasons.append("green_confidence")
    if margin < GREEN_MARGIN_THRESHOLD:
        rejection_reasons.append("green_margin")
    if rejection_reasons:
        return LearningPolicyDecision(
            mode="none",
            score=score,
            confidence=round(confidence, 4),
            margin=round(margin, 4),
            auto_apply=False,
            level="",
            rejection_reasons=rejection_reasons,
        )

    blue_rejection_reasons: list[str] = []
    if confirmation_count <= BLUE_ACCEPT_CONFIRMATION_THRESHOLD:
        blue_rejection_reasons.append("confirmations")
    if confidence < BLUE_CONFIDENCE_THRESHOLD:
        blue_rejection_reasons.append("blue_confidence")
    if margin < BLUE_MARGIN_THRESHOLD:
        blue_rejection_reasons.append("blue_margin")
    if conflicts:
        blue_rejection_reasons.extend(f"conflict:{reason}" for reason in conflicts)

    mode = "green" if blue_rejection_reasons else "blue"
    if score >= 0.86:
        level = "high"
    elif score >= 0.72:
        level = "medium"
    else:
        level = "low"

    return LearningPolicyDecision(
        mode=mode,
        score=score,
        confidence=round(confidence, 4),
        margin=round(margin, 4),
        auto_apply=mode == "blue",
        level=level,
        rejection_reasons=blue_rejection_reasons,
    )
