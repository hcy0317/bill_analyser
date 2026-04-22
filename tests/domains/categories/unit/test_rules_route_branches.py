from __future__ import annotations

import asyncio
from typing import Any, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import rules as rules_module

rules_module = cast("Any", rules_module)


@pytest.fixture(name="rules_route_app")
def rules_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct rules route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeLoop:
    """Minimal event-loop adapter for direct route invocation tests."""

    def run_until_complete(self, coroutine: Any) -> Any:
        return asyncio.run(coroutine)

    def close(self) -> None:
        return None


class FakeRulesDB:
    """Async DB stub for rules overview route tests."""

    def __init__(self) -> None:
        self.learning_count = 2
        self.learning_rules = [
            {
                "id": 11,
                "match_type": "counterparty",
                "match_value": "招商银行",
                "learned_type": "expense",
                "learned_category_id": 7,
                "enabled": True,
                "applied_count": 3,
            },
            {
                "id": 12,
                "match_type": "description",
                "match_value": "基金定投",
                "learned_type": "expense",
                "learned_category_id": 9,
                "enabled": False,
                "applied_count": 1,
            },
        ]
        self.category_rules = [
            {"id": 21, "name": "基金申购", "enabled": True},
            {"id": 22, "name": "理财赎回", "enabled": True},
        ]
        self.recurring_rules = [
            {
                "id": 31,
                "name": "房租",
                "amount": 3500,
                "frequency": "monthly",
                "enabled": 1,
                "next_date": "2026-05-01",
            }
        ]
        self.get_category_rules_calls: list[dict[str, Any]] = []
        self.connection_requested = False

    async def count_import_learning_rules(self, *, user_id: int) -> int:
        _ = user_id
        return self.learning_count

    async def get_import_learning_rules(self, *, user_id: int, limit: int) -> list[dict[str, Any]]:
        _ = (user_id, limit)
        return [dict(item) for item in self.learning_rules]

    async def get_category_rules(
        self,
        *,
        user_id: int,
        category_id: int | None = None,
        enabled_only: bool = True,
    ) -> list[dict[str, Any]]:
        self.get_category_rules_calls.append(
            {
                "user_id": user_id,
                "category_id": category_id,
                "enabled_only": enabled_only,
            }
        )
        return [dict(item) for item in self.category_rules]

    async def get_all_templates(self, *, user_id: int, template_type: int) -> list[dict[str, Any]]:
        _ = (user_id, template_type)
        return [dict(item) for item in self.recurring_rules]

    async def _get_connection(self) -> Any:
        self.connection_requested = True
        raise AssertionError("rules overview should not query legacy category_keywords")


def _install_fake_loop(monkeypatch: pytest.MonkeyPatch, loop: FakeLoop) -> None:
    monkeypatch.setattr(rules_module.asyncio, "new_event_loop", lambda: loop)
    monkeypatch.setattr(rules_module.asyncio, "set_event_loop", lambda _loop: None)


def _unwrap_all(func: Any) -> Any:
    first = getattr(func, "__wrapped__", None)
    if first is None:
        return func

    second = getattr(first, "__wrapped__", None)
    return second or first


def test_rules_overview_uses_category_rules_as_canonical_source(
    rules_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Rule overview should stop reading legacy category keywords."""
    db = FakeRulesDB()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(rules_module, "_get_db", lambda: db)
    monkeypatch.setattr(rules_module, "_get_user_id", lambda: 9)

    route = _unwrap_all(rules_module.get_rules_overview)

    with rules_route_app.test_request_context("/api/rules/overview", method="GET"):
        payload = route().get_json() or {}

    assert payload["success"] is True
    assert payload["data"] == {
        "learningRules": [
            {
                "id": 11,
                "matchType": "counterparty",
                "matchValue": "招商银行",
                "learnedType": "expense",
                "learnedCategoryId": 7,
                "enabled": True,
                "appliedCount": 3,
                "source": "learning",
            },
            {
                "id": 12,
                "matchType": "description",
                "matchValue": "基金定投",
                "learnedType": "expense",
                "learnedCategoryId": 9,
                "enabled": False,
                "appliedCount": 1,
                "source": "learning",
            },
        ],
        "learningRuleCount": 2,
        "categoryRuleCount": 2,
        "recurringRules": [
            {
                "id": 31,
                "name": "房租",
                "amount": 3500,
                "frequency": "monthly",
                "enabled": True,
                "nextDate": "2026-05-01",
                "source": "recurring",
            }
        ],
        "recurringRuleCount": 1,
        "totalRuleCount": 5,
    }
    assert "categoryKeywords" not in payload["data"]
    assert "categoryKeywordCount" not in payload["data"]
    assert db.get_category_rules_calls == [
        {
            "user_id": 9,
            "category_id": None,
            "enabled_only": True,
        }
    ]
    assert db.connection_requested is False


def test_rules_overview_falls_back_to_zero_when_category_rules_load_fails(
    rules_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Category rule summary should degrade gracefully without reviving legacy keyword reads."""
    db = FakeRulesDB()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(rules_module, "_get_db", lambda: db)
    monkeypatch.setattr(rules_module, "_get_user_id", lambda: 9)

    async def raise_rules_error(*, user_id: int, category_id: int | None = None, enabled_only: bool = True) -> list[dict[str, Any]]:
        _ = (user_id, category_id, enabled_only)
        raise RuntimeError("category rules unavailable")

    monkeypatch.setattr(db, "get_category_rules", raise_rules_error)
    route = _unwrap_all(rules_module.get_rules_overview)

    with rules_route_app.test_request_context("/api/rules/overview", method="GET"):
        payload = route().get_json() or {}

    assert payload["success"] is True
    assert payload["data"]["categoryRuleCount"] == 0
    assert payload["data"]["totalRuleCount"] == 3
    assert "categoryKeywords" not in payload["data"]
    assert "categoryKeywordCount" not in payload["data"]
    assert db.connection_requested is False
