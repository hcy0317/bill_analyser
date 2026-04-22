"""Regression tests for import-session scoped LLM analysis."""

from __future__ import annotations

import asyncio
import json
import time
from typing import Any

from bill_analyser.core.llm_provider import LLMResponse
from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _ensure_test_expense_category,
    _get_current_user_id,
)


class _FakeLLMProvider:
    async def generate(self, **kwargs) -> LLMResponse:  # pragma: no cover - trivial async stub
        return LLMResponse(
            content=json.dumps(
                [
                    {
                        "rule_name": "咖啡消费",
                        "rule_expression": "OR={星巴克咖啡}",
                        "confidence": 0.91,
                        "explanation": "样本交易都来自同一商户",
                    }
                ],
                ensure_ascii=False,
            ),
            model="fake-model",
            provider="fake-provider",
            tokens_used=42,
            raw_response={"stub": True},
        )


def _list_llm_candidates_for_user(*, user_id: int) -> list[dict]:
    from bill_analyser.api.app import db

    async def _list() -> list[dict]:
        return await db.get_llm_candidates(user_id=user_id, limit=100)

    return asyncio.run(_list())


def _reset_llm_rate_limit_state() -> None:
    from bill_analyser.core import llm_learning_service

    llm_learning_service._RATE_LIMIT_BUCKETS.clear()  # pylint: disable=protected-access


class TestLLMImportSessionAnalysisAPI:
    """LLM analyze-transactions should support import-session preview inputs."""

    def test_llm_analysis_uses_import_session_preview_updates(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_analysis")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-import-session-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare() -> tuple[int, int]:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            first_preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            second_preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 18:00:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-09 08:00:00",
                    "preview_type": "支出",
                    "preview_amount": 16.0,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "午餐",
                    "preview_counterparty": "公司食堂",
                    "preview_payment_method": "微信",
                    "preview_description": "午餐消费",
                    "preview_parser_id": "wechat",
                },
                user_id=current_user_id,
            )
            return int(first_preview_id), int(second_preview_id)

        first_preview_id, second_preview_id = asyncio.run(_prepare())

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": session_id,
                "preview_updates": [
                    {
                        "id": first_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    },
                    {
                        "id": second_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    },
                ],
            },
        )

        assert response.status_code == 200
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["mode"] == "import_session"
        assert payload["data"]["session_id"] == session_id
        assert payload["data"]["candidates_created"] == 1
        assert len(payload["data"]["candidates"]) == 1
        candidate = payload["data"]["candidates"][0]
        assert candidate["source_preview_ids"] == [first_preview_id, second_preview_id]
        assert candidate["rule_expression"] == "OR={星巴克咖啡}"

        llm_candidates = _list_llm_candidates_for_user(user_id=current_user_id)
        session_candidates = [
            item
            for item in llm_candidates
            if item.get("suggested_rule_expression") == "OR={星巴克咖啡}"
        ]
        assert len(session_candidates) == 1
        assert json.loads(session_candidates[0]["source_bill_ids"]) == [first_preview_id, second_preview_id]

    def test_llm_analysis_returns_404_for_missing_import_session(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_missing")

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": "does-not-exist",
                "preview_updates": [],
            },
        )

        assert response.status_code == 404
        assert response.get_json()["error"] == "Import session not found"

    def test_llm_analysis_rejects_too_many_preview_updates(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_too_many_rows")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-llm-import-too-many-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare() -> None:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)

        asyncio.run(_prepare())

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": session_id,
                "preview_updates": [{"id": index + 1} for index in range(21)],
            },
        )

        assert response.status_code == 400
        assert "Too many preview updates submitted" in response.get_json()["error"]

    def test_llm_analysis_rejects_too_many_preview_update_items_even_for_same_id(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(
            client,
            "test_llm_import_session_too_many_update_items",
        )
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-llm-import-too-many-items-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare() -> int:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare())

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": session_id,
                "preview_updates": [{"id": preview_id} for _ in range(21)],
            },
        )

        assert response.status_code == 400
        assert "Too many preview updates submitted" in response.get_json()["error"]

    def test_llm_analysis_persists_preview_updates_but_filters_by_explicit_preview_ids(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(
            client,
            "test_llm_import_session_preview_id_precedence",
        )
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-import-preview-ids-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare() -> tuple[int, int]:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            first_preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            second_preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 18:00:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(first_preview_id), int(second_preview_id)

        first_preview_id, second_preview_id = asyncio.run(_prepare())

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": session_id,
                "preview_ids": [first_preview_id],
                "preview_updates": [
                    {
                        "id": first_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    },
                    {
                        "id": second_preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    },
                ],
            },
        )

        assert response.status_code == 200
        payload = response.get_json()
        assert payload["data"]["candidates_created"] == 1
        assert payload["data"]["candidates"][0]["source_preview_ids"] == [first_preview_id]

        from bill_analyser.api.app import db

        async def _fetch_previews() -> tuple[dict[str, Any], dict[str, Any]]:
            first = await db.get_preview_bill_by_id(first_preview_id, user_id=current_user_id)
            second = await db.get_preview_bill_by_id(second_preview_id, user_id=current_user_id)
            return first, second

        first_preview, second_preview = asyncio.run(_fetch_previews())
        assert first_preview["preview_main_category"]
        assert second_preview["preview_main_category"] == first_preview["preview_main_category"]

    def test_llm_analysis_rate_limit_is_shared_across_service_instances(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_rate_limit")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-import-rate-limit-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FakeLLMProvider())
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare() -> int:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "星巴克咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare())

        def _post_once() -> Any:
            return client.post(
                "/api/llm/analyze-transactions",
                headers=auth_headers,
                json={
                    "session_id": session_id,
                    "preview_updates": [
                        {
                            "id": preview_id,
                            "preview_type": "支出",
                            "category_id": int(category["id"]),
                        }
                    ],
                },
            )

        for _ in range(10):
            response = _post_once()
            assert response.status_code == 200

        blocked_response = _post_once()
        assert blocked_response.status_code == 429
        assert "Rate limit exceeded" in blocked_response.get_json()["error"]

    def test_llm_candidate_routes_are_user_scoped(self, client):
        _reset_llm_rate_limit_state()
        owner_headers = _build_isolated_auth_headers(client, "test_llm_candidate_owner")
        intruder_headers = _build_isolated_auth_headers(client, "test_llm_candidate_intruder")
        owner_user_id = _get_current_user_id(client, owner_headers)

        from bill_analyser.api import app as api_app

        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare_candidate() -> int:
            return await api_app.db.create_llm_candidate(
                user_id=owner_user_id,
                type="classification",
                source_bill_ids=[101],
                suggested_main_category="餐饮",
                suggested_sub_category="咖啡",
                suggested_rule_expression=None,
                confidence=0.91,
                llm_provider="fake-provider",
                llm_model="fake-model",
                llm_response_raw=json.dumps({"stub": True}, ensure_ascii=False),
            )

        candidate_id = asyncio.run(_prepare_candidate())

        intruder_get = client.get(f"/api/llm/candidates/{candidate_id}", headers=intruder_headers)
        assert intruder_get.status_code == 404
        assert intruder_get.get_json()["error"] == f"Candidate {candidate_id} not found"

        intruder_accept = client.post(
            f"/api/llm/candidates/{candidate_id}/accept",
            headers=intruder_headers,
        )
        assert intruder_accept.status_code == 404
        assert intruder_accept.get_json()["error"] == f"Candidate {candidate_id} not found"

        intruder_reject = client.post(
            f"/api/llm/candidates/{candidate_id}/reject",
            headers=intruder_headers,
        )
        assert intruder_reject.status_code == 404
        assert intruder_reject.get_json()["error"] == f"Candidate {candidate_id} not found"

        owner_get = client.get(f"/api/llm/candidates/{candidate_id}", headers=owner_headers)
        assert owner_get.status_code == 200
        assert owner_get.get_json()["data"]["id"] == candidate_id
