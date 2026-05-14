from __future__ import annotations

import asyncio
import json
import time
from datetime import datetime

import pytest

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


def _create_composite_learning_rule_via_db(
    user_id: int,
    *,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    learned_type: str,
    learned_category_id: int | None = None,
    learned_source_account_id: int | None = None,
    learned_destination_account_id: int | None = None,
) -> int:
    from src.api.app import db

    async def _create() -> int:
        conn = await db._get_connection()  # pylint: disable=protected-access
        now = datetime.now().isoformat()
        rule_hash = db.build_composite_match_hash(  # pylint: disable=protected-access
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        assert rule_hash is not None
        cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id, learned_source_account_id, learned_destination_account_id,
                enabled, parser_id, composite_match_hash,
                match_features_json, applied_count, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                "composite",
                rule_hash,
                rule_hash,
                learned_type,
                learned_category_id,
                learned_source_account_id,
                learned_destination_account_id,
                parser_id,
                rule_hash,
                json.dumps(
                    {
                        "counterparty": counterparty,
                        "description": description,
                        "parser_id": parser_id,
                        "payment_method": payment_method,
                    },
                    ensure_ascii=False,
                ),
                6,
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid)

    return asyncio.run(_create())


def _list_bill_pair_feedback_via_db(*, user_id: int, candidate_id: str | None = None) -> list[dict[str, object]]:
    from src.api.app import db

    async def _list() -> list[dict[str, object]]:
        conn = await db._get_connection()  # pylint: disable=protected-access
        if candidate_id is None:
            query = "SELECT * FROM bill_pair_feedback WHERE user_id = ? ORDER BY id ASC"
            params: tuple[object, ...] = (user_id,)
        else:
            query = "SELECT * FROM bill_pair_feedback WHERE user_id = ? AND candidate_id = ? ORDER BY id ASC"
            params = (user_id, candidate_id)

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        feedback_rows: list[dict[str, object]] = []
        for row in rows:
            row_dict = dict(row)
            feedback_rows.append(
                {
                    **row_dict,
                    "id": int(row_dict.get("id") or 0),
                    "user_id": int(row_dict.get("user_id") or 0),
                    "candidate_id": str(row_dict.get("candidate_id") or ""),
                    "action": str(row_dict.get("action") or ""),
                    "payload": json.loads(str(row_dict.get("payload_json") or "{}")),
                    "created_at": str(row_dict.get("created_at") or ""),
                }
            )
        return feedback_rows

    return asyncio.run(_list())


class TestMatchingAPI:
    """matching API 回归。"""

    def test_matching_investment_settings_endpoint_is_retired(self, client):
        """投资识别设置 API 已退役，关键词主链应迁入分类规则。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_investment_settings")

        get_before = client.get("/api/matching/investment-settings", headers=auth_headers)
        assert get_before.status_code == 410
        assert get_before.get_json() == {
            "success": False,
            "error": "Investment recognition settings are managed by category rules",
        }

        update_payload = {
            "importLearningEnabled": False,
            "investmentPlatformKeywords": ["蚂蚁财富", "京东金融"],
            "investmentProductKeywords": ["指数基金", "REITs"],
            "investmentExcludeKeywords": ["还款", "账单"],
        }

        update_response = client.put(
            "/api/matching/investment-settings",
            json=update_payload,
            headers=auth_headers,
        )
        assert update_response.status_code == 410, update_response.get_data(as_text=True)
        assert update_response.get_json() == {
            "success": False,
            "error": "Investment recognition settings are managed by category rules",
        }

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

    def test_matching_reconcile_history_returns_transfer_candidates_for_requested_bills(self, client):
        """reconcile-history MVP 应按请求顺序聚合唯一 formal-bill transfer 候选。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile 源账户 A")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile 目标账户 A")
        third_account_id = _create_account_via_db(current_user_id, "pytest reconcile 源账户 B")
        fourth_account_id = _create_account_via_db(current_user_id, "pytest reconcile 目标账户 B")

        first_anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-51.0,
            bill_type="支出",
            date="2026-07-25 09:00:00",
            description="matching reconcile first anchor",
        )
        first_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=51.0,
            bill_type="收入",
            date="2026-07-25 09:03:00",
            description="matching reconcile first candidate",
        )
        second_anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=-62.0,
            bill_type="支出",
            date="2026-07-25 10:00:00",
            description="matching reconcile second anchor",
        )
        second_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=fourth_account_id,
            amount=62.0,
            bill_type="收入",
            date="2026-07-25 10:04:00",
            description="matching reconcile second candidate",
        )

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [second_anchor_bill_id, first_anchor_bill_id, second_anchor_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 2,
            "candidateCount": 2,
            "linkedPairCount": 0,
        }
        results = data["data"]["results"]
        assert [item["billId"] for item in results] == [second_anchor_bill_id, first_anchor_bill_id]
        assert results[0]["linkedPair"] is None
        assert results[0]["candidates"][0]["candidateId"] == (
            f"bill:{second_anchor_bill_id}:transfer:{second_candidate_bill_id}"
        )
        assert results[1]["candidates"][0]["candidateId"] == (
            f"bill:{first_anchor_bill_id}:transfer:{first_candidate_bill_id}"
        )

    def test_matching_reconcile_history_rejects_invalid_requests_and_is_user_scoped(self, client):
        """reconcile-history MVP 应校验 billIds，并保持 formal-bill user scope。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)

        invalid_request_response = client.post(
            "/api/matching/reconcile-history",
            json=[11],
            headers=primary_headers,
        )
        assert invalid_request_response.status_code == 400
        assert invalid_request_response.get_json()["error"] == "Invalid request"

        missing_bill_ids_response = client.post(
            "/api/matching/reconcile-history",
            json={},
            headers=primary_headers,
        )
        assert missing_bill_ids_response.status_code == 400
        assert missing_bill_ids_response.get_json()["error"] == "billIds is required"

        empty_bill_ids_response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": []},
            headers=primary_headers,
        )
        assert empty_bill_ids_response.status_code == 400
        assert empty_bill_ids_response.get_json()["error"] == "billIds must be a non-empty list"

        invalid_bill_ids_response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": ["abc"]},
            headers=primary_headers,
        )
        assert invalid_bill_ids_response.status_code == 400
        assert invalid_bill_ids_response.get_json()["error"] == "Invalid billIds"

        bool_bill_id_response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [True]},
            headers=primary_headers,
        )
        assert bool_bill_id_response.status_code == 400
        assert bool_bill_id_response.get_json()["error"] == "Invalid billIds"

        source_account_id = _create_account_via_db(primary_user_id, "pytest reconcile scope 源账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-41.0,
            bill_type="支出",
            date="2026-07-25 11:00:00",
            description="matching reconcile scope anchor",
        )

        scoped_response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [anchor_bill_id]},
            headers=secondary_headers,
        )
        assert scoped_response.status_code == 404
        assert scoped_response.get_json()["error"] == "Bill not found"

    def test_matching_reconcile_history_includes_existing_linked_pair_and_empty_candidates(self, client):
        """reconcile-history MVP 应复用 formal transfer linkedPair 读语义。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_linked_pair")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile pair 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile pair 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-73.0,
            bill_type="支出",
            date="2026-07-25 12:00:00",
            description="matching reconcile pair anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=73.0,
            bill_type="收入",
            date="2026-07-25 12:03:00",
            description="matching reconcile pair candidate",
        )

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=auth_headers,
        )
        assert pair_response.status_code == 200

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [anchor_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 1,
            "candidateCount": 0,
            "linkedPairCount": 1,
        }
        result = data["data"]["results"][0]
        assert result["billId"] == anchor_bill_id
        assert result["linkedPair"]["otherBillId"] == candidate_bill_id
        assert result["candidates"] == []

    def test_matching_reconcile_history_counts_same_linked_pair_once_when_both_sides_requested(self, client):
        """当请求同一手工 pair 两侧 bill 时，linkedPairCount 应按唯一 pair 计数。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_pair_count")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile count 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile count 目标账户")
        left_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-84.0,
            bill_type="支出",
            date="2026-07-25 13:00:00",
            description="matching reconcile count left",
        )
        right_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=84.0,
            bill_type="收入",
            date="2026-07-25 13:03:00",
            description="matching reconcile count right",
        )

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": left_bill_id, "candidateBillId": right_bill_id},
            headers=auth_headers,
        )
        assert pair_response.status_code == 200

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [left_bill_id, right_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 2,
            "candidateCount": 0,
            "linkedPairCount": 1,
        }
        assert [item["billId"] for item in data["data"]["results"]] == [left_bill_id, right_bill_id]

    def test_matching_reconcile_history_remains_transfer_only_when_bill_has_learning_candidates(self, client):
        """reconcile-history 当前契约仍应保持 transfer-only，不应透出 historical learning candidates。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_transfer_only")
        current_user_id = _get_current_user_id(client, auth_headers)

        _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest reconcile learning vendor",
            description="pytest reconcile learning note",
            payment_method="银行卡",
            learned_type="支出",
        )

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile learning 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile learning 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="支出",
            date="2026-07-25 14:00:00",
            description="pytest reconcile learning note",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=58.0,
            bill_type="收入",
            date="2026-07-25 14:03:00",
            description="pytest reconcile transfer candidate",
        )

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [anchor_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 1,
            "candidateCount": 1,
            "linkedPairCount": 0,
        }
        result = data["data"]["results"][0]
        assert result["billId"] == anchor_bill_id
        assert [candidate["candidateId"] for candidate in result["candidates"]] == [
            f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"
        ]
        assert all(candidate["kind"] == "transfer" for candidate in result["candidates"])

    def test_matching_reconcile_history_remains_transfer_only_when_bill_has_investment_candidates(self, client):
        """reconcile-history 当前契约仍应保持 transfer-only，不应透出 historical investment candidates。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_reconcile_history_transfer_only_investment")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile investment 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile investment 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="投资",
            date="2026-07-25 14:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=58.0,
            bill_type="投资",
            date="2026-07-25 14:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [anchor_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 1,
            "candidateCount": 1,
            "linkedPairCount": 0,
        }
        result = data["data"]["results"][0]
        assert result["billId"] == anchor_bill_id
        assert [candidate["candidateId"] for candidate in result["candidates"]] == [
            f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"
        ]
        assert all(candidate["kind"] == "transfer" for candidate in result["candidates"])

    def test_matching_reconcile_history_keeps_investment_linked_pair_out_of_transfer_only_batch(self, client):
        """reconcile-history 当前契约仍应保持 transfer-only，不应把 investment linkedPair 暴露进 batch 结果。"""
        auth_headers = _build_isolated_auth_headers(
            client, "test_matching_reconcile_history_transfer_only_investment_linked"
        )
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest reconcile investment linked 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest reconcile investment linked 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="投资",
            date="2026-07-25 14:10:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=58.0,
            bill_type="投资",
            date="2026-07-25 14:13:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        candidate_id = f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert accept_response.status_code == 200

        response = client.post(
            "/api/matching/reconcile-history",
            json={"billIds": [anchor_bill_id]},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["summary"] == {
            "billCount": 1,
            "candidateCount": 0,
            "linkedPairCount": 0,
        }
        result = data["data"]["results"][0]
        assert result["billId"] == anchor_bill_id
        assert result["linkedPair"] is None
        assert result["candidates"] == []

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

    def test_matching_manual_investment_pair_delete_restores_candidates_for_formal_bill(self, client):
        """investment/manual 删除后，应恢复 linkedPair 为空且 investment candidate 重新出现。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_manual_investment_pair")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest 投资手工配对转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest 投资手工配对转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-166.0,
            bill_type="投资",
            date="2026-07-14 12:00:00",
            description="蚂蚁财富 手工投资配对 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=166.0,
            bill_type="投资",
            date="2026-07-14 12:02:00",
            description="蚂蚁财富 手工投资配对 卖出",
        )

        before_pair_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert before_pair_response.status_code == 200
        before_candidates = before_pair_response.get_json()["data"]["candidates"]
        assert any(
            candidate["candidateId"] == f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"
            for candidate in before_candidates
        )

        response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id, "pairType": "investment"},
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["pair"]["pairType"] == "investment"
        assert data["data"]["pair"]["leftBillId"] == min(anchor_bill_id, candidate_bill_id)
        assert data["data"]["pair"]["rightBillId"] == max(anchor_bill_id, candidate_bill_id)

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert follow_up_response.status_code == 200
        follow_up_data = follow_up_response.get_json()
        assert follow_up_data["data"]["linkedPair"]["pairType"] == "investment"
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
        assert delete_data["data"]["pair"]["pairType"] == "investment"

        restored_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert restored_response.status_code == 200
        restored_data = restored_response.get_json()
        assert restored_data["data"]["linkedPair"] is None
        assert any(
            candidate["candidateId"] == f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"
            for candidate in restored_data["data"]["candidates"]
        )

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

    def test_matching_manual_pair_rejects_invalid_request_values(self, client):
        """manual-pair 应拒绝非法 pairType、布尔 ID 与浮点 ID。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_manual_pair_invalid_request")

        invalid_pair_type_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": 11, "candidateBillId": 12, "pairType": "crypto"},
            headers=auth_headers,
        )
        assert invalid_pair_type_response.status_code == 400
        assert invalid_pair_type_response.get_json()["error"] == "Invalid pairType"

        falsey_pair_type_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": 11, "candidateBillId": 12, "pairType": []},
            headers=auth_headers,
        )
        assert falsey_pair_type_response.status_code == 400
        assert falsey_pair_type_response.get_json()["error"] == "Invalid pairType"

        bool_bill_id_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": True, "candidateBillId": 12},
            headers=auth_headers,
        )
        assert bool_bill_id_response.status_code == 400
        assert bool_bill_id_response.get_json()["error"] == "Invalid request"

        float_candidate_bill_id_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": 11, "candidateBillId": 12.5},
            headers=auth_headers,
        )
        assert float_candidate_bill_id_response.status_code == 400
        assert float_candidate_bill_id_response.get_json()["error"] == "Invalid request"

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

    def test_matching_pairs_includes_investment_manual_pairs(self, client):
        """pair 列表应返回当前用户下的 mixed manual pairs，至少包含 investment/manual。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_pairs_list_investment_manual")
        current_user_id = _get_current_user_id(client, auth_headers)

        transfer_source_account_id = _create_account_via_db(current_user_id, "pytest pair list transfer 源账户")
        transfer_target_account_id = _create_account_via_db(current_user_id, "pytest pair list transfer 目标账户")
        investment_source_account_id = _create_account_via_db(current_user_id, "pytest pair list investment 源账户")
        investment_target_account_id = _create_account_via_db(current_user_id, "pytest pair list investment 目标账户")

        transfer_left_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=transfer_source_account_id,
            amount=-118.0,
            bill_type="支出",
            date="2026-07-19 10:00:00",
            description="matching api mixed pair transfer left",
        )
        transfer_right_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=transfer_target_account_id,
            amount=118.0,
            bill_type="收入",
            date="2026-07-19 10:03:00",
            description="matching api mixed pair transfer right",
        )
        investment_left_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=investment_source_account_id,
            amount=-128.0,
            bill_type="投资",
            date="2026-07-19 11:00:00",
            description="蚂蚁财富 mixed pair investment 买入",
        )
        investment_right_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=investment_target_account_id,
            amount=128.0,
            bill_type="投资",
            date="2026-07-19 11:03:00",
            description="蚂蚁财富 mixed pair investment 卖出",
        )

        transfer_pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": transfer_left_bill_id, "candidateBillId": transfer_right_bill_id},
            headers=auth_headers,
        )
        assert transfer_pair_response.status_code == 200
        investment_pair_response = client.post(
            "/api/matching/manual-pair",
            json={
                "billId": investment_left_bill_id,
                "candidateBillId": investment_right_bill_id,
                "pairType": "investment",
            },
            headers=auth_headers,
        )
        assert investment_pair_response.status_code == 200
        investment_pair = investment_pair_response.get_json()["data"]["pair"]
        assert investment_pair["pairType"] == "investment"

        response = client.get("/api/matching/pairs", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        pairs = data["data"]["pairs"]
        assert [pair["pairType"] for pair in pairs] == ["investment", "transfer"]
        assert [pair["id"] for pair in pairs] == [
            int(investment_pair["id"]),
            transfer_pair_response.get_json()["data"]["pair"]["id"],
        ]
        assert pairs[0]["leftBill"]["description"] == "蚂蚁财富 mixed pair investment 买入"
        assert pairs[0]["rightBill"]["description"] == "蚂蚁财富 mixed pair investment 卖出"
        assert pairs[1]["leftBill"]["description"] == "matching api mixed pair transfer left"

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
        accept_preview = next(item for item in accept_data["data"]["preview"] if int(item["id"]) == preview_id)
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

    @pytest.mark.skip(
        reason="Preview recurring candidate path now runs through Rust import runtime, not Flask bills import routes."
    )
    def test_matching_candidate_accept_accepts_preview_recurring_candidate_and_preserves_stale_guard(self, client):
        """generic accept 应复用 preview recurring 绑定语义，并保持 stale-state 保护。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_recurring")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        bill_comment = f"pytest recurring accept {int(time.time() * 1000)}"

        _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest matching recurring accept 一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9100,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest matching recurring accept 二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9100,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1",
        )

        session_id = f"pytest-matching-accept-recurring-{int(time.time() * 1000)}"
        _create_test_import_session(
            session_id,
            [
                {
                    "date": "2026-03-09 10:30:00",
                    "amount": -91.0,
                    "type": "支出",
                    "description": bill_comment,
                    "counterparty": "pytest recurring accept vendor",
                    "payment_method": "pytest recurring accept account",
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
        recurring_id = int(preview_item.get("preview_recurring_id") or 0)
        assert recurring_id > 0

        def _build_expected_state(item: dict[str, object]) -> dict[str, object]:
            category_id = _find_category_id_by_name(
                str(item.get("preview_main_category") or ""),
                str(item.get("preview_sub_category") or ""),
                user_id=current_user_id,
            )
            transfer_details = ((item.get("matching") or {}).get("transfer") or {})
            transfer_review_status = str(transfer_details.get("review_status") or "").strip().lower()
            if transfer_review_status not in {"accepted", "rejected"}:
                transfer_review_status = "pending" if str(transfer_details.get("candidate_type") or "").strip() else ""
            return {
                "sessionId": session_id,
                "reviewStatus": transfer_review_status,
                "previewType": item.get("preview_type"),
                "categoryId": category_id,
                "recurringId": item.get("preview_recurring_id"),
            }

        clear_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": _build_expected_state(preview_item)},
            headers=auth_headers,
        )
        assert clear_response.status_code == 200
        cleared_preview = next(item for item in clear_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id)
        assert cleared_preview["preview_recurring_id"] in (None, "", 0)

        accept_payload = {
            "recurringId": recurring_id,
            "expectedState": _build_expected_state(cleared_preview),
        }
        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json=accept_payload,
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["action"] == "accept"
        assert accept_data["data"]["previewId"] == preview_id
        assert accept_data["data"]["sessionId"] == session_id
        assert accept_data["data"]["recurringId"] == recurring_id
        accepted_preview = next(item for item in accept_data["data"]["preview"] if int(item["id"]) == preview_id)
        assert accepted_preview["preview_recurring_id"] == recurring_id
        assert accepted_preview["matching"]["recurring"]["id"] == recurring_id

        session_follow_up_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_follow_up_response.status_code == 200
        recurring_candidate = next(
            candidate
            for candidate in session_follow_up_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert recurring_candidate["status"] == "confirmed"
        assert recurring_candidate["details"]["id"] == recurring_id

        stale_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json=accept_payload,
            headers=auth_headers,
        )
        assert stale_response.status_code == 409
        assert stale_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_session_candidates_no_longer_surface_preview_investment_candidate(self, client):
        """导入预览投资类型应直接走分类规则结果，不再暴露独立 investment candidate。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_investment")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-accept-investment-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 09:20:00",
                    "preview_type": "投资",
                    "preview_amount": 68.8,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "黄金ETF 自动定投",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        asyncio.run(_create_preview_item())
        session_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_response.status_code == 200
        assert all(
            candidate["kind"] != "investment"
            for candidate in session_response.get_json()["data"]["candidates"]
        )

    def test_matching_candidate_accept_accepts_preview_learning_candidate_with_rule_id_and_reject_restores_preview_fields(self, client):
        """preview learning accept 应绑定 ruleId 应用 preview 字段，随后 reject 应恢复 accept 前 snapshot。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_learning")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-accept-learning-{int(time.time() * 1000)}"

        from src.api.app import db

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest preview learning 初始账户")
        learned_source_account_id = _create_account_via_db(current_user_id, "pytest preview learning 学习源账户")
        learned_destination_account_id = _create_account_via_db(current_user_id, "pytest preview learning 学习目标账户")

        learned_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert learned_category_id is not None

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _update_learning_rule() -> None:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_source_account_id = ?,
                    learned_destination_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    int(learned_category_id),
                    learned_source_account_id,
                    learned_destination_account_id,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()

        asyncio.run(_update_learning_rule())

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 09:40:00",
                    "preview_type": "支出",
                    "preview_amount": 38.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:learning"
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
            "sourceAccountId": initial_source_account_id,
            "destinationAccountId": None,
        }

        session_before_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_before_response.status_code == 200
        learning_candidate = next(
            candidate
            for candidate in session_before_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert learning_candidate["status"] == "pending"
        assert learning_candidate["details"]["rule_id"] == rule_id

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"ruleId": rule_id, "expectedState": expected_state},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["action"] == "accept"
        accept_preview = next(item for item in accept_data["data"]["preview"] if int(item["id"]) == preview_id)
        assert accept_preview["preview_type"] == "收入"
        assert accept_preview["preview_main_category"] == "餐饮"
        assert accept_preview["preview_sub_category"] == "咖啡"
        assert accept_preview["preview_source_account_id"] == learned_source_account_id
        assert accept_preview["preview_destination_account_id"] == learned_destination_account_id
        assert accept_preview["matching"]["learning"]["rule_id"] == rule_id
        assert accept_preview["matching"]["learning"]["review_status"] == "accepted"
        assert accept_preview["matching"]["learning"]["suppressed"] is False

        session_after_accept_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_after_accept_response.status_code == 200
        accepted_candidate = next(
            candidate
            for candidate in session_after_accept_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert accepted_candidate["status"] == "accepted"
        assert accepted_candidate["preview"]["preview_type"] == "收入"
        assert accepted_candidate["details"]["rule_id"] == rule_id

        reject_expected_state = {
            "sessionId": session_id,
            "reviewStatus": "accepted",
            "previewType": "收入",
            "categoryId": _find_category_id_by_name("餐饮", "咖啡", user_id=current_user_id),
            "recurringId": None,
            "sourceAccountId": learned_source_account_id,
            "destinationAccountId": learned_destination_account_id,
        }
        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": reject_expected_state},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        assert reject_data["success"] is True
        reject_preview = next(item for item in reject_data["data"]["preview"] if int(item["id"]) == preview_id)
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["preview_main_category"] == ""
        assert reject_preview["preview_sub_category"] == ""
        assert reject_preview["preview_source_account_id"] == initial_source_account_id
        assert reject_preview["preview_destination_account_id"] in (None, "", 0)
        assert reject_preview["matching"]["learning"]["review_status"] == "rejected"
        assert reject_preview["matching"]["learning"]["suppressed"] is True

        session_after_reject_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_after_reject_response.status_code == 200
        rejected_candidate = next(
            candidate
            for candidate in session_after_reject_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert rejected_candidate["status"] == "rejected"
        assert rejected_candidate["preview"]["preview_type"] == "支出"

    def test_matching_candidate_accept_rejects_preview_learning_without_valid_rule_id(self, client):
        """preview learning accept 应要求有效 ruleId，且只接受当前可见 candidate 对应的 rule。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_learning_rule_id")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-rule-id-{int(time.time() * 1000)}"

        _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="瑞幸咖啡",
            description="到店消费",
            payment_method="支付宝",
            learned_type="支出",
        )
        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 10:10:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:learning"
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
            "sourceAccountId": None,
            "destinationAccountId": None,
        }

        missing_rule_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )
        assert missing_rule_response.status_code == 400
        assert missing_rule_response.get_json()["error"] == "Missing ruleId"

        invalid_rule_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"ruleId": True, "expectedState": expected_state},
            headers=auth_headers,
        )
        assert invalid_rule_response.status_code == 400
        assert invalid_rule_response.get_json()["error"] == "Invalid request"

        wrong_rule_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"ruleId": rule_id + 999, "expectedState": expected_state},
            headers=auth_headers,
        )
        assert wrong_rule_response.status_code == 400
        assert wrong_rule_response.get_json()["error"] == "Learning candidate not available"

    def test_matching_candidate_accept_ignores_stale_learning_rule_account_ids(self, client):
        """preview learning accept 遇到已失效账户 ID 时，只应用仍然有效的学习结果字段。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_learning_stale_accounts")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-stale-accounts-{int(time.time() * 1000)}"

        from src.api.app import db

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest stale rule 初始账户")
        learned_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert learned_category_id is not None

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _prepare_rule_and_preview() -> int:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_source_account_id = ?,
                    learned_destination_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    int(learned_category_id),
                    999999,
                    999998,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 10:40:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare_rule_and_preview())
        candidate_id = f"preview:{preview_id}:learning"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": initial_source_account_id,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_preview = next(
            item for item in accept_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert accept_preview["preview_type"] == "收入"
        assert accept_preview["preview_main_category"] == "餐饮"
        assert accept_preview["preview_sub_category"] == "咖啡"
        assert accept_preview["preview_source_account_id"] == initial_source_account_id
        assert accept_preview["preview_destination_account_id"] in (None, "", 0)

    def test_matching_candidate_accept_ignores_stale_learning_rule_category_id(self, client):
        """preview learning accept 遇到已失效分类 ID 时，不应按同名文本绑定任意分类。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_learning_stale_category")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-stale-category-{int(time.time() * 1000)}"

        from src.api.app import db

        asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _prepare_rule_and_preview() -> int:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    999999,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 11:10:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare_rule_and_preview())
        candidate_id = f"preview:{preview_id}:learning"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": None,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_preview = next(
            item for item in accept_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert accept_preview["preview_type"] == "收入"
        assert accept_preview["preview_main_category"] == ""
        assert accept_preview["preview_sub_category"] == ""
        assert accept_preview["category_id"] in (None, "", 0)

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
        """generic accept 应拒绝非法 candidateId、缺失 recurringId 与仍未支持 family。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_invalid")

        malformed_response = client.post(
            "/api/matching/candidates/not-a-valid-candidate-id/accept",
            json={},
            headers=auth_headers,
        )
        assert malformed_response.status_code == 400
        assert malformed_response.get_json()["error"] == "Invalid candidateId"

        missing_recurring_id_response = client.post(
            "/api/matching/candidates/preview:99:recurring/accept",
            json={"expectedState": {}},
            headers=auth_headers,
        )
        assert missing_recurring_id_response.status_code == 400
        assert missing_recurring_id_response.get_json()["error"] == "Missing recurringId"

        for invalid_recurring_id in (True, 1.9, "1.0"):
            invalid_recurring_id_response = client.post(
                "/api/matching/candidates/preview:99:recurring/accept",
                json={"recurringId": invalid_recurring_id, "expectedState": {}},
                headers=auth_headers,
            )
            assert invalid_recurring_id_response.status_code == 400
            assert invalid_recurring_id_response.get_json()["error"] == "Invalid request"

        unsupported_response = client.post(
            "/api/matching/candidates/preview:99:mystery/accept",
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

    @pytest.mark.skip(
        reason="Preview recurring candidate path now runs through Rust import runtime, not Flask bills import routes."
    )
    def test_matching_candidate_accept_is_user_scoped_for_preview_recurring(self, client):
        """generic accept 的 preview recurring 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_recurring_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_recurring_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        source_account = _ensure_test_account(client, primary_headers)
        category = _ensure_test_expense_category(client, primary_headers)

        _create_test_recurring_template(
            client,
            primary_headers,
            name="pytest matching recurring accept scope 一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9300,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        _create_test_recurring_template(
            client,
            primary_headers,
            name="pytest matching recurring accept scope 二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9300,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1",
        )

        session_id = f"pytest-matching-accept-recurring-scope-{int(time.time() * 1000)}"
        _create_test_import_session(
            session_id,
            [
                {
                    "date": "2026-03-09 10:30:00",
                    "amount": -93.0,
                    "type": "支出",
                    "description": "pytest recurring accept scope preview",
                    "counterparty": "pytest recurring accept scope vendor",
                    "payment_method": "pytest recurring accept scope account",
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
            f"/api/matching/candidates/preview:{preview_id}:recurring/accept",
            json={
                "recurringId": preview_item.get("preview_recurring_id"),
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": preview_item.get("preview_type"),
                    "categoryId": _find_category_id_by_name(
                        str(preview_item.get("preview_main_category") or ""),
                        str(preview_item.get("preview_sub_category") or ""),
                        user_id=primary_user_id,
                    ),
                    "recurringId": preview_item.get("preview_recurring_id"),
                },
            },
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Preview bill not found"

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
                    "preview_main_category": "pytest transfer reject main",
                    "preview_sub_category": "pytest transfer reject sub",
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
        reject_preview = next(item for item in reject_data["data"]["preview"] if int(item["id"]) == preview_id)
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["preview_main_category"] == "pytest transfer reject main"
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

    @pytest.mark.skip(
        reason="Preview investment refresh path now runs through Rust import runtime, not Flask bills import routes."
    )
    def test_matching_session_candidates_hide_preview_investment_candidate_after_refresh(self, client):
        """导入预览 investment row 刷新前后都不应出现独立 investment candidate。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_preview_investment")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-reject-investment-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 09:20:00",
                    "preview_type": "投资",
                    "preview_amount": 68.8,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "黄金ETF 自动定投",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        session_before_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_before_response.status_code == 200
        assert all(
            candidate["kind"] != "investment"
            for candidate in session_before_response.get_json()["data"]["candidates"]
        )

        refresh_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            json={
                "decision": "reject",
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "投资",
                    "categoryId": None,
                    "recurringId": None,
                },
            },
            headers=auth_headers,
        )
        assert refresh_response.status_code in {200, 409}

        session_after_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_after_response.status_code == 200
        assert all(
            candidate["kind"] != "investment"
            for candidate in session_after_response.get_json()["data"]["candidates"]
        )

    def test_matching_candidate_reject_rejects_preview_learning_candidate_and_projects_rejected_status(self, client):
        """preview learning reject 应走 preview-scoped feedback，并把 session candidate 状态投影为 rejected。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_preview_learning")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-reject-learning-{int(time.time() * 1000)}"

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest reject learning 初始账户")

        _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="支出",
        )

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 09:40:00",
                    "preview_type": "支出",
                    "preview_amount": 38.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:learning"
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": None,
            "recurringId": None,
            "sourceAccountId": initial_source_account_id,
            "destinationAccountId": None,
        }

        session_before_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_before_response.status_code == 200
        session_before_candidates = session_before_response.get_json()["data"]["candidates"]
        learning_candidate = next(
            candidate for candidate in session_before_candidates if candidate["candidate_id"] == candidate_id
        )
        assert learning_candidate["status"] == "pending"

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={
                "expectedState": expected_state,
                "responseMode": "preview-item",
            },
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        assert reject_data["success"] is True
        assert reject_data["data"]["candidateId"] == candidate_id
        assert reject_data["data"]["action"] == "reject"
        assert reject_data["data"]["previewId"] == preview_id
        assert reject_data["data"]["sessionId"] == session_id
        assert "preview" not in reject_data["data"]
        reject_preview = reject_data["data"]["previewItem"]
        assert int(reject_preview["id"]) == preview_id
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["matching"]["learning"]["review_status"] == "rejected"
        assert reject_preview["matching"]["learning"]["suppressed"] is True

        session_follow_up_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_follow_up_response.status_code == 200
        session_follow_up_candidates = session_follow_up_response.get_json()["data"]["candidates"]
        rejected_candidate = next(
            candidate for candidate in session_follow_up_candidates if candidate["candidate_id"] == candidate_id
        )
        assert rejected_candidate["status"] == "rejected"
        assert rejected_candidate["details"]["suppressed"] is True

        stale_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={"expectedState": expected_state},
            headers=auth_headers,
        )
        assert stale_response.status_code == 409
        assert stale_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_candidate_reject_returns_409_when_preview_learning_accounts_changed_after_accept(self, client):
        """preview learning 在 accept 后若账户字段已被其他修改，旧 expectedState 再 reject 应返回 409。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_preview_learning_stale_account")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-stale-account-{int(time.time() * 1000)}"

        from src.api.app import db

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest stale learning 初始账户")
        learned_source_account_id = _create_account_via_db(current_user_id, "pytest stale learning 学习账户")
        mutated_source_account_id = _create_account_via_db(current_user_id, "pytest stale learning 改写账户")
        learned_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert learned_category_id is not None

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _prepare_rule_and_preview() -> int:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_source_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    int(learned_category_id),
                    learned_source_account_id,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 12:10:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare_rule_and_preview())
        candidate_id = f"preview:{preview_id}:learning"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": initial_source_account_id,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert accept_response.status_code == 200

        asyncio.run(
            db.update_preview_bill(
                preview_id,
                {"preview_source_account_id": mutated_source_account_id},
                user_id=current_user_id,
            )
        )

        stale_reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "accepted",
                    "previewType": "收入",
                    "categoryId": int(learned_category_id),
                    "recurringId": None,
                    "sourceAccountId": learned_source_account_id,
                    "destinationAccountId": None,
                }
            },
            headers=auth_headers,
        )
        assert stale_reject_response.status_code == 409
        assert stale_reject_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_candidate_reject_preserves_manual_preview_learning_edits_after_accept(self, client):
        """preview learning 在 accept 后若用户手改了 learning 管辖字段，后续 reject 不应恢复更早 snapshot。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_preview_learning_manual_edit")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-manual-edit-{int(time.time() * 1000)}"

        from src.api.app import db

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest manual learning 初始账户")
        learned_source_account_id = _create_account_via_db(current_user_id, "pytest manual learning 学习账户")
        manually_edited_source_account_id = _create_account_via_db(current_user_id, "pytest manual learning 手改账户")
        learned_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert learned_category_id is not None

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _prepare_rule_and_preview() -> int:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_source_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    int(learned_category_id),
                    learned_source_account_id,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 12:30:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare_rule_and_preview())
        candidate_id = f"preview:{preview_id}:learning"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": initial_source_account_id,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert accept_response.status_code == 200

        asyncio.run(
            db.update_preview_bill(
                preview_id,
                {"preview_source_account_id": manually_edited_source_account_id},
                user_id=current_user_id,
            )
        )

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "accepted",
                    "previewType": "收入",
                    "categoryId": int(learned_category_id),
                    "recurringId": None,
                    "sourceAccountId": manually_edited_source_account_id,
                    "destinationAccountId": None,
                }
            },
            headers=auth_headers,
        )
        assert reject_response.status_code == 200
        reject_preview = next(
            item for item in reject_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert reject_preview["preview_type"] == "收入"
        assert reject_preview["preview_main_category"] == "餐饮"
        assert reject_preview["preview_sub_category"] == "咖啡"
        assert reject_preview["preview_source_account_id"] == manually_edited_source_account_id
        assert reject_preview["matching"]["learning"]["review_status"] == "rejected"
        assert reject_preview["matching"]["learning"]["suppressed"] is True

    def test_matching_candidate_accept_keeps_learning_candidate_visible_after_text_edit_until_reject(self, client):
        """preview learning accept 后即便文本特征改到不再命中，accepted candidate 仍应可见并可 reject。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_learning_text_edit")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-text-edit-{int(time.time() * 1000)}"

        from src.api.app import db

        initial_source_account_id = _create_account_via_db(current_user_id, "pytest text edit 初始账户")
        learned_source_account_id = _create_account_via_db(current_user_id, "pytest text edit 学习账户")
        learned_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "餐饮",
                    "sub_category": "咖啡",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert learned_category_id is not None

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        async def _prepare_rule_and_preview() -> int:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_source_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    int(learned_category_id),
                    learned_source_account_id,
                    rule_id,
                    current_user_id,
                ),
            )
            await conn.commit()
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 12:50:00",
                    "preview_type": "支出",
                    "preview_amount": 30.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_source_account_id": initial_source_account_id,
                    "preview_destination_account_id": None,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare_rule_and_preview())
        candidate_id = f"preview:{preview_id}:learning"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": initial_source_account_id,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert accept_response.status_code == 200

        asyncio.run(
            db.update_preview_bill(
                preview_id,
                {
                    "preview_counterparty": "完全不相关商户",
                    "preview_description": "不再匹配长期学习",
                    "preview_payment_method": "现金",
                },
                user_id=current_user_id,
            )
        )

        session_after_edit_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_after_edit_response.status_code == 200
        accepted_candidate = next(
            candidate
            for candidate in session_after_edit_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert accepted_candidate["status"] == "accepted"
        assert accepted_candidate["details"]["rule_id"] == rule_id

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "accepted",
                    "previewType": "收入",
                    "categoryId": int(learned_category_id),
                    "recurringId": None,
                    "sourceAccountId": learned_source_account_id,
                    "destinationAccountId": None,
                }
            },
            headers=auth_headers,
        )
        assert reject_response.status_code == 200
        reject_preview = next(
            item for item in reject_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["preview_main_category"] == ""
        assert reject_preview["preview_sub_category"] == ""

        session_after_reject_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_after_reject_response.status_code == 200
        rejected_candidate = next(
            candidate
            for candidate in session_after_reject_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert rejected_candidate["status"] == "rejected"
        assert rejected_candidate["details"]["rule_id"] == rule_id

        reaccept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "rejected",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": initial_source_account_id,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert reaccept_response.status_code == 200
        reaccept_preview = next(
            item for item in reaccept_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )
        assert reaccept_preview["preview_type"] == "收入"
        assert reaccept_preview["preview_main_category"] == "餐饮"
        assert reaccept_preview["preview_sub_category"] == "咖啡"

    def test_matching_candidate_learning_rule_switch_resets_old_review_state(self, client):
        """当 live learning rule 切换到另一条规则时，旧 rule 的 review_status 不应串到新 rule 上。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_learning_rule_switch")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-learning-rule-switch-{int(time.time() * 1000)}"

        rule_a = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )
        rule_b = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="瑞幸咖啡",
            description="线上点单",
            payment_method="微信支付",
            learned_type="支出",
        )

        from src.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 13:20:00",
                    "preview_type": "支出",
                    "preview_amount": 26.0,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:learning"

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": None,
                    "destinationAccountId": None,
                }
            },
            headers=auth_headers,
        )
        assert reject_response.status_code == 200

        asyncio.run(
            db.update_preview_bill(
                preview_id,
                {
                    "preview_counterparty": "瑞幸咖啡",
                    "preview_description": "线上点单加冰",
                    "preview_payment_method": "微信支付",
                },
                user_id=current_user_id,
            )
        )

        switched_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert switched_response.status_code == 200
        switched_candidate = next(
            candidate
            for candidate in switched_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        assert switched_candidate["status"] == "pending"
        assert switched_candidate["details"]["rule_id"] == rule_b

        stale_accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_a,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": None,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert stale_accept_response.status_code == 400
        assert stale_accept_response.get_json()["error"] == "Learning candidate not available"

        accept_switched_rule_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_b,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                    "sourceAccountId": None,
                    "destinationAccountId": None,
                },
            },
            headers=auth_headers,
        )
        assert accept_switched_rule_response.status_code == 200

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
        assert [candidate["billId"] for candidate in unified_follow_up_data["candidates"]] == [
            retained_candidate_bill_id
        ]

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

    def test_matching_candidate_reject_transfer_hides_same_pair_from_investment_family(self, client):
        """transfer reject 在 mixed selector 下仍应按 logical pair 隐藏同一 investment-like pair。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_transfer_hides_investment")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest transfer reject investment 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest transfer reject investment 目标账户")
        third_account_id = _create_account_via_db(current_user_id, "pytest transfer reject investment 第三账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-71.0,
            bill_type="投资",
            date="2026-07-27 09:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        rejected_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=71.0,
            bill_type="投资",
            date="2026-07-27 09:02:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        retained_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=71.0,
            bill_type="投资",
            date="2026-07-27 09:05:00",
            description="蚂蚁财富 黄金ETF 自动定投 赎回",
        )
        candidate_id = f"bill:{anchor_bill_id}:transfer:{rejected_candidate_bill_id}"

        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_candidates = initial_response.get_json()["data"]["candidates"]
        initial_transfer_candidates = [candidate for candidate in initial_candidates if candidate["kind"] == "transfer"]
        initial_investment_candidates = [
            candidate for candidate in initial_candidates if candidate["kind"] == "investment"
        ]
        assert [candidate["billId"] for candidate in initial_transfer_candidates] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]
        assert [candidate["billId"] for candidate in initial_investment_candidates] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert reject_response.status_code == 200

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert follow_up_response.status_code == 200
        follow_up_candidates = follow_up_response.get_json()["data"]["candidates"]
        follow_up_transfer_candidates = [candidate for candidate in follow_up_candidates if candidate["kind"] == "transfer"]
        follow_up_investment_candidates = [
            candidate for candidate in follow_up_candidates if candidate["kind"] == "investment"
        ]
        assert [candidate["billId"] for candidate in follow_up_transfer_candidates] == [retained_candidate_bill_id]
        assert [candidate["billId"] for candidate in follow_up_investment_candidates] == [retained_candidate_bill_id]

    def test_matching_bill_candidates_include_historical_learning_candidates(self, client):
        """historical bill candidates 应返回 formal-bill learning 候选与稳定 candidateId。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_learning_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest learning vendor",
            description="pytest learning note",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(current_user_id, "pytest learning 历史账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-45.6,
            bill_type="支出",
            date="2026-07-24 13:00:00",
            description="pytest learning note",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        candidates = data["data"]["candidates"]
        learning_candidate = next(candidate for candidate in candidates if candidate["kind"] == "learning")
        assert learning_candidate["candidateId"].startswith(f"bill:{anchor_bill_id}:learning:{rule_id}:")
        assert learning_candidate["ruleId"] == rule_id
        assert learning_candidate["recommendedType"] == "支出"
        assert learning_candidate["summary"]

        unified_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_response.status_code == 200
        unified_candidates = unified_response.get_json()["data"]["candidates"]
        assert any(candidate["candidateId"] == learning_candidate["candidateId"] for candidate in unified_candidates)

    def test_matching_candidate_reject_rejects_historical_learning_candidate_and_filters_followup_reads(self, client):
        """historical formal-bill learning reject 应持久化 suppression，并过滤后续 bill/unified 候选读取。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_learning")
        current_user_id = _get_current_user_id(client, auth_headers)

        rejected_rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest bill learning vendor",
            description="pytest bill learning anchor",
            payment_method="银行卡",
            learned_type="支出",
        )
        retained_rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest bill learning vendor",
            description="pytest bill learning anchor extra",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(current_user_id, "pytest bill learning reject 账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-63.0,
            bill_type="支出",
            date="2026-07-24 14:00:00",
            description="pytest bill learning anchor",
        )
        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_candidates = [
            candidate
            for candidate in initial_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert [candidate["ruleId"] for candidate in initial_candidates] == [rejected_rule_id, retained_rule_id]
        candidate_id = next(
            candidate["candidateId"] for candidate in initial_candidates if candidate["ruleId"] == rejected_rule_id
        )

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        assert reject_response.get_json()["data"] == {
            "candidateId": candidate_id,
            "action": "reject",
        }

        bill_follow_up_response = client.get(
            f"/api/matching/bills/{anchor_bill_id}/candidates",
            headers=auth_headers,
        )
        assert bill_follow_up_response.status_code == 200
        bill_learning_candidates = [
            candidate
            for candidate in bill_follow_up_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert [candidate["ruleId"] for candidate in bill_learning_candidates] == [retained_rule_id]

        unified_follow_up_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_follow_up_response.status_code == 200
        unified_learning_candidates = [
            candidate
            for candidate in unified_follow_up_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert [candidate["ruleId"] for candidate in unified_learning_candidates] == [retained_rule_id]

        repeat_reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert repeat_reject_response.status_code == 400
        assert repeat_reject_response.get_json()["error"] == "Learning candidate not available"

    def test_matching_candidate_accept_applies_historical_learning_candidate_to_bill(self, client):
        """historical formal-bill learning accept 应把 rule 结果应用到正式账单。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_learning")
        current_user_id = _get_current_user_id(client, auth_headers)

        from src.api.app import db

        target_category_id = asyncio.run(
            db.create_category(
                {
                    "type": 1,
                    "main_category": "pytest bill learning accept main",
                    "sub_category": "pytest bill learning accept sub",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
        )
        assert target_category_id is not None
        destination_account_id = _create_account_via_db(current_user_id, "pytest bill learning accept 目标账户")

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest accept learning vendor",
            description="pytest accept learning note",
            payment_method="银行卡",
            learned_type="收入",
        )

        async def _update_learning_rule() -> None:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET learned_category_id = ?,
                    learned_destination_account_id = ?
                WHERE id = ? AND user_id = ?
                """,
                (int(target_category_id), destination_account_id, rule_id, current_user_id),
            )
            await conn.commit()

        asyncio.run(_update_learning_rule())

        source_account_id = _create_account_via_db(current_user_id, "pytest bill learning accept 源账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-72.5,
            bill_type="支出",
            date="2026-07-24 16:00:00",
            description="pytest accept learning note",
        )
        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        candidate_id = next(
            candidate["candidateId"]
            for candidate in initial_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["action"] == "accept"
        accepted_bill = accept_data["data"]["bill"]
        assert accepted_bill["id"] == anchor_bill_id
        assert accepted_bill["type"] == "收入"
        assert accepted_bill["mainCategory"] == "pytest bill learning accept main"
        assert accepted_bill["subCategory"] == "pytest bill learning accept sub"
        assert accepted_bill["destinationAccountId"] == destination_account_id

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert follow_up_response.status_code == 200
        learning_candidates = [
            candidate
            for candidate in follow_up_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert learning_candidates == []

        repeat_accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert repeat_accept_response.status_code == 400
        assert repeat_accept_response.get_json()["error"] == "Learning candidate not available"

    def test_matching_candidate_accept_is_user_scoped_for_historical_learning(self, client):
        """generic accept 的 historical learning 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_bill_learning_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_bill_learning_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)

        rule_id = _create_composite_learning_rule_via_db(
            primary_user_id,
            parser_id="wechat",
            counterparty="pytest bill learning accept scope vendor",
            description="pytest bill learning accept scope note",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(primary_user_id, "pytest bill learning accept scope 账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-41.0,
            bill_type="支出",
            date="2026-07-24 17:00:00",
            description="pytest bill learning accept scope note",
        )

        candidate_id = next(
            candidate["candidateId"]
            for candidate in client.get(
                f"/api/matching/bills/{anchor_bill_id}/candidates",
                headers=primary_headers,
            ).get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=secondary_headers,
        )
        assert accept_response.status_code == 404
        assert accept_response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_accept_can_resolve_historical_learning_candidate_without_bill_changes(self, client):
        """当 historical learning candidate 不会改动 bill 字段时，首次 accept 仍应记为 resolved 并隐藏后续重复候选。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_learning_noop")
        current_user_id = _get_current_user_id(client, auth_headers)

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest learning noop vendor",
            description="pytest learning noop note",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(current_user_id, "pytest learning noop 账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-33.0,
            bill_type="支出",
            date="2026-07-26 10:00:00",
            description="pytest learning noop note",
        )
        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_learning_candidates = [
            candidate
            for candidate in initial_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert [candidate["ruleId"] for candidate in initial_learning_candidates] == [rule_id]
        candidate_id = initial_learning_candidates[0]["candidateId"]

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["bill"]["id"] == anchor_bill_id
        assert accept_data["data"]["bill"]["type"] == "支出"

        follow_up_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert follow_up_response.status_code == 200
        follow_up_learning_candidates = [
            candidate
            for candidate in follow_up_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert follow_up_learning_candidates == []

        repeat_accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert repeat_accept_response.status_code == 400
        assert repeat_accept_response.get_json()["error"] == "Learning candidate not available"

    @pytest.mark.skip(
        reason="Preview recurring candidate path now runs through Rust import runtime, not Flask bills import routes."
    )
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
            transfer_details = ((item.get("matching") or {}).get("transfer") or {})
            transfer_review_status = str(transfer_details.get("review_status") or "").strip().lower()
            if transfer_review_status not in {"accepted", "rejected"}:
                transfer_review_status = "pending" if str(transfer_details.get("candidate_type") or "").strip() else ""
            return {
                "sessionId": session_id,
                "reviewStatus": transfer_review_status,
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
        cleared_preview = next(item for item in reject_data["data"]["preview"] if int(item["id"]) == preview_id)
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

        malformed_response = client.post(
            "/api/matching/candidates/not-a-valid-candidate-id/reject",
            json={},
            headers=auth_headers,
        )
        assert malformed_response.status_code == 400
        assert malformed_response.get_json()["error"] == "Invalid candidateId"

        unsupported_response = client.post(
            "/api/matching/candidates/preview:99:mystery/reject",
            json={},
            headers=auth_headers,
        )
        assert unsupported_response.status_code == 400
        assert unsupported_response.get_json()["error"] == "Candidate family not supported"

    def test_matching_bill_candidates_include_historical_investment_candidates(self, client):
        """historical bill selector 应返回 formal-bill investment 候选与稳定 candidateId。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_investment_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest investment 历史源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest investment 历史目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-45.6,
            bill_type="投资",
            date="2026-07-24 13:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=45.6,
            bill_type="投资",
            date="2026-07-24 13:02:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        candidates = data["data"]["candidates"]
        transfer_candidates = [candidate for candidate in candidates if candidate["kind"] == "transfer"]
        investment_candidates = [candidate for candidate in candidates if candidate["kind"] == "investment"]
        assert [candidate["billId"] for candidate in transfer_candidates] == [candidate_bill_id]
        assert [candidate["candidateId"] for candidate in investment_candidates] == [
            f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"
        ]
        assert [candidate["billId"] for candidate in investment_candidates] == [candidate_bill_id]

        unified_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_response.status_code == 200
        unified_candidates = unified_response.get_json()["data"]["candidates"]
        assert any(candidate["candidateId"] == f"bill:{anchor_bill_id}:investment:{candidate_bill_id}" for candidate in unified_candidates)

    def test_matching_candidate_reject_rejects_historical_investment_candidate_and_filters_only_investment_followup_reads(self, client):
        """historical formal-bill investment reject 应持久化 suppression，并只过滤 investment family。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_investment")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest bill investment reject 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest bill investment reject 目标账户")
        third_account_id = _create_account_via_db(current_user_id, "pytest bill investment reject 第三账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-57.0,
            bill_type="投资",
            date="2026-07-23 09:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        rejected_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=57.0,
            bill_type="投资",
            date="2026-07-23 09:02:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        retained_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=third_account_id,
            amount=57.0,
            bill_type="投资",
            date="2026-07-23 09:05:00",
            description="蚂蚁财富 黄金ETF 自动定投 赎回",
        )
        candidate_id = f"bill:{anchor_bill_id}:investment:{rejected_candidate_bill_id}"

        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_candidates = initial_response.get_json()["data"]["candidates"]
        initial_transfer_candidates = [candidate for candidate in initial_candidates if candidate["kind"] == "transfer"]
        initial_investment_candidates = [candidate for candidate in initial_candidates if candidate["kind"] == "investment"]
        assert [candidate["billId"] for candidate in initial_transfer_candidates] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]
        assert [candidate["billId"] for candidate in initial_investment_candidates] == [
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
        follow_up_transfer_candidates = [
            candidate for candidate in bill_follow_up_data["candidates"] if candidate["kind"] == "transfer"
        ]
        follow_up_investment_candidates = [
            candidate for candidate in bill_follow_up_data["candidates"] if candidate["kind"] == "investment"
        ]
        assert [candidate["billId"] for candidate in follow_up_transfer_candidates] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]
        assert [candidate["billId"] for candidate in follow_up_investment_candidates] == [retained_candidate_bill_id]

        repeated_reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert repeated_reject_response.status_code == 200
        assert repeated_reject_response.get_json()["data"] == {
            "candidateId": candidate_id,
            "action": "reject",
        }

        unified_follow_up_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_follow_up_response.status_code == 200
        unified_follow_up_data = unified_follow_up_response.get_json()["data"]
        unified_investment_candidates = [
            candidate for candidate in unified_follow_up_data["candidates"] if candidate["kind"] == "investment"
        ]
        assert [candidate["billId"] for candidate in unified_investment_candidates] == [retained_candidate_bill_id]

        reverse_follow_up_response = client.get(
            f"/api/matching/bills/{rejected_candidate_bill_id}/candidates",
            headers=auth_headers,
        )
        assert reverse_follow_up_response.status_code == 200
        reverse_candidates = reverse_follow_up_response.get_json()["data"]["candidates"]
        reverse_transfer_candidates = [candidate for candidate in reverse_candidates if candidate["kind"] == "transfer"]
        reverse_investment_candidates = [candidate for candidate in reverse_candidates if candidate["kind"] == "investment"]
        assert anchor_bill_id in {candidate["billId"] for candidate in reverse_transfer_candidates}
        assert anchor_bill_id not in {candidate["billId"] for candidate in reverse_investment_candidates}

    def test_matching_bill_candidates_hide_learning_candidates_when_transfer_pair_already_exists(self, client):
        """已存在 transfer linkedPair 的正式账单不应继续透出 learning candidates。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_candidates_linked_pair_hides_learning")
        current_user_id = _get_current_user_id(client, auth_headers)

        _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest linked pair learning vendor",
            description="pytest linked pair learning note",
            payment_method="银行卡",
            learned_type="支出",
        )

        source_account_id = _create_account_via_db(current_user_id, "pytest linked pair learning 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest linked pair learning 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-52.0,
            bill_type="支出",
            date="2026-07-26 09:00:00",
            description="pytest linked pair learning note",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=52.0,
            bill_type="收入",
            date="2026-07-26 09:03:00",
            description="pytest linked pair transfer candidate",
        )

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=auth_headers,
        )
        assert pair_response.status_code == 200

        bill_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert bill_response.status_code == 200
        bill_data = bill_response.get_json()["data"]
        assert bill_data["linkedPair"]["otherBillId"] == candidate_bill_id
        assert bill_data["candidates"] == []

        unified_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_response.status_code == 200
        unified_data = unified_response.get_json()["data"]
        assert unified_data["linkedPair"]["otherBillId"] == candidate_bill_id
        assert unified_data["candidates"] == []

    def test_matching_candidate_accept_accepts_historical_investment_candidate_and_allows_manual_delete(self, client):
        """historical formal-bill investment accept 应写入 manual pair，并允许通过现有 pair delete 撤销。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_investment")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest accept investment 历史转出账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest accept investment 历史转入账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-72.0,
            bill_type="投资",
            date="2026-07-21 10:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=72.0,
            bill_type="投资",
            date="2026-07-21 10:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        candidate_id = f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["candidateId"] == candidate_id
        assert accept_data["data"]["pair"]["pairType"] == "investment"
        assert accept_data["data"]["pair"]["leftBillId"] == min(anchor_bill_id, candidate_bill_id)
        assert accept_data["data"]["pair"]["rightBillId"] == max(anchor_bill_id, candidate_bill_id)

        bill_follow_up_response = client.get(
            f"/api/matching/bills/{anchor_bill_id}/candidates",
            headers=auth_headers,
        )
        assert bill_follow_up_response.status_code == 200
        bill_follow_up_data = bill_follow_up_response.get_json()["data"]
        assert bill_follow_up_data["linkedPair"]["pairType"] == "investment"
        assert bill_follow_up_data["linkedPair"]["otherBillId"] == candidate_bill_id
        assert bill_follow_up_data["candidates"] == []

        unified_follow_up_response = client.get(
            f"/api/matching/candidates?billId={anchor_bill_id}",
            headers=auth_headers,
        )
        assert unified_follow_up_response.status_code == 200
        unified_follow_up_data = unified_follow_up_response.get_json()["data"]
        assert unified_follow_up_data["linkedPair"]["pairType"] == "investment"
        assert unified_follow_up_data["linkedPair"]["otherBillId"] == candidate_bill_id
        assert unified_follow_up_data["candidates"] == []

        list_pairs_response = client.get("/api/matching/pairs", headers=auth_headers)
        assert list_pairs_response.status_code == 200
        list_pairs = list_pairs_response.get_json()["data"]["pairs"]
        assert len(list_pairs) == 1
        assert list_pairs[0]["id"] == accept_data["data"]["pair"]["id"]
        assert list_pairs[0]["pairType"] == "investment"
        assert list_pairs[0]["source"] == "manual"
        assert list_pairs[0]["leftBillId"] == min(anchor_bill_id, candidate_bill_id)
        assert list_pairs[0]["rightBillId"] == max(anchor_bill_id, candidate_bill_id)
        assert list_pairs[0]["leftBill"]["id"] == list_pairs[0]["leftBillId"]
        assert list_pairs[0]["rightBill"]["id"] == list_pairs[0]["rightBillId"]

        delete_response = client.delete(
            f"/api/matching/pairs/{accept_data['data']['pair']['id']}",
            headers=auth_headers,
        )
        assert delete_response.status_code == 200
        assert delete_response.get_json()["data"]["pair"]["pairType"] == "investment"

        restored_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert restored_response.status_code == 200
        restored_candidates = restored_response.get_json()["data"]["candidates"]
        restored_investment_candidates = [
            candidate for candidate in restored_candidates if candidate["kind"] == "investment"
        ]
        assert [candidate["billId"] for candidate in restored_investment_candidates] == [candidate_bill_id]

    def test_matching_candidate_accept_blocks_historical_investment_candidate_after_prior_reject(self, client):
        """historical investment generic reject 后，不应再通过 generic accept 绕回同一 logical pair。"""
        auth_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_bill_investment_after_reject"
        )
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest accept reject investment 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest accept reject investment 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-61.0,
            bill_type="投资",
            date="2026-07-21 12:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=61.0,
            bill_type="投资",
            date="2026-07-21 12:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        candidate_id = f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert reject_response.status_code == 200

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert accept_response.status_code == 409
        assert accept_response.get_json()["error"] == "Bills already rejected for investment pairing"

    def test_matching_candidate_accept_is_user_scoped_for_historical_investment(self, client):
        """generic accept 的 historical investment 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_bill_investment_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_bill_investment_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest accept investment scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest accept investment scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-68.0,
            bill_type="投资",
            date="2026-07-23 11:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=68.0,
            bill_type="投资",
            date="2026-07-23 11:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )

        accept_response = client.post(
            f"/api/matching/candidates/bill:{anchor_bill_id}:investment:{candidate_bill_id}/accept",
            json={},
            headers=secondary_headers,
        )
        assert accept_response.status_code == 404
        assert accept_response.get_json()["error"] == "Bill not found"

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

    @pytest.mark.skip(
        reason="Preview recurring candidate path now runs through Rust import runtime, not Flask bills import routes."
    )
    def test_matching_candidate_reject_is_user_scoped_for_preview_recurring(self, client):
        """generic reject 的 preview recurring 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_recurring_scope_primary")
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_recurring_scope_secondary"
        )
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
                    "reviewStatus": "pending",
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

    @pytest.mark.skip(
        reason="Preview recurring candidate path now runs through Rust import runtime, not Flask bills import routes."
    )
    def test_matching_candidate_accept_rejects_preview_recurring_with_stale_transfer_review_state(self, client):
        """当 transfer review 已变化时，preview recurring generic accept 应拒绝旧 reviewStatus 快照。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_preview_recurring_transfer_stale")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)

        from src.api.app import db

        recurring_template = _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest matching recurring accept stale transfer",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9700,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        session_id = f"pytest-matching-accept-recurring-transfer-stale-{int(time.time() * 1000)}"

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-27 09:20:00",
                    "preview_type": "支出",
                    "preview_amount": 97.0,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_counterparty": "pytest recurring transfer stale vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest recurring transfer stale preview",
                    "preview_recurring_id": recurring_template["id"],
                    "preview_recurring_name": recurring_template["name"],
                    "preview_recurring_candidate_count": 1,
                    "preview_recurring_match_score": 0.88,
                    "preview_recurring_match_reasons": "schedule|amount",
                    "preview_recurring_matched_date": "2026-07-27",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[171, 172],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        candidate_id = f"preview:{preview_id}:recurring"
        category_id = _find_category_id_by_name("餐饮", "早餐", user_id=current_user_id)
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": category_id,
            "recurringId": recurring_template["id"],
        }

        transfer_reject_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            json={"decision": "reject", "expectedState": expected_state},
            headers=auth_headers,
        )
        assert transfer_reject_response.status_code == 200

        stale_accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={"recurringId": recurring_template["id"], "expectedState": expected_state},
            headers=auth_headers,
        )
        assert stale_accept_response.status_code == 409
        assert stale_accept_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_session_candidates_preview_investment_remains_user_scoped(self, client):
        """不同用户不应通过 session candidates 看到彼此的预览投资候选。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_investment_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_investment_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-reject-investment-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 11:20:00",
                    "preview_type": "投资",
                    "preview_amount": 19.8,
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "黄金ETF 自动定投",
                },
                user_id=primary_user_id,
            )
            return int(preview_id)

        asyncio.run(_create_primary_preview())
        preview_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Import session not found"

    def test_matching_session_candidates_preview_investment_absence_is_stable_for_owner(self, client):
        """预览所有者读取 session candidates 时也不应再看到 investment kind。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_investment_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_investment_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-accept-investment-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 11:20:00",
                    "preview_type": "投资",
                    "preview_amount": 19.8,
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "黄金ETF 自动定投",
                },
                user_id=primary_user_id,
            )
            return int(preview_id)

        asyncio.run(_create_primary_preview())
        preview_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=primary_headers,
        )
        assert preview_response.status_code == 200
        assert all(candidate["kind"] != "investment" for candidate in preview_response.get_json()["data"]["candidates"])

    def test_matching_candidate_accept_is_user_scoped_for_preview_learning(self, client):
        """generic accept 的 preview learning 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_learning_scope_primary")
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_accept_learning_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-accept-learning-scope-{int(time.time() * 1000)}"

        rule_id = _create_composite_learning_rule_via_db(
            primary_user_id,
            parser_id="alipay",
            counterparty="星巴克",
            description="咖啡消费",
            payment_method="支付宝",
            learned_type="支出",
        )

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 11:40:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=primary_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_primary_preview())
        preview_response = client.post(
            f"/api/matching/candidates/preview:{preview_id}:learning/accept",
            json={
                "ruleId": rule_id,
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                },
            },
            headers=secondary_headers,
        )
        assert preview_response.status_code == 404
        assert preview_response.get_json()["error"] == "Preview bill not found"

    def test_matching_candidate_reject_is_user_scoped_for_preview_learning(self, client):
        """generic reject 的 preview learning 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_learning_scope_primary")
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_learning_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-matching-reject-learning-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_primary_preview() -> int:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-24 11:40:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=primary_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_primary_preview())
        preview_response = client.post(
            f"/api/matching/candidates/preview:{preview_id}:learning/reject",
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

    def test_matching_candidate_clear_restores_preview_learning_to_pending(self, client):
        """preview learning generic clear 应把已决策状态恢复为 pending，并保留新的手工编辑字段。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_clear_preview_learning")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-clear-learning-{int(time.time() * 1000)}"

        ensured_category = _ensure_test_expense_category(client, auth_headers)
        category_id = int(ensured_category["id"])
        source_account_id = _create_account_via_db(current_user_id, "pytest learning clear 源账户")
        destination_account_id = _create_account_via_db(current_user_id, "pytest learning clear 目标账户")
        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
            learned_category_id=category_id,
            learned_source_account_id=source_account_id,
            learned_destination_account_id=destination_account_id,
        )

        from src.api.app import db

        async def _create_preview() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-01 10:10:00",
                    "preview_type": "支出",
                    "preview_amount": 38.0,
                    "preview_source_account_id": source_account_id,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview())
        candidate_id = f"preview:{preview_id}:learning"

        session_before_accept_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_before_accept_response.status_code == 200
        learning_candidate_before_accept = next(
            candidate
            for candidate in session_before_accept_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        accept_expected_state = {
            "sessionId": session_id,
            "reviewStatus": learning_candidate_before_accept["status"],
            "previewType": learning_candidate_before_accept["preview"]["preview_type"],
            "categoryId": None,
            "recurringId": learning_candidate_before_accept["preview"].get("preview_recurring_id"),
            "sourceAccountId": learning_candidate_before_accept["preview"].get("preview_source_account_id"),
            "destinationAccountId": learning_candidate_before_accept["preview"].get("preview_destination_account_id"),
        }

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": accept_expected_state,
            },
            headers=auth_headers,
        )
        assert accept_response.status_code == 200
        accepted_preview = next(
            item for item in accept_response.get_json()["data"]["preview"] if int(item["id"]) == preview_id
        )

        async def _manually_edit_preview_accounts() -> None:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                "UPDATE bills_preview SET preview_source_account_id = ?, preview_destination_account_id = ? WHERE id = ? AND user_id = ?",
                (destination_account_id, source_account_id, preview_id, current_user_id),
            )
            await conn.commit()

        asyncio.run(_manually_edit_preview_accounts())

        clear_response = client.post(
            f"/api/matching/candidates/{candidate_id}/clear",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "accepted",
                    "previewType": accepted_preview["preview_type"],
                    "categoryId": category_id,
                    "recurringId": None,
                    "sourceAccountId": destination_account_id,
                    "destinationAccountId": source_account_id,
                }
            },
            headers=auth_headers,
        )

        assert clear_response.status_code == 200
        payload = clear_response.get_json()
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == candidate_id
        assert payload["data"]["action"] == "clear"
        refreshed_preview = next(item for item in payload["data"]["preview"] if int(item["id"]) == preview_id)
        assert refreshed_preview["matching"]["learning"]["review_status"] == "pending"
        assert refreshed_preview["matching"]["learning"]["suppressed"] is False
        assert refreshed_preview["preview_source_account_id"] == destination_account_id
        assert refreshed_preview["preview_destination_account_id"] == source_account_id

    def test_matching_candidate_clear_rejects_stale_preview_learning_state(self, client):
        """preview learning generic clear 应对过期 expectedState 返回 409。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_clear_preview_learning_stale")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-clear-learning-stale-{int(time.time() * 1000)}"
        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="收入",
        )

        from src.api.app import db

        async def _create_preview() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-01 10:40:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克",
                    "preview_payment_method": "支付宝",
                    "preview_description": "咖啡消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview())
        candidate_id = f"preview:{preview_id}:learning"

        session_before_accept_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_before_accept_response.status_code == 200
        learning_candidate_before_accept = next(
            candidate
            for candidate in session_before_accept_response.get_json()["data"]["candidates"]
            if candidate["candidate_id"] == candidate_id
        )
        accept_expected_state = {
            "sessionId": session_id,
            "reviewStatus": learning_candidate_before_accept["status"],
            "previewType": learning_candidate_before_accept["preview"]["preview_type"],
            "categoryId": None,
            "recurringId": learning_candidate_before_accept["preview"].get("preview_recurring_id"),
            "sourceAccountId": learning_candidate_before_accept["preview"].get("preview_source_account_id"),
            "destinationAccountId": learning_candidate_before_accept["preview"].get("preview_destination_account_id"),
        }

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={
                "ruleId": rule_id,
                "expectedState": accept_expected_state,
            },
            headers=auth_headers,
        )
        assert accept_response.status_code == 200

        clear_response = client.post(
            f"/api/matching/candidates/{candidate_id}/clear",
            json={
                "expectedState": {
                    "sessionId": session_id,
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                }
            },
            headers=auth_headers,
        )

        assert clear_response.status_code == 409
        assert clear_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_matching_session_candidates_do_not_surface_preview_investment_after_manual_feedback_rows(self, client):
        """即便 preview 行是投资类型，session candidates 也不再生成 investment kind。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_clear_preview_investment")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-clear-investment-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-02 09:15:00",
                    "preview_type": "投资",
                    "preview_amount": 199.5,
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "黄金 ETF 定投",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        asyncio.run(_create_preview())
        session_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        assert session_response.status_code == 200
        assert all(candidate["kind"] != "investment" for candidate in session_response.get_json()["data"]["candidates"])

    def test_matching_session_candidates_preview_investment_absence_survives_repeated_reads(self, client):
        """重复读取 session candidates 时，preview investment kind 仍应保持隐藏。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_clear_preview_investment_stale")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-matching-clear-investment-stale-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_preview() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-02 09:40:00",
                    "preview_type": "投资",
                    "preview_amount": 88.0,
                    "preview_counterparty": "蚂蚁财富",
                    "preview_payment_method": "支付宝",
                    "preview_description": "基金定投",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        asyncio.run(_create_preview())

        first_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )
        second_response = client.get(
            f"/api/matching/candidates?sessionId={session_id}",
            headers=auth_headers,
        )

        assert first_response.status_code == 200
        assert second_response.status_code == 200
        assert all(candidate["kind"] != "investment" for candidate in first_response.get_json()["data"]["candidates"])
        assert all(candidate["kind"] != "investment" for candidate in second_response.get_json()["data"]["candidates"])

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

    def test_matching_candidate_accept_records_feedback_event_for_historical_transfer(self, client):
        """historical transfer generic accept 成功后，应追加一条 bill_pair_feedback 事件。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_transfer_feedback")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest feedback transfer accept 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest feedback transfer accept 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-48.0,
            bill_type="支出",
            date="2026-07-28 09:00:00",
            description="pytest feedback transfer accept anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=48.0,
            bill_type="收入",
            date="2026-07-28 09:03:00",
            description="pytest feedback transfer accept candidate",
        )
        candidate_id = f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"

        assert _list_bill_pair_feedback_via_db(user_id=current_user_id, candidate_id=candidate_id) == []

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )

        assert accept_response.status_code == 200
        feedback_rows = _list_bill_pair_feedback_via_db(user_id=current_user_id, candidate_id=candidate_id)
        assert len(feedback_rows) == 1
        assert feedback_rows[0]["action"] == "accept"
        assert feedback_rows[0]["payload"] == {
            "scope": "bill",
            "kind": "transfer",
            "bill_id": anchor_bill_id,
            "candidate_bill_id": candidate_bill_id,
            "pair": {
                "id": accept_response.get_json()["data"]["pair"]["id"],
                "pair_type": "transfer",
                "source": "manual",
                "left_bill_id": min(anchor_bill_id, candidate_bill_id),
                "right_bill_id": max(anchor_bill_id, candidate_bill_id),
            },
        }

    def test_matching_candidate_reject_records_feedback_event_for_historical_investment(self, client):
        """historical investment generic reject 成功后，应追加一条 bill_pair_feedback 事件。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_investment_feedback")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest feedback investment reject 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest feedback investment reject 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="投资",
            date="2026-07-28 10:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=58.0,
            bill_type="投资",
            date="2026-07-28 10:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )
        candidate_id = f"bill:{anchor_bill_id}:investment:{candidate_bill_id}"

        assert _list_bill_pair_feedback_via_db(user_id=current_user_id, candidate_id=candidate_id) == []

        reject_response = client.post(
            f"/api/matching/candidates/{candidate_id}/reject",
            json={},
            headers=auth_headers,
        )

        assert reject_response.status_code == 200
        feedback_rows = _list_bill_pair_feedback_via_db(user_id=current_user_id, candidate_id=candidate_id)
        assert len(feedback_rows) == 1
        assert feedback_rows[0]["action"] == "reject"
        assert feedback_rows[0]["payload"] == {
            "scope": "bill",
            "kind": "investment",
            "bill_id": anchor_bill_id,
            "candidate_bill_id": candidate_bill_id,
        }

    def test_matching_candidate_accept_conflict_does_not_record_feedback_event_for_historical_transfer(self, client):
        """historical transfer generic accept 失败时，不应误写 bill_pair_feedback 事件。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_transfer_feedback_conflict")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest feedback transfer conflict 源账户")
        target_account_id = _create_account_via_db(current_user_id, "pytest feedback transfer conflict 目标账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-68.0,
            bill_type="支出",
            date="2026-07-28 11:00:00",
            description="pytest feedback transfer conflict anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=target_account_id,
            amount=68.0,
            bill_type="收入",
            date="2026-07-28 11:03:00",
            description="pytest feedback transfer conflict candidate",
        )
        candidate_id = f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}"

        pair_response = client.post(
            "/api/matching/manual-pair",
            json={"billId": anchor_bill_id, "candidateBillId": candidate_bill_id},
            headers=auth_headers,
        )
        assert pair_response.status_code == 200

        accept_response = client.post(
            f"/api/matching/candidates/{candidate_id}/accept",
            json={},
            headers=auth_headers,
        )

        assert accept_response.status_code == 409
        assert _list_bill_pair_feedback_via_db(user_id=current_user_id, candidate_id=candidate_id) == []

    def test_matching_bill_feedback_returns_related_events_for_current_user(self, client):
        """bill-scoped feedback 读侧应返回当前用户与该 bill 相关的 formal-bill matching 事件。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_feedback_current_user")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest feedback read 源账户")
        transfer_target_account_id = _create_account_via_db(current_user_id, "pytest feedback read 转账目标账户")
        investment_target_account_id = _create_account_via_db(current_user_id, "pytest feedback read 投资目标账户")
        unrelated_source_account_id = _create_account_via_db(current_user_id, "pytest feedback read 无关源账户")
        unrelated_target_account_id = _create_account_via_db(current_user_id, "pytest feedback read 无关目标账户")

        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="投资",
            date="2026-07-30 09:00:00",
            description="蚂蚁财富 feedback read anchor 买入",
        )
        transfer_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=transfer_target_account_id,
            amount=58.0,
            bill_type="收入",
            date="2026-07-30 09:03:00",
            description="pytest feedback read transfer candidate",
        )
        investment_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=investment_target_account_id,
            amount=58.0,
            bill_type="投资",
            date="2026-07-30 09:05:00",
            description="蚂蚁财富 feedback read investment 卖出",
        )
        unrelated_anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=unrelated_source_account_id,
            amount=-41.0,
            bill_type="支出",
            date="2026-07-30 10:00:00",
            description="pytest feedback read unrelated anchor",
        )
        unrelated_candidate_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=unrelated_target_account_id,
            amount=41.0,
            bill_type="收入",
            date="2026-07-30 10:02:00",
            description="pytest feedback read unrelated candidate",
        )

        transfer_candidate_id = f"bill:{anchor_bill_id}:transfer:{transfer_candidate_bill_id}"
        investment_candidate_id = f"bill:{anchor_bill_id}:investment:{investment_candidate_bill_id}"
        unrelated_candidate_id = f"bill:{unrelated_anchor_bill_id}:transfer:{unrelated_candidate_bill_id}"

        transfer_reject_response = client.post(
            f"/api/matching/candidates/{transfer_candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert transfer_reject_response.status_code == 200

        investment_reject_response = client.post(
            f"/api/matching/candidates/{investment_candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert investment_reject_response.status_code == 200

        unrelated_accept_response = client.post(
            f"/api/matching/candidates/{unrelated_candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert unrelated_accept_response.status_code == 200

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/feedback", headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["billId"] == anchor_bill_id
        assert [event["candidateId"] for event in data["data"]["events"]] == [
            investment_candidate_id,
            transfer_candidate_id,
        ]
        assert [event["action"] for event in data["data"]["events"]] == ["reject", "reject"]
        assert data["data"]["events"][0]["payload"] == {
            "scope": "bill",
            "kind": "investment",
            "bill_id": anchor_bill_id,
            "candidate_bill_id": investment_candidate_bill_id,
        }
        assert data["data"]["events"][1]["payload"] == {
            "scope": "bill",
            "kind": "transfer",
            "bill_id": anchor_bill_id,
            "candidate_bill_id": transfer_candidate_bill_id,
        }

    def test_matching_bill_feedback_returns_empty_list_for_existing_bill_without_events(self, client):
        """bill 存在但没有 feedback 事件时，应返回 200 + 空数组。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_bill_feedback_empty")
        current_user_id = _get_current_user_id(client, auth_headers)

        source_account_id = _create_account_via_db(current_user_id, "pytest feedback empty 源账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-35.0,
            bill_type="支出",
            date="2026-07-30 11:00:00",
            description="pytest feedback empty anchor",
        )

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/feedback", headers=auth_headers)

        assert response.status_code == 200
        assert response.get_json() == {
            "success": True,
            "data": {
                "billId": anchor_bill_id,
                "events": [],
            },
        }

    def test_matching_bill_feedback_is_user_scoped(self, client):
        """bill-scoped feedback 读侧不应跨用户泄露 bill 或事件。"""
        primary_headers = _build_isolated_auth_headers(client, "test_matching_bill_feedback_scope_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_matching_bill_feedback_scope_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest feedback scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest feedback scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-47.0,
            bill_type="支出",
            date="2026-07-30 12:00:00",
            description="pytest feedback scope anchor",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=47.0,
            bill_type="收入",
            date="2026-07-30 12:02:00",
            description="pytest feedback scope candidate",
        )

        reject_response = client.post(
            f"/api/matching/candidates/bill:{anchor_bill_id}:transfer:{candidate_bill_id}/reject",
            json={},
            headers=primary_headers,
        )
        assert reject_response.status_code == 200

        response = client.get(f"/api/matching/bills/{anchor_bill_id}/feedback", headers=secondary_headers)

        assert response.status_code == 404
        assert response.get_json()["success"] is False
        assert response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_reject_is_user_scoped_for_historical_learning(self, client):
        """generic reject 的 historical learning 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_bill_learning_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_bill_learning_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)

        rule_id = _create_composite_learning_rule_via_db(
            primary_user_id,
            parser_id="wechat",
            counterparty="pytest bill learning scope vendor",
            description="pytest bill learning scope note",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(primary_user_id, "pytest bill learning scope 账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-54.0,
            bill_type="支出",
            date="2026-07-24 15:00:00",
            description="pytest bill learning scope note",
        )

        candidate_id = next(
            candidate["candidateId"]
            for candidate in client.get(
                f"/api/matching/bills/{anchor_bill_id}/candidates",
                headers=primary_headers,
            ).get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )

        reject_response = client.post(f"/api/matching/candidates/{candidate_id}/reject", json={}, headers=secondary_headers)
        assert reject_response.status_code == 404
        assert reject_response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_reject_is_user_scoped_for_historical_investment(self, client):
        """generic reject 的 historical investment 分支不应跨用户生效。"""
        primary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_bill_investment_scope_primary"
        )
        secondary_headers = _build_isolated_auth_headers(
            client, "test_matching_candidate_reject_bill_investment_scope_secondary"
        )
        primary_user_id = _get_current_user_id(client, primary_headers)

        source_account_id = _create_account_via_db(primary_user_id, "pytest bill investment scope 源账户")
        target_account_id = _create_account_via_db(primary_user_id, "pytest bill investment scope 目标账户")
        anchor_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=source_account_id,
            amount=-68.0,
            bill_type="投资",
            date="2026-07-23 11:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
        )
        candidate_bill_id = _create_bill_via_db(
            primary_user_id,
            source_account_id=target_account_id,
            amount=68.0,
            bill_type="投资",
            date="2026-07-23 11:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
        )

        reject_response = client.post(
            f"/api/matching/candidates/bill:{anchor_bill_id}:investment:{candidate_bill_id}/reject",
            json={},
            headers=secondary_headers,
        )
        assert reject_response.status_code == 404
        assert reject_response.get_json()["error"] == "Bill not found"

    def test_matching_candidate_accept_rejects_stale_historical_learning_candidate_revision(self, client):
        """当同一 rule 原地更新后，旧 revision 的 historical learning candidate 不应再被 accept。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_accept_bill_learning_stale")
        current_user_id = _get_current_user_id(client, auth_headers)

        from src.api.app import db

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest learning stale vendor",
            description="pytest learning stale note",
            payment_method="银行卡",
            learned_type="收入",
        )
        source_account_id = _create_account_via_db(current_user_id, "pytest learning stale 账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-38.0,
            bill_type="支出",
            date="2026-07-26 12:00:00",
            description="pytest learning stale note",
        )

        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_candidate_id = next(
            candidate["candidateId"]
            for candidate in initial_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )

        async def _bump_rule_revision() -> None:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                "UPDATE import_learning_rules SET learned_type = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                ("支出", "2026-07-26T12:05:00", rule_id, current_user_id),
            )
            await conn.commit()

        asyncio.run(_bump_rule_revision())

        refreshed_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert refreshed_response.status_code == 200
        refreshed_candidate_id = next(
            candidate["candidateId"]
            for candidate in refreshed_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )
        assert refreshed_candidate_id != initial_candidate_id

        stale_accept_response = client.post(
            f"/api/matching/candidates/{initial_candidate_id}/accept",
            json={},
            headers=auth_headers,
        )
        assert stale_accept_response.status_code == 400
        assert stale_accept_response.get_json()["error"] == "Learning candidate not available"

    def test_matching_candidate_reject_does_not_hide_new_historical_learning_rule_revision(self, client):
        """旧 revision 的 resolved suppression 不应继续隐藏同一 rule_id 的新 revision 候选。"""
        auth_headers = _build_isolated_auth_headers(client, "test_matching_candidate_reject_bill_learning_revision")
        current_user_id = _get_current_user_id(client, auth_headers)

        from src.api.app import db

        rule_id = _create_composite_learning_rule_via_db(
            current_user_id,
            parser_id="wechat",
            counterparty="pytest learning revision vendor",
            description="pytest learning revision note",
            payment_method="银行卡",
            learned_type="支出",
        )
        source_account_id = _create_account_via_db(current_user_id, "pytest learning revision 账户")
        anchor_bill_id = _create_bill_via_db(
            current_user_id,
            source_account_id=source_account_id,
            amount=-41.0,
            bill_type="支出",
            date="2026-07-26 13:00:00",
            description="pytest learning revision note",
        )

        initial_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert initial_response.status_code == 200
        initial_candidate_id = next(
            candidate["candidateId"]
            for candidate in initial_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning" and candidate["ruleId"] == rule_id
        )

        reject_response = client.post(
            f"/api/matching/candidates/{initial_candidate_id}/reject",
            json={},
            headers=auth_headers,
        )
        assert reject_response.status_code == 200

        hidden_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert hidden_response.status_code == 200
        hidden_learning_candidates = [
            candidate
            for candidate in hidden_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert hidden_learning_candidates == []

        async def _bump_rule_revision() -> None:
            conn = await db._get_connection()  # pylint: disable=protected-access
            await conn.execute(
                "UPDATE import_learning_rules SET learned_type = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                ("收入", "2026-07-26T13:05:00", rule_id, current_user_id),
            )
            await conn.commit()

        asyncio.run(_bump_rule_revision())

        refreshed_response = client.get(f"/api/matching/bills/{anchor_bill_id}/candidates", headers=auth_headers)
        assert refreshed_response.status_code == 200
        refreshed_learning_candidates = [
            candidate
            for candidate in refreshed_response.get_json()["data"]["candidates"]
            if candidate["kind"] == "learning"
        ]
        assert len(refreshed_learning_candidates) == 1
        assert refreshed_learning_candidates[0]["candidateId"] != initial_candidate_id
