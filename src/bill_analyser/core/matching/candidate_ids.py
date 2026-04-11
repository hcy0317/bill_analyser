"""Helpers for stable matching candidate identifiers."""

from __future__ import annotations

from typing import Any


def build_formal_transfer_candidate_id(anchor_bill_id: Any, candidate_bill_id: Any) -> str:
    """Build a stable candidate id for a historical formal-bill transfer candidate."""
    return f"bill:{int(anchor_bill_id)}:transfer:{int(candidate_bill_id)}"


def parse_matching_candidate_id(candidate_id: str) -> dict[str, Any] | None:
    """Parse a stable matching candidate id into a normalized scope descriptor."""
    normalized_candidate_id = str(candidate_id or "").strip()
    if not normalized_candidate_id:
        return None

    parts = normalized_candidate_id.split(":")
    parsed_candidate: dict[str, Any] | None = None
    try:
        if len(parts) == 3 and parts[0] == "preview":
            preview_id = int(parts[1])
            if preview_id > 0:
                parsed_candidate = {
                    "scope": "preview",
                    "preview_id": preview_id,
                    "kind": str(parts[2] or "").strip().lower(),
                }
        elif len(parts) == 4 and parts[0] == "bill":
            bill_id = int(parts[1])
            candidate_bill_id = int(parts[3])
            if bill_id > 0 and candidate_bill_id > 0:
                parsed_candidate = {
                    "scope": "bill",
                    "bill_id": bill_id,
                    "candidate_bill_id": candidate_bill_id,
                    "kind": str(parts[2] or "").strip().lower(),
                }
    except (TypeError, ValueError):
        return None

    return parsed_candidate
