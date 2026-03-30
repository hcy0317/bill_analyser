from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from collections.abc import Callable

import pytest
from flask import Flask

from bill_analyser.api.routes import templates as templates_module


@pytest.fixture(name="templates_route_app")
def templates_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct templates route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeTemplatesDB:
    """Synchronous DB stub when _run_async is patched to identity."""

    def __init__(self) -> None:
        self.templates: list[dict[str, Any]] = [
            {"id": 1, "name": "早餐模板", "templateType": 1, "sourceAmount": 1200, "hidden": False},
            {"id": 2, "name": "工资模板", "templateType": 2, "sourceAmount": 880000, "hidden": False},
        ]
        self.display_order_result = True
        self.return_template_on_lookup = True

    def get_all_templates(self, user_id: int = 0, template_type: int | None = None) -> list[dict[str, Any]]:
        _ = user_id
        if template_type is None:
            return [dict(template) for template in self.templates]
        return [dict(template) for template in self.templates if int(template["templateType"]) == int(template_type)]

    def get_template_by_id(
        self,
        template_id: int,
        user_id: int = 0,
        template_type: int | None = None,
    ) -> dict[str, Any] | None:
        _ = user_id
        template = next((template for template in self.templates if int(template["id"]) == int(template_id)), None)
        if template is None:
            return None
        if template_type is not None and int(template["templateType"]) != int(template_type):
            return None
        return dict(template) if self.return_template_on_lookup else None

    def create_template(self, payload: dict[str, Any], user_id: int = 0) -> int:
        _ = user_id
        next_id = max(int(template["id"]) for template in self.templates) + 1 if self.templates else 1
        self.templates.append({"id": next_id, **payload})
        return next_id

    def update_template(
        self,
        template_id: int,
        payload: dict[str, Any],
        user_id: int = 0,
        template_type: int | None = None,
    ) -> bool:
        _ = (user_id, template_type)
        for template in self.templates:
            if int(template["id"]) == int(template_id):
                template.update(payload)
                return True
        return False

    def delete_template(self, template_id: int, user_id: int = 0, template_type: int | None = None) -> bool:
        _ = (user_id, template_type)
        original_count = len(self.templates)
        self.templates = [template for template in self.templates if int(template["id"]) != int(template_id)]
        return len(self.templates) != original_count

    def update_template_display_orders(
        self,
        orders: list[tuple[int, int]],
        template_type: int | None = None,
        user_id: int = 0,
    ) -> bool:
        _ = (orders, template_type, user_id)
        return self.display_order_result


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


def test_template_routes_cover_helper_crud_and_display_order_branches(
    templates_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """模板路由应覆盖模板类型解析、CRUD 与排序分支。"""
    get_template_type_helper = getattr(templates_module, "_get_template_type")
    db = FakeTemplatesDB()
    monkeypatch.setattr(templates_module, "_run_async", lambda value: value)
    monkeypatch.setattr(templates_module, "get_app_context", lambda: db)
    monkeypatch.setattr(templates_module, "_get_request_user_id", lambda: 7)

    get_templates = _unwrap(templates_module.get_templates)
    get_template = _unwrap(templates_module.get_template)
    create_template = _unwrap(templates_module.create_template)
    update_template = _unwrap(templates_module.update_template)
    delete_template = _unwrap(templates_module.delete_template)
    update_display_orders = _unwrap(templates_module.update_template_display_orders)

    with templates_route_app.test_request_context("/api/templates/?templateType=2"):
        assert get_template_type_helper() == 2
    with templates_route_app.test_request_context("/api/templates/", method="POST", json={"templateType": "bad"}):
        assert get_template_type_helper(default=1) == 1

    with templates_route_app.test_request_context("/api/templates/?templateType=1"):
        payload = get_templates().get_json() or {}
        assert payload["success"] is True
        assert [item["id"] for item in payload["result"]] == [1]

    with templates_route_app.test_request_context("/api/templates/999?templateType=1"):
        response, status = get_template(999)
        assert status == 404
        assert response.get_json()["error"] == "Template not found"

    with templates_route_app.test_request_context("/api/templates/", method="POST", json={}):
        response, status = create_template()
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    db.return_template_on_lookup = False
    with templates_route_app.test_request_context(
        "/api/templates/",
        method="POST",
        json={"templateType": 1, "name": "新模板", "sourceAmount": 1000},
    ):
        response, status = create_template()
        payload = response.get_json() or {}
        assert status == 201
        assert payload["result"]["id"] == "3"

    db.return_template_on_lookup = True
    with templates_route_app.test_request_context(
        "/api/templates/3?templateType=1",
        method="PUT",
        data="null",
        content_type="application/json",
    ):
        response, status = update_template(3)
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with templates_route_app.test_request_context(
        "/api/templates/999?templateType=1",
        method="PUT",
        json={"templateType": 1, "name": "missing"},
    ):
        response, status = update_template(999)
        assert status == 404
        assert response.get_json()["error"] == "Template not found"

    with templates_route_app.test_request_context(
        "/api/templates/3?templateType=1",
        method="PUT",
        json={"templateType": 1, "name": "新模板-更新", "sourceAmount": 2222},
    ):
        payload = update_template(3).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "新模板-更新"

    with templates_route_app.test_request_context("/api/templates/999?templateType=1", method="DELETE"):
        response, status = delete_template(999)
        assert status == 404
        assert response.get_json()["error"] == "Template not found"

    with templates_route_app.test_request_context("/api/templates/display-orders", method="PUT", json={}):
        response, status = update_display_orders()
        assert status == 400
        assert response.get_json()["error"] == "Missing newDisplayOrders"

    with templates_route_app.test_request_context(
        "/api/templates/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "oops", "displayOrder": "1"}]},
    ):
        response, status = update_display_orders()
        assert status == 500
        assert "invalid literal" in response.get_json()["error"]

    db.display_order_result = True
    with templates_route_app.test_request_context(
        "/api/templates/display-orders",
        method="PUT",
        json={"templateType": 1, "newDisplayOrders": [{"id": "1", "displayOrder": "2"}]},
    ):
        payload = update_display_orders().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True


def test_template_routes_cover_helpers_success_paths_and_error_handlers(
    templates_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """模板路由应覆盖 helper、本体成功路径与异常兜底。"""
    run_async_helper = getattr(templates_module, "_run_async")
    get_template_type_helper = getattr(templates_module, "_get_template_type")
    get_request_user_id_helper = getattr(templates_module, "_get_request_user_id")

    async def _sample_coroutine() -> str:
        return "ok"

    assert run_async_helper(_sample_coroutine()) == "ok"

    db = FakeTemplatesDB()
    monkeypatch.setattr(templates_module, "_run_async", lambda value: value)
    monkeypatch.setattr(templates_module, "get_app_context", lambda: db)
    monkeypatch.setattr(templates_module, "_get_request_user_id", lambda: 9)

    get_templates = _unwrap(templates_module.get_templates)
    get_template = _unwrap(templates_module.get_template)
    create_template = _unwrap(templates_module.create_template)
    update_template = _unwrap(templates_module.update_template)
    delete_template = _unwrap(templates_module.delete_template)
    update_display_orders = _unwrap(templates_module.update_template_display_orders)

    with templates_route_app.test_request_context("/api/templates/"):
        assert get_template_type_helper(default=7) == 7
        cast("Any", templates_module.request).user_id = 12
        assert get_request_user_id_helper() == 12

    with templates_route_app.app_context():
        templates_route_app.config["DB_INSTANCE"] = db
        assert templates_module.get_app_context() is db

    with templates_route_app.test_request_context("/api/templates/1?templateType=1"):
        payload = get_template(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 1

    with templates_route_app.test_request_context(
        "/api/templates/1?templateType=1",
        method="DELETE",
    ):
        payload = delete_template(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True

    list_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(list_fail_db, "get_all_templates", lambda *args, **kwargs: _raise_runtime_error("list boom"))
    monkeypatch.setattr(templates_module, "get_app_context", lambda: list_fail_db)
    with templates_route_app.test_request_context("/api/templates/?templateType=1"):
        response, status = get_templates()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    detail_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(
        detail_fail_db,
        "get_template_by_id",
        lambda *args, **kwargs: _raise_runtime_error("detail boom"),
    )
    monkeypatch.setattr(templates_module, "get_app_context", lambda: detail_fail_db)
    with templates_route_app.test_request_context("/api/templates/1?templateType=1"):
        response, status = get_template(1)
        assert status == 500
        assert response.get_json()["error"] == "detail boom"

    create_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(
        create_fail_db,
        "create_template",
        lambda *args, **kwargs: _raise_runtime_error("create boom"),
    )
    monkeypatch.setattr(templates_module, "get_app_context", lambda: create_fail_db)
    with templates_route_app.test_request_context(
        "/api/templates/",
        method="POST",
        json={"templateType": 1, "name": "异常模板"},
    ):
        response, status = create_template()
        assert status == 500
        assert response.get_json()["error"] == "create boom"

    update_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(
        update_fail_db,
        "update_template",
        lambda *args, **kwargs: _raise_runtime_error("update boom"),
    )
    monkeypatch.setattr(templates_module, "get_app_context", lambda: update_fail_db)
    with templates_route_app.test_request_context(
        "/api/templates/2?templateType=2",
        method="PUT",
        json={"templateType": 2, "name": "异常更新"},
    ):
        response, status = update_template(2)
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    delete_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(
        delete_fail_db,
        "delete_template",
        lambda *args, **kwargs: _raise_runtime_error("delete boom"),
    )
    monkeypatch.setattr(templates_module, "get_app_context", lambda: delete_fail_db)
    with templates_route_app.test_request_context("/api/templates/2?templateType=2", method="DELETE"):
        response, status = delete_template(2)
        assert status == 500
        assert response.get_json()["error"] == "delete boom"

    order_fail_db = FakeTemplatesDB()
    monkeypatch.setattr(
        order_fail_db,
        "update_template_display_orders",
        lambda *args, **kwargs: _raise_runtime_error("order boom"),
    )
    monkeypatch.setattr(templates_module, "get_app_context", lambda: order_fail_db)
    with templates_route_app.test_request_context(
        "/api/templates/display-orders",
        method="PUT",
        json={"templateType": 1, "newDisplayOrders": [{"id": "1", "displayOrder": "3"}]},
    ):
        response, status = update_display_orders()
        assert status == 500
        assert response.get_json()["error"] == "order boom"
