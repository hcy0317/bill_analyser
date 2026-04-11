from __future__ import annotations

from bill_analyser.core.matching import build_transfer_pair_candidates


def test_build_transfer_pair_candidates_adds_anchor_scoped_candidate_ids_and_keeps_order() -> None:
    """历史 transfer candidate 应生成稳定 candidate_id，且不改变既有排序。"""
    anchor_bill = {
        "id": 11,
        "date": "2026-07-20 09:00:00",
        "type": "支出",
        "amount": -88.0,
        "source_account_id": 1,
    }
    candidate_bills = [
        {
            "id": 12,
            "date": "2026-07-20 09:03:00",
            "type": "收入",
            "amount": 88.0,
            "counterparty": "pytest transfer near",
            "description": "pytest transfer near",
            "payment_method": "银行卡",
            "main_category": "转账",
            "sub_category": "历史后配对",
            "source_account_id": 2,
            "destination_account_id": 0,
        },
        {
            "id": 13,
            "date": "2026-07-21 09:03:00",
            "type": "收入",
            "amount": 88.0,
            "counterparty": "pytest transfer far",
            "description": "pytest transfer far",
            "payment_method": "银行卡",
            "main_category": "转账",
            "sub_category": "历史后配对",
            "source_account_id": 3,
            "destination_account_id": 0,
        },
    ]

    result = build_transfer_pair_candidates(anchor_bill, candidate_bills)

    assert [candidate["candidate_id"] for candidate in result] == [
        "bill:11:transfer:12",
        "bill:11:transfer:13",
    ]
    assert [candidate["bill_id"] for candidate in result] == [12, 13]
    assert result[0]["score"] >= result[1]["score"]


def test_build_transfer_pair_candidates_candidate_id_depends_on_anchor_bill_id() -> None:
    """历史 candidate_id 应随 anchor bill 变化，而不是被 pair 归一化。"""
    first_anchor_bill = {
        "id": 21,
        "date": "2026-07-20 10:00:00",
        "type": "支出",
        "amount": -66.0,
        "source_account_id": 1,
    }
    second_anchor_bill = {
        "id": 22,
        "date": "2026-07-20 10:01:00",
        "type": "支出",
        "amount": -66.0,
        "source_account_id": 2,
    }
    candidate_bill = {
        "id": 31,
        "date": "2026-07-20 10:03:00",
        "type": "收入",
        "amount": 66.0,
        "counterparty": "pytest transfer target",
        "description": "pytest transfer target",
        "payment_method": "银行卡",
        "main_category": "转账",
        "sub_category": "历史后配对",
        "source_account_id": 3,
        "destination_account_id": 0,
    }

    first_result = build_transfer_pair_candidates(first_anchor_bill, [candidate_bill])
    second_result = build_transfer_pair_candidates(second_anchor_bill, [candidate_bill])

    assert first_result[0]["candidate_id"] == "bill:21:transfer:31"
    assert second_result[0]["candidate_id"] == "bill:22:transfer:31"
