"""Regression tests for the import-learning promote REST API."""

from __future__ import annotations

import asyncio
import time
from typing import Any

from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _ensure_test_account,
    _ensure_test_expense_category,
    _get_current_user_id,
)


def _list_learning_rules_for_user(*, user_id: int) -> list[dict[str, Any]]:
    from bill_analyser.api.app import db

    async def _list() -> list[dict[str, Any]]:
        return await db.get_import_learning_rules(
            user_id=user_id,
            enabled_only=False,
            limit=100,
        )

    return asyncio.run(_list())


class TestBillsImportLearningPromoteAPI:
    """长期学习提升接口回归。"""

    def test_import_learning_promote_supports_explicit_preview_ids_selection(self, client):
        auth_headers = _build_isolated_auth_headers(
            client,
            "test_bills_learning_promote_preview_ids",
        )
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-learning-promote-preview-ids-{int(time.time() * 1000)}"

        from bill_analyser.api.app import db

        async def _prepare() -> tuple[int, int]:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            selected_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-03 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            skipped_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-03 18:30:00",
                    "preview_type": "支出",
                    "preview_amount": 24.8,
                    "preview_counterparty": "盒马鲜生",
                    "preview_payment_method": "微信支付",
                    "preview_description": "水果消费",
                    "preview_parser_id": "wechat",
                },
                user_id=current_user_id,
            )
            await db.save_import_annotation_samples(
                session_id,
                [
                    {
                        "id": selected_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                    {
                        "id": skipped_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                ],
                user_id=current_user_id,
            )
            return int(selected_preview_id), int(skipped_preview_id)

        selected_preview_id, skipped_preview_id = asyncio.run(_prepare())

        response = client.post(
            f"/api/bills/import/v2/learning/{session_id}/promote",
            headers=auth_headers,
            json={"previewIds": [selected_preview_id]},
        )

        assert response.status_code == 200
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["success"] is True
        assert payload["data"]["session_id"] == session_id
        assert payload["data"]["selected_samples"] == 1
        assert payload["data"]["rules_total"] == 1
        assert payload["data"]["created"] == 1
        assert payload["data"]["updated"] == 0

        rules = _list_learning_rules_for_user(user_id=current_user_id)
        session_rules = [rule for rule in rules if rule.get("source_session_id") == session_id]

        assert len(session_rules) == 1
        assert int(session_rules[0]["source_preview_id"] or 0) == selected_preview_id
        assert int(session_rules[0]["source_preview_id"] or 0) != skipped_preview_id

    def test_import_learning_promote_treats_empty_preview_ids_as_zero_selection(self, client):
        auth_headers = _build_isolated_auth_headers(
            client,
            "test_bills_learning_promote_empty_preview_ids",
        )
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-learning-promote-empty-preview-ids-{int(time.time() * 1000)}"

        from bill_analyser.api.app import db

        async def _prepare() -> None:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-04 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 18.5,
                    "preview_counterparty": "全家便利店",
                    "preview_payment_method": "支付宝",
                    "preview_description": "早餐面包",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            await db.save_import_annotation_samples(
                session_id,
                [
                    {
                        "id": preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    }
                ],
                user_id=current_user_id,
            )

        asyncio.run(_prepare())

        response = client.post(
            f"/api/bills/import/v2/learning/{session_id}/promote",
            headers=auth_headers,
            json={"previewIds": []},
        )

        assert response.status_code == 200
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["success"] is True
        assert payload["data"]["selected_samples"] == 0
        assert payload["data"]["rules_total"] == 0
        assert payload["data"]["created"] == 0
        assert payload["data"]["updated"] == 0

        rules = _list_learning_rules_for_user(user_id=current_user_id)
        session_rules = [rule for rule in rules if rule.get("source_session_id") == session_id]
        assert session_rules == []

    def test_import_learning_promote_is_user_scoped_and_404_for_missing_session(self, client):
        primary_headers = _build_isolated_auth_headers(
            client,
            "test_bills_learning_promote_primary",
        )
        secondary_headers = _build_isolated_auth_headers(
            client,
            "test_bills_learning_promote_secondary",
        )
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-learning-promote-scope-{int(time.time() * 1000)}"

        from bill_analyser.api.app import db

        async def _create_session() -> None:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)

        asyncio.run(_create_session())

        missing_response = client.post(
            "/api/bills/import/v2/learning/does-not-exist/promote",
            headers=primary_headers,
            json={},
        )
        assert missing_response.status_code == 404
        assert missing_response.get_json()["error"] == "Import session not found"

        scoped_response = client.post(
            f"/api/bills/import/v2/learning/{session_id}/promote",
            headers=secondary_headers,
            json={},
        )
        assert scoped_response.status_code == 404
        assert scoped_response.get_json()["error"] == "Import session not found"
