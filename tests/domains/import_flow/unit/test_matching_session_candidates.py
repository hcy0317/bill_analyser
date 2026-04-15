from __future__ import annotations

import pytest

from bill_analyser.core.bill_service import BillService
from bill_analyser.core.matching import build_matching_session_candidates


def test_build_matching_session_candidates_projects_transfer_recurring_and_context() -> None:
    """matching session helper 应将 preview[].matching 投影成稳定候选列表。"""
    result = build_matching_session_candidates(
        "session-matching-1",
        [
            {
                "id": 7,
                "preview_date": "2026-04-10 09:00:00",
                "preview_type": "支出",
                "preview_amount": 88.8,
                "preview_destination_amount": 88.8,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_source_account_id": 1,
                "preview_destination_account_id": 2,
                "preview_counterparty": "pytest vendor",
                "preview_payment_method": "银行卡",
                "preview_description": "pytest matching",
                "preview_selected": True,
                "matching": {
                    "transfer": {
                        "candidate_type": "转账",
                        "score": 0.88,
                        "level": "high",
                        "reason": "dedup_pair",
                        "review_status": "pending",
                        "reviewed_type": "",
                        "suppressed": False,
                    },
                    "investment": {"score": 0, "level": "", "reason": "", "platform": "", "product": ""},
                    "learning": {
                        "rule_id": None,
                        "score": 0,
                        "level": "",
                        "reason": "",
                        "recommended_type": "",
                        "summary": "",
                    },
                    "recurring": {
                        "id": 9,
                        "name": "每月早餐",
                        "candidate_count": 1,
                        "match_score": 0.91,
                        "match_reasons": "schedule|amount",
                        "matched_date": "2026-04-10",
                    },
                    "dedup": {"type": "transfer", "source_ids": [91, 92]},
                    "parser": {"id": "wechat", "tags": ["parser:wechat", "channel:wallet"]},
                    "annotation": {"is_manually_annotated": True},
                },
            }
        ],
    )

    assert result["session_id"] == "session-matching-1"
    assert result["summary"] == {
        "preview_count": 1,
        "candidate_count": 2,
        "counts_by_kind": {
            "transfer": 1,
            "investment": 0,
            "learning": 0,
            "recurring": 1,
        },
    }
    assert [candidate["candidate_id"] for candidate in result["candidates"]] == [
        "preview:7:transfer",
        "preview:7:recurring",
    ]
    assert result["candidates"][0]["context"] == {
        "dedup": {"type": "transfer", "source_ids": [91, 92]},
        "parser": {"id": "wechat", "tags": ["parser:wechat", "channel:wallet"]},
        "annotation": {"is_manually_annotated": True},
    }
    assert result["candidates"][1]["score"] == 0.91
    assert result["candidates"][1]["level"] == "high"
    assert result["candidates"][1]["reason"] == "schedule|amount"
    assert result["candidates"][1]["status"] == "confirmed"


def test_build_matching_session_candidates_keeps_pending_recurring_after_manual_clear() -> None:
    """清除 preview recurring 绑定后，若仍有候选数量，应保留 pending recurring candidate。"""
    result = build_matching_session_candidates(
        "session-cleared-recurring",
        [
            {
                "id": 21,
                "preview_date": "2026-04-11 08:00:00",
                "preview_type": "支出",
                "preview_amount": 20.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_source_account_id": 1,
                "preview_destination_account_id": None,
                "preview_counterparty": "早餐店",
                "preview_payment_method": "银行卡",
                "preview_description": "pending recurring",
                "preview_selected": True,
                "matching": {
                    "transfer": {},
                    "investment": {},
                    "learning": {},
                    "recurring": {
                        "id": None,
                        "name": "",
                        "candidate_count": 2,
                        "match_score": 0,
                        "match_reasons": "",
                        "matched_date": "",
                    },
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ],
    )

    assert result["summary"]["candidate_count"] == 1
    assert result["candidates"][0]["kind"] == "recurring"
    assert result["candidates"][0]["status"] == "pending"
    assert result["candidates"][0]["details"]["candidate_count"] == 2


def test_build_matching_session_candidates_keeps_reviewed_transfer_without_live_suggestion() -> None:
    """已 accept/reject 的 transfer 决策即使不再有 candidate_type，也应保留为候选记录。"""
    result = build_matching_session_candidates(
        "session-reviewed-transfer",
        [
            {
                "id": 12,
                "preview_date": "2026-04-10 10:00:00",
                "preview_type": "转账",
                "preview_amount": 12.3,
                "preview_destination_amount": 12.3,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": 1,
                "preview_destination_account_id": 2,
                "preview_counterparty": "内部转账",
                "preview_payment_method": "银行卡",
                "preview_description": "accepted transfer",
                "preview_selected": True,
                "matching": {
                    "transfer": {
                        "candidate_type": "",
                        "score": 0.0,
                        "level": "",
                        "reason": "",
                        "review_status": "accepted",
                        "reviewed_type": "转账",
                        "suppressed": False,
                    },
                    "investment": {},
                    "learning": {},
                    "recurring": {},
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ],
    )

    assert result["summary"]["candidate_count"] == 1
    assert result["candidates"][0]["kind"] == "transfer"
    assert result["candidates"][0]["status"] == "accepted"
    assert result["candidates"][0]["details"]["reviewed_type"] == "转账"


def test_build_matching_session_candidates_projects_rejected_investment_candidate_status() -> None:
    """investment candidate 在 preview feedback reject 后仍应保留并投影为 rejected。"""
    result = build_matching_session_candidates(
        "session-reviewed-investment",
        [
            {
                "id": 31,
                "preview_date": "2026-04-12 10:00:00",
                "preview_type": "投资",
                "preview_amount": 88.0,
                "preview_destination_amount": 88.0,
                "preview_main_category": "投资理财",
                "preview_sub_category": "基金",
                "preview_source_account_id": 1,
                "preview_destination_account_id": 2,
                "preview_counterparty": "蚂蚁财富",
                "preview_payment_method": "支付宝",
                "preview_description": "黄金ETF 自动定投",
                "preview_selected": True,
                "matching": {
                    "transfer": {},
                    "investment": {
                        "score": 0.81,
                        "level": "high",
                        "reason": "investment_keyword",
                        "platform": "蚂蚁财富",
                        "product": "黄金ETF",
                        "review_status": "rejected",
                        "suppressed": True,
                    },
                    "learning": {},
                    "recurring": {},
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ],
    )

    assert result["summary"]["candidate_count"] == 1
    assert result["summary"]["counts_by_kind"]["investment"] == 1
    assert result["candidates"][0]["kind"] == "investment"
    assert result["candidates"][0]["status"] == "rejected"
    assert result["candidates"][0]["details"]["suppressed"] is True


def test_build_matching_session_candidates_projects_rejected_learning_candidate_status() -> None:
    """learning candidate 在 preview feedback reject 后仍应保留并投影为 rejected。"""
    result = build_matching_session_candidates(
        "session-reviewed-learning",
        [
            {
                "id": 32,
                "preview_date": "2026-04-12 10:00:00",
                "preview_type": "支出",
                "preview_amount": 38.0,
                "preview_destination_amount": 0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": 1,
                "preview_destination_account_id": None,
                "preview_counterparty": "星巴克",
                "preview_payment_method": "支付宝",
                "preview_description": "咖啡消费",
                "preview_selected": True,
                "matching": {
                    "transfer": {},
                    "investment": {},
                    "learning": {
                        "rule_id": 42,
                        "score": 0.88,
                        "level": "high",
                        "reason": "parser_id:exact",
                        "recommended_type": "支出",
                        "summary": "支出 | 餐饮/咖啡",
                        "review_status": "rejected",
                        "suppressed": True,
                    },
                    "recurring": {},
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ],
    )

    assert result["summary"]["candidate_count"] == 1
    assert result["summary"]["counts_by_kind"]["learning"] == 1
    assert result["candidates"][0]["kind"] == "learning"
    assert result["candidates"][0]["status"] == "rejected"
    assert result["candidates"][0]["details"]["suppressed"] is True


def test_build_matching_session_candidates_projects_accepted_learning_candidate_status() -> None:
    """learning candidate 在 preview feedback accept 后仍应保留并投影为 accepted。"""
    result = build_matching_session_candidates(
        "session-accepted-learning",
        [
            {
                "id": 33,
                "preview_date": "2026-04-12 11:00:00",
                "preview_type": "收入",
                "preview_amount": 38.0,
                "preview_destination_amount": 0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "咖啡",
                "preview_source_account_id": 1,
                "preview_destination_account_id": 2,
                "preview_counterparty": "星巴克",
                "preview_payment_method": "支付宝",
                "preview_description": "咖啡消费",
                "preview_selected": True,
                "matching": {
                    "transfer": {},
                    "investment": {},
                    "learning": {
                        "rule_id": 43,
                        "score": 0.9,
                        "level": "high",
                        "reason": "parser_id:exact",
                        "recommended_type": "收入",
                        "summary": "收入 | 餐饮/咖啡 | 星巴克账户 → 会员账户",
                        "review_status": "accepted",
                        "suppressed": False,
                    },
                    "recurring": {},
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ],
    )

    assert result["summary"]["candidate_count"] == 1
    assert result["summary"]["counts_by_kind"]["learning"] == 1
    assert result["candidates"][0]["kind"] == "learning"
    assert result["candidates"][0]["status"] == "accepted"
    assert result["candidates"][0]["details"]["rule_id"] == 43
    assert result["candidates"][0]["details"]["suppressed"] is False


@pytest.mark.asyncio
async def test_bill_service_get_matching_session_candidates_wraps_preview_projection(monkeypatch: pytest.MonkeyPatch) -> None:
    """BillService 应提供 matching session 候选的薄包装方法。"""
    service = BillService(db=None)

    async def fake_get_import_preview(session_id: str, selected_only: bool = False, user_id: int = 1):
        assert session_id == "session-service-wrapper"
        assert selected_only is False
        assert user_id == 7
        return [
            {
                "id": 1,
                "preview_date": "2026-04-10 11:00:00",
                "preview_type": "支出",
                "preview_amount": 18.8,
                "preview_destination_amount": 0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": 1,
                "preview_destination_account_id": None,
                "preview_counterparty": "测试商户",
                "preview_payment_method": "支付宝",
                "preview_description": "早餐",
                "preview_selected": True,
                "matching": {
                    "transfer": {
                        "candidate_type": "转账",
                        "score": 0.66,
                        "level": "medium",
                        "reason": "keyword:转账",
                        "review_status": "pending",
                        "reviewed_type": "",
                        "suppressed": False,
                    },
                    "investment": {},
                    "learning": {},
                    "recurring": {},
                    "dedup": {},
                    "parser": {},
                    "annotation": {},
                },
            }
        ]

    monkeypatch.setattr(service, "get_import_preview", fake_get_import_preview)

    result = await service.get_matching_session_candidates("session-service-wrapper", user_id=7)

    assert result["session_id"] == "session-service-wrapper"
    assert result["summary"]["candidate_count"] == 1
    assert result["candidates"][0]["kind"] == "transfer"


@pytest.mark.asyncio
async def test_bill_service_reconcile_matching_history_aggregates_unique_bill_results(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService 应聚合唯一 billIds，并按请求顺序返回 formal-bill transfer 结果。"""
    service = BillService(db=None)
    calls: list[tuple[int, int]] = []

    async def fake_get_matching_bill_candidates(bill_id: int, user_id: int = 1) -> dict[str, object]:
        calls.append((bill_id, user_id))
        return {
            "success": True,
            "bill_id": bill_id,
            "linked_pair": None,
            "candidates": [
                {
                    "candidate_id": f"bill:{bill_id}:transfer:{bill_id + 100}",
                    "bill_id": bill_id + 100,
                    "score": 0.91,
                    "level": "high",
                    "reason": "same_amount|opposite_sign",
                    "bill": {"id": bill_id + 100},
                }
            ],
        }

    monkeypatch.setattr(service, "get_matching_bill_candidates", fake_get_matching_bill_candidates)

    result = await service.reconcile_matching_history([12, 11, 12], user_id=7)

    assert calls == [(12, 7), (11, 7)]
    assert result["success"] is True
    assert result["summary"] == {
        "bill_count": 2,
        "candidate_count": 2,
        "linked_pair_count": 0,
    }
    assert [item["bill_id"] for item in result["results"]] == [12, 11]
    assert result["results"][0]["candidates"][0]["candidate_id"] == "bill:12:transfer:112"


@pytest.mark.asyncio
async def test_bill_service_reconcile_matching_history_preserves_first_bill_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService 历史调和聚合遇到 bill 级失败时应原样返回错误。"""
    service = BillService(db=None)

    async def fake_get_matching_bill_candidates(bill_id: int, user_id: int = 1) -> dict[str, object]:
        _ = user_id
        if bill_id == 99:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        return {
            "success": True,
            "bill_id": bill_id,
            "linked_pair": None,
            "candidates": [],
        }

    monkeypatch.setattr(service, "get_matching_bill_candidates", fake_get_matching_bill_candidates)

    result = await service.reconcile_matching_history([11, 99], user_id=7)

    assert result == {
        "success": False,
        "error": "Bill not found",
        "status_code": 404,
        "bill_id": 99,
    }


@pytest.mark.asyncio
async def test_bill_service_reconcile_matching_history_counts_unique_linked_pairs_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """当请求同时包含同一 pair 两侧 bill 时，linked_pair_count 应按唯一 pair 计数。"""
    service = BillService(db=None)

    async def fake_get_matching_bill_candidates(bill_id: int, user_id: int = 1) -> dict[str, object]:
        _ = user_id
        return {
            "success": True,
            "bill_id": bill_id,
            "linked_pair": {
                "id": 3,
                "pair_type": "transfer",
                "source": "manual",
                "left_bill_id": 11,
                "right_bill_id": 12,
                "other_bill_id": 12 if bill_id == 11 else 11,
            },
            "candidates": [],
        }

    monkeypatch.setattr(service, "get_matching_bill_candidates", fake_get_matching_bill_candidates)

    result = await service.reconcile_matching_history([11, 12], user_id=7)

    assert result["success"] is True
    assert result["summary"] == {
        "bill_count": 2,
        "candidate_count": 0,
        "linked_pair_count": 1,
    }


@pytest.mark.asyncio
async def test_bill_service_reconcile_matching_history_keeps_investment_linked_pair_out_of_transfer_only_batch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """reconcile-history 当前仍应保持 transfer-only，不应把 investment linkedPair 外溢到批量结果。"""
    service = BillService(db=None)

    async def fake_get_matching_bill_candidates(bill_id: int, user_id: int = 1) -> dict[str, object]:
        _ = user_id
        return {
            "success": True,
            "bill_id": bill_id,
            "linked_pair": {
                "id": 17,
                "pair_type": "investment",
                "source": "manual",
                "left_bill_id": 11,
                "right_bill_id": 12,
                "other_bill_id": 12 if bill_id == 11 else 11,
            },
            "candidates": [],
        }

    monkeypatch.setattr(service, "get_matching_bill_candidates", fake_get_matching_bill_candidates)

    result = await service.reconcile_matching_history([11], user_id=7)

    assert result == {
        "success": True,
        "results": [
            {
                "bill_id": 11,
                "linked_pair": None,
                "candidates": [],
            }
        ],
        "summary": {
            "bill_count": 1,
            "candidate_count": 0,
            "linked_pair_count": 0,
        },
    }


@pytest.mark.asyncio
async def test_bill_service_update_preview_recurring_match_rejects_stale_transfer_review(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService recurring 写路径应把 transfer review 状态纳入 CAS 检查。"""

    class FakeDb:
        def __init__(self) -> None:
            self.update_calls: list[tuple[int, int | None, dict[str, object], int]] = []

        async def get_preview_bill_by_id(self, preview_id: int, user_id: int = 1) -> dict[str, object] | None:
            _ = (preview_id, user_id)
            return {
                "id": 1,
                "session_id": "session-recurring-cas",
                "preview_type": "支出",
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_recurring_id": None,
                "preview_matching_feedback_json": "",
                "preview_matching_feedback": {},
            }

        async def update_preview_recurring_match(
            self,
            preview_id: int,
            recurring_id: int | None,
            *,
            user_id: int = 1,
            expected_state: dict[str, object] | None = None,
        ) -> dict[str, object]:
            self.update_calls.append((preview_id, recurring_id, dict(expected_state or {}), user_id))
            return {"id": preview_id, "session_id": "session-recurring-cas"}

    fake_db = FakeDb()
    service = BillService(db=fake_db)  # type: ignore[arg-type]

    async def fake_get_preview_category_id(_preview: dict[str, object], user_id: int = 1) -> int | None:
        _ = user_id
        return 10

    async def fake_get_preview_transfer_review_status(_preview: dict[str, object]) -> str:
        return "rejected"

    monkeypatch.setattr(service, "_get_preview_category_id", fake_get_preview_category_id)
    monkeypatch.setattr(service, "_get_preview_transfer_review_status", fake_get_preview_transfer_review_status)

    result = await service.update_preview_recurring_match(
        1,
        9,
        expected_state={
            "sessionId": "session-recurring-cas",
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": 10,
            "recurringId": None,
        },
        user_id=7,
    )

    assert result == {
        "success": False,
        "error": "Preview state changed, please refresh",
        "status_code": 409,
    }
    assert fake_db.update_calls == []


@pytest.mark.asyncio
async def test_bill_service_accept_matching_candidate_dispatches_preview_recurring_to_update_preview_recurring_match(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService generic accept 应把 preview recurring 分发到 update_preview_recurring_match。"""
    service = BillService(db=None)
    calls: list[tuple[int, int, dict[str, object] | None, int]] = []

    async def fake_update_preview_recurring_match(
        preview_id: int,
        recurring_id: int | None,
        *,
        expected_state: dict[str, object] | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, int(recurring_id or 0), expected_state, user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-preview-recurring",
            "recurring_id": recurring_id,
            "preview": [
                {
                    "id": preview_id,
                    "preview_recurring_id": recurring_id,
                    "matching": {"recurring": {"id": recurring_id, "candidate_count": 2}},
                }
            ],
        }

    monkeypatch.setattr(service, "update_preview_recurring_match", fake_update_preview_recurring_match)

    expected_state = {
        "sessionId": "session-preview-recurring",
        "reviewStatus": "pending",
        "previewType": "支出",
        "categoryId": 8,
        "recurringId": None,
    }
    result = await service._accept_matching_candidate(  # pylint: disable=protected-access
        "preview:1:recurring",
        {"recurringId": 9, "expectedState": expected_state},
        user_id=7,
    )

    assert result == {
        "success": True,
        "candidate_id": "preview:1:recurring",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-preview-recurring",
        "recurring_id": 9,
        "preview": [
            {
                "id": 1,
                "preview_recurring_id": 9,
                "matching": {"recurring": {"id": 9, "candidate_count": 2}},
            }
        ],
    }
    assert calls == [(1, 9, expected_state, 7)]


@pytest.mark.asyncio
async def test_bill_service_accept_matching_candidate_dispatches_preview_investment_to_feedback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService generic accept 应把 preview investment 分发到 preview-scoped feedback 写路径。"""
    service = BillService(db=None)
    calls: list[tuple[int, str, dict[str, object], int]] = []

    async def fake_apply_preview_investment_decision(
        preview_id: int,
        decision: str,
        *,
        expected_state: dict[str, object] | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, decision, dict(expected_state or {}), user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-investment-accept",
            "preview": [
                {
                    "id": preview_id,
                    "matching": {"investment": {"review_status": "accepted", "suppressed": False}},
                }
            ],
        }

    monkeypatch.setattr(service, "apply_preview_investment_decision", fake_apply_preview_investment_decision)

    payload = {
        "expectedState": {
            "sessionId": "session-investment-accept",
            "reviewStatus": "pending",
            "previewType": "投资",
            "categoryId": 10,
            "recurringId": None,
        }
    }
    result = await service._accept_matching_candidate(  # pylint: disable=protected-access
        "preview:1:investment",
        payload,
        user_id=7,
    )

    assert calls == [
        (
            1,
            "accept",
            {
                "sessionId": "session-investment-accept",
                "reviewStatus": "pending",
                "previewType": "投资",
                "categoryId": 10,
                "recurringId": None,
            },
            7,
        )
    ]
    assert result == {
        "success": True,
        "candidate_id": "preview:1:investment",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-investment-accept",
        "preview": [
            {
                "id": 1,
                "matching": {"investment": {"review_status": "accepted", "suppressed": False}},
            }
        ],
    }


@pytest.mark.asyncio
async def test_bill_service_accept_matching_candidate_dispatches_preview_learning_with_rule_id(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """BillService generic accept 应把 preview learning 分发到带 ruleId 绑定的写路径。"""
    service = BillService(db=None)
    calls: list[tuple[int, str, int, dict[str, object], int]] = []

    async def fake_apply_preview_learning_decision(
        preview_id: int,
        decision: str,
        *,
        expected_state: dict[str, object] | None = None,
        rule_id: int | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, decision, int(rule_id or 0), dict(expected_state or {}), user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-learning-accept",
            "preview": [
                {
                    "id": preview_id,
                    "preview_type": "收入",
                    "matching": {
                        "learning": {
                            "rule_id": int(rule_id or 0),
                            "review_status": "accepted",
                            "suppressed": False,
                        }
                    },
                }
            ],
        }

    monkeypatch.setattr(service, "apply_preview_learning_decision", fake_apply_preview_learning_decision)

    payload = {
        "ruleId": 42,
        "expectedState": {
            "sessionId": "session-learning-accept",
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
        },
    }
    result = await service._accept_matching_candidate(  # pylint: disable=protected-access
        "preview:1:learning",
        payload,
        user_id=7,
    )

    assert calls == [
        (
            1,
            "accept",
            42,
            {
                "sessionId": "session-learning-accept",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
            },
            7,
        )
    ]
    assert result == {
        "success": True,
        "candidate_id": "preview:1:learning",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-learning-accept",
        "preview": [
            {
                "id": 1,
                "preview_type": "收入",
                "matching": {
                    "learning": {
                        "rule_id": 42,
                        "review_status": "accepted",
                        "suppressed": False,
                    }
                },
            }
        ],
    }


@pytest.mark.asyncio
async def test_bill_service_accept_matching_candidate_dispatches_historical_investment_to_persisted_pair(
) -> None:
    """generic accept 的 bill investment 分支应复用 persisted pair link 写路径。"""

    class FakeDb:
        def __init__(self) -> None:
            self.calls: list[tuple[int, int, int]] = []

        async def create_manual_investment_pair(
            self,
            bill_id: int,
            candidate_bill_id: int,
            *,
            user_id: int = 1,
        ) -> dict[str, int | str]:
            self.calls.append((bill_id, candidate_bill_id, user_id))
            return {
                "id": 91,
                "pair_type": "investment",
                "source": "manual",
                "left_bill_id": min(bill_id, candidate_bill_id),
                "right_bill_id": max(bill_id, candidate_bill_id),
            }

    fake_db = FakeDb()
    service = BillService(db=fake_db)  # type: ignore[arg-type]

    result = await service._accept_matching_candidate(  # pylint: disable=protected-access
        "bill:11:investment:12",
        {},
        user_id=7,
    )

    assert fake_db.calls == [(11, 12, 7)]
    assert result == {
        "success": True,
        "candidate_id": "bill:11:investment:12",
        "action": "accept",
        "pair": {
            "id": 91,
            "pair_type": "investment",
            "source": "manual",
            "left_bill_id": 11,
            "right_bill_id": 12,
        },
    }


@pytest.mark.asyncio
async def test_bill_service_reject_matching_candidate_dispatches_preview_recurring_to_clear_match(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject 的 preview recurring 分支应复用 clear recurring-match 写路径。"""
    service = BillService(db=None)
    calls: list[tuple[int, int | None, dict[str, object], int]] = []

    async def fake_update_preview_recurring_match(
        preview_id: int,
        recurring_id: int | None,
        *,
        expected_state: dict[str, object] | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, recurring_id, dict(expected_state or {}), user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-recurring-reject",
            "preview": [
                {
                    "id": preview_id,
                    "matching": {
                        "recurring": {
                            "id": None,
                            "candidate_count": 2,
                        }
                    },
                }
            ],
        }

    monkeypatch.setattr(service, "update_preview_recurring_match", fake_update_preview_recurring_match)

    payload = {
        "expectedState": {
            "sessionId": "session-recurring-reject",
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": 10,
            "recurringId": 9,
        }
    }
    result = await service._reject_matching_candidate(
        "preview:1:recurring",
        payload,
        user_id=7,
    )

    assert calls == [
        (
            1,
            None,
            {
                "sessionId": "session-recurring-reject",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": 10,
                "recurringId": 9,
            },
            7,
        )
    ]
    assert result == {
        "success": True,
        "candidate_id": "preview:1:recurring",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-recurring-reject",
        "preview": [
            {
                "id": 1,
                "matching": {
                    "recurring": {
                        "id": None,
                        "candidate_count": 2,
                    }
                },
            }
        ],
    }


@pytest.mark.asyncio
async def test_bill_service_reject_matching_candidate_dispatches_preview_investment_to_feedback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject 的 preview investment 分支应复用 preview-scoped feedback 写路径。"""
    service = BillService(db=None)
    calls: list[tuple[int, str, dict[str, object], int]] = []

    async def fake_apply_preview_investment_decision(
        preview_id: int,
        decision: str,
        *,
        expected_state: dict[str, object] | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, decision, dict(expected_state or {}), user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-investment-reject",
            "preview": [
                {
                    "id": preview_id,
                    "matching": {"investment": {"review_status": "rejected", "suppressed": True}}
                }
            ],
        }

    monkeypatch.setattr(service, "apply_preview_investment_decision", fake_apply_preview_investment_decision)

    payload = {
        "expectedState": {
            "sessionId": "session-investment-reject",
            "reviewStatus": "pending",
            "previewType": "投资",
            "categoryId": 10,
            "recurringId": None,
        }
    }
    result = await service._reject_matching_candidate(
        "preview:1:investment",
        payload,
        user_id=7,
    )

    assert calls == [
        (
            1,
            "reject",
            {
                "sessionId": "session-investment-reject",
                "reviewStatus": "pending",
                "previewType": "投资",
                "categoryId": 10,
                "recurringId": None,
            },
            7,
        )
    ]
    assert result == {
        "success": True,
        "candidate_id": "preview:1:investment",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-investment-reject",
        "preview": [
            {
                "id": 1,
                "matching": {"investment": {"review_status": "rejected", "suppressed": True}}
            }
        ],
    }


@pytest.mark.asyncio
async def test_bill_service_reject_matching_candidate_dispatches_preview_learning_to_feedback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject 的 preview learning 分支应复用 preview-scoped feedback 写路径。"""
    service = BillService(db=None)
    calls: list[tuple[int, str, dict[str, object], int]] = []

    async def fake_apply_preview_learning_decision(
        preview_id: int,
        decision: str,
        *,
        expected_state: dict[str, object] | None = None,
        user_id: int = 1,
    ) -> dict[str, object]:
        calls.append((preview_id, decision, dict(expected_state or {}), user_id))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-learning-reject",
            "preview": [
                {
                    "id": preview_id,
                    "matching": {"learning": {"review_status": "rejected", "suppressed": True}}
                }
            ],
        }

    monkeypatch.setattr(service, "apply_preview_learning_decision", fake_apply_preview_learning_decision, raising=False)

    payload = {
        "expectedState": {
            "sessionId": "session-learning-reject",
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
        }
    }
    result = await service._reject_matching_candidate(
        "preview:1:learning",
        payload,
        user_id=7,
    )

    assert calls == [
        (
            1,
            "reject",
            {
                "sessionId": "session-learning-reject",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
            },
            7,
        )
    ]
    assert result == {
        "success": True,
        "candidate_id": "preview:1:learning",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-learning-reject",
        "preview": [
            {
                "id": 1,
                "matching": {"learning": {"review_status": "rejected", "suppressed": True}}
            }
        ],
    }


@pytest.mark.asyncio
async def test_bill_service_reject_matching_candidate_dispatches_historical_transfer_to_persisted_suppression() -> None:
    """generic reject 的 bill transfer 分支应复用 persisted suppression 写路径。"""

    class FakeDb:
        def __init__(self) -> None:
            self.calls: list[tuple[int, int, int]] = []

        async def reject_bill_transfer_candidate(
            self,
            bill_id: int,
            candidate_bill_id: int,
            *,
            user_id: int = 1,
        ) -> dict[str, int]:
            self.calls.append((bill_id, candidate_bill_id, user_id))
            return {
                "left_bill_id": min(bill_id, candidate_bill_id),
                "right_bill_id": max(bill_id, candidate_bill_id),
            }

    fake_db = FakeDb()
    service = BillService(db=fake_db)  # type: ignore[arg-type]

    result = await service._reject_matching_candidate(
        "bill:11:transfer:12",
        {},
        user_id=7,
    )

    assert fake_db.calls == [(11, 12, 7)]
    assert result == {
        "success": True,
        "candidate_id": "bill:11:transfer:12",
        "action": "reject",
    }


@pytest.mark.asyncio
async def test_bill_service_reject_matching_candidate_dispatches_historical_investment_to_persisted_suppression(
) -> None:
    """generic reject 的 bill investment 分支应复用 persisted suppression 写路径。"""

    class FakeDb:
        def __init__(self) -> None:
            self.calls: list[tuple[int, int, int]] = []

        async def reject_bill_investment_candidate(
            self,
            bill_id: int,
            candidate_bill_id: int,
            *,
            user_id: int = 1,
        ) -> dict[str, int]:
            self.calls.append((bill_id, candidate_bill_id, user_id))
            return {
                "left_bill_id": min(bill_id, candidate_bill_id),
                "right_bill_id": max(bill_id, candidate_bill_id),
            }

    fake_db = FakeDb()
    service = BillService(db=fake_db)  # type: ignore[arg-type]

    result = await service._reject_matching_candidate(
        "bill:11:investment:12",
        {},
        user_id=7,
    )

    assert fake_db.calls == [(11, 12, 7)]
    assert result == {
        "success": True,
        "candidate_id": "bill:11:investment:12",
        "action": "reject",
    }
