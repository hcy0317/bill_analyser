"""Helpers for stable matching candidate identifiers."""

from __future__ import annotations

import hashlib
import json
from typing import Any


def build_formal_transfer_candidate_id(anchor_bill_id: Any, candidate_bill_id: Any) -> str:
    """Build a stable candidate id for a historical formal-bill transfer candidate."""
    return f"bill:{int(anchor_bill_id)}:transfer:{int(candidate_bill_id)}"


def normalize_learning_rule_revision(raw_rule_revision: Any) -> str:
    normalized_rule_revision = "".join(character for character in str(raw_rule_revision or "").strip() if character.isalnum())
    return normalized_rule_revision or "0"


def build_learning_rule_revision(rule: dict[str, Any]) -> str:
    """Build a semantic revision token for a learning rule from its matching/apply-relevant fields."""

    def _normalize_nullable_id(raw_value: Any) -> str:
        return "" if raw_value in (None, "", 0, "0") else str(int(raw_value))

    revision_payload = {
        "match_type": str(rule.get("match_type") or ""),
        "match_value": str(rule.get("match_value") or ""),
        "normalized_match_value": str(rule.get("normalized_match_value") or ""),
        "parser_id": str(rule.get("parser_id") or ""),
        "composite_match_hash": str(rule.get("composite_match_hash") or ""),
        "match_features_json": str(rule.get("match_features_json") or ""),
        "learned_type": str(rule.get("learned_type") or ""),
        "learned_category_id": _normalize_nullable_id(rule.get("learned_category_id")),
        "learned_source_account_id": _normalize_nullable_id(rule.get("learned_source_account_id")),
        "learned_destination_account_id": _normalize_nullable_id(rule.get("learned_destination_account_id")),
    }
    revision_source = json.dumps(revision_payload, ensure_ascii=False, sort_keys=True)
    return hashlib.sha1(revision_source.encode("utf-8")).hexdigest()[:16]


def build_formal_learning_candidate_id(anchor_bill_id: Any, rule_id: Any, rule_revision: Any) -> str:
    """Build a stable, revision-aware candidate id for a historical formal-bill learning candidate."""
    return f"bill:{int(anchor_bill_id)}:learning:{int(rule_id)}:{normalize_learning_rule_revision(rule_revision)}"


def parse_matching_candidate_id(candidate_id: str) -> dict[str, Any] | None:
    """Parse a stable matching candidate id into a normalized scope descriptor."""
    normalized_candidate_id = str(candidate_id or "").strip()
    if not normalized_candidate_id:
        return None

    parts = normalized_candidate_id.split(":")
    parsed_candidate: dict[str, Any] | None = None
    try:
        if (
            len(parts) == 6
            and parts[0] == "reconcile"
            and parts[1] == "import"
            and parts[3] == "bill"
        ):
            kind = str(parts[2] or "").strip().lower()
            bill_id = int(parts[4])
            import_key_hash = str(parts[5] or "").strip()
            if kind in {"transfer", "duplicate"} and bill_id > 0 and import_key_hash:
                parsed_candidate = {
                    "scope": "reconciliation",
                    "kind": kind,
                    "existing_bill_id": bill_id,
                    "import_key_hash": import_key_hash,
                }
        elif len(parts) == 3 and parts[0] == "preview":
            preview_id = int(parts[1])
            if preview_id > 0:
                parsed_candidate = {
                    "scope": "preview",
                    "preview_id": preview_id,
                    "kind": str(parts[2] or "").strip().lower(),
                }
        elif len(parts) == 5 and parts[0] == "bill" and str(parts[2] or "").strip().lower() == "learning":
            bill_id = int(parts[1])
            rule_id = int(parts[3])
            rule_revision = str(parts[4] or "").strip()
            if bill_id > 0 and rule_id > 0 and rule_revision:
                parsed_candidate = {
                    "scope": "bill",
                    "bill_id": bill_id,
                    "kind": "learning",
                    "rule_id": rule_id,
                    "rule_revision": rule_revision,
                }
        elif len(parts) == 4 and parts[0] == "bill":
            bill_id = int(parts[1])
            target_id = int(parts[3])
            kind = str(parts[2] or "").strip().lower()
            if bill_id > 0 and target_id > 0:
                parsed_candidate = {
                    "scope": "bill",
                    "bill_id": bill_id,
                    "kind": kind,
                }
                if kind == "learning":
                    parsed_candidate["rule_id"] = target_id
                else:
                    parsed_candidate["candidate_bill_id"] = target_id
    except (TypeError, ValueError):
        return None

    return parsed_candidate
