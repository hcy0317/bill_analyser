from __future__ import annotations

from collections.abc import Callable
from types import SimpleNamespace
from typing import Any, cast

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

    def match_category(self, bill: dict[str, Any]) -> tuple[str, str]:
        comment = str(bill.get("comment") or "")
        if "早餐" in comment:
            return ("餐饮", "早餐")
        return ("交通", "地铁")


class FakeCategoriesDB:
    """Synchronous DB stub when _run_async is patched to identity."""

    def __init__(self) -> None:
        self.categories: dict[int, dict[str, Any]] = {
            1: {"id": 1, "main_category": "餐饮", "sub_category": "", "type": 3, "priority": 1, "hidden": False},
            2: {"id": 2, "main_category": "餐饮", "sub_category": "早餐", "type": 3, "priority": 2, "hidden": False},
            3: {"id": 3, "main_category": "交通", "sub_category": "", "type": 3, "priority": 3, "hidden": False},
        }
        self.bills: list[dict[str, Any]] = [
            {"id": 101, "main_category": "", "sub_category": "", "comment": "早餐店"},
            {"id": 102, "main_category": "已有分类", "sub_category": "已有子类", "comment": "地铁站"},
        ]
        self.updated_bills: list[tuple[int, dict[str, Any]]] = []

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

    def query_bills(self, page: int, page_size: int, filters: dict[str, Any], user_id: int = 0):
        _ = (page, page_size, filters, user_id)
        return ([dict(item) for item in self.bills], len(self.bills))

    def update_bill(self, bill_id: int, payload: dict[str, Any], user_id: int = 0) -> bool:
        _ = user_id
        self.updated_bills.append((bill_id, payload))
        for bill in self.bills:
            if bill["id"] == bill_id:
                bill.update(payload)
                return True
        return False


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


def _unwrap_response(result: Any) -> Any:
    if isinstance(result, tuple):
        return result[0]
    return result


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


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
    get_category = _unwrap(categories_module.get_category)
    export_categories = _unwrap(categories_module.export_categories)
    import_categories = _unwrap(categories_module.import_categories)
    recategorize_all_bills = _unwrap(categories_module.recategorize_all_bills)

    monkeypatch.setattr(categories_module, "get_categories", get_categories)

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

    with categories_route_app.test_request_context("/api/categories/virtual_餐饮"):
        payload = _unwrap_response(get_category("virtual_餐饮")).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "餐饮"

    with categories_route_app.test_request_context("/api/categories/2"):
        payload = _unwrap_response(get_category("2")).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["parentId"] == "1"

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
        payload = _unwrap_response(get_category_tree()).get_json() or {}
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

    with categories_route_app.test_request_context("/api/categories/not-an-int"):
        response, status = get_category("not-an-int")
        assert status == 400
        assert response.get_json()["error"] == "Invalid category ID"

    with categories_route_app.test_request_context("/api/categories/999"):
        response, status = get_category("999")
        assert status == 404
        assert response.get_json()["error"] == "Category not found"

    with categories_route_app.test_request_context("/api/categories/export"):
        payload = export_categories().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]
        assert set(payload["result"][0].keys()) == {
            "type",
            "main_category",
            "sub_category",
            "priority",
            "keywords",
            "description",
            "icon",
            "color",
            "hidden",
        }

    with categories_route_app.test_request_context(
        "/api/categories/import",
        method="POST",
        json={},
    ):
        response, status = import_categories()
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with categories_route_app.test_request_context(
        "/api/categories/import",
        method="POST",
        json={"result": "not-a-list"},
    ):
        response, status = import_categories()
        assert status == 400
        assert response.get_json()["error"] == "Invalid format, expected list of categories"

    with categories_route_app.test_request_context(
        "/api/categories/import",
        method="POST",
        json=[
            {"main_category": "理财", "sub_category": "基金", "type": 2, "keywords": "基金"},
            {"main_category": "出行", "sub_category": "", "description": "更新出行"},
            {"sub_category": "缺主类"},
        ],
    ):
        payload = import_categories().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {"imported": 1, "updated": 1, "skipped": 1}

    with categories_route_app.test_request_context(
        "/api/categories/update-all",
        method="POST",
        json={"force": False},
    ):
        payload = recategorize_all_bills().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {"total": 2, "updated": 1}

    with categories_route_app.test_request_context(
        "/api/categories/update-all",
        method="POST",
        json={"force": True},
    ):
        payload = recategorize_all_bills().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {"total": 2, "updated": 2}


def test_categories_routes_cover_error_paths(
    categories_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """分类路由应在底层服务抛错时返回受控 500 响应。"""
    monkeypatch.setattr(categories_module, "_run_async", lambda value: value)
    monkeypatch.setattr(categories_module, "_get_request_user_id", lambda: 7)
    monkeypatch.setattr(
        categories_module.category_adapter,
        "format_list_response",
        lambda categories: {"success": True, "result": list(categories)},
    )
    monkeypatch.setattr(categories_module.category_adapter, "get_flat_list", lambda categories: list(categories))
    monkeypatch.setattr(categories_module, "save_config", lambda filename, rules: None)

    get_categories = _unwrap(categories_module.get_categories)
    create_category = _unwrap(categories_module.create_category)
    update_category = _unwrap(categories_module.update_category)
    move_categories = _unwrap(categories_module.move_categories)
    delete_category = _unwrap(categories_module.delete_category)
    get_flat_categories = _unwrap(categories_module.get_flat_categories)
    get_category_rules = _unwrap(categories_module.get_category_rules)
    update_category_rules = _unwrap(categories_module.update_category_rules)
    get_category_statistics = _unwrap(categories_module.get_category_statistics)
    get_all_categories = _unwrap(categories_module.get_all_categories)
    batch_create_categories = _unwrap(categories_module.batch_create_categories)
    recategorize_all_bills = _unwrap(categories_module.recategorize_all_bills)
    export_categories = _unwrap(categories_module.export_categories)
    import_categories = _unwrap(categories_module.import_categories)
    get_category = _unwrap(categories_module.get_category)

    list_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(list_fail_db, "get_all_categories", lambda *args, **kwargs: _raise_runtime_error("list boom"))
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (list_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/"):
        response, status = get_categories()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    create_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(create_fail_db, "create_category", lambda *args, **kwargs: None)
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (create_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "失败分类", "parentId": "0", "type": 3},
    ):
        response, status = create_category()
        assert status == 500
        assert response.get_json()["error"] == "Failed to create category"

    update_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        update_fail_db,
        "update_category",
        lambda *args, **kwargs: _raise_runtime_error("update boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (update_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/1",
        method="PUT",
        json={"comment": "fail"},
    ):
        response, status = update_category("1")
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (update_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/move",
        method="POST",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": 9}]},
    ):
        response, status = move_categories()
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    delete_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        delete_fail_db,
        "delete_category",
        lambda *args, **kwargs: _raise_runtime_error("delete boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (delete_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/1", method="DELETE"):
        response, status = delete_category("1")
        assert status == 500
        assert response.get_json()["error"] == "delete boom"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (list_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/flat"):
        response, status = get_flat_categories()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (FakeCategoriesDB(), object(), object()))
    with categories_route_app.test_request_context("/api/categories/rules"):
        response, status = get_category_rules()
        assert status == 500
        assert "rules" in response.get_json()["error"]

    class ExplodingRuleEngine(FakeCategoryEngine):
        def load_rules(self) -> None:
            raise RuntimeError("rules boom")

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (FakeCategoriesDB(), object(), ExplodingRuleEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/rules",
        method="PUT",
        json={"rules": [{"keyword": "炸裂", "category": "测试"}]},
    ):
        response, status = update_category_rules()
        assert status == 500
        assert response.get_json()["error"] == "rules boom"

    stats_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        stats_fail_db,
        "get_category_statistics",
        lambda *args, **kwargs: _raise_runtime_error("stats boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (stats_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/statistics?period=month"):
        response, status = get_category_statistics()
        assert status == 500
        assert response.get_json()["error"] == "stats boom"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (list_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/all"):
        response, status = get_all_categories()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    batch_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        batch_fail_db,
        "create_category",
        lambda *args, **kwargs: _raise_runtime_error("batch boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (batch_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/batch",
        method="POST",
        json={"categories": [{"name": "异常分类", "subCategories": []}]},
    ):
        response, status = batch_create_categories()
        assert status == 500
        assert response.get_json()["error"] == "batch boom"

    query_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(query_fail_db, "query_bills", lambda *args, **kwargs: _raise_runtime_error("query boom"))
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (query_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/update-all",
        method="POST",
        json={"force": True},
    ):
        response, status = recategorize_all_bills()
        assert status == 500
        assert response.get_json()["error"] == "query boom"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (list_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/export"):
        response, status = export_categories()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    import_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        import_fail_db,
        "create_category",
        lambda *args, **kwargs: _raise_runtime_error("import boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (import_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/import",
        method="POST",
        json=[{"main_category": "异常导入", "sub_category": "子类"}],
    ):
        response, status = import_categories()
        assert status == 500
        assert response.get_json()["error"] == "import boom"

    get_fail_db = FakeCategoriesDB()
    monkeypatch.setattr(
        get_fail_db,
        "get_category_by_id",
        lambda *args, **kwargs: _raise_runtime_error("get boom"),
    )
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (get_fail_db, object(), FakeCategoryEngine()))
    with categories_route_app.test_request_context("/api/categories/1"):
        response, status = get_category("1")
        assert status == 500
        assert response.get_json()["error"] == "get boom"


def test_categories_routes_cover_remaining_branch_closures(
    categories_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """分类路由应覆盖剩余高收益分支，包括显式上下文、二级创建、字段映射与 parentId 兜底。"""
    db = FakeCategoriesDB()
    engine = FakeCategoryEngine()
    original_get_app_context = categories_module.get_app_context

    def format_list_response(categories: list[dict[str, Any]]) -> dict[str, Any]:
        return {"success": True, "result": list(categories)}

    create_category = _unwrap(categories_module.create_category)
    update_category = _unwrap(categories_module.update_category)
    move_categories = _unwrap(categories_module.move_categories)
    get_category_statistics = _unwrap(categories_module.get_category_statistics)
    update_all_categories = _unwrap(categories_module.update_all_categories)
    batch_create_categories = _unwrap(categories_module.batch_create_categories)
    recategorize_all_bills = _unwrap(categories_module.recategorize_all_bills)
    import_categories = _unwrap(categories_module.import_categories)
    get_category = _unwrap(categories_module.get_category)

    with categories_route_app.app_context():
        categories_route_app.config["DB_INSTANCE"] = db
        categories_route_app.config["BILL_SERVICE_INSTANCE"] = object()
        categories_route_app.config["CATEGORY_ENGINE_INSTANCE"] = engine
        app_db, app_bill_service, app_engine = original_get_app_context(user_id=99)
        assert app_db is db
        assert app_bill_service is not None
        assert app_engine is engine

    monkeypatch.setattr(categories_module, "_run_async", lambda value: value)
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (db, object(), engine))
    monkeypatch.setattr(categories_module, "_get_request_user_id", lambda: 21)
    monkeypatch.setattr(categories_module.category_adapter, "format_list_response", format_list_response)
    monkeypatch.setattr(categories_module.category_adapter, "get_flat_list", lambda categories: list(categories))
    monkeypatch.setattr(categories_module, "get_categories", _unwrap(categories_module.get_categories))

    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "早餐", "parentId": "1"},
    ):
        payload = create_category().get_json() or {}
        assert payload["success"] is True
        assert payload["message"] == "Category already exists"
        assert payload["result"]["parentId"] == "1"

    exploding_parent_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (exploding_parent_db, object(), engine))
    monkeypatch.setattr(exploding_parent_db, "get_category_by_id", lambda *args, **kwargs: _raise_runtime_error("parent lookup boom"))
    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "异常子类", "parentId": "3"},
    ):
        response, status = create_category()
        assert status == 500
        assert response.get_json()["error"] == "parent lookup boom"

    missing_create_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (missing_create_db, object(), engine))
    monkeypatch.setattr(missing_create_db, "create_category", lambda *args, **kwargs: None)
    with categories_route_app.test_request_context(
        "/api/categories/",
        method="POST",
        json={"name": "公交", "parentId": "3"},
    ):
        response, status = create_category()
        assert status == 500
        assert response.get_json()["error"] == "Failed to create category"

    virtual_create_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (virtual_create_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/virtual_新主类",
        method="PUT",
        json={"comment": "created-from-virtual", "visible": False, "icon": "mdi-star", "color": "#123456"},
    ):
        payload = update_category("virtual_新主类").get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "新主类"
        created = next(item for item in virtual_create_db.categories.values() if item["main_category"] == "新主类" and not item["sub_category"])
        assert created["hidden"] is True
        assert created["icon"] == "mdi-star"
        assert created["color"] == "#123456"

    normal_update_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (normal_update_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/2",
        method="PUT",
        json={
            "name": "早午餐",
            "comment": "更新描述",
            "displayOrder": 8,
            "keywords": "早午餐",
            "type": 2,
            "visible": False,
            "icon": "mdi-food-outline",
            "color": "#abcdef",
        },
    ):
        payload = update_category("2").get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "早午餐"
        assert payload["result"]["visible"] is False
        assert normal_update_db.categories[2]["sub_category"] == "早午餐"
        assert normal_update_db.categories[2]["priority"] == 8
        assert normal_update_db.categories[2]["keywords"] == "早午餐"
        assert normal_update_db.categories[2]["type"] == 2
        assert normal_update_db.categories[2]["hidden"] is True
        assert normal_update_db.categories[2]["icon"] == "mdi-food-outline"
        assert normal_update_db.categories[2]["color"] == "#abcdef"

    same_name_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (same_name_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/1",
        method="PUT",
        json={"name": "餐饮", "comment": "同名更新"},
    ):
        payload = update_category("1").get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "餐饮"
        assert same_name_db.categories[1]["main_category"] == "餐饮"
        assert same_name_db.categories[1]["description"] == "同名更新"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (normal_update_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/1",
        method="PUT",
        json={"name": "餐饮新", "comment": "主类改名"},
    ):
        payload = update_category("1").get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "餐饮新"
        assert normal_update_db.categories[1]["main_category"] == "餐饮新"
        assert normal_update_db.categories[2]["main_category"] == "餐饮新"

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (normal_update_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/move",
        method="POST",
        json={"newDisplayOrders": [{"id": None, "displayOrder": 99}, {"id": "1", "displayOrder": 12}]},
    ):
        payload = move_categories().get_json() or {}
        assert payload["success"] is True
        assert normal_update_db.categories[1]["priority"] == 12

    stats_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (stats_db, object(), engine))
    monkeypatch.setattr(
        stats_db,
        "get_category_statistics",
        lambda *args, **kwargs: [
            {"main_category": "餐饮", "sub_category": "早餐", "total_amount": -10, "count": 1},
            {"main_category": "餐饮", "sub_category": "午餐", "total_amount": -20, "count": 2},
            {"main_category": "餐饮", "sub_category": "", "total_amount": -5, "count": 1},
        ],
    )
    with categories_route_app.test_request_context("/api/categories/statistics?period=month"):
        payload = get_category_statistics().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["餐饮"]["total_amount"] == 35.0
        assert payload["result"]["餐饮"]["count"] == 4
        assert payload["result"]["餐饮"]["sub_categories"]["午餐"]["count"] == 2

    with categories_route_app.test_request_context(
        "/api/categories/all",
        method="PUT",
        data="null",
        content_type="application/json",
    ):
        response, status = update_all_categories()
        assert status == 400
        assert response.get_json()["error"] == "categories are required"

    with categories_route_app.test_request_context(
        "/api/categories/all",
        method="PUT",
        json={"categories": []},
    ):
        with monkeypatch.context() as local_patch:
            failing_request = SimpleNamespace(get_json=lambda *args, **kwargs: _raise_runtime_error("update all boom"))
            local_patch.setattr(categories_module, "request", failing_request)
            response, status = update_all_categories()
            assert status == 500
            assert response.get_json()["error"] == "update all boom"

    batch_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (batch_db, object(), engine))
    original_batch_create = batch_db.create_category

    def create_category_with_failures(payload: dict[str, Any], user_id: int = 0) -> int | None:
        if payload.get("main_category") == "创建失败主类" and payload.get("sub_category") == "":
            return None
        if payload.get("main_category") == "子类失败主类" and payload.get("sub_category") == "失败子类":
            return None
        return original_batch_create(payload, user_id=user_id)

    monkeypatch.setattr(batch_db, "create_category", create_category_with_failures)
    with categories_route_app.test_request_context(
        "/api/categories/batch",
        method="POST",
        json={
            "categories": [
                {"subCategories": [{"name": "应跳过的孤儿子类"}]},
                {"name": "餐饮", "subCategories": [{"name": "早餐"}, {}]},
                {"name": "娱乐", "subCategories": [{"name": "桌游", "type": 4}]},
                {"name": "创建失败主类", "subCategories": [{"name": "补偿子类"}]},
                {"name": "子类失败主类", "subCategories": [{"name": "失败子类"}, {"name": "成功子类"}]},
            ]
        },
    ):
        payload = _unwrap_response(batch_create_categories()).get_json() or {}
        assert payload["success"] is True
        assert any(item["main_category"] == "娱乐" for item in batch_db.categories.values())
        assert any(item["sub_category"] == "桌游" and item["type"] == 4 for item in batch_db.categories.values())
        assert any(item["main_category"] == "创建失败主类" and item["sub_category"] == "补偿子类" for item in batch_db.categories.values())
        assert any(item["main_category"] == "子类失败主类" and item["sub_category"] == "成功子类" for item in batch_db.categories.values())

    import_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (import_db, object(), engine))
    with categories_route_app.test_request_context(
        "/api/categories/import",
        method="POST",
        json={
            "categories": [
                {"main_category": "交通", "sub_category": "公交", "type": 3},
                {"main_category": "交通", "sub_category": "", "description": "更新交通"},
            ]
        },
    ):
        payload = import_categories().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {"imported": 1, "updated": 1, "skipped": 0}

    parent_fallback_db = FakeCategoriesDB()
    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (parent_fallback_db, object(), engine))
    monkeypatch.setattr(parent_fallback_db, "get_category_by_name", lambda main_category, sub_category, user_id=0: None if not sub_category else FakeCategoriesDB.get_category_by_name(parent_fallback_db, main_category, sub_category, user_id))
    with categories_route_app.test_request_context("/api/categories/2"):
        payload = _unwrap_response(get_category("2")).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["parentId"] == "virtual_餐饮"

    with categories_route_app.test_request_context("/api/categories/3"):
        payload = _unwrap_response(get_category("3")).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["parentId"] == "0"

    class EmptyMatchEngine(FakeCategoryEngine):
        def match_category(self, bill: dict[str, Any]) -> tuple[str, str]:
            _ = bill
            return ("", "")

    monkeypatch.setattr(categories_module, "get_app_context", lambda user_id=None: (db, object(), EmptyMatchEngine()))
    with categories_route_app.test_request_context(
        "/api/categories/update-all",
        method="POST",
        json={"force": True},
    ):
        payload = recategorize_all_bills().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {"total": 2, "updated": 0}
