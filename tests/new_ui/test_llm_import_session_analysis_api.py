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


class _CapturingLLMProvider:
    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []

    async def generate(self, **kwargs) -> LLMResponse:  # pragma: no cover - trivial async stub
        self.calls.append(kwargs)
        return LLMResponse(
            content=json.dumps(
                [
                    {
                        "rule_name": "高级配置咖啡",
                        "rule_expression": "OR={高级配置咖啡}",
                        "confidence": 0.93,
                        "explanation": "advanced settings applied",
                    }
                ],
                ensure_ascii=False,
            ),
            model="capture-model",
            provider="capture-provider",
            tokens_used=64,
            raw_response={"stub": True},
        )


class _NoRuleLLMProvider:
    async def generate(self, **kwargs) -> LLMResponse:  # pragma: no cover - trivial async stub
        return LLMResponse(
            content="[]",
            model="fake-model",
            provider="fake-provider",
            tokens_used=12,
            raw_response={"stub": True},
        )


class _FailingLLMProvider:
    async def generate(self, **kwargs) -> LLMResponse:
        raise RuntimeError("provider unavailable")


class _FakePreviewRecommendationProvider:
    def __init__(self, suggestions: list[dict[str, Any]]) -> None:
        self.suggestions = suggestions
        self.calls: list[dict[str, Any]] = []

    async def generate(self, **kwargs) -> LLMResponse:  # pragma: no cover - trivial async stub
        self.calls.append(kwargs)
        return LLMResponse(
            content=json.dumps(self.suggestions, ensure_ascii=False),
            model="preview-model",
            provider="preview-provider",
            tokens_used=36,
            raw_response={"stub": True},
        )


class _RuleSynthesisLLMProvider:
    def __init__(self, candidates: list[dict[str, Any]]) -> None:
        self.candidates = candidates
        self.calls: list[dict[str, Any]] = []

    async def generate(self, **kwargs) -> LLMResponse:  # pragma: no cover - trivial async stub
        self.calls.append(kwargs)
        return LLMResponse(
            content=json.dumps(self.candidates, ensure_ascii=False),
            model="rule-synthesis-model",
            provider="rule-synthesis-provider",
            tokens_used=48,
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


def _list_llm_memory_events_for_user(*, user_id: int, session_id: str | None = None) -> list[dict]:
    from bill_analyser.api.app import db

    async def _list() -> list[dict]:
        return await db.get_llm_memory_events(user_id=user_id, session_id=session_id, limit=100)

    return asyncio.run(_list())


def _set_llm_memory_events_created_at(*, event_ids: list[int], created_at: str) -> None:
    from bill_analyser.api.app import db

    async def _update() -> None:
        conn = await db._get_connection()
        placeholders = ", ".join("?" for _ in event_ids)
        await conn.execute(
            f"UPDATE llm_memory_events SET created_at = ? WHERE id IN ({placeholders})",
            (created_at, *event_ids),
        )
        await conn.commit()

    asyncio.run(_update())


def _create_test_account(client, auth_headers, *, name: str) -> dict[str, Any]:
    response = client.post(
        "/api/accounts/",
        headers=auth_headers,
        json={
            "name": name,
            "category": 1,
            "type": 1,
            "icon": "1",
            "color": "00ccff",
            "currency": "CNY",
            "balance": 0,
            "comment": "pytest llm preview recommendation account",
            "hidden": False,
            "aliases": [],
        },
    )
    assert response.status_code == 201, response.get_data(as_text=True)
    return response.get_json()["result"]


def _get_category_path_by_id(*, user_id: int, category_id: int) -> tuple[str, str]:
    from bill_analyser.api.app import db

    async def _fetch() -> tuple[str, str]:
        category = await db.get_category_by_id(category_id, user_id=user_id)
        assert category is not None
        return (
            str(category.get("main_category") or ""),
            str(category.get("sub_category") or ""),
        )

    return asyncio.run(_fetch())


def _prepare_llm_preview_fixture(
    client,
    auth_headers,
    *,
    prefix: str,
    preview_patch: dict[str, Any] | None = None,
) -> dict[str, Any]:
    current_user_id = _get_current_user_id(client, auth_headers)
    category = _ensure_test_expense_category(client, auth_headers)
    main_category, sub_category = _get_category_path_by_id(
        user_id=current_user_id,
        category_id=int(category["id"]),
    )
    source_account = _create_test_account(
        client,
        auth_headers,
        name=f"{prefix}-source-{int(time.time() * 1000)}",
    )
    destination_account = _create_test_account(
        client,
        auth_headers,
        name=f"{prefix}-destination-{int(time.time() * 1000)}",
    )
    session_id = f"{prefix}-session-{int(time.time() * 1000)}"

    from bill_analyser.api import app as api_app

    async def _prepare() -> int:
        await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
        preview_payload = {
            "preview_date": "2026-08-08 09:00:00",
            "preview_type": "支出",
            "preview_amount": 32.5,
            "preview_counterparty": "测试咖啡店",
            "preview_payment_method": "支付宝",
            "preview_description": "早餐咖啡",
            "preview_parser_id": "alipay",
        }
        if preview_patch:
            preview_payload.update(preview_patch)

        preview_id = await api_app.db.insert_preview_bill(
            session_id,
            preview_payload,
            user_id=current_user_id,
        )
        return int(preview_id)

    preview_id = asyncio.run(_prepare())
    return {
        "current_user_id": current_user_id,
        "session_id": session_id,
        "preview_id": preview_id,
        "category": category,
        "main_category": main_category,
        "sub_category": sub_category,
        "source_account": source_account,
        "destination_account": destination_account,
    }


def _seed_rule_synthesis_knowledge_pack(
    *,
    user_id: int,
    category_id: int,
    category_path: tuple[str, str],
) -> None:
    from bill_analyser.api.app import db
    from bill_analyser.core.import_learning.model import MODEL_KEY

    async def _seed() -> None:
        conn = await db._get_connection()
        snapshot_cursor = await conn.execute(
            """
            INSERT INTO import_learning_dataset_snapshots (
                user_id, name, corpus_sample_count, filters_json,
                status, created_at, updated_at
            ) VALUES (?, 'pytest-rule-synthesis', 11, '{}', 'ready', datetime('now', 'localtime'), datetime('now', 'localtime'))
            """,
            (user_id,),
        )
        dataset_snapshot_id = int(snapshot_cursor.lastrowid or 0)
        rule_cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id,
                learned_source_account_id, learned_destination_account_id,
                enabled, parser_id, composite_match_hash, match_features_json,
                applied_count, created_at, updated_at
            ) VALUES (?, 'composite', ?, ?, '支出', ?, NULL, NULL, 1, ?, ?, ?, ?, datetime('now', 'localtime'), datetime('now', 'localtime'))
            """,
            (
                user_id,
                "c=星巴克咖啡|d=早餐咖啡|m=支付宝",
                "c=星巴克咖啡|d=早餐咖啡|m=支付宝",
                category_id,
                "alipay",
                "c=星巴克咖啡|d=早餐咖啡|m=支付宝",
                json.dumps(
                    {
                        "counterparty": "星巴克咖啡",
                        "description": "早餐咖啡",
                        "payment_method": "支付宝",
                    },
                    ensure_ascii=False,
                ),
                6,
            ),
        )
        rule_id = int(rule_cursor.lastrowid or 0)
        suggestion_cursor = await conn.execute(
            """
            INSERT INTO import_learning_suggestions (
                user_id, match_type, match_value, normalized_match_value,
                composite_match_hash, match_features_json,
                suggested_type, suggested_category_id,
                suggested_source_account_id, suggested_destination_account_id,
                sample_count, source_session_ids_json, source_preview_ids_json,
                status, summary, created_at, updated_at
            ) VALUES (?, 'composite', ?, ?, ?, ?, '支出', ?, NULL, NULL, 5, '[]', '[]', 'pending', ?, datetime('now', 'localtime'), datetime('now', 'localtime'))
            """,
            (
                user_id,
                "c=瑞幸咖啡|d=门店咖啡|m=微信",
                "c=瑞幸咖啡|d=门店咖啡|m=微信",
                "c=瑞幸咖啡|d=门店咖啡|m=微信",
                json.dumps(
                    {
                        "counterparty": "瑞幸咖啡",
                        "description": "门店咖啡",
                        "payment_method": "微信",
                    },
                    ensure_ascii=False,
                ),
                category_id,
                f"{category_path[0]}/{category_path[1]}",
            ),
        )
        suggestion_id = int(suggestion_cursor.lastrowid or 0)
        await conn.execute(
            """
            INSERT INTO import_learning_concept_stats (
                user_id, concept_key, concept_type,
                accepted_count, rejected_count, auto_applied_count, rollback_count, updated_at
            ) VALUES (?, ?, 'rule', 4, 0, 2, 0, datetime('now', 'localtime'))
            """,
            (user_id, f"rule:{rule_id}"),
        )
        await conn.execute(
            """
            INSERT INTO import_learning_concept_stats (
                user_id, concept_key, concept_type,
                accepted_count, rejected_count, auto_applied_count, rollback_count, updated_at
            ) VALUES (?, ?, 'suggestion', 3, 0, 1, 0, datetime('now', 'localtime'))
            """,
            (user_id, f"suggestion:{suggestion_id}"),
        )
        await conn.execute(
            """
            INSERT INTO import_learning_model_registry (
                user_id, model_key, model_version, dataset_snapshot_id,
                status, metrics_json, created_at, updated_at
            ) VALUES (?, ?, 'v321', ?, 'active', ?, datetime('now', 'localtime'), datetime('now', 'localtime'))
            """,
            (
                user_id,
                MODEL_KEY,
                dataset_snapshot_id,
                json.dumps(
                    {
                        "feature_schema_version": "v-test",
                        "policy_version": "policy-test",
                        "sample_count": 11,
                    },
                    ensure_ascii=False,
                ),
            ),
        )
        await conn.execute(
            """
            INSERT INTO llm_memory_events (
                user_id, event_type, decision,
                suggested_main_category, suggested_sub_category,
                metadata, created_at
            ) VALUES (?, 'feedback', 'accept', ?, ?, ?, datetime('now', 'localtime'))
            """,
            (
                user_id,
                category_path[0],
                category_path[1],
                "咖啡门店交易通常接受为餐饮/咖啡",
            ),
        )
        await conn.commit()

    asyncio.run(_seed())


def _get_preview_item_from_page(client, auth_headers, *, session_id: str, preview_id: int) -> dict[str, Any]:
    response = client.get(
        f"/api/bills/import/v2/preview/{session_id}?page=1&page_size=20",
        headers=auth_headers,
    )
    assert response.status_code == 200, response.get_data(as_text=True)
    payload = response.get_json()
    assert payload["success"] is True
    preview_rows = payload["data"]["preview"]
    return next(row for row in preview_rows if int(row["id"]) == int(preview_id))


class TestLLMImportSessionAnalysisAPI:
    """LLM analyze-transactions should support import-session preview inputs."""

    def test_llm_advanced_settings_normalizer_ignores_json_scalars(self):
        from bill_analyser.core.db_llm_config import normalize_llm_advanced_settings

        assert normalize_llm_advanced_settings("[]") == {}
        assert normalize_llm_advanced_settings('"plain"') == {}
        assert normalize_llm_advanced_settings("1") == {}

    def test_saved_llm_config_persists_advanced_settings_and_redacts_api_keys(
        self,
        client,
    ):
        auth_headers = _build_isolated_auth_headers(client, "test_llm_config_advanced_redaction")
        current_user_id = _get_current_user_id(client, auth_headers)
        config_name = f"pytest-advanced-redaction-{int(time.time() * 1000)}"
        initial_secret = "sk-initial-secret-should-not-leak"
        updated_secret = "sk-updated-secret-should-not-leak"

        create_response = client.post(
            "/api/llm/configs",
            headers=auth_headers,
            json={
                "name": config_name,
                "provider": "openai",
                "model": "gpt-test",
                "api_key": initial_secret,
                "base_url": "https://example.test/v1",
                "advanced_settings": {
                    "reasoning_depth": "high",
                    "temperature": 0.55,
                    "max_tokens": 1234,
                    "system_prompt": "系统提示词",
                    "classification_prompt_template": "分类 {transactions_json}",
                    "rule_prompt_template": "规则 {category_name} {transactions_json}",
                },
            },
        )

        assert create_response.status_code == 200
        create_payload = create_response.get_json()
        assert initial_secret not in json.dumps(create_payload, ensure_ascii=False)
        created_config = create_payload["data"]
        assert created_config["api_key"] == "********"
        assert created_config["has_api_key"] is True
        assert created_config["advanced_settings"]["reasoning_depth"] == "high"
        assert created_config["advanced_settings"]["temperature"] == 0.55
        assert created_config["advanced_settings"]["max_tokens"] == 1234
        config_id = int(created_config["id"])

        update_response = client.put(
            f"/api/llm/configs/{config_id}",
            headers=auth_headers,
            json={
                "api_key": updated_secret,
                "advanced_settings": {
                    "reasoning_depth": "low",
                    "temperature": 0.7,
                    "max_tokens": 2048,
                    "system_prompt": "更新后的系统提示词",
                    "rule_prompt_template": "更新规则 {category_name}",
                },
            },
        )

        assert update_response.status_code == 200
        update_payload = update_response.get_json()
        update_payload_text = json.dumps(update_payload, ensure_ascii=False)
        assert initial_secret not in update_payload_text
        assert updated_secret not in update_payload_text
        updated_config = update_payload["data"]
        assert updated_config["api_key"] == "********"
        assert updated_config["advanced_settings"]["reasoning_depth"] == "low"
        assert updated_config["advanced_settings"]["temperature"] == 0.7
        assert updated_config["advanced_settings"]["max_tokens"] == 2048

        list_response = client.get("/api/llm/configs", headers=auth_headers)
        assert list_response.status_code == 200
        list_payload_text = json.dumps(list_response.get_json(), ensure_ascii=False)
        assert initial_secret not in list_payload_text
        assert updated_secret not in list_payload_text

        from bill_analyser.api import app as api_app

        async def _fetch_saved_config() -> dict[str, Any]:
            configs = await api_app.db.get_llm_configs(user_id=current_user_id)
            return next(item for item in configs if int(item["id"]) == config_id)

        saved_config = asyncio.run(_fetch_saved_config())
        assert saved_config["api_key"] == updated_secret
        assert saved_config["advanced_settings"]["reasoning_depth"] == "low"
        assert saved_config["advanced_settings"]["system_prompt"] == "更新后的系统提示词"

    def test_active_llm_config_feeds_advanced_settings_without_prompt_secret_leak(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_config_advanced_runtime")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-advanced-runtime-{int(time.time() * 1000)}"
        config_name = f"pytest-advanced-runtime-{int(time.time() * 1000)}"
        secret = "sk-runtime-secret-should-not-leak"
        capturing_provider = _CapturingLLMProvider()

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        create_response = client.post(
            "/api/llm/configs",
            headers=auth_headers,
            json={
                "name": config_name,
                "provider": "openai",
                "model": "gpt-advanced-runtime",
                "api_key": secret,
                "base_url": "https://example.test/v1",
                "is_active": True,
                "advanced_settings": {
                    "reasoning_depth": "high",
                    "temperature": 0.72,
                    "max_tokens": 321,
                    "system_prompt": "自定义系统提示词",
                    "rule_prompt_template": "自定义规则提示 {category_name} {transactions_json}",
                },
            },
        )
        assert create_response.status_code == 200
        assert secret not in json.dumps(create_response.get_json(), ensure_ascii=False)
        config_id = int(create_response.get_json()["data"]["id"])

        activate_response = client.post(
            f"/api/llm/configs/{config_id}/activate",
            headers=auth_headers,
        )
        assert activate_response.status_code == 200
        assert secret not in json.dumps(activate_response.get_json(), ensure_ascii=False)

        monkeypatch.setattr(
            llm_routes.ProviderFactory,
            "create",
            lambda *_args, **_kwargs: capturing_provider,
        )

        async def _prepare() -> int:
            await api_app.db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await api_app.db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-08-08 09:00:00",
                    "preview_type": "支出",
                    "preview_amount": 32.5,
                    "preview_counterparty": "高级配置咖啡",
                    "preview_payment_method": "支付宝",
                    "preview_description": "门店消费",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_prepare())

        analyze_response = client.post(
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

        assert analyze_response.status_code == 200
        analyze_payload_text = json.dumps(analyze_response.get_json(), ensure_ascii=False)
        assert secret not in analyze_payload_text
        assert len(capturing_provider.calls) == 1
        provider_call = capturing_provider.calls[0]
        provider_call_text = json.dumps(provider_call, ensure_ascii=False, default=str)
        assert secret not in provider_call_text
        assert provider_call["system_prompt"] == "自定义系统提示词"
        assert provider_call["temperature"] == 0.72
        assert provider_call["max_tokens"] == 321
        assert provider_call["reasoning_depth"] == "high"
        assert "自定义规则提示" in provider_call["prompt"]
        assert "高级配置咖啡" in provider_call["prompt"]

    def test_active_llm_config_does_not_enable_other_users(
        self,
        client,
        monkeypatch,
    ):
        user_a_headers = _build_isolated_auth_headers(client, "test_llm_config_scope_a")
        user_b_headers = _build_isolated_auth_headers(client, "test_llm_config_scope_b")
        secret_a = "sk-user-a-secret-should-not-cross-users"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        api_app.app.config["LLM_CONFIG"] = {
            "enabled": False,
            "provider": "openai",
            "provider_config": {"model": "disabled-global"},
        }
        api_app.app.config["LLM_CONFIG_BY_USER"] = {}

        create_response = client.post(
            "/api/llm/configs",
            headers=user_a_headers,
            json={
                "name": f"pytest-user-a-active-{int(time.time() * 1000)}",
                "provider": "openai",
                "model": "gpt-user-a",
                "api_key": secret_a,
                "is_active": True,
                "advanced_settings": {"system_prompt": "user-a-only-system-prompt"},
            },
        )
        assert create_response.status_code == 200

        user_b_config_response = client.get("/api/llm/config", headers=user_b_headers)
        assert user_b_config_response.status_code == 200
        user_b_config = user_b_config_response.get_json()["data"]
        assert user_b_config["enabled"] is False
        assert user_b_config["model"] == "disabled-global"

        provider_factory_calls: list[tuple[tuple[Any, ...], dict[str, Any]]] = []

        def _capture_provider_factory(*args: Any, **kwargs: Any) -> _FakeLLMProvider:
            provider_factory_calls.append((args, kwargs))
            return _FakeLLMProvider()

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", _capture_provider_factory)

        user_b_analyze_response = client.post(
            "/api/llm/analyze-transactions",
            headers=user_b_headers,
            json={},
        )
        user_b_analyze_payload = user_b_analyze_response.get_json()
        assert user_b_analyze_response.status_code == 400
        assert user_b_analyze_payload["code"] == "LLM_DISABLED"
        assert provider_factory_calls == []
        assert secret_a not in json.dumps(user_b_analyze_payload, ensure_ascii=False)

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

    def test_llm_analysis_returns_stable_code_when_llm_disabled(self, client):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_disabled")

        from bill_analyser.api import app as api_app

        api_app.app.config["LLM_CONFIG"] = {
            "enabled": False,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        response = client.post(
            "/api/llm/analyze-transactions",
            headers=auth_headers,
            json={
                "session_id": "pytest-disabled-session",
                "preview_updates": [],
            },
        )

        payload = response.get_json()
        assert response.status_code == 400
        assert payload["success"] is False
        assert payload["error"] == "LLM service is not enabled"
        assert payload["code"] == "LLM_DISABLED"

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
        payload = response.get_json()
        assert payload["error"] == "Import session not found"
        assert payload["code"] == "IMPORT_SESSION_NOT_FOUND"

    def test_llm_analysis_rejects_empty_preview_selection(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_empty_selection")
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-llm-import-empty-{int(time.time() * 1000)}"

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
                "preview_updates": [],
            },
        )

        payload = response.get_json()
        assert response.status_code == 400
        assert payload["code"] == "PREVIEW_SELECTION_EMPTY"

    def test_llm_analysis_rejects_insufficient_preview_rows(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(
            client,
            "test_llm_import_session_insufficient_rows",
        )
        current_user_id = _get_current_user_id(client, auth_headers)
        session_id = f"pytest-llm-import-insufficient-{int(time.time() * 1000)}"

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
                "preview_updates": [{"id": preview_id}],
            },
        )

        payload = response.get_json()
        assert response.status_code == 422
        assert payload["code"] == "PREVIEW_SELECTION_INSUFFICIENT"

    def test_llm_analysis_returns_provider_error_code_for_runtime_failure(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_provider_failure")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-import-provider-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _FailingLLMProvider())
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
                "preview_updates": [
                    {
                        "id": preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    }
                ],
            },
        )

        payload = response.get_json()
        assert response.status_code == 503
        assert payload["code"] == "LLM_PROVIDER_UNAVAILABLE"
        assert "LLM request failed" in payload["error"]

    def test_llm_analysis_allows_zero_candidates_when_no_rules_satisfy_conditions(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_import_session_zero_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        session_id = f"pytest-llm-import-zero-{int(time.time() * 1000)}"

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: _NoRuleLLMProvider())
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
                "preview_updates": [
                    {
                        "id": preview_id,
                        "preview_type": "支出",
                        "category_id": int(category["id"]),
                    }
                ],
            },
        )

        payload = response.get_json()
        assert response.status_code == 200
        assert payload["success"] is True
        assert payload["data"]["candidates_created"] == 0
        assert payload["data"]["candidates"] == []

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
        payload = response.get_json()
        assert "Too many preview updates submitted" in payload["error"]
        assert payload["code"] == "PREVIEW_SELECTION_TOO_LARGE"

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
        payload = response.get_json()
        assert "Too many preview updates submitted" in payload["error"]
        assert payload["code"] == "PREVIEW_SELECTION_TOO_LARGE"

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
        blocked_payload = blocked_response.get_json()
        assert "Rate limit exceeded" in blocked_payload["error"]
        assert blocked_payload["code"] == "LLM_RATE_LIMITED"

    def test_llm_preview_recommend_applies_blank_preview_fields_and_writes_memory(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_recommend_apply")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-apply",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        provider = _FakePreviewRecommendationProvider(
            [
                {
                    "preview_id": fixture["preview_id"],
                    "suggested_main_category": fixture["main_category"],
                    "suggested_sub_category": fixture["sub_category"],
                    "suggested_source_account": fixture["source_account"]["name"],
                    "suggested_destination_account": fixture["destination_account"]["name"],
                    "confidence": 0.88,
                    "reason": "历史记忆建议",
                }
            ]
        )
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"], "preview_type": "支出"}],
            },
        )

        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["count"] == 1
        suggestion_result = payload["data"]["suggestions"][0]
        assert "preview_main_category" in suggestion_result["applied_fields"]
        if fixture["sub_category"]:
            assert "preview_sub_category" in suggestion_result["applied_fields"]
        assert "preview_source_account_id" in suggestion_result["applied_fields"]
        assert "preview_destination_account_id" in suggestion_result["applied_fields"]
        assert suggestion_result["matching"]["llm"]["review_status"] == "pending"
        assert fixture["source_account"]["name"] in provider.calls[0]["prompt"]
        assert fixture["destination_account"]["name"] in provider.calls[0]["prompt"]

        preview_item = _get_preview_item_from_page(
            client,
            auth_headers,
            session_id=fixture["session_id"],
            preview_id=fixture["preview_id"],
        )
        assert preview_item["preview_main_category"] == fixture["main_category"]
        assert preview_item["preview_sub_category"] == fixture["sub_category"]
        assert int(preview_item["preview_source_account_id"]) == int(fixture["source_account"]["id"])
        assert int(preview_item["preview_destination_account_id"]) == int(fixture["destination_account"]["id"])
        assert preview_item["matching"]["llm"]["suggested_source_account"] == fixture["source_account"]["name"]
        assert preview_item["matching"]["llm"]["review_status"] == "pending"

        events = _list_llm_memory_events_for_user(
            user_id=fixture["current_user_id"],
            session_id=fixture["session_id"],
        )
        assert len(events) == 1
        assert events[0]["event_type"] == "recommendation"
        assert events[0]["decision"] is None

    def test_llm_preview_recommend_does_not_override_existing_preview_fields(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_recommend_no_override")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-no-override",
        )
        source_account_id = int(fixture["source_account"]["id"])

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        provider = _FakePreviewRecommendationProvider(
            [
                {
                    "preview_id": fixture["preview_id"],
                    "suggested_main_category": "LLM不同分类",
                    "suggested_sub_category": "LLM不同子分类",
                    "suggested_source_account": fixture["destination_account"]["name"],
                    "confidence": 0.73,
                    "reason": "不应覆盖已有草稿",
                }
            ]
        )
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        async def _seed_existing_values() -> None:
            await api_app.db.update_preview_bills_batch(
                fixture["session_id"],
                [
                    {
                        "id": fixture["preview_id"],
                        "preview_main_category": "已有主分类",
                        "preview_sub_category": "已有子分类",
                        "preview_source_account_id": source_account_id,
                    }
                ],
                user_id=fixture["current_user_id"],
            )

        asyncio.run(_seed_existing_values())

        response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"], "preview_type": "支出"}],
            },
        )

        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json()
        assert payload["success"] is True
        suggestion_result = payload["data"]["suggestions"][0]
        assert suggestion_result["applied_fields"] == []

        preview_item = _get_preview_item_from_page(
            client,
            auth_headers,
            session_id=fixture["session_id"],
            preview_id=fixture["preview_id"],
        )
        assert preview_item["preview_main_category"] == "已有主分类"
        assert preview_item["preview_sub_category"] == "已有子分类"
        assert int(preview_item["preview_source_account_id"]) == source_account_id
        assert preview_item["matching"]["llm"]["suggested_main_category"] == "LLM不同分类"
        assert preview_item["matching"]["llm"]["review_status"] == "pending"

    def test_llm_preview_recommend_accept_keeps_preview_and_projects_matching_signal(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_recommend_accept")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-accept",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        suggestion = {
            "preview_id": fixture["preview_id"],
            "suggested_main_category": fixture["main_category"],
            "suggested_sub_category": fixture["sub_category"],
            "suggested_source_account": fixture["source_account"]["name"],
            "suggested_destination_account": fixture["destination_account"]["name"],
            "confidence": 0.91,
            "reason": "接受黄色建议",
        }
        provider = _FakePreviewRecommendationProvider([suggestion])
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        recommend_response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"]}],
            },
        )
        assert recommend_response.status_code == 200, recommend_response.get_data(as_text=True)
        llm_payload = recommend_response.get_json()["data"]["suggestions"][0]["matching"]["llm"]

        accept_response = client.post(
            "/api/llm/preview-recommend/accept",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_id": fixture["preview_id"],
                "suggestion": llm_payload,
            },
        )

        assert accept_response.status_code == 200, accept_response.get_data(as_text=True)
        accept_payload = accept_response.get_json()
        assert accept_payload["success"] is True
        assert accept_payload["data"]["decision"] == "accept"
        assert accept_payload["data"]["restored"] is False

        preview_item = _get_preview_item_from_page(
            client,
            auth_headers,
            session_id=fixture["session_id"],
            preview_id=fixture["preview_id"],
        )
        assert preview_item["preview_main_category"] == fixture["main_category"]
        assert preview_item["preview_sub_category"] == fixture["sub_category"]
        assert preview_item["matching"]["llm"]["review_status"] == "accepted"
        assert preview_item["matching"]["llm"]["suppressed"] is False

        events = _list_llm_memory_events_for_user(
            user_id=fixture["current_user_id"],
            session_id=fixture["session_id"],
        )
        assert len(events) == 2
        assert any(event["event_type"] == "feedback" and event["decision"] == "accept" for event in events)
        assert any(event["event_type"] == "recommendation" for event in events)

    def test_llm_preview_recommend_reject_restores_snapshot_and_writes_feedback(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_recommend_reject")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-reject",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        suggestion = {
            "preview_id": fixture["preview_id"],
            "suggested_main_category": fixture["main_category"],
            "suggested_sub_category": fixture["sub_category"],
            "suggested_source_account": fixture["source_account"]["name"],
            "suggested_destination_account": fixture["destination_account"]["name"],
            "confidence": 0.79,
            "reason": "拒绝黄色建议",
        }
        provider = _FakePreviewRecommendationProvider([suggestion])
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        recommend_response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"]}],
            },
        )
        assert recommend_response.status_code == 200, recommend_response.get_data(as_text=True)
        llm_payload = recommend_response.get_json()["data"]["suggestions"][0]["matching"]["llm"]

        reject_response = client.post(
            "/api/llm/preview-recommend/reject",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_id": fixture["preview_id"],
                "suggestion": llm_payload,
                "user_correction": {
                    "category": "人工分类",
                    "account": "手工账户链路",
                },
            },
        )

        assert reject_response.status_code == 200, reject_response.get_data(as_text=True)
        reject_payload = reject_response.get_json()
        assert reject_payload["success"] is True
        assert reject_payload["data"]["decision"] == "reject"
        assert reject_payload["data"]["restored"] is True

        preview_item = _get_preview_item_from_page(
            client,
            auth_headers,
            session_id=fixture["session_id"],
            preview_id=fixture["preview_id"],
        )
        assert preview_item["preview_main_category"] == ""
        assert preview_item["preview_sub_category"] == ""
        assert not preview_item.get("preview_source_account_id")
        assert not preview_item.get("preview_destination_account_id")
        assert preview_item["matching"]["llm"]["review_status"] == "rejected"
        assert preview_item["matching"]["llm"]["suppressed"] is True

        events = _list_llm_memory_events_for_user(
            user_id=fixture["current_user_id"],
            session_id=fixture["session_id"],
        )
        assert len(events) == 2
        reject_events = [
            event for event in events
            if event["event_type"] == "feedback" and event["decision"] == "reject"
        ]
        assert len(reject_events) == 1
        assert reject_events[0]["user_correction_category"] == "人工分类"
        assert reject_events[0]["user_correction_account"] == "手工账户链路"
        assert any(event["event_type"] == "recommendation" for event in events)

    def test_llm_preview_recommend_accept_does_not_require_live_provider_after_recommendation(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_accept_no_provider")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-accept-no-provider",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        suggestion = {
            "preview_id": fixture["preview_id"],
            "suggested_main_category": fixture["main_category"],
            "suggested_sub_category": fixture["sub_category"],
            "suggested_source_account": fixture["source_account"]["name"],
            "suggested_destination_account": fixture["destination_account"]["name"],
            "confidence": 0.91,
            "reason": "接受后不应再依赖 provider",
        }
        provider = _FakePreviewRecommendationProvider([suggestion])
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        recommend_response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"]}],
            },
        )
        assert recommend_response.status_code == 200, recommend_response.get_data(as_text=True)
        llm_payload = recommend_response.get_json()["data"]["suggestions"][0]["matching"]["llm"]

        def _provider_should_not_be_created(*_args, **_kwargs):
            raise AssertionError("accept should not create a live LLM provider")

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", _provider_should_not_be_created)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": False,
            "provider": "openai",
            "provider_config": {"model": "disabled-model"},
        }

        accept_response = client.post(
            "/api/llm/preview-recommend/accept",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_id": fixture["preview_id"],
                "suggestion": llm_payload,
            },
        )

        assert accept_response.status_code == 200, accept_response.get_data(as_text=True)
        accept_payload = accept_response.get_json()
        assert accept_payload["success"] is True
        assert accept_payload["data"]["decision"] == "accept"
        assert accept_payload["data"]["matching"]["llm"]["review_status"] == "accepted"

    def test_llm_preview_recommend_reject_does_not_require_live_provider_after_recommendation(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_preview_reject_no_provider")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-preview-reject-no-provider",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        suggestion = {
            "preview_id": fixture["preview_id"],
            "suggested_main_category": fixture["main_category"],
            "suggested_sub_category": fixture["sub_category"],
            "suggested_source_account": fixture["source_account"]["name"],
            "suggested_destination_account": fixture["destination_account"]["name"],
            "confidence": 0.79,
            "reason": "拒绝后不应再依赖 provider",
        }
        provider = _FakePreviewRecommendationProvider([suggestion])
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        recommend_response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"]}],
            },
        )
        assert recommend_response.status_code == 200, recommend_response.get_data(as_text=True)
        llm_payload = recommend_response.get_json()["data"]["suggestions"][0]["matching"]["llm"]

        def _provider_should_not_be_created(*_args, **_kwargs):
            raise AssertionError("reject should not create a live LLM provider")

        monkeypatch.setattr(llm_routes.ProviderFactory, "create", _provider_should_not_be_created)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": False,
            "provider": "openai",
            "provider_config": {"model": "disabled-model"},
        }

        reject_response = client.post(
            "/api/llm/preview-recommend/reject",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_id": fixture["preview_id"],
                "suggestion": llm_payload,
            },
        )

        assert reject_response.status_code == 200, reject_response.get_data(as_text=True)
        reject_payload = reject_response.get_json()
        assert reject_payload["success"] is True
        assert reject_payload["data"]["decision"] == "reject"
        assert reject_payload["data"]["matching"]["llm"]["review_status"] == "rejected"

    def test_llm_memory_route_orders_same_second_feedback_before_pending_and_counts_filtered_total(
        self,
        client,
        monkeypatch,
    ):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_memory_order_and_total")
        fixture = _prepare_llm_preview_fixture(
            client,
            auth_headers,
            prefix="pytest-llm-memory-order-total",
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        suggestion = {
            "preview_id": fixture["preview_id"],
            "suggested_main_category": fixture["main_category"],
            "suggested_sub_category": fixture["sub_category"],
            "suggested_source_account": fixture["source_account"]["name"],
            "suggested_destination_account": fixture["destination_account"]["name"],
            "confidence": 0.91,
            "reason": "同秒排序应优先反馈事件",
        }
        provider = _FakePreviewRecommendationProvider([suggestion])
        monkeypatch.setattr(llm_routes.ProviderFactory, "create", lambda *_args, **_kwargs: provider)
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "preview-model"},
        }

        recommend_response = client.post(
            "/api/llm/preview-recommend",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_updates": [{"id": fixture["preview_id"]}],
            },
        )
        assert recommend_response.status_code == 200, recommend_response.get_data(as_text=True)
        recommend_result = recommend_response.get_json()["data"]["suggestions"][0]

        accept_response = client.post(
            "/api/llm/preview-recommend/accept",
            headers=auth_headers,
            json={
                "session_id": fixture["session_id"],
                "preview_id": fixture["preview_id"],
                "suggestion": recommend_result["matching"]["llm"],
            },
        )
        assert accept_response.status_code == 200, accept_response.get_data(as_text=True)
        accept_result = accept_response.get_json()["data"]

        _set_llm_memory_events_created_at(
            event_ids=[int(recommend_result["event_id"]), int(accept_result["event_id"])],
            created_at="2026-04-30 08:00:00",
        )

        memory_response = client.get(
            "/api/llm/memory",
            headers=auth_headers,
            query_string={"session_id": fixture["session_id"]},
        )
        assert memory_response.status_code == 200, memory_response.get_data(as_text=True)
        memory_payload = memory_response.get_json()
        assert memory_payload["success"] is True
        assert memory_payload["total"] == 2
        assert len(memory_payload["data"]) == 2
        assert memory_payload["data"][0]["event_type"] == "feedback"
        assert memory_payload["data"][0]["decision"] == "accept"
        assert memory_payload["data"][1]["event_type"] == "recommendation"

        filtered_response = client.get(
            "/api/llm/memory",
            headers=auth_headers,
            query_string={
                "session_id": fixture["session_id"],
                "event_type": "feedback",
            },
        )
        assert filtered_response.status_code == 200, filtered_response.get_data(as_text=True)
        filtered_payload = filtered_response.get_json()
        assert filtered_payload["success"] is True
        assert filtered_payload["total"] == 1
        assert len(filtered_payload["data"]) == 1
        assert filtered_payload["data"][0]["event_type"] == "feedback"

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

    def test_llm_rule_synthesis_builds_rule_center_candidates(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_rule_synthesis_candidates")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        main_category, sub_category = _get_category_path_by_id(
            user_id=current_user_id,
            category_id=int(category["id"]),
        )
        expected_category_name = "/".join(part for part in (main_category, sub_category) if part)

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        _seed_rule_synthesis_knowledge_pack(
            user_id=current_user_id,
            category_id=int(category["id"]),
            category_path=(main_category, sub_category),
        )
        monkeypatch.setattr(
            llm_routes.ProviderFactory,
            "create",
            lambda *_args, **_kwargs: _RuleSynthesisLLMProvider(
                [
                    {
                        "rule_name": "咖啡门店归纳规则",
                        "suggested_main_category": main_category,
                        "suggested_sub_category": sub_category,
                        "rule_expression": "OR={星巴克,瑞幸}+AND={咖啡}+NOT={退款}",
                        "confidence": 0.92,
                        "reason": "长期学习规则和接受反馈都指向咖啡门店消费。",
                    }
                ]
            ),
        )
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        response = client.post(
            "/api/llm/rule-synthesis",
            headers=auth_headers,
            json={"limit": 4},
        )

        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["mode"] == "rule_synthesis"
        assert payload["data"]["candidates_created"] == 1
        assert payload["data"]["knowledge_summary_pack"]["categories"]
        candidate = payload["data"]["candidates"][0]
        assert candidate["type"] == "rule_synthesis"
        assert candidate["category_name"] == expected_category_name
        assert candidate["rule_expression"] == "OR={星巴克,瑞幸}+AND={咖啡}+NOT={退款}"

        llm_candidates = [
            item for item in _list_llm_candidates_for_user(user_id=current_user_id)
            if item.get("type") == "rule_synthesis"
        ]
        assert len(llm_candidates) == 1
        assert llm_candidates[0]["suggested_rule_expression"] == "OR={星巴克,瑞幸}+AND={咖啡}+NOT={退款}"

    def test_llm_rule_synthesis_skips_invalid_rule_expression_candidates(self, client, monkeypatch):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_rule_synthesis_invalid_rule")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        main_category, sub_category = _get_category_path_by_id(
            user_id=current_user_id,
            category_id=int(category["id"]),
        )

        from bill_analyser.api import app as api_app
        from bill_analyser.api.routes import llm as llm_routes

        _seed_rule_synthesis_knowledge_pack(
            user_id=current_user_id,
            category_id=int(category["id"]),
            category_path=(main_category, sub_category),
        )
        monkeypatch.setattr(
            llm_routes.ProviderFactory,
            "create",
            lambda *_args, **_kwargs: _RuleSynthesisLLMProvider(
                [
                    {
                        "rule_name": "坏规则",
                        "suggested_main_category": main_category,
                        "suggested_sub_category": sub_category,
                        "rule_expression": "OR={星巴克",
                        "confidence": 0.88,
                        "reason": "malformed",
                    }
                ]
            ),
        )
        api_app.app.config["LLM_CONFIG"] = {
            "enabled": True,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        response = client.post(
            "/api/llm/rule-synthesis",
            headers=auth_headers,
            json={"limit": 4},
        )

        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["candidates_created"] == 0
        assert payload["data"]["candidates"] == []

    def test_llm_candidate_accept_does_not_require_live_provider_for_rule_review(self, client):
        _reset_llm_rate_limit_state()
        auth_headers = _build_isolated_auth_headers(client, "test_llm_candidate_accept_without_provider")
        current_user_id = _get_current_user_id(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        main_category, sub_category = _get_category_path_by_id(
            user_id=current_user_id,
            category_id=int(category["id"]),
        )

        from bill_analyser.api import app as api_app

        api_app.app.config["LLM_CONFIG"] = {
            "enabled": False,
            "provider": "openai",
            "provider_config": {"model": "fake-model"},
        }

        async def _prepare_candidate() -> int:
            return await api_app.db.create_llm_candidate(
                user_id=current_user_id,
                type="rule_synthesis",
                source_bill_ids=[],
                suggested_main_category=main_category,
                suggested_sub_category=sub_category,
                suggested_rule_expression="OR={星巴克,瑞幸}+AND={咖啡}",
                confidence=0.9,
                llm_provider="fake-provider",
                llm_model="fake-model",
                llm_response_raw=json.dumps({"reason": "offline review"}, ensure_ascii=False),
            )

        candidate_id = asyncio.run(_prepare_candidate())

        response = client.post(
            f"/api/llm/candidates/{candidate_id}/accept",
            headers=auth_headers,
        )

        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json()
        assert payload["success"] is True
        assert payload["data"]["status"] == "accepted"
        assert payload["data"]["created_rule_id"] > 0
