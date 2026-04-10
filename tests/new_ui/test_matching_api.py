from __future__ import annotations

import asyncio
import time

from tests.new_ui.test_bills_api import _build_isolated_auth_headers, _get_current_user_id


class TestMatchingAPI:
    """matching 只读 session 候选 API 回归。"""

    def test_matching_session_candidates_returns_projected_candidates(self, client):
        """应返回由 preview[].matching 投影得到的显式 candidate 列表。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_api_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-session-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> None:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-06-10 10:20:00",
                    "preview_type": "支出",
                    "preview_amount": 88.8,
                    "preview_destination_amount": 88.8,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_source_account_id": 11,
                    "preview_destination_account_id": 22,
                    "preview_counterparty": "pytest matching vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest matching api",
                    "preview_parser_id": "wechat",
                    "preview_parser_tags": ["parser:wechat", "channel:wallet"],
                    "preview_recurring_id": 9,
                    "preview_recurring_name": "pytest recurring candidate",
                    "preview_recurring_candidate_count": 1,
                    "preview_recurring_match_score": 0.88,
                    "preview_recurring_match_reasons": "schedule|amount",
                    "preview_recurring_matched_date": "2026-06-10",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[91, 92],
            )

        asyncio.run(_create_preview_item())

        response = client.get(f"/api/matching/sessions/{session_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["session_id"] == session_id
        assert data["data"]["summary"] == {
            "preview_count": 1,
            "candidate_count": 2,
            "counts_by_kind": {
                "transfer": 1,
                "investment": 0,
                "learning": 0,
                "recurring": 1,
            },
        }

        candidates = data["data"]["candidates"]
        assert [candidate["candidate_id"] for candidate in candidates] == [
            f"preview:{candidates[0]['preview_id']}:transfer",
            f"preview:{candidates[1]['preview_id']}:recurring",
        ]
        assert candidates[0]["kind"] == "transfer"
        assert candidates[0]["status"] == "pending"
        assert candidates[0]["details"]["candidate_type"] == "转账"
        assert candidates[0]["context"]["dedup"] == {"type": "transfer", "source_ids": [91, 92]}
        assert candidates[0]["context"]["parser"] == {
            "id": "wechat",
            "tags": ["parser:wechat", "channel:wallet"],
        }
        assert candidates[1]["kind"] == "recurring"
        assert candidates[1]["score"] == 0.88
        assert candidates[1]["level"] == "high"
        assert candidates[1]["details"]["name"] == "pytest recurring candidate"

    def test_matching_session_candidates_returns_empty_list_for_empty_session(self, client):
        """session 存在但没有 preview/candidate 时应返回空列表，而不是 404。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_api_empty")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-empty-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_empty_session() -> None:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)

        asyncio.run(_create_empty_session())

        response = client.get(f"/api/matching/sessions/{session_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"] == {
            "session_id": session_id,
            "summary": {
                "preview_count": 0,
                "candidate_count": 0,
                "counts_by_kind": {
                    "transfer": 0,
                    "investment": 0,
                    "learning": 0,
                    "recurring": 0,
                },
            },
            "candidates": [],
        }

    def test_matching_session_candidates_is_user_scoped(self, client):
        """非当前用户的 import session 不应泄露 matching 候选。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_api_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_api_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-user-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_session() -> None:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)

        asyncio.run(_create_primary_session())

        response = client.get(f"/api/matching/sessions/{session_id}/candidates", headers=secondary_headers)

        assert response.status_code == 404
        data = response.get_json()
        assert data["success"] is False
        assert data["error"] == "Import session not found"
