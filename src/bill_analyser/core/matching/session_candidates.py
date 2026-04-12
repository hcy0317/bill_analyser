"""Helpers for projecting preview matching payloads into session-scoped candidates."""

from __future__ import annotations

from typing import Any

_CANDIDATE_KIND_ORDER = ("transfer", "investment", "learning", "recurring")


def _normalize_dict(raw_value: Any) -> dict[str, Any]:
    return dict(raw_value) if isinstance(raw_value, dict) else {}


def _normalize_list(raw_value: Any) -> list[Any]:
    if raw_value in (None, ""):
        return []
    if isinstance(raw_value, list):
        return list(raw_value)
    if isinstance(raw_value, (tuple, set)):
        return list(raw_value)
    return [raw_value]


def _coerce_float(raw_value: Any) -> float:
    try:
        return float(raw_value or 0.0)
    except (TypeError, ValueError):
        return 0.0


def _derive_level_from_score(score: float) -> str:
    if score >= 0.8:
        return "high"
    if score >= 0.65:
        return "medium"
    if score > 0:
        return "low"
    return ""


def _has_transfer_candidate(details: dict[str, Any]) -> bool:
    review_status = str(details.get("review_status") or "").strip().lower()
    return bool(str(details.get("candidate_type") or "").strip()) or review_status in {
        "accepted",
        "rejected",
    }


def _has_investment_candidate(details: dict[str, Any]) -> bool:
    review_status = str(details.get("review_status") or "").strip().lower()
    has_signal = _coerce_float(details.get("score")) > 0 or any(
        str(details.get(field) or "").strip()
        for field in ("level", "reason", "platform", "product")
    )
    return has_signal or review_status in {"accepted", "rejected"}


def _has_learning_candidate(details: dict[str, Any]) -> bool:
    review_status = str(details.get("review_status") or "").strip().lower()
    return (
        details.get("rule_id") not in (None, "")
        or _coerce_float(details.get("score")) > 0
        or any(
            str(details.get(field) or "").strip()
            for field in ("level", "reason", "recommended_type", "summary")
        )
        or review_status in {"accepted", "rejected"}
    )


def _has_recurring_candidate(details: dict[str, Any]) -> bool:
    return (
        details.get("id") not in (None, "")
        or int(details.get("candidate_count", 0) or 0) > 0
        or any(
            str(details.get(field) or "").strip()
            for field in ("name", "match_reasons", "matched_date")
        )
        or _coerce_float(details.get("match_score")) > 0
    )


def _should_include_candidate(kind: str, details: dict[str, Any]) -> bool:
    if kind == "transfer":
        return _has_transfer_candidate(details)
    if kind == "investment":
        return _has_investment_candidate(details)
    if kind == "learning":
        return _has_learning_candidate(details)
    if kind == "recurring":
        return _has_recurring_candidate(details)
    return False


def _build_candidate_context(matching: dict[str, Any]) -> dict[str, Any]:
    dedup = _normalize_dict(matching.get("dedup"))
    parser = _normalize_dict(matching.get("parser"))
    annotation = _normalize_dict(matching.get("annotation"))

    return {
        "dedup": {
            "type": str(dedup.get("type") or ""),
            "source_ids": _normalize_list(dedup.get("source_ids")),
        },
        "parser": {
            "id": str(parser.get("id") or ""),
            "tags": [str(tag) for tag in _normalize_list(parser.get("tags")) if str(tag)],
        },
        "annotation": {
            "is_manually_annotated": bool(annotation.get("is_manually_annotated")),
        },
    }


def _build_candidate_preview(preview: dict[str, Any]) -> dict[str, Any]:
    return {
        "preview_date": preview.get("preview_date", ""),
        "preview_type": preview.get("preview_type", ""),
        "preview_amount": preview.get("preview_amount", 0),
        "preview_destination_amount": preview.get("preview_destination_amount", 0),
        "preview_main_category": preview.get("preview_main_category", ""),
        "preview_sub_category": preview.get("preview_sub_category", ""),
        "preview_source_account_id": preview.get("preview_source_account_id"),
        "preview_destination_account_id": preview.get("preview_destination_account_id"),
        "preview_counterparty": preview.get("preview_counterparty", ""),
        "preview_payment_method": preview.get("preview_payment_method", ""),
        "preview_description": preview.get("preview_description", ""),
        "preview_selected": bool(preview.get("preview_selected", False)),
    }


def _build_candidate_details(kind: str, details: dict[str, Any]) -> dict[str, Any]:
    if kind == "recurring":
        return {
            "id": details.get("id"),
            "name": str(details.get("name") or ""),
            "candidate_count": int(details.get("candidate_count", 0) or 0),
            "match_score": _coerce_float(details.get("match_score")),
            "match_reasons": str(details.get("match_reasons") or ""),
            "matched_date": str(details.get("matched_date") or ""),
        }

    if kind == "transfer":
        return {
            "candidate_type": str(details.get("candidate_type") or ""),
            "score": _coerce_float(details.get("score")),
            "level": str(details.get("level") or ""),
            "reason": str(details.get("reason") or ""),
            "review_status": str(details.get("review_status") or ""),
            "reviewed_type": str(details.get("reviewed_type") or ""),
            "suppressed": bool(details.get("suppressed")),
        }

    if kind == "investment":
        return {
            "score": _coerce_float(details.get("score")),
            "level": str(details.get("level") or ""),
            "reason": str(details.get("reason") or ""),
            "platform": str(details.get("platform") or ""),
            "product": str(details.get("product") or ""),
            "review_status": str(details.get("review_status") or ""),
            "suppressed": bool(details.get("suppressed")),
        }

    return {
        "rule_id": details.get("rule_id"),
        "score": _coerce_float(details.get("score")),
        "level": str(details.get("level") or ""),
        "reason": str(details.get("reason") or ""),
        "recommended_type": str(details.get("recommended_type") or ""),
        "summary": str(details.get("summary") or ""),
        "review_status": str(details.get("review_status") or ""),
        "suppressed": bool(details.get("suppressed")),
    }


def _build_candidate(
    session_id: str,
    preview: dict[str, Any],
    kind: str,
    details: dict[str, Any],
    context: dict[str, Any],
) -> dict[str, Any]:
    preview_id = int(preview.get("id") or 0)
    normalized_details = _build_candidate_details(kind, details)

    if kind == "recurring":
        score = _coerce_float(normalized_details.get("match_score"))
        level = _derive_level_from_score(score)
        reason = str(normalized_details.get("match_reasons") or "")
        status = "confirmed" if normalized_details.get("id") not in (None, "") else "pending"
    elif kind == "investment":
        score = _coerce_float(normalized_details.get("score"))
        level = str(normalized_details.get("level") or "")
        reason = str(normalized_details.get("reason") or "")
        status = str(normalized_details.get("review_status") or "") or "pending"
    elif kind == "learning":
        score = _coerce_float(normalized_details.get("score"))
        level = str(normalized_details.get("level") or "")
        reason = str(normalized_details.get("reason") or "")
        status = str(normalized_details.get("review_status") or "") or "pending"
    else:
        score = _coerce_float(normalized_details.get("score"))
        level = str(normalized_details.get("level") or "")
        reason = str(normalized_details.get("reason") or "")
        if kind == "transfer":
            status = str(normalized_details.get("review_status") or "") or "pending"
        else:
            status = "pending"

    return {
        "candidate_id": f"preview:{preview_id}:{kind}",
        "kind": kind,
        "session_id": session_id,
        "preview_id": preview_id,
        "score": score,
        "level": level,
        "reason": reason,
        "status": status,
        "details": normalized_details,
        "preview": _build_candidate_preview(preview),
        "context": context,
    }


def build_matching_session_candidates(
    session_id: str,
    previews: list[dict[str, Any]] | None,
) -> dict[str, Any]:
    """Project additive preview matching payloads into session-scoped candidate lists."""
    candidates: list[dict[str, Any]] = []
    counts_by_kind = dict.fromkeys(_CANDIDATE_KIND_ORDER, 0)

    for preview in previews or []:
        matching = _normalize_dict(preview.get("matching"))
        context = _build_candidate_context(matching)

        for kind in _CANDIDATE_KIND_ORDER:
            details = _normalize_dict(matching.get(kind))
            if not _should_include_candidate(kind, details):
                continue

            candidates.append(
                _build_candidate(session_id, preview, kind, details, context)
            )
            counts_by_kind[kind] += 1

    return {
        "session_id": session_id,
        "summary": {
            "preview_count": len(previews or []),
            "candidate_count": len(candidates),
            "counts_by_kind": counts_by_kind,
        },
        "candidates": candidates,
    }
