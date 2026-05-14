from __future__ import annotations

import asyncio
import time

import pytest

from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _ensure_test_account,
    _ensure_test_expense_category,
    _get_current_user_id,
)

pytestmark = pytest.mark.skip(
    reason=(
        "Import-session learning suggestion routes moved from the deleted Flask bills "
        "route package to Rust import runtime contracts."
    )
)


def _create_composite_import_learning_rule(
    *,
    user_id: int,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    learned_type: str,
) -> int:
    from src.api.app import db

    async def _create() -> int:
        conn = await db._get_connection()  # pylint: disable=protected-access
        now = "2026-08-01T10:00:00"
        composite_hash = db.build_composite_match_hash(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        match_features = db.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        assert composite_hash is not None
        assert match_features is not None
        cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, enabled, source_session_id, source_preview_id,
                parser_id, composite_match_hash, match_features_json,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                "composite",
                composite_hash,
                composite_hash,
                learned_type,
                "pytest-session",
                None,
                parser_id,
                composite_hash,
                __import__("json").dumps(match_features, ensure_ascii=False, sort_keys=True),
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    return asyncio.run(_create())


class TestBillsLearningSuggestionsAPI:
    """session 级 learning suggestions 只读接口回归。"""

    def test_import_learning_suggestions_returns_grouped_session_suggestions(self, client):
        auth_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-learning-suggestions-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _prepare() -> list[int]:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            first_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-02 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            second_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-02 18:30:00",
                    "preview_type": "支出",
                    "preview_amount": 29.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            await db.save_import_annotation_samples(
                session_id,
                [
                    {
                        "id": first_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                    {
                        "id": second_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                ],
                user_id=current_user_id,
            )
            return [int(first_preview_id), int(second_preview_id)]

        preview_ids = asyncio.run(_prepare())

        response = client.get(
            f"/api/bills/import/v2/learning/{session_id}/suggestions",
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["sessionId"] == session_id
        assert data["data"]["totalCount"] == 1
        suggestion = data["data"]["suggestions"][0]
        assert suggestion["matchType"] == "composite"
        assert suggestion["sampleCount"] == 2
        assert suggestion["sourcePreviewIds"] == preview_ids
        assert suggestion["matchFeatures"] == {
            "parser_id": "alipay",
            "counterparty": "星巴克咖啡",
            "description": "门店消费",
            "payment_method": "支付宝",
        }
        assert suggestion["learnedType"] == "支出"
        assert suggestion["learnedCategoryId"] == int(category["id"])
        assert suggestion["learnedCategoryName"]
        assert suggestion["learnedSourceAccountId"] == int(source_account["id"])
        assert suggestion["learnedSourceAccountName"] == source_account["name"]
        assert suggestion["learnedDestinationAccountId"] in (None, "", 0)
        assert suggestion["summary"]

    def test_import_learning_suggestions_post_stages_preview_updates_and_filters_selection(self, client):
        auth_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions_post")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-learning-suggestions-post-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _prepare() -> tuple[int, int, int]:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            first_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-05 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            second_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-05 18:30:00",
                    "preview_type": "支出",
                    "preview_amount": 29.0,
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
                    "preview_date": "2026-08-05 20:00:00",
                    "preview_type": "支出",
                    "preview_amount": 16.5,
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
                        "id": skipped_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    }
                ],
                user_id=current_user_id,
            )
            return int(first_preview_id), int(second_preview_id), int(skipped_preview_id)

        first_preview_id, second_preview_id, skipped_preview_id = asyncio.run(_prepare())

        response = client.post(
            f"/api/bills/import/v2/learning/{session_id}/suggestions",
            headers=auth_headers,
            json={
                "preview_updates": [
                    {
                        "id": first_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                    {
                        "id": second_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                ]
            },
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["sessionId"] == session_id
        assert data["data"]["totalCount"] == 1
        suggestion = data["data"]["suggestions"][0]
        assert suggestion["sourcePreviewIds"] == [first_preview_id, second_preview_id]
        assert skipped_preview_id not in suggestion["sourcePreviewIds"]

    def test_import_learning_suggestions_post_crops_same_group_sources_to_selected_subset(self, client):
        auth_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions_crop_sources")
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-learning-suggestions-crop-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _prepare() -> tuple[int, int, int]:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            first_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-06 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 21.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            second_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-06 12:00:00",
                    "preview_type": "支出",
                    "preview_amount": 25.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            third_preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-06 18:00:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            await db.save_import_annotation_samples(
                session_id,
                [
                    {
                        "id": third_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    }
                ],
                user_id=current_user_id,
            )
            return int(first_preview_id), int(second_preview_id), int(third_preview_id)

        first_preview_id, second_preview_id, third_preview_id = asyncio.run(_prepare())

        response = client.post(
            f"/api/bills/import/v2/learning/{session_id}/suggestions",
            headers=auth_headers,
            json={
                "preview_updates": [
                    {
                        "id": first_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                    {
                        "id": second_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                        "preview_source_account_id": int(source_account["id"]),
                        "preview_destination_account_id": None,
                    },
                ]
            },
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"]["totalCount"] == 1
        suggestion = data["data"]["suggestions"][0]
        assert suggestion["sampleCount"] == 2
        assert suggestion["sourcePreviewIds"] == [first_preview_id, second_preview_id]
        assert third_preview_id not in suggestion["sourcePreviewIds"]

    def test_import_learning_suggestions_is_user_scoped_and_404_for_missing_session(self, client):
        primary_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        session_id = f"pytest-learning-suggestions-scope-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _create_session() -> None:
            await db.create_import_session(session_id, user_id=primary_user_id, file_count=1)

        asyncio.run(_create_session())

        missing_response = client.get(
            "/api/bills/import/v2/learning/does-not-exist/suggestions",
            headers=primary_headers,
        )
        assert missing_response.status_code == 404
        assert missing_response.get_json()["error"] == "Import session not found"

        scoped_response = client.get(
            f"/api/bills/import/v2/learning/{session_id}/suggestions",
            headers=secondary_headers,
        )
        assert scoped_response.status_code == 404
        assert scoped_response.get_json()["error"] == "Import session not found"

    def test_import_learning_suggestions_filters_existing_composite_rule(self, client):
        auth_headers = _build_isolated_auth_headers(client, "test_bills_learning_suggestions_existing_rule")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-learning-suggestions-existing-rule-{int(time.time() * 1000)}"

        from src.api.app import db

        async def _prepare() -> None:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-02 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            await db.save_import_annotation_samples(
                session_id,
                [{"id": preview_id, "preview_type": "支出"}],
                user_id=current_user_id,
            )

        asyncio.run(_prepare())
        _create_composite_import_learning_rule(
            user_id=current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="支出",
        )

        response = client.get(
            f"/api/bills/import/v2/learning/{session_id}/suggestions",
            headers=auth_headers,
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["data"] == {
            "sessionId": session_id,
            "totalCount": 0,
            "suggestions": [],
        }
