from __future__ import annotations

import asyncio
import time

from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _create_test_import_session,
    _create_test_recurring_template,
    _ensure_test_account,
    _ensure_test_expense_category,
    _find_category_id_by_name,
    _get_current_user_id,
)


def _create_account_via_db(user_id: int, name: str) -> int:
    from src.api.app import db

    async def _create() -> int:
        account_id = await db.create_account(
            {
                "name": name,
                "type": 1,
                "category": "asset",
                "currency": "CNY",
                "icon": "",
                "color": "",
                "balance": 0.0,
                "initial_balance": 0.0,
                "hidden": False,
                "display_order": 0,
                "comment": "",
                "aliases": [],
            },
            user_id=user_id,
        )
        assert account_id is not None
        return int(account_id)

    return asyncio.run(_create())


def _create_bill_via_db(
    user_id: int,
    *,
    source_account_id: int,
    amount: float,
    bill_type: str,
    date: str,
    description: str,
    destination_account_id: int = 0,
) -> int:
    from src.api.app import db

    async def _create() -> int:
        bill_id = await db.create_bill(
            {
                "date": date,
                "type": bill_type,
                "amount": amount,
                "counterparty": "pytest matching api",
                "description": description,
                "payment_method": "银行卡",
                "main_category": "转账",
                "sub_category": "历史后配对",
                "source_account_id": source_account_id,
                "destination_account_id": destination_account_id,
                "destination_amount": 0.0,
            },
            user_id=user_id,
        )
        assert bill_id is not None
        return int(bill_id)

    return asyncio.run(_create())


class TestMatchingAPI:
    """matching API 回归。"""

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
        assert candidates[1]["status"] == "confirmed"
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

    def test_matching_bill_candidates_returns_sorted_candidates_for_formal_bill(self, client):
        """历史正式账单应返回按分数/时间差排序的 transfer-only 候选。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 历史转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest 历史转入账户")
        third_account_id = _create_account_via_db(current_user_id, "pytest 历史第三账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-99.0,
            bill_type="支出",
            date="2026-07-10 10:00:00",
            description="matching api anchor bill",
        )
        near_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=99.0,
            bill_type="收入",
            date="2026-07-10 10:03:00",
            description="matching api near candidate",
        )
        far_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=99.0,
            bill_type="收入",
            date="2026-07-11 10:03:00",
            description="matching api far candidate",
        )
        _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=-99.0,
            bill_type="支出",
            date="2026-07-10 10:01:00",
            description="matching api invalid same sign",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["billId"] == anchor_bill_id
        assert data["data"]["linkedPair"] is None
        candidates = data["data"]["candidates"]
        assert [candidate["candidateId"] for candidate in candidates] == [
            f"bill:{anchor_bill_id}:transfer:{near_candidate_bill_id}",
            f"bill:{anchor_bill_id}:transfer:{far_candidate_bill_id}",
        ]
        assert [candidate["billId"] for candidate in candidates] == [
            near_candidate_bill_id,
            far_candidate_bill_id,
        ]
        assert candidates[0]["score"] >= candidates[1]["score"]

    def test_matching_bill_candidates_returns_empty_list_for_existing_bill_without_candidates(self, client):
        """历史账单存在但没有 transfer 候选时应返回 200 + 空数组。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_candidates_empty")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 空候选账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-35.0,
            bill_type="支出",
            date="2026-07-12 09:00:00",
            description="matching api empty anchor",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"] == {
            "billId": anchor_bill_id,
            "linkedPair": None,
            "candidates": [],
        }

    def test_matching_bill_candidates_and_manual_pair_are_user_scoped(self, client):
        """历史正式账单 matching 的读写都不应跨用户泄露。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_bill_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_bill_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        secondary_user_id = _get_current_user_id(client, secondary_headers)

        primary_account_id = _create_account_via_db(primary_user_id, "pytest 主用户账户")
        primary_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=primary_account_id,
            amount=-45.0,
            bill_type="支出",
            date="2026-07-13 10:00:00",
            description="matching api user scope anchor",
        )
        secondary_account_id = _create_account_via_db(secondary_user_id, "pytest 次用户账户")
        secondary_bill_id = _create_bill_via_db(
            secondary_user_id,
            source_account_id=secondary_account_id,
            amount=45.0,
            bill_type="收入",
            date="2026-07-13 10:01:00",
            description="matching api user scope candidate",
        )

        get_response = client.get(f"/api/matching/bills/{primary_bill_id}/candidates", headers=secondary_headers)
        assert get_response.status_code == 404
        assert get_response.get_json()["error"] == "Bill not found"

        post_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": primary_bill_id, "candidateBillId": secondary_bill_id},
            headers=secondary_headers,
        )
        assert post_response.status_code == 404
        assert post_response.get_json()["error"] == "Bill not found"

    def test_matching_manual_pair_delete_restores_candidates_for_formal_bill(self, client):
        """手工后配对删除后，应恢复 linkedPair 为空且候选重新出现。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_manual_pair")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 手工配对转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest 手工配对转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-66.0,
            bill_type="支出",
            date="2026-07-14 11:00:00",
            description="matching api manual pair anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=66.0,
            bill_type="收入",
            date="2026-07-14 11:02:00",
            description="matching api manual pair candidate",
        )

        response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["pair"]["pairType"] == "transfer"
        assert data["data"]["pair"]["leftBillId"] == min(anchor_bill_id, candidate_bill_id)
        assert data["data"]["pair"]["rightBillId"] == max(anchor_bill_id, candidate_bill_id)

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert follow_up_response.status_code == 200
        follow_up_data = follow_up_response.get_json()
        assert follow_up_data["data"]["linkedPair"]["otherBillId"] == candidate_bill_id
        assert follow_up_data["data"]["candidates"] == []

        delete_response = client.delete(
            f"/api/matching/pairs/{data['data']['pair']['id']}",
            headers=auth_headers,
        )

        assert delete_response.status_code == 200
        delete_data = delete_response.get_json()
        assert delete_data["success"] is True
        assert delete_data["data"]["pair"]["id"] == data["data"]["pair"]["id"]

        restored_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert restored_response.status_code == 200
        restored_data = restored_response.get_json()
        assert restored_data["data"]["linkedPair"] is None
        assert [candidate["billId"] for candidate in restored_data["data"]["candidates"]] == [candidate_bill_id]

    def test_matching_manual_pair_rejects_self_pair_and_existing_link_conflicts(self, client):
        """手工后配对应拒绝 self-pair 与已存在 pair 的冲突写入。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_manual_pair_conflict")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 冲突账户 A")
        target_account_id = _create_account_via_db(current_user_id, "pytest 冲突账户 B")
        third_account_id = _create_account_via_db(current_user_id, "pytest 冲突账户 C")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-77.0,
            bill_type="支出",
            date="2026-07-15 08:00:00",
            description="matching api conflict anchor",
        )
        first_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=77.0,
            bill_type="收入",
            date="2026-07-15 08:02:00",
            description="matching api first candidate",
        )
        second_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=77.0,
            bill_type="收入",
            date="2026-07-15 08:04:00",
            description="matching api second candidate",
        )

        self_pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": anchor_bill_id},
            headers=auth_headers,
        )
        assert self_pair_response.status_code == 400
        assert self_pair_response.get_json()["error"] == "billId and candidateBillId must be different"

        first_pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": first_candidate_bill_id},
            headers=auth_headers,
        )
        assert first_pair_response.status_code == 200

        conflicting_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": second_candidate_bill_id},
            headers=auth_headers,
        )
        assert conflicting_response.status_code == 409
        assert conflicting_response.get_json()["error"] == "Bills already belong to an existing transfer pair"

    def test_matching_pair_delete_is_user_scoped(self, client):
        """历史正式账单手工配对删除不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_pair_delete_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_pair_delete_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest 删除隔离转出账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest 删除隔离转入账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-81.0,
            bill_type="支出",
            date="2026-07-15 10:00:00",
            description="matching api delete scope anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=81.0,
            bill_type="收入",
            date="2026-07-15 10:02:00",
            description="matching api delete scope candidate",
        )

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=primary_headers,
        )
        assert pair_response.status_code == 200
        pair_id = pair_response.get_json()["data"]["pair"]["id"]

        delete_response = client.delete(f"/api/matching/pairs/{pair_id}", headers=secondary_headers)
        assert delete_response.status_code == 404
        assert delete_response.get_json()["error"] == "Pair not found"

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=primary_headers)
        assert follow_up_response.status_code == 200
        assert follow_up_response.get_json()["data"]["linkedPair"]["id"] == pair_id

    def test_matching_pairs_lists_current_user_persisted_pairs_with_bill_summaries(self, client):
        """pair 列表应返回当前用户已持久化的 manual transfer pairs 与左右账单摘要。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_pairs_list")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest pair list 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest pair list 目标账户")
        third_account_id = _create_account_via_db(current_user_id, "pytest pair list 第三账户")

        first_left_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-91.0,
            bill_type="支出",
            date="2026-07-18 15:00:00",
            description="matching api pair list first left",
        )
        first_right_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=91.0,
            bill_type="收入",
            date="2026-07-18 15:02:00",
            description="matching api pair list first right",
        )
        second_left_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-92.0,
            bill_type="支出",
            date="2026-07-18 16:00:00",
            description="matching api pair list second left",
        )
        second_right_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=92.0,
            bill_type="收入",
            date="2026-07-18 16:02:00",
            description="matching api pair list second right",
        )

        first_pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": first_left_bill_id, "candidateBillId": first_right_bill_id},
            headers=auth_headers,
        )
        second_pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": second_left_bill_id, "candidateBillId": second_right_bill_id},
            headers=auth_headers,
        )
        assert first_pair_response.status_code == 200
        assert second_pair_response.status_code == 200
        first_pair_id = first_pair_response.get_json()["data"]["pair"]["id"]
        second_pair_id = second_pair_response.get_json()["data"]["pair"]["id"]

        response = client.get("/api/matching/pairs", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        pairs = data["data"]["pairs"]
        assert [pair["id"] for pair in pairs] == [second_pair_id, first_pair_id]
        assert pairs[0]["leftBill"]["id"] == min(second_left_bill_id, second_right_bill_id)
        assert pairs[0]["rightBill"]["id"] == max(second_left_bill_id, second_right_bill_id)
        assert pairs[0]["leftBill"]["paymentMethod"] == "银行卡"
        assert pairs[1]["rightBill"]["description"] == "matching api pair list first right"

    def test_matching_pairs_returns_empty_list_for_user_without_pairs(self, client):
        """没有 pair 的用户读取列表时应返回 200 + 空数组。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_pairs_list_empty")

        response = client.get("/api/matching/pairs", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"] == {"pairs": []}

    def test_matching_pairs_does_not_leak_other_users_pairs(self, client):
        """pair 列表不应泄露其他用户已持久化的配对关系。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_pairs_list_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_pairs_list_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest pair leak 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest pair leak 目标账户")
        left_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-101.0,
            bill_type="支出",
            date="2026-07-18 17:00:00",
            description="matching api pair leak left",
        )
        right_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=101.0,
            bill_type="收入",
            date="2026-07-18 17:02:00",
            description="matching api pair leak right",
        )

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": left_bill_id, "candidateBillId": right_bill_id},
            headers=primary_headers,
        )
        assert pair_response.status_code == 200

        response = client.get("/api/matching/pairs", headers=secondary_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"] == {"pairs": []}

    def test_matching_bill_candidates_support_slash_and_cn_date_formats(self, client):
        """历史 matching API 应兼容验证器已接受的 `/` 与中文日期格式。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_date_formats")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 日期格式转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest 日期格式转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-108.0,
            bill_type="支出",
            date="2026/07/16 11:00:00",
            description="matching api slash format anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=108.0,
            bill_type="收入",
            date="2026年07月16日 11:03:00",
            description="matching api cn format candidate",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert [candidate["billId"] for candidate in data["data"]["candidates"]] == [candidate_bill_id]

    def test_matching_unified_candidates_rejects_invalid_selector_input(self, client):
        """统一 candidates 入口应要求且仅允许一个合法 selector。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_unified_invalid_selector")

        missing_selector_response = client.get("/api/matching/candidates", headers=auth_headers)
        assert missing_selector_response.status_code == 400
        assert missing_selector_response.get_json()["error"] == "Exactly one of sessionId or billId is required"

        duplicate_selector_response = client.get(
            "/api/matching/candidates?sessionId=session-1&billId=11",
            headers=auth_headers,
        )
        assert duplicate_selector_response.status_code == 400
        assert duplicate_selector_response.get_json()["error"] == "Exactly one of sessionId or billId is required"

        invalid_bill_id_response = client.get(
            "/api/matching/candidates?billId=abc",
            headers=auth_headers,
        )
        assert invalid_bill_id_response.status_code == 400
        assert invalid_bill_id_response.get_json()["error"] == "Invalid billId"

    def test_matching_unified_candidates_session_selector_matches_existing_session_endpoint(self, client):
        """统一 candidates 入口的 session 分支应复用现有 session 候选读模型。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_unified_session_selector")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-unified-session-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> None:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-06-18 10:20:00",
                    "preview_type": "支出",
                    "preview_amount": 18.8,
                    "preview_destination_amount": 18.8,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_source_account_id": 11,
                    "preview_destination_account_id": 22,
                    "preview_counterparty": "pytest unified session vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest unified matching api",
                    "preview_parser_id": "wechat",
                    "preview_parser_tags": ["parser:wechat", "channel:wallet"],
                    "preview_recurring_id": 19,
                    "preview_recurring_name": "pytest unified recurring candidate",
                    "preview_recurring_candidate_count": 1,
                    "preview_recurring_match_score": 0.78,
                    "preview_recurring_match_reasons": "schedule|amount",
                    "preview_recurring_matched_date": "2026-06-18",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[191, 192],
            )

        asyncio.run(_create_preview_item())

        old_response = client.get(
            f"/api/matching/sessions/{session_id}/candidates",
            headers=auth_headers,
        )
        unified_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )

        assert old_response.status_code == 200
        assert unified_response.status_code == 200
        assert unified_response.get_json()["data"] == old_response.get_json()["data"]

    def test_matching_unified_candidates_bill_selector_matches_existing_bill_endpoint(self, client):
        """统一 candidates 入口的 bill 分支应复用现有历史账单候选读模型。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_unified_bill_selector")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest unified 历史转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest unified 历史转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-55.0,
            bill_type="支出",
            date="2026-07-20 09:00:00",
            description="matching unified api anchor bill",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=55.0,
            bill_type="收入",
            date="2026-07-20 09:03:00",
            description="matching unified api candidate bill",
        )

        old_response = client.get(
            f"/api/matching/bills/{anchor_bill_id}/candidates",
            headers=auth_headers,
        )
        unified_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )

        assert old_response.status_code == 200
        assert unified_response.status_code == 200
        old_data = old_response.get_json()["data"]
        unified_data = unified_response.get_json()["data"]
        assert unified_data == old_data
        assert unified_data["candidates"][0]["candidateId"] == f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"

    def test_matching_unified_candidates_is_user_scoped_for_session_and_bill_selectors(self, client):
        """统一 candidates 入口的 session/bill 分支都不应跨用户泄露。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_unified_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_unified_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-unified-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_session() -> None:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)

        asyncio.run(_create_primary_session())

        source_account_id = _create_account_via_db(primary_user_id, "pytest unified scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest unified scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-65.0,
            bill_type="支出",
            date="2026-07-20 11:00:00",
            description="matching unified scope anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=65.0,
            bill_type="收入",
            date="2026-07-20 11:03:00",
            description="matching unified scope candidate",
        )
        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=primary_headers,
        )
        assert pair_response.status_code == 200

        session_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=secondary_headers,
        )
        assert session_response.status_code == 404
        assert session_response.get_json()["error"] == "Import session not found"

        bill_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=secondary_headers,
        )
        assert bill_response.status_code == 404
        assert bill_response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_accept_accepts_preview_transfer_candidate_and_preserves_stale_guard(self, client):
        """generic accept 第一刀应复用 preview transfer accept 与 stale-state 保护。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-accept-preview-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-21 09:20:00",
                    "preview_type": "支出",
                    "preview_amount": 28.8,
                    "preview_counterparty": "pytest generic accept vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest generic accept preview",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[81, 82],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:transfer"
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
        }

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["action"] == "accept"
        assert accept_data["data"]["previewId"] == preview_id
        accept_preview = next(
            item for item in accept_data["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert accept_preview["preview_type"] == "转账"
        assert accept_preview["matching"]["transfer"]["review_status"] == "accepted"

        stale_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )

        assert stale_response.status_code == 409
        stale_data = stale_response.get_json()
        assert stale_data["success"] is False
        assert stale_data["error"] == "Preview state changed, please refresh"

    def test_matching_candidate_accept_accepts_historical_transfer_candidate(self, client):
        """generic accept 第一刀应复用历史 formal-bill transfer 的手工配对写路径。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest accept 历史转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest accept 历史转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-72.0,
            bill_type="支出",
            date="2026-07-21 10:00:00",
            description="matching generic accept anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=72.0,
            bill_type="收入",
            date="2026-07-21 10:03:00",
            description="matching generic accept candidate",
        )
        candidate_id = f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["pair"]["pairType"] == "transfer"
        assert accept_data["data"]["pair"]["leftBillId"] == min(anchor_bill_id, candidate_bill_id)
        assert accept_data["data"]["pair"]["rightBillId"] == max(anchor_bill_id, candidate_bill_id)

        follow_up_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert follow_up_response.status_code == 200
        follow_up_data = follow_up_response.get_json()
        assert follow_up_data["data"]["linkedPair"]["otherBillId"] == candidate_bill_id
        assert follow_up_data["data"]["candidates"] == []

    def test_matching_candidate_accept_rejects_invalid_or_unsupported_candidate_ids(self, client):
        """generic accept 第一刀应拒绝非法 candidateId 和未支持 family。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_invalid")

        malformed_response = client.post(
            "/api/matching/candidates/not-a-valid-candidate-id/accept",
            json={},
            headers=auth_headers,
        )
        assert malformed_response.status_code == 400
        assert malformed_response.get_json()["error"] == "Invalid candidateId"

        unsupported_response = client.post(
            "/api/matching/candidates/preview:99:recurring/accept",
            json={},
            headers=auth_headers,
        )
        assert unsupported_response.status_code == 400
        assert unsupported_response.get_json()["error"] == "Candidate family not supported"

    def test_matching_candidate_accept_is_user_scoped_for_preview_and_bill_transfer(self, client):
        """generic accept 第一刀的 preview/bill transfer 分支都不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-accept-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-21 11:20:00",
                    "preview_type": "支出",
                    "preview_amount": 18.8,
                    "preview_counterparty": "pytest accept scope vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest accept scope preview",
                },
                user_id=primary_user_id,
                dedup_type="transfer",
                dedup_source_ids=[71, 72],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_primary_preview())
        preview_response = client.post(
            f"/api/matching/candidates/preview:{preview_id}:transfer/accept",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                }
            },
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Preview bill not found"

        source_account_id = _create_account_via_db(primary_user_id, "pytest accept scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest accept scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-82.0,
            bill_type="支出",
            date="2026-07-21 11:40:00",
            description="pytest accept scope anchor bill",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=82.0,
            bill_type="收入",
            date="2026-07-21 11:43:00",
            description="pytest accept scope candidate bill",
        )
        bill_response = client.post(
            f"/api/matching/candidates/bill:{anchor_bill_id}:transfer:{candidate_bill_id}/accept",
            json={},
            headers=secondary_headers,
        )
        assert bill_response.status_code == 404
        assert bill_response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_reject_rejects_preview_transfer_candidate_and_preserves_stale_guard(self, client):
        """generic reject 第一刀应复用 preview transfer reject 与 stale-state 保护。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_preview")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-reject-preview-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-22 09:20:00",
                    "preview_type": "支出",
                    "preview_amount": 38.8,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_counterparty": "pytest generic reject vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest generic reject preview",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[61, 62],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:transfer"
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
        }

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        assert reject_data["success"] is True
        assert reject_data["data"]["candidateId"] == candidate_id
        assert reject_data["data"]["action"] == "reject"
        assert reject_data["data"]["previewId"] == preview_id
        reject_preview = next(
            item for item in reject_data["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["preview_main_category"] == "餐饮"
        assert reject_preview["matching"]["transfer"]["review_status"] == "rejected"
        assert reject_preview["matching"]["transfer"]["suppressed"] is True

        session_follow_up_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_follow_up_response.status_code == 200
        session_follow_up_candidates = session_follow_up_response.get_json()["data"]["candidates"]
        transfer_candidate = next(
            candidate for candidate in session_follow_up_candidates if candidate["candidate_id"] == candidate_id
        )
        assert transfer_candidate["status"] == "rejected"

        stale_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )

        assert stale_response.status_code == 409
        stale_data = stale_response.get_json()
        assert stale_data["success"] is False
        assert stale_data["error"] == "Preview state changed, please refresh"

    def test_matching_candidate_reject_rejects_historical_transfer_candidate_and_filters_followup_reads(self, client):
        """historical formal-bill transfer reject 应持久化 suppression，并过滤后续 bill/unified 候选读取。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest bill reject 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest bill reject 目标账户")
        third_account_id = _create_account_via_db(current_user_id, "pytest bill reject 第三账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-57.0,
            bill_type="支出",
            date="2026-07-23 09:00:00",
            description="matching generic bill reject anchor",
        )
        rejected_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=57.0,
            bill_type="收入",
            date="2026-07-23 09:02:00",
            description="matching generic bill reject candidate",
        )
        retained_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=57.0,
            bill_type="收入",
            date="2026-07-23 09:05:00",
            description="matching generic bill retained candidate",
        )
        candidate_id = f"bill:{anchor_bill_id}:transfer:{rejected_candidate_bill_id}"

        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        assert [candidate["billId"] for candidate in initial_response.get_json()["data"]["candidates"]] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        assert reject_data["success"] is True
        assert reject_data["data"] == {
            "candidateId": candidate_id,
            "action": "reject",
        }

        bill_follow_up_response = client.get(
            f"/api/matching/bills/{anchor_bill_id}/candidates",
            headers=auth_headers,
        )
        assert bill_follow_up_response.status_code == 200
        bill_follow_up_data = bill_follow_up_response.get_json()["data"]
        assert bill_follow_up_data["linkedPair"] is None
        assert [candidate["billId"] for candidate in bill_follow_up_data["candidates"]] == [retained_candidate_bill_id]

        unified_follow_up_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_follow_up_response.status_code == 200
        unified_follow_up_data = unified_follow_up_response.get_json()["data"]
        assert unified_follow_up_data["linkedPair"] is None
        assert [candidate["billId"] for candidate in unified_follow_up_data["candidates"]] == [retained_candidate_bill_id]

        reverse_follow_up_response = client.get(
            f"/api/matching/bills/{rejected_candidate_bill_id}/candidates",
            headers=auth_headers,
        )
        assert reverse_follow_up_response.status_code == 200
        reverse_candidates = reverse_follow_up_response.get_json()["data"]["candidates"]
        assert anchor_bill_id not in {candidate["billId"] for candidate in reverse_candidates}

        accept_after_reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert accept_after_reject_response.status_code == 409
        assert accept_after_reject_response.get_json()["error"] == "Bills already rejected for transfer pairing"

    def test_matching_candidate_reject_clears_preview_recurring_match_and_returns_pending_candidate(self, client):
        """generic reject 的 preview recurring 分支应复用 clear recurring-match 语义。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_recurring")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        bill_comment = f"pytest recurring reject {int(time.time() * 1000)}"

        _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest matching recurring reject 一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest matching recurring reject 二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1",
        )

        session_id = f"pytest-matching-reject-recurring-{int(time.time() * 1000)}"
        _create_test_import_session(
            session_id,
            [
                {
                    "date": "2026-03-09 10:30:00",
                    "amount": -76.0,
                    "type": "支出",
                    "description": bill_comment,
                    "counterparty": "pytest recurring reject vendor",
                    "payment_method": "pytest recurring account",
                    "source_account_id": source_account["id"],
                }
            ],
            parser_id="wechat",
            user_id=current_user_id,
        )

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            json={"session_id": session_id},
            headers=auth_headers,
        )
        assert dedup_response.status_code == 200
        preview_item = dedup_response.get_json()["data"]["preview"][0]
        preview_id = int(preview_item["id"])
        candidate_id = f"preview:{preview_id}:recurring"
        assert preview_item.get("preview_recurring_id") not in (None, "", 0)
        assert int(preview_item.get("preview_recurring_candidate_count") or 0) >= 2

        def _build_expected_state(item: dict[str, object]) -> dict[str, object]:
            category_id = _find_category_id_by_name(
                str(item.get("preview_main_category") or ""),
                str(item.get("preview_sub_category") or ""),
                user_id=current_user_id,
            )
            return {
                "sessionId": session_id,
                "previewType": item.get("preview_type"),
                "categoryId": category_id,
                "recurringId": item.get("preview_recurring_id"),
            }

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": _build_expected_state(preview_item)},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        assert reject_data["success"] is True
        assert reject_data["data"]["candidateId"] == candidate_id
        assert reject_data["data"]["action"] == "reject"
        assert reject_data["data"]["previewId"] == preview_id
        cleared_preview = next(
            item for item in reject_data["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert cleared_preview["preview_recurring_id"] in (None, "", 0)
        assert cleared_preview["preview_recurring_name"] == ""
        assert int(cleared_preview["preview_recurring_candidate_count"] or 0) >= 2
        assert cleared_preview["preview_recurring_match_score"] == 0
        assert cleared_preview["matching"]["recurring"]["id"] is None

        session_follow_up_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_follow_up_response.status_code == 200
        session_follow_up_candidates = session_follow_up_response.get_json()["data"]["candidates"]
        recurring_candidate = next(
            candidate for candidate in session_follow_up_candidates if candidate["candidate_id"] == candidate_id
        )
        assert recurring_candidate["status"] == "pending"
        assert recurring_candidate["details"]["candidate_count"] >= 2

        stale_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": _build_expected_state(preview_item)},
            headers=auth_headers,
        )
        assert stale_response.status_code == 409
        assert stale_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_candidate_reject_rejects_invalid_or_unsupported_candidate_ids(self, client):
        """generic reject 当前切片应拒绝非法 candidateId 与仍未支持的 family。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_invalid")
        current_user_id = _get_current_user_id(client, auth_headers)

        malformed_response = client.post(
            "/api/matching/candidates/not-a-valid-candidate-id/reject",
            json={},
            headers=auth_headers,
        )
        assert malformed_response.status_code == 400
        assert malformed_response.get_json()["error"] == "Invalid candidateId"

        unsupported_response = client.post(
            "/api/matching/candidates/preview:99:investment/reject",
            json={},
            headers=auth_headers,
        )
        assert unsupported_response.status_code == 400
        assert unsupported_response.get_json()["error"] == "Candidate family not supported"

        historical_unsupported_response = client.post(
            "/api/matching/candidates/bill:11:investment:12/reject",
            json={},
            headers=auth_headers,
        )
        assert historical_unsupported_response.status_code == 400
        assert historical_unsupported_response.get_json()["error"] == "Candidate family not supported"

    def test_matching_candidate_reject_is_user_scoped_for_preview_transfer(self, client):
        """generic reject 第一刀的 preview transfer 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-reject-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-22 11:20:00",
                    "preview_type": "支出",
                    "preview_amount": 21.8,
                    "preview_counterparty": "pytest reject scope vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest reject scope preview",
                },
                user_id=primary_user_id,
                dedup_type="transfer",
                dedup_source_ids=[51, 52],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_primary_preview())
        preview_response = client.post(
            f"/api/matching/candidates/preview:{preview_id}:transfer/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                }
            },
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Preview bill not found"

    def test_matching_candidate_reject_is_user_scoped_for_preview_recurring(self, client):
        """generic reject 的 preview recurring 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_recurring_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_recurring_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        source_account = _ensure_test_account(client, primary_headers)
        category = _ensure_test_expense_category(client, primary_headers)

        _create_test_recurring_template(
            client,
            primary_headers,
            name="pytest matching recurring scope 一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=8800,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        _create_test_recurring_template(
            client,
            primary_headers,
            name="pytest matching recurring scope 二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=8800,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1",
        )

        session_id = f"pytest-matching-reject-recurring-scope-{int(time.time() * 1000)}"
        _create_test_import_session(
            session_id,
            [
                {
                    "date": "2026-03-09 10:30:00",
                    "amount": -88.0,
                    "type": "支出",
                    "description": "pytest recurring reject scope preview",
                    "counterparty": "pytest recurring reject scope vendor",
                    "payment_method": "pytest recurring scope account",
                    "source_account_id": source_account["id"],
                }
            ],
            parser_id="wechat",
            user_id=primary_user_id,
        )

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            json={"session_id": session_id},
            headers=primary_headers,
        )
        assert dedup_response.status_code == 200
        preview_item = dedup_response.get_json()["data"]["preview"][0]
        preview_id = int(preview_item["id"])

        preview_response = client.post(
            f"/api/matching/candidates/preview:{preview_id}:recurring/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "previewType": preview_item.get("preview_type"),
                    "categoryId": _find_category_id_by_name(
                        str(preview_item.get("preview_main_category") or ""),
                        str(preview_item.get("preview_sub_category") or ""),
                        user_id=primary_user_id,
                    ),
                    "recurringId": preview_item.get("preview_recurring_id"),
                }
            },
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Preview bill not found"

    def test_matching_candidate_reject_is_user_scoped_for_historical_transfer(self, client):
        """generic reject 的 historical transfer 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest bill reject scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest bill reject scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-68.0,
            bill_type="支出",
            date="2026-07-23 11:00:00",
            description="matching bill reject scope anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=68.0,
            bill_type="收入",
            date="2026-07-23 11:03:00",
            description="matching bill reject scope candidate",
        )

        reject_response = client.post(
            f"/api/matching/candidates/bill:{anchor_bill_id}:transfer:{candidate_bill_id}/reject",
            json={},
            headers=secondary_headers,
        )
        assert reject_response.status_code == 404
        assert reject_response.get_json()["error"] == "Bill not found"
