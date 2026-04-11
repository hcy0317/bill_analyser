from __future__ import annotations

import asyncio
import time

from tests.new_ui.test_bills_api import _build_isolated_auth_headers, _get_current_user_id


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
