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
