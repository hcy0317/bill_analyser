"""Helpers for additive import-preview matching payloads."""

from __future__ import annotations

from typing import Any

from .models import (
    AnnotationMatchingPayload,
    DedupMatchingPayload,
    InvestmentMatchingPayload,
    LearningMatchingPayload,
    LLMRecommendationPayload,
    ParserMatchingPayload,
    PreviewMatchingPayload,
    RecurringMatchingPayload,
    ReconciliationMatchingPayload,
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


_PARSER_DISPLAY_LABELS = {
    "wechat": "微信",
    "alipay": "支付宝",
    "abc": "农业银行",
    "ccb": "建设银行",
    "cmbc": "民生银行",
    "icbc": "工商银行",
    "generic": "通用来源",
}


def _parser_display_label(parser_id: Any) -> str:
    """Convert normalized parser IDs into user-facing source labels."""
    normalized_parser_id = str(parser_id or "").strip().lower()
    if not normalized_parser_id:
        return ""
    return _PARSER_DISPLAY_LABELS.get(normalized_parser_id, normalized_parser_id)


def _dedupe_text_items(items: list[str]) -> list[str]:
    """Deduplicate user-facing text fragments while preserving order."""
    deduped: list[str] = []
    for item in items:
        normalized_item = str(item or "").strip()
        if normalized_item and normalized_item not in deduped:
            deduped.append(normalized_item)
    return deduped


def _infer_source_labels_from_preview_parser_metadata(preview: dict[str, Any]) -> list[str]:
    """Best-effort parser-only fallback when structured source-chain metadata is incomplete."""
    primary_parser_id = str(preview.get("preview_parser_id") or "").strip().lower()
    parser_groups = _build_parser_tag_groups(
        preview.get("preview_parser_tags"),
        primary_parser_id=primary_parser_id,
    )
    parser_labels = [
        _parser_display_label(str(group.get("parser_id") or "").strip().lower())
        for group in parser_groups
        if str(group.get("parser_id") or "").strip()
    ]
    if primary_parser_id:
        parser_labels.insert(0, _parser_display_label(primary_parser_id))
    return _dedupe_text_items([label for label in parser_labels if label])


def _build_parser_tag_groups(raw_tags: Any, *, primary_parser_id: str = "") -> list[dict[str, Any]]:
    """Group parser tags by parser occurrence while preserving tag order."""
    tags = [str(tag).strip() for tag in _normalize_list_value(raw_tags) if str(tag).strip()]
    groups: list[dict[str, Any]] = []
    leading_tags: list[str] = []

    for tag in tags:
        if tag.startswith("parser:"):
            parser_id = tag.split(":", 1)[1].strip().lower()
            if groups and groups[-1]["parser_id"] == parser_id:
                if tag not in groups[-1]["tags"]:
                    groups[-1]["tags"].append(tag)
                continue

            group_tags = [*leading_tags, tag] if leading_tags else [tag]
            groups.append({"parser_id": parser_id, "tags": group_tags})
            leading_tags = []
            continue

        if groups:
            if tag not in groups[-1]["tags"]:
                groups[-1]["tags"].append(tag)
        elif tag not in leading_tags:
            leading_tags.append(tag)

    normalized_primary_parser_id = str(primary_parser_id or "").strip().lower()
    if not normalized_primary_parser_id:
        return groups

    primary_group_index = next(
        (
            index
            for index, group in enumerate(groups)
            if str(group.get("parser_id") or "").strip().lower() == normalized_primary_parser_id
        ),
        None,
    )
    if primary_group_index is None:
        return [
            {"parser_id": normalized_primary_parser_id, "tags": [f"parser:{normalized_primary_parser_id}"]},
            *groups,
        ]

    if primary_group_index == 0:
        return groups

    primary_group = groups.pop(primary_group_index)
    return [primary_group, *groups]


def _extract_channel_value(tags: list[str]) -> str:
    """Return the first normalized channel tag value from a grouped tag list."""
    for tag in tags:
        if tag.startswith("channel:"):
            return tag.split(":", 1)[1].strip().lower()
    return ""


def _select_source_label(parser_id: str) -> str:
    """Use parser-derived labels as the canonical source display."""
    parser_label = _parser_display_label(parser_id)
    return parser_label or parser_id


def _build_preview_source_chain(preview: dict[str, Any]) -> list[dict[str, Any]]:
    """Derive stable source-chain metadata from persisted parser tags."""
    primary_parser_id = str(preview.get("preview_parser_id") or "").strip().lower()
    tag_groups = _build_parser_tag_groups(
        preview.get("preview_parser_tags"),
        primary_parser_id=primary_parser_id,
    )
    dedup_type = str(preview.get("dedup_type") or "").strip().lower()
    is_transfer_pair = dedup_type == "transfer" and len(tag_groups) >= 2
    is_platform_duplicate = dedup_type == "platform_bank" and len(tag_groups) >= 2

    source_chain: list[dict[str, Any]] = []
    for index, group in enumerate(tag_groups):
        parser_id = str(group.get("parser_id") or "").strip().lower()
        if not parser_id:
            continue

        if is_transfer_pair:
            role = "outgoing" if index == 0 else "incoming"
        elif is_platform_duplicate:
            role = "kept" if index == 0 else "duplicate"
        else:
            role = "primary" if index == 0 else "related"

        parser_label = _parser_display_label(parser_id)
        label = _select_source_label(parser_id)
        account_id = None
        if role in {"outgoing", "primary", "kept"}:
            account_id = _normalize_int_or_none(preview.get("preview_source_account_id"))
        elif role == "incoming":
            account_id = _normalize_int_or_none(preview.get("preview_destination_account_id"))

        source_chain.append(
            {
                "position": index,
                "role": role,
                "parser_id": parser_id,
                "parser_label": parser_label,
                "label": label or parser_label or parser_id,
                "channel": _extract_channel_value(list(group.get("tags") or [])),
                "tags": [str(tag) for tag in list(group.get("tags") or []) if str(tag).strip()],
                "account_id": account_id,
            }
        )

    return source_chain


def _build_dedup_source_metadata(
    preview: dict[str, Any],
    source_chain: list[dict[str, Any]],
) -> tuple[int, list[str], list[dict[str, Any]]]:
    """Project persisted dedup hints into stable source metadata."""
    dedup_type = str(preview.get("dedup_type") or "").strip().lower()
    dedup_source_ids = _normalize_source_ids_value(preview.get("dedup_source_ids"))
    has_meaningful_dedup = dedup_type not in {"", "remaining"}

    if dedup_type in {"transfer", "platform_bank"} and len(source_chain) >= 2:
        relevant_sources = [dict(source) for source in source_chain]
    else:
        relevant_sources = []

    source_labels = _dedupe_text_items(
        [
        str(source.get("label") or source.get("parser_label") or source.get("parser_id") or "").strip()
        for source in relevant_sources
        if str(source.get("label") or source.get("parser_label") or source.get("parser_id") or "").strip()
        ]
    )
    if dedup_type == "platform_bank" and len(source_labels) < 2:
        source_labels = _dedupe_text_items([
            *source_labels,
            *_infer_source_labels_from_preview_parser_metadata(preview),
        ])

    source_count = 0
    if has_meaningful_dedup:
        source_count = max(len(dedup_source_ids), len(relevant_sources), len(source_labels))

    return source_count, source_labels, relevant_sources


def _select_reconciliation_candidate(candidates: list[dict[str, Any]]) -> dict[str, Any]:
    status_order = {
        "merged": 0,
        "accepted": 0,
        "pending": 1,
        "rejected": 2,
        "rolled_back": 3,
    }
    return sorted(
        candidates,
        key=lambda candidate: (
            status_order.get(str(candidate.get("status") or ""), 9),
            -float(candidate.get("score") or 0.0),
            int(candidate.get("id") or 0),
        ),
    )[0] if candidates else {}


def _build_reconciliation_payload(
    candidates: list[dict[str, Any]] | None,
) -> ReconciliationMatchingPayload:
    candidate = _select_reconciliation_candidate(
        [dict(item) for item in list(candidates or []) if isinstance(item, dict)]
    )
    if not candidate:
        return ReconciliationMatchingPayload()

    return ReconciliationMatchingPayload(
        candidate_id=str(candidate.get("candidate_id") or ""),
        candidate_type=str(candidate.get("candidate_type") or ""),
        status=str(candidate.get("status") or ""),
        existing_bill_id=_normalize_int_or_none(candidate.get("existing_bill_id")),
        group_id=_normalize_int_or_none(candidate.get("group_id")),
        score=float(candidate.get("score") or 0.0),
        level=str(candidate.get("level") or ""),
        reason=str(candidate.get("reason") or ""),
        signal_label=str(candidate.get("signal_label") or ""),
        source_chain=(
            list(candidate.get("source_chain") or [])
            if isinstance(candidate.get("source_chain"), list)
            else []
        ),
    )


def build_preview_matching_payload(
    preview: dict[str, Any],
    *,
    transfer_suggestion: dict[str, Any] | None = None,
    investment_signal: dict[str, Any] | None = None,
    learning_recommendation: dict[str, Any] | None = None,
    llm_recommendation: dict[str, Any] | None = None,
    matching_feedback: dict[str, Any] | None = None,
    is_manually_annotated: bool = False,
    reconciliation_candidates: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    # pylint: disable=too-many-arguments,too-many-locals
    """Group existing preview hints into a stable additive matching payload."""
    transfer_suggestion = transfer_suggestion or {}
    investment_signal = investment_signal or {}
    learning_recommendation = learning_recommendation or {}
    llm_recommendation = llm_recommendation or {}
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
    feedback_learning_rule_id = _normalize_int_or_none(learning_feedback.get("rule_id"))
    live_learning_rule_id = _normalize_int_or_none(learning_recommendation.get("rule_id"))
    learning_review_status = str(learning_feedback.get("review_status") or "").strip().lower()
    if learning_review_status in {"accepted", "rejected"}:
        if has_learning_signal and live_learning_rule_id not in (None, feedback_learning_rule_id):
            learning_review_status = "pending"
    else:
        learning_review_status = "pending" if has_learning_signal else ""
    learning_suppressed = bool(learning_feedback.get("suppressed")) or learning_review_status == "rejected"
    learning_rule_id = live_learning_rule_id
    if learning_rule_id is None:
        learning_rule_id = feedback_learning_rule_id

    if learning_review_status in {"accepted", "rejected"} and not has_learning_signal:
        learning_suppressed = learning_review_status == "rejected"

    if learning_review_status == "accepted" and not has_learning_signal:
        learning_suppressed = False

    if learning_review_status in {"accepted", "rejected"} and not str(learning_recommendation.get("summary") or ""):
        learning_recommendation = {
            **learning_recommendation,
            "rule_id": learning_rule_id,
        }

    learning_suppressed = learning_suppressed and (
        has_learning_signal or learning_review_status in {"accepted", "rejected"}
    )

    parser_source_chain = _build_preview_source_chain(preview)
    dedup_source_count, dedup_source_labels, dedup_sources = _build_dedup_source_metadata(
        preview,
        parser_source_chain,
    )
    transfer_source_chain = [dict(source) for source in parser_source_chain if source.get("role") in {"outgoing", "incoming"}]
    transfer_pair_order = "outgoing_first" if len(transfer_source_chain) >= 2 else ""

    # LLM yellow signal
    llm_feedback = (
        matching_feedback.get("llm") if isinstance(matching_feedback, dict) else {}
    )
    if not isinstance(llm_feedback, dict):
        llm_feedback = {}
    explicit_llm_signal = bool(
        str(llm_recommendation.get("suggested_main_category") or "").strip()
        or str(llm_recommendation.get("suggested_sub_category") or "").strip()
        or str(llm_recommendation.get("suggested_source_account") or "").strip()
        or str(llm_recommendation.get("suggested_destination_account") or "").strip()
        or str(llm_recommendation.get("reason") or "").strip()
        or float(llm_recommendation.get("confidence", 0.0) or 0.0) > 0.0
    )
    if not explicit_llm_signal and llm_feedback:
        llm_recommendation = {
            "suggested_main_category": llm_feedback.get("suggested_main_category"),
            "suggested_sub_category": llm_feedback.get("suggested_sub_category"),
            "suggested_source_account": llm_feedback.get("suggested_source_account"),
            "suggested_destination_account": llm_feedback.get("suggested_destination_account"),
            "confidence": llm_feedback.get("confidence", 0.0),
            "reason": llm_feedback.get("reason", ""),
        }
    has_llm_signal = bool(
        str(llm_recommendation.get("suggested_main_category") or "").strip()
        or str(llm_recommendation.get("suggested_sub_category") or "").strip()
        or str(llm_recommendation.get("suggested_source_account") or "").strip()
        or str(llm_recommendation.get("suggested_destination_account") or "").strip()
        or str(llm_recommendation.get("reason") or "").strip()
        or float(llm_recommendation.get("confidence", 0.0) or 0.0) > 0.0
    )
    llm_review_status = str(llm_feedback.get("review_status") or "").strip().lower()
    if llm_review_status not in {"accepted", "rejected"}:
        llm_review_status = "pending" if has_llm_signal else ""
    llm_suppressed = bool(llm_feedback.get("suppressed")) or llm_review_status == "rejected"

    payload = PreviewMatchingPayload(
        transfer=TransferMatchingPayload(
            candidate_type=str(transfer_suggestion.get("suggested_preview_type") or ""),
            score=float(transfer_suggestion.get("score", 0.0) or 0.0),
            level=str(transfer_suggestion.get("level") or ""),
            reason=str(transfer_suggestion.get("reason") or ""),
            review_status=review_status,
            reviewed_type=reviewed_type,
            suppressed=suppressed,
            pair_order=transfer_pair_order,
            source_chain=transfer_source_chain,
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
        llm=LLMRecommendationPayload(
            suggested_main_category=str(llm_recommendation.get("suggested_main_category") or ""),
            suggested_sub_category=str(llm_recommendation.get("suggested_sub_category") or ""),
            suggested_source_account=str(llm_recommendation.get("suggested_source_account") or ""),
            suggested_destination_account=str(llm_recommendation.get("suggested_destination_account") or ""),
            confidence=float(llm_recommendation.get("confidence", 0.0) or 0.0),
            reason=str(llm_recommendation.get("reason") or ""),
            review_status=llm_review_status,
            suppressed=llm_suppressed,
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
            source_count=dedup_source_count,
            source_labels=dedup_source_labels,
            sources=dedup_sources,
        ),
        parser=ParserMatchingPayload(
            id=str(preview.get("preview_parser_id") or ""),
            tags=[
                str(tag)
                for tag in _normalize_list_value(preview.get("preview_parser_tags"))
                if str(tag)
            ],
            source_chain=parser_source_chain,
        ),
        annotation=AnnotationMatchingPayload(
            is_manually_annotated=bool(is_manually_annotated),
        ),
        reconciliation=_build_reconciliation_payload(reconciliation_candidates),
    )
    result = payload.to_dict()
    learning_extras = {
        key: learning_recommendation.get(key)
        for key in (
            "source",
            "mode",
            "auto_apply",
            "model_version",
            "confidence",
            "margin",
            "confirmations",
            "feature_schema_version",
            "policy_version",
        )
        if learning_recommendation.get(key) not in (None, "")
    }
    if learning_extras:
        result["learning"].update(learning_extras)
    return result
