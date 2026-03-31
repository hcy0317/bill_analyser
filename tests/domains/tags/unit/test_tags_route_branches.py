from __future__ import annotations

from collections.abc import Callable
from typing import Any, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import tags as tags_module


@pytest.fixture(name="tags_route_app")
def tags_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct tags route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeTagsDB:
    """Synchronous DB stub when _run_async is patched to identity."""

    def __init__(self) -> None:
        self.tags: list[dict[str, Any]] = [
            {"id": 1, "name": "早餐", "color": "#FFAA00", "icon": "1"},
            {"id": 2, "name": "通勤", "color": "#00AAFF", "icon": "2"},
        ]
        self.display_order_success = True

    def get_all_tags(self, user_id: int = 0) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(tag) for tag in self.tags]

    def get_tag_by_id(self, tag_id: int, user_id: int = 0) -> dict[str, Any] | None:
        _ = user_id
        return next((dict(tag) for tag in self.tags if int(tag["id"]) == int(tag_id)), None)

    def create_tag(self, payload: dict[str, Any], user_id: int = 0) -> int:
        _ = user_id
        next_id = max(int(tag["id"]) for tag in self.tags) + 1 if self.tags else 1
        tag = {"id": next_id, **payload}
        self.tags.append(tag)
        return next_id

    def update_tag(self, tag_id: int, payload: dict[str, Any], user_id: int = 0) -> bool:
        _ = user_id
        for tag in self.tags:
            if int(tag["id"]) == int(tag_id):
                tag.update(payload)
                return True
        return False

    def delete_tag(self, tag_id: int, user_id: int = 0) -> bool:
        _ = user_id
        original_count = len(self.tags)
        self.tags = [tag for tag in self.tags if int(tag["id"]) != int(tag_id)]
        return len(self.tags) != original_count

    def update_tag_display_orders(self, orders: list[tuple[int, int]], user_id: int = 0) -> bool:
        _ = (orders, user_id)
        return self.display_order_success


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


def test_tag_routes_cover_crud_batch_and_display_order_branches(
    tags_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """标签路由应覆盖 CRUD、批量创建和显示顺序更新的主要分支。"""
    db = FakeTagsDB()
    monkeypatch.setattr(tags_module, "_run_async", lambda value: value)
    monkeypatch.setattr(tags_module, "get_app_context", lambda: db)
    monkeypatch.setattr(tags_module, "_get_request_user_id", lambda: 7)

    get_tags = _unwrap(tags_module.get_tags)
    get_tag = _unwrap(tags_module.get_tag)
    create_tag = _unwrap(tags_module.create_tag)
    update_tag = _unwrap(tags_module.update_tag)
    delete_tag = _unwrap(tags_module.delete_tag)
    create_tags_batch = _unwrap(tags_module.create_tags_batch)
    update_display_orders = _unwrap(tags_module.update_tag_display_orders_rest)

    with tags_route_app.test_request_context("/api/tags/"):
        payload = get_tags().get_json() or {}
        assert payload["success"] is True
        assert len(payload["result"]) == 2

    with tags_route_app.test_request_context("/api/tags/999"):
        response, status = get_tag(999)
        assert status == 404
        assert response.get_json()["error"] == "Tag not found"

    with tags_route_app.test_request_context("/api/tags/", method="POST", json={}):
        response, status = create_tag()
        assert status == 400
        assert response.get_json()["error"] == "name is required"

    with tags_route_app.test_request_context("/api/tags/", method="POST", json={"name": "预算标签"}):
        response, status = create_tag()
        assert status == 201
        assert response.get_json()["result"]["name"] == "预算标签"

    with tags_route_app.test_request_context(
        "/api/tags/3",
        method="PUT",
        data="null",
        content_type="application/json",
    ):
        response, status = update_tag(3)
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with tags_route_app.test_request_context("/api/tags/999", method="PUT", json={"name": "不存在"}):
        response, status = update_tag(999)
        assert status == 404
        assert response.get_json()["error"] == "Tag not found"

    with tags_route_app.test_request_context("/api/tags/3", method="PUT", json={"name": "预算标签-更新"}):
        payload = update_tag(3).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "预算标签-更新"

    with tags_route_app.test_request_context("/api/tags/999", method="DELETE"):
        response, status = delete_tag(999)
        assert status == 404
        assert response.get_json()["error"] == "Tag not found"

    with tags_route_app.test_request_context("/api/tags/batch", method="POST", json={"tags": "bad"}):
        response, status = create_tags_batch()
        assert status == 400
        assert "non-empty array" in response.get_json()["error"]

    with tags_route_app.test_request_context("/api/tags/batch", method="POST", json={"tags": [{}]}):
        response, status = create_tags_batch()
        assert status == 400
        assert "non-empty name" in response.get_json()["error"]

    with tags_route_app.test_request_context(
        "/api/tags/batch",
        method="POST",
        json={"tags": [{"name": "预算标签-更新"}], "skipExists": False},
    ):
        response, status = create_tags_batch()
        assert status == 409
        assert "Tag already exists" in response.get_json()["error"]

    with tags_route_app.test_request_context(
        "/api/tags/batch",
        method="POST",
        json={"tags": [{"name": "预算标签-更新"}, {"name": "批量新增"}], "skipExists": True},
    ):
        response, status = create_tags_batch()
        payload = response.get_json() or {}
        assert status == 201
        assert payload["success"] is True
        assert [item["name"] for item in payload["result"]] == ["预算标签-更新", "批量新增"]

    with tags_route_app.test_request_context("/api/tags/display-orders", method="PUT", json={}):
        response, status = update_display_orders()
        assert status == 400
        assert response.get_json()["error"] == "newDisplayOrders is required"

    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": "bad"},
    ):
        response, status = update_display_orders()
        assert status == 400
        assert response.get_json()["error"] == "newDisplayOrders must be an array"

    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "1"}]},
    ):
        response, status = update_display_orders()
        assert status == 400
        assert "displayOrder" in response.get_json()["error"]

    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "x", "displayOrder": "1"}]},
    ):
        response, status = update_display_orders()
        assert status == 400
        assert "Invalid id or displayOrder" in response.get_json()["error"]

    db.display_order_success = False
    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": "2"}]},
    ):
        response, status = update_display_orders()
        assert status == 500
        assert response.get_json()["error"] == "Failed to update display orders"

    db.display_order_success = True
    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": "2"}]},
    ):
        payload = update_display_orders().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True


def test_tag_routes_cover_helpers_success_paths_and_error_handlers(
    tags_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """标签路由应覆盖 helper、本体成功路径与异常兜底。"""
    run_async_helper = getattr(tags_module, "_run_async")
    get_request_user_id_helper = getattr(tags_module, "_get_request_user_id")

    async def _sample_coroutine() -> str:
        return "ok"

    assert run_async_helper(_sample_coroutine()) == "ok"

    db = FakeTagsDB()
    monkeypatch.setattr(tags_module, "_run_async", lambda value: value)
    monkeypatch.setattr(tags_module, "get_app_context", lambda: db)
    monkeypatch.setattr(tags_module, "_get_request_user_id", lambda: 9)

    get_tags = _unwrap(tags_module.get_tags)
    get_tag = _unwrap(tags_module.get_tag)
    create_tag = _unwrap(tags_module.create_tag)
    update_tag = _unwrap(tags_module.update_tag)
    delete_tag = _unwrap(tags_module.delete_tag)
    create_tags_batch = _unwrap(tags_module.create_tags_batch)
    update_display_orders = _unwrap(tags_module.update_tag_display_orders_rest)

    with tags_route_app.test_request_context("/api/tags/"):
        cast("Any", tags_module.request).user_id = 12
        assert get_request_user_id_helper() == 12

    with tags_route_app.app_context():
        tags_route_app.config["DB_INSTANCE"] = db
        assert tags_module.get_app_context() is db

    with tags_route_app.test_request_context("/api/tags/1"):
        payload = get_tag(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 1

    with tags_route_app.test_request_context("/api/tags/1", method="DELETE"):
        payload = delete_tag(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True

    list_fail_db = FakeTagsDB()
    monkeypatch.setattr(list_fail_db, "get_all_tags", lambda *args, **kwargs: _raise_runtime_error("list boom"))
    monkeypatch.setattr(tags_module, "get_app_context", lambda: list_fail_db)
    with tags_route_app.test_request_context("/api/tags/"):
        response, status = get_tags()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    detail_fail_db = FakeTagsDB()
    monkeypatch.setattr(
        detail_fail_db,
        "get_tag_by_id",
        lambda *args, **kwargs: _raise_runtime_error("detail boom"),
    )
    monkeypatch.setattr(tags_module, "get_app_context", lambda: detail_fail_db)
    with tags_route_app.test_request_context("/api/tags/1"):
        response, status = get_tag(1)
        assert status == 500
        assert response.get_json()["error"] == "detail boom"

    create_fail_db = FakeTagsDB()
    monkeypatch.setattr(create_fail_db, "create_tag", lambda *args, **kwargs: _raise_runtime_error("create boom"))
    monkeypatch.setattr(tags_module, "get_app_context", lambda: create_fail_db)
    with tags_route_app.test_request_context("/api/tags/", method="POST", json={"name": "异常标签"}):
        response, status = create_tag()
        assert status == 500
        assert response.get_json()["error"] == "create boom"

    update_fail_db = FakeTagsDB()
    monkeypatch.setattr(update_fail_db, "update_tag", lambda *args, **kwargs: _raise_runtime_error("update boom"))
    monkeypatch.setattr(tags_module, "get_app_context", lambda: update_fail_db)
    with tags_route_app.test_request_context("/api/tags/2", method="PUT", json={"name": "异常更新"}):
        response, status = update_tag(2)
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    delete_fail_db = FakeTagsDB()
    monkeypatch.setattr(delete_fail_db, "delete_tag", lambda *args, **kwargs: _raise_runtime_error("delete boom"))
    monkeypatch.setattr(tags_module, "get_app_context", lambda: delete_fail_db)
    with tags_route_app.test_request_context("/api/tags/2", method="DELETE"):
        response, status = delete_tag(2)
        assert status == 500
        assert response.get_json()["error"] == "delete boom"

    batch_fail_db = FakeTagsDB()
    monkeypatch.setattr(batch_fail_db, "create_tag", lambda *args, **kwargs: _raise_runtime_error("batch boom"))
    monkeypatch.setattr(tags_module, "get_app_context", lambda: batch_fail_db)
    with tags_route_app.test_request_context(
        "/api/tags/batch",
        method="POST",
        json={"tags": [{"name": "全新标签"}]},
    ):
        response, status = create_tags_batch()
        assert status == 500
        assert response.get_json()["error"] == "batch boom"

    order_fail_db = FakeTagsDB()
    monkeypatch.setattr(
        order_fail_db,
        "update_tag_display_orders",
        lambda *args, **kwargs: _raise_runtime_error("order boom"),
    )
    monkeypatch.setattr(tags_module, "get_app_context", lambda: order_fail_db)
    with tags_route_app.test_request_context(
        "/api/tags/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": "3"}]},
    ):
        response, status = update_display_orders()
        assert status == 500
        assert response.get_json()["error"] == "order boom"


def test_tag_batch_create_continues_when_created_tag_lookup_returns_none(
    tags_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """批量创建标签时，即使某次创建后详情回查为空，也应继续处理后续标签。"""
    db = FakeTagsDB()
    missing_lookup_ids: set[int] = set()

    original_create_tag = db.create_tag
    original_get_tag_by_id = db.get_tag_by_id

    def create_tag_and_mark_missing(payload: dict[str, Any], user_id: int = 0) -> int:
        tag_id = original_create_tag(payload, user_id=user_id)
        if payload.get("name") == "空回查标签":
            missing_lookup_ids.add(tag_id)
        return tag_id

    def get_tag_by_id_with_gap(tag_id: int, user_id: int = 0) -> dict[str, Any] | None:
        if int(tag_id) in missing_lookup_ids:
            return None
        return original_get_tag_by_id(tag_id, user_id=user_id)

    monkeypatch.setattr(tags_module, "_run_async", lambda value: value)
    monkeypatch.setattr(tags_module, "get_app_context", lambda: db)
    monkeypatch.setattr(tags_module, "_get_request_user_id", lambda: 13)
    monkeypatch.setattr(db, "create_tag", create_tag_and_mark_missing)
    monkeypatch.setattr(db, "get_tag_by_id", get_tag_by_id_with_gap)

    create_tags_batch = _unwrap(tags_module.create_tags_batch)

    with tags_route_app.test_request_context(
        "/api/tags/batch",
        method="POST",
        json={"tags": [{"name": "空回查标签"}, {"name": "正常标签"}]},
    ):
        response, status = create_tags_batch()
        payload = response.get_json() or {}
        assert status == 201
        assert payload["success"] is True
        assert [item["name"] for item in payload["result"]] == ["正常标签"]
