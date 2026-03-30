from __future__ import annotations

from typing import Any, Callable, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import categories as categories_module


@pytest.fixture(name="categories_route_app")
def categories_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct categories route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeCategoryEngine:
    """Minimal category engine stub for route tests."""

    def __init__(self) -> None:
        self.rules = [{"keyword": "早餐", "category": "餐饮"}]
        self.load_calls = 0

    def load_rules(self) -> None:
        self.load_calls += 1


class FakeCategoriesDB:
    """Synchronous DB stub when _run_async is patched to identity."""

    def __init__(self) -> None:
        self.categories: dict[int, dict[str, Any]] = {
            1: {"id": 1, "main_category": "餐饮", "sub_category": "", "type": 3, "priority": 1, "hidden": False},
            2: {"id": 2, "main_category": "餐饮", "sub_category": "早餐", "type": 3, "priority": 2, "hidden": False},
            3: {"id": 3, "main_category": "交通", "sub_category": "", "type": 3, "priority": 3, "hidden": False},
        }

    def get_all_categories(self, user_id: int = 0) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.categories.values()]

    def get_category_by_name(self, main_category: str, sub_category: str, user_id: int = 0) -> dict[str, Any] | None:
        _ = user_id
        return next(
            (
                dict(item)
                for item in self.categories.values()
                if item["main_category"] == main_category and item["sub_category"] == sub_category
            ),
            None,
        )

    def create_category(self, payload: dict[str, Any], user_id: int = 0) -> int:
        _ = user_id
        next_id = max(self.categories) + 1
        self.categories[next_id] = {"id": next_id, **payload}
        return next_id

    def get_category_by_id(self, category_id: int, user_id: int = 0) -> dict[str, Any] | None:
        _ = user_id
        category = self.categories.get(int(category_id))
        return dict(category) if category else None

    def update_main_category_name(self, old_name: str, new_name: str) -> bool:
        for category in self.categories.values():
            if category["main_category"] == old_name:
                category["main_category"] = new_name
        return True

    def update_category(self, category_id: int, payload: dict[str, Any], user_id: int = 0) -> bool:
        _ = user_id
        category = self.categories.get(int(category_id))
        if category is None:
            return False
        category.update(payload)
        return True

    def delete_categories_by_main_category(self, main_category: str) -> bool:
        original = len(self.categories)
        self.categories = {
            key: value for key, value in self.categories.items() if value["main_category"] != main_category
        }
        return len(self.categories) != original

    def delete_category(self, category_id: int, user_id: int = 0) -> bool:
        _ = user_id
        return self.categories.pop(int(category_id), None) is not None

    def get_category_statistics(
        self,
        period: str,
        start_date: str | None = None,
        end_date: str | None = None,
        user_id: int = 0,
    ) -> list[dict[str, Any]]:
        _ = (period, start_date, end_date, user_id)
        return [
            {"main_category": "餐饮", "sub_category": "早餐", "total_amount": -18.5, "count": 2},
            {"main_category": "交通", "sub_category": "", "total_amount": -12, "count": 1},
        ]


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast(Any, func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast(Callable[..., Any], current)

    second = getattr(first, "__wrapped__", None)
    return cast(Callable[..., Any], second or first)


def test_categories_routes_cover_crud_rules_statistics_and_batch_create(
    categories_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """分类路由应覆盖创建、更新、删除、规则、统计和批量创建等主要分支。"""
    db = FakeCategoriesDB()
    engine = FakeCategoryEngine()
    saved_configs: list[tuple[str, Any]] = []

    def format_list_response(categories: list[dict[str, Any]]) -> dict[str, Any]:
        return {"success": True, "result": list(categories)}

    def get_flat_list(categories: list[dict[str, Any]]) -> list[dict[str, Any]]:
        return [{"id": str(item["id"]), "name": item["sub_category"] or item["main_category"]} for item in categories]

    monkeypatch.setattr(categories_module, "_run_async", lambda value: value)
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (db, object(), engine))
    monkeypatch.setattr(categories_module, "_get_request_user_id", lambda: 7)
    monkeypatch.setattr(categories_module.category_adapter, "format_list_response", format_list_response)
    monkeypatch.setattr(categories_module.category_adapter, "get_flat_list", get_flat_list)
    monkeypatch.setattr(categories_module, "save_config", lambda filename, rules: saved_configs.append((filename, rules)))

    get_categories = _unwrap(categories_module.get_categories)
    create_category = _unwrap(categories_module.create_category)
    update_category = _unwrap(categories_module.update_category)
    move_categories = _unwrap(categories_module.move_categories)
    delete_category = _unwrap(categories_module.delete_category)
    get_flat_categories = _unwrap(categories_module.get_flat_categories)
    get_category_rules = _unwrap(categories_module.get_category_rules)
    update_category_rules = _unwrap(categories_module.update_category_rules)
    get_category_statistics = _unwrap(categories_module.get_category_statistics)
    get_category_tree = _unwrap(categories_module.get_category_tree)
    get_all_categories = _unwrap(categories_module.get_all_categories)
    update_all_categories = _unwrap(categories_module.update_all_categories)
    batch_create_categories = _unwrap(categories_module.batch_create_categories)

    with categories_route_app.test_request_context("/api/categories/"):
        payload = get_categories().get_json() or {}
        assert payload["success"] is True
        assert len(payload["result"]) == 3

    with categories_route_app.test_request_context("/api/categories/", method="POST", json={}):
        response, status = create_category()
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with categories_route_app.test_request_context("/api/categories/", method="POST", json={"comment": "x"}):
        response, status = create_category()
        assert status == 400
        assert response.get_json()["error"] == "Category name is required"

    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "餐饮", "parentId": "0", "type": 3},
    ):
        payload = create_category().get_json() or {}
        assert payload["success"] is True
        assert payload["message"] == "Category already exists"

    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "理财", "parentId": "0", "type": 2},
    ):
        response, status = create_category()
        payload = response.get_json() or {}
        assert status == 201
        assert payload["result"]["name"] == "理财"

    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "地铁", "parentId": "999"},
    ):
        response, status = create_category()
        assert status == 404
        assert response.get_json()["error"] == "Parent category not found"

    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "午餐", "parentId": "virtual_餐饮"},
    ):
        payload = create_category().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["parentId"] == "virtual_餐饮"

    with categories_route_app.test_request_context(
        "/api/categories/not-an-int",
        method="PUT",
        json={"name": "oops"},
    ):
        response, status = update_category("not-an-int")
        assert status == 400
        assert response.get_json()["error"] == "Invalid category ID"

    with categories_route_app.test_request_context(
        "/api/categories/virtual_交通",
        method="PUT",
        json={"name": "出行", "comment": "updated"},
    ):
        payload = update_category("virtual_交通").get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "出行"

    with categories_route_app.test_request_context(
        "/api/categories/999",
        method="PUT",
        json={"name": "missing"},
    ):
        response, status = update_category("999")
        assert status == 404
        assert response.get_json()["error"] == "Category not found"

    with categories_route_app.test_request_context(
        "/api/categories/move",
        method="POST",
        json={"newDisplayOrders": []},
    ):
        payload = move_categories().get_json() or {}
        assert payload["success"] is True

    with categories_route_app.test_request_context(
        "/api/categories/move",
        method="POST",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": 10}]},
    ):
        payload = move_categories().get_json() or {}
        assert payload["success"] is True
        assert db.categories[1]["priority"] == 10

    with categories_route_app.test_request_context("/api/categories/not-an-int", method="DELETE"):
        response, status = delete_category("not-an-int")
        assert status == 400
        assert response.get_json()["error"] == "Invalid category ID"

    with categories_route_app.test_request_context("/api/categories/virtual_餐饮", method="DELETE"):
        payload = delete_category("virtual_餐饮").get_json() or {}
        assert payload["success"] is True
        assert all(item["main_category"] != "餐饮" for item in db.categories.values())

    with categories_route_app.test_request_context("/api/categories/999", method="DELETE"):
        response, status = delete_category("999")
        assert status == 404
        assert response.get_json()["error"] == "Category not found or delete failed"

    with categories_route_app.test_request_context("/api/categories/flat"):
        payload = get_flat_categories().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]

    with categories_route_app.test_request_context("/api/categories/rules"):
        payload = get_category_rules().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == engine.rules

    with categories_route_app.test_request_context("/api/categories/rules", method="PUT", json={}):
        response, status = update_category_rules()
        assert status == 400
        assert response.get_json()["error"] == "rules are required"

    with categories_route_app.test_request_context(
        "/api/categories/rules",
        method="PUT",
        json={"rules": [{"keyword": "咖啡", "category": "餐饮"}]},
    ):
        payload = update_category_rules().get_json() or {}
        assert payload["success"] is True
        assert saved_configs == [("categories.json", [{"keyword": "咖啡", "category": "餐饮"}])]
        assert engine.load_calls == 1

    with categories_route_app.test_request_context("/api/categories/statistics?period=month"):
        payload = get_category_statistics().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["交通"]["total_amount"] == 12.0
        assert payload["result"]["餐饮"]["sub_categories"]["早餐"]["count"] == 2

    with categories_route_app.test_request_context("/api/categories/tree"):
        payload = get_category_tree().get_json() or {}
        assert payload["success"] is True

    with categories_route_app.test_request_context("/api/categories/all"):
        payload = get_all_categories().get_json() or {}
        assert payload["success"] is True
        assert len(payload["result"]) == len(db.categories)

    with categories_route_app.test_request_context("/api/categories/all", method="PUT", json={}):
        response, status = update_all_categories()
        assert status == 400
        assert response.get_json()["error"] == "categories are required"

    with categories_route_app.test_request_context(
        "/api/categories/all",
        method="PUT",
        json={"categories": []},
    ):
        payload = update_all_categories().get_json() or {}
        assert payload["success"] is True

    with categories_route_app.test_request_context("/api/categories/batch", method="POST", json={}):
        response, status = batch_create_categories()
        assert status == 400
        assert response.get_json()["error"] == "No categories provided"

    with categories_route_app.test_request_context(
        "/api/categories/batch",
        method="POST",
        json={
            "categories": [
                {"name": "娱乐", "subCategories": [{"name": "电影"}]},
                {"name": "出行", "subCategories": [{"name": "地铁"}]},
            ]
        },
    ):
        payload = batch_create_categories().get_json() or {}
        assert payload["success"] is True
        assert any(item["main_category"] == "娱乐" for item in db.categories.values())
