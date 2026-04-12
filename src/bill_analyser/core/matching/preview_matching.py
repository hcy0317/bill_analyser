"""Helpers for additive import-preview matching payloads."""

from __future__ import annotations

from typing import Any

from .models import (
    AnnotationMatchingPayload,
    DedupMatchingPayload,
    InvestmentMatchingPayload,
    LearningMatchingPayload,
    ParserMatchingPayload,
    PreviewMatchingPayload,
    RecurringMatchingPayload,
    TransferMatchingPayload,
)


def _normalize_list_value(raw_value: Any) -> list[Any]:
    """Normalize list-like values while preserving additive compatibility."""
    if raw_value in (None, ""):
        return []
    if isinstance(raw_value, list):
        return list(raw_value)
    if isinstance(raw_value, (tuple, set)):
        return list(raw_value)
    return [raw_value]


def _normalize_source_ids_value(raw_value: Any) -> list[int | str]:
    """Normalize persisted dedup source IDs from CSV/list shapes into arrays.

    Preserve additive compatibility for both CSV and list-like inputs.
    """
    if raw_value in (None, ""):
        return []

    if isinstance(raw_value, str):
        parts = [part.strip() for part in raw_value.split(",") if part.strip()]
    else:
        parts = [
            str(part).strip()
            for part in _normalize_list_value(raw_value)
            if str(part).strip()
        ]

    normalized_parts: list[int | str] = []
    for part in parts:
        try:
            normalized_parts.append(int(part))
        except (TypeError, ValueError):
            normalized_parts.append(part)
    return normalized_parts


def _normalize_int_or_none(raw_value: Any) -> int | None:
    """Normalize numeric identifiers while keeping blank values empty."""
    if raw_value in (None, ""):
        return None
    try:
        return int(raw_value)
    except (TypeError, ValueError):
        return None


def build_preview_matching_payload(
    preview: dict[str, Any],
    *,
    transfer_suggestion: dict[str, Any] | None = None,
    investment_signal: dict[str, Any] | None = None,
    learning_recommendation: dict[str, Any] | None = None,
    matching_feedback: dict[str, Any] | None = None,
    is_manually_annotated: bool = False,
) -> dict[str, Any]:
    # pylint: disable=too-many-arguments,too-many-locals
    """Group existing preview hints into a stable additive matching payload."""
    transfer_suggestion = transfer_suggestion or {}
    investment_signal = investment_signal or {}
    learning_recommendation = learning_recommendation or {}
    matching_feedback = matching_feedback or {}

    transfer_feedback = (
        matching_feedback.get("transfer") if isinstance(matching_feedback, dict) else {}
    )
    if not isinstance(transfer_feedback, dict):
        transfer_feedback = {}

    investment_feedback = (
        matching_feedback.get("investment") if isinstance(matching_feedback, dict) else {}
    )
    if not isinstance(investment_feedback, dict):
        investment_feedback = {}

    learning_feedback = (
        matching_feedback.get("learning") if isinstance(matching_feedback, dict) else {}
    )
    if not isinstance(learning_feedback, dict):
        learning_feedback = {}

    review_status = str(transfer_feedback.get("review_status") or "").strip().lower()
    if review_status not in {"accepted", "rejected"}:
        review_status = "pending" if transfer_suggestion.get("suggested_preview_type") else ""
    reviewed_type = str(transfer_feedback.get("reviewed_type") or "")
    suppressed = bool(transfer_feedback.get("suppressed")) or review_status == "rejected"

    has_investment_signal = bool(float(investment_signal.get("score", 0.0) or 0.0) > 0.0) or any(
        str(investment_signal.get(field) or "").strip()
        for field in ("level", "reason", "platform", "product")
    )
    investment_review_status = str(
        investment_feedback.get("review_status") or ""
    ).strip().lower()
    if investment_review_status not in {"accepted", "rejected"} or not has_investment_signal:
        investment_review_status = "pending" if has_investment_signal else ""
    investment_suppressed = has_investment_signal and (
        bool(investment_feedback.get("suppressed")) or investment_review_status == "rejected"
    )

    has_learning_signal = (
        _normalize_int_or_none(learning_recommendation.get("rule_id")) is not None
        or bool(float(learning_recommendation.get("score", 0.0) or 0.0) > 0.0)
        or any(
            str(learning_recommendation.get(field) or "").strip()
            for field in ("level", "reason", "recommended_type", "summary")
        )
    )
    learning_review_status = str(learning_feedback.get("review_status") or "").strip().lower()
    if learning_review_status not in {"accepted", "rejected"} or not has_learning_signal:
        learning_review_status = "pending" if has_learning_signal else ""
    learning_suppressed = has_learning_signal and (
        bool(learning_feedback.get("suppressed")) or learning_review_status == "rejected"
    )

    payload = PreviewMatchingPayload(
        transfer=TransferMatchingPayload(
            candidate_type=str(transfer_suggestion.get("suggested_preview_type") or ""),
            score=float(transfer_suggestion.get("score", 0.0) or 0.0),
            level=str(transfer_suggestion.get("level") or ""),
            reason=str(transfer_suggestion.get("reason") or ""),
            review_status=review_status,
            reviewed_type=reviewed_type,
            suppressed=suppressed,
        ),
        investment=InvestmentMatchingPayload(
            score=float(investment_signal.get("score", 0.0) or 0.0),
            level=str(investment_signal.get("level") or ""),
            reason=str(investment_signal.get("reason") or ""),
            platform=str(investment_signal.get("platform") or ""),
            product=str(investment_signal.get("product") or ""),
            review_status=investment_review_status,
            suppressed=investment_suppressed,
        ),
        learning=LearningMatchingPayload(
            rule_id=_normalize_int_or_none(learning_recommendation.get("rule_id")),
            score=float(learning_recommendation.get("score", 0.0) or 0.0),
            level=str(learning_recommendation.get("level") or ""),
            reason=str(learning_recommendation.get("reason") or ""),
            recommended_type=str(learning_recommendation.get("recommended_type") or ""),
            summary=str(learning_recommendation.get("summary") or ""),
            review_status=learning_review_status,
            suppressed=learning_suppressed,
        ),
        recurring=RecurringMatchingPayload(
            id=_normalize_int_or_none(preview.get("preview_recurring_id")),
            name=str(preview.get("preview_recurring_name") or ""),
            candidate_count=int(
                preview.get("preview_recurring_candidate_count", 0) or 0
            ),
            match_score=float(preview.get("preview_recurring_match_score", 0.0) or 0.0),
            match_reasons=str(preview.get("preview_recurring_match_reasons") or ""),
            matched_date=str(preview.get("preview_recurring_matched_date") or ""),
        ),
        dedup=DedupMatchingPayload(
            type=str(preview.get("dedup_type") or ""),
            source_ids=_normalize_source_ids_value(preview.get("dedup_source_ids")),
        ),
        parser=ParserMatchingPayload(
            id=str(preview.get("preview_parser_id") or ""),
            tags=[
                str(tag)
                for tag in _normalize_list_value(preview.get("preview_parser_tags"))
                if str(tag)
            ],
        ),
        annotation=AnnotationMatchingPayload(is_manually_annotated=bool(is_manually_annotated)),
    )
    return payload.to_dict()
