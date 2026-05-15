from __future__ import annotations

from bill_analyser.core.matching import preview_matching as pm
from bill_analyser.core.matching.candidate_ids import (
    build_formal_learning_candidate_id,
    build_formal_transfer_candidate_id,
    build_learning_rule_revision,
    normalize_learning_rule_revision,
    parse_matching_candidate_id,
)
from bill_analyser.core.matching.transfer_candidates import (
    _coerce_float,
    _coerce_int,
    _derive_level,
    build_transfer_pair_candidate,
)


def test_candidate_id_helpers_cover_formal_preview_reconcile_and_invalid_shapes() -> None:
    assert build_formal_transfer_candidate_id("10", "11") == "bill:10:transfer:11"
    assert normalize_learning_rule_revision(" rev:1-2 ") == "rev12"
    assert normalize_learning_rule_revision(None) == "0"

    revision = build_learning_rule_revision(
        {
            "match_type": "composite",
            "match_value": "Coffee",
            "normalized_match_value": "coffee",
            "parser_id": "wechat",
            "composite_match_hash": "hash",
            "match_features_json": '{"counterparty":"Coffee"}',
            "learned_type": "expense",
            "learned_category_id": "6",
            "learned_source_account_id": "0",
            "learned_destination_account_id": None,
        }
    )
    assert len(revision) == 16
    assert build_formal_learning_candidate_id(30, 8, "rev:1-2") == "bill:30:learning:8:rev12"

    assert parse_matching_candidate_id("") is None
    assert parse_matching_candidate_id("reconcile:import:duplicate:bill:401:preview-3") == {
        "scope": "reconciliation",
        "kind": "duplicate",
        "existing_bill_id": 401,
        "import_key_hash": "preview-3",
    }
    assert parse_matching_candidate_id("preview:12:transfer") == {
        "scope": "preview",
        "preview_id": 12,
        "kind": "transfer",
    }
    assert parse_matching_candidate_id(f"bill:30:learning:8:{revision}") == {
        "scope": "bill",
        "bill_id": 30,
        "kind": "learning",
        "rule_id": 8,
        "rule_revision": revision,
    }
    assert parse_matching_candidate_id("bill:30:transfer:31") == {
        "scope": "bill",
        "bill_id": 30,
        "kind": "transfer",
        "candidate_bill_id": 31,
    }
    assert parse_matching_candidate_id("bill:x:transfer:31") is None


def test_transfer_candidate_helpers_cover_rejection_edges_and_score_levels() -> None:
    anchor = {
        "id": 100,
        "date": "2026-04-01 09:00:00",
        "type": "expense",
        "amount": -50,
        "source_account_id": 10,
    }
    target = {
        "id": 101,
        "date": "2026-04-03 09:00:00",
        "type": "income",
        "amount": 50,
        "counterparty": "Card",
        "description": "Transfer in",
        "payment_method": "card",
        "main_category": "Transfer",
        "sub_category": "",
        "source_account_id": 11,
        "destination_account_id": "0",
    }

    candidate = build_transfer_pair_candidate(anchor, target)
    assert candidate is not None
    assert candidate["candidate_id"] == "bill:100:transfer:101"
    assert candidate["reason"].endswith("date_window")
    assert candidate["bill"]["destination_account_id"] == 0

    assert _coerce_float("not-a-number") == 0.0
    assert _coerce_int("not-a-number") is None
    assert _derive_level(0.7) == "medium"
    assert _derive_level(0.2) == "low"
    assert _derive_level(0.0) == ""

    assert build_transfer_pair_candidate({**anchor, "type": "transfer"}, target) is None
    assert build_transfer_pair_candidate({**anchor, "id": 101}, target) is None
    assert build_transfer_pair_candidate(anchor, {**target, "amount": -50}) is None
    assert build_transfer_pair_candidate(anchor, {**target, "source_account_id": 10}) is None
    assert build_transfer_pair_candidate(anchor, {**target, "source_account_id": ""}) is None
    assert build_transfer_pair_candidate(anchor, {**target, "date": "bad-date"}) is None
    assert build_transfer_pair_candidate(anchor, {**target, "date": "2026-04-10 09:00:00"}) is None


def test_preview_matching_payload_covers_source_reconciliation_and_feedback_edges() -> None:
    assert pm._normalize_list_value(None) == []
    assert pm._normalize_list_value(("a", "b")) == ["a", "b"]
    assert pm._normalize_list_value("tag") == ["tag"]
    assert pm._normalize_source_ids_value("1, x, 2") == [1, "x", 2]
    assert pm._normalize_int_or_none("bad") is None
    assert pm._parser_display_label("") == ""
    assert pm._parser_display_label("abc") == "农业银行"

    groups = pm._build_parser_tag_groups(
        ["channel:bank", "parser:alipay", "tag:a", "parser:wechat", "tag:b"],
        primary_parser_id="wechat",
    )
    assert [group["parser_id"] for group in groups] == ["wechat", "alipay"]
    assert pm._build_parser_tag_groups(["tag:a"], primary_parser_id="abc")[0]["parser_id"] == "abc"

    preview = {
        "preview_parser_id": "abc",
        "preview_parser_tags": [
            "parser:abc",
            "channel:bank",
            "parser:alipay",
            "channel:wallet",
        ],
        "dedup_type": "platform_bank",
        "dedup_source_ids": ("201", "202"),
        "preview_source_account_id": "10",
        "preview_destination_account_id": "11",
        "preview_recurring_id": "9",
        "preview_recurring_name": "Monthly Rent",
        "preview_recurring_candidate_count": 2,
        "preview_recurring_match_score": 0.91,
        "preview_recurring_match_reasons": "same_amount|monthly",
        "preview_recurring_matched_date": "2026-04-01",
    }

    source_chain = pm._build_preview_source_chain({**preview, "dedup_type": "transfer"})
    assert [source["role"] for source in source_chain] == ["outgoing", "incoming"]
    assert [source["account_id"] for source in source_chain] == [10, 11]

    payload = pm.build_preview_matching_payload(
        preview,
        transfer_suggestion={"suggested_preview_type": "transfer", "score": 0.7, "level": "medium", "reason": "pair"},
        investment_signal={"score": 0.8, "level": "high", "reason": "keyword", "platform": "Acme", "product": "Fund"},
        learning_recommendation={},
        llm_recommendation={},
        matching_feedback={
            "transfer": "stale",
            "investment": {"review_status": "rejected", "suppressed": True},
            "learning": {"review_status": "accepted", "rule_id": 55},
            "llm": {
                "review_status": "rejected",
                "suppressed": True,
                "suggested_main_category": "Food",
                "confidence": 0.6,
                "reason": "feedback only",
            },
        },
        reconciliation_candidates=[
            {"id": 2, "candidate_id": "pending", "status": "pending", "score": 0.99},
            {
                "id": 1,
                "candidate_id": "merged",
                "candidate_type": "duplicate",
                "status": "merged",
                "existing_bill_id": "401",
                "group_id": "7",
                "score": 0.7,
                "level": "high",
                "reason": "same",
                "signal_label": "duplicate import",
                "source_chain": [{"label": "import"}],
            },
        ],
    )

    assert payload["transfer"]["review_status"] == "pending"
    assert payload["investment"]["review_status"] == "rejected"
    assert payload["investment"]["suppressed"] is True
    assert payload["learning"]["review_status"] == "accepted"
    assert payload["learning"]["suppressed"] is False
    assert payload["llm"]["review_status"] == "rejected"
    assert payload["llm"]["suggested_main_category"] == "Food"
    assert payload["dedup"]["source_count"] == 2
    assert payload["dedup"]["source_labels"] == ["农业银行", "支付宝"]
    assert payload["reconciliation"]["candidate_id"] == "merged"
    assert payload["reconciliation"]["existing_bill_id"] == 401
