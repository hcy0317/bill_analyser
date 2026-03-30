from __future__ import annotations

from typing import Any, Callable, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import accounts as accounts_module


@pytest.fixture(name="accounts_route_app")
def accounts_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct accounts route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeAccountsDB:
    """Synchronous DB stub when _run_async is patched to identity."""

    def __init__(self) -> None:
        self.accounts: dict[int, dict[str, Any]] = {
            1: {"id": 1, "name": "主账户", "display_order": 1},
            2: {"id": 2, "name": "旧子账户", "parent_id": 1, "display_order": 2},
            3: {"id": 3, "name": "待删除子账户", "parent_id": 1, "display_order": 3},
        }
        self.return_none_accounts = False
        self.password_valid = True
        self.move_result: dict[str, Any] = {"success": True, "moved_count": 2}
        self.clear_result: dict[str, Any] = {"success": True, "deleted_count": 3}

    def get_all_accounts(self, user_id: int = 0) -> list[dict[str, Any]] | None:
        _ = user_id
        if self.return_none_accounts:
            return None
        return [dict(account) for account in self.accounts.values() if account.get("parent_id") is None]

    def get_account_by_id(self, account_id: int, user_id: int = 0) -> dict[str, Any] | None:
        _ = user_id
        account = self.accounts.get(int(account_id))
        return dict(account) if account else None

    def get_sub_accounts(self, account_id: int, user_id: int = 0) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(account) for account in self.accounts.values() if int(account.get("parent_id") or 0) == int(account_id)]

    def create_account(self, payload: dict[str, Any], user_id: int = 0) -> int:
        _ = user_id
        next_id = max(self.accounts) + 1 if self.accounts else 1
        self.accounts[next_id] = {"id": next_id, **payload}
        return next_id

    def update_account(self, account_id: int, payload: dict[str, Any], user_id: int = 0) -> bool:
        _ = user_id
        account = self.accounts.get(int(account_id))
        if account is None:
            return False
        account.update(payload)
        return True

    def delete_account(self, account_id: int, user_id: int = 0) -> bool:
        _ = user_id
        return self.accounts.pop(int(account_id), None) is not None

    def sync_all_account_balances(self, user_id: int = 0) -> dict[str, Any]:
        _ = user_id
        return {
            "total_accounts": 2,
            "synced_accounts": 2,
            "discrepancies": [{"account_id": 1, "old_balance": 100, "new_balance": 120, "diff": 20}],
            "errors": [],
        }

    def verify_operation_password(self, password: str) -> bool:
        return self.password_valid and password == "ok"

    def create_audit_log(self, **payload: Any) -> None:
        _ = payload

    def move_all_transactions(self, _from_account: int, _to_account: int, user_id: int = 0) -> dict[str, Any]:
        _ = user_id
        return dict(self.move_result)

    def delete_all_transactions_by_account(self, _account_id: int, user_id: int = 0) -> dict[str, Any]:
        _ = user_id
        return dict(self.clear_result)


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast(Any, func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast(Callable[..., Any], current)

    second = getattr(first, "__wrapped__", None)
    return cast(Callable[..., Any], second or first)


def test_accounts_routes_cover_crud_sort_sync_and_transaction_actions(
    accounts_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """账户路由应覆盖 CRUD、排序、同步余额、迁移交易与清空交易分支。"""
    db = FakeAccountsDB()

    def format_list_response(accounts: list[dict[str, Any]], build_hierarchy_flag: bool = True) -> dict[str, Any]:
        return {
            "success": True,
            "result": list(accounts),
            "build_hierarchy": build_hierarchy_flag,
        }

    monkeypatch.setattr(accounts_module, "_run_async", lambda value: value)
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: db)
    monkeypatch.setattr(accounts_module, "_get_request_user_id", lambda: 7)
    monkeypatch.setattr(accounts_module.account_adapter, "frontend_to_backend", lambda payload: dict(payload))
    monkeypatch.setattr(accounts_module.account_adapter, "backend_to_frontend", lambda payload: dict(payload))
    monkeypatch.setattr(accounts_module.account_adapter, "format_list_response", format_list_response)

    get_accounts = _unwrap(accounts_module.get_accounts)
    get_account = _unwrap(accounts_module.get_account)
    create_account = _unwrap(accounts_module.create_account)
    update_account = _unwrap(accounts_module.update_account)
    delete_account = _unwrap(accounts_module.delete_account)
    update_display_orders = _unwrap(accounts_module.update_account_display_orders)
    sync_all_balances = _unwrap(accounts_module.sync_all_balances)
    move_all_transactions = _unwrap(accounts_module.move_all_transactions_rest)
    clear_transactions = _unwrap(accounts_module.clear_all_transactions_by_account_rest)

    db.return_none_accounts = True
    with accounts_route_app.test_request_context("/api/accounts/"):
        payload = get_accounts().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == []
    db.return_none_accounts = False

    with accounts_route_app.test_request_context("/api/accounts/999"):
        response, status = get_account(999)
        assert status == 404
        assert response.get_json()["error"] == "Account not found"

    with accounts_route_app.test_request_context("/api/accounts/1"):
        payload = get_account(1).get_json() or {}
        assert payload["success"] is True
        assert len(payload["result"]["subAccounts"]) == 2

    with accounts_route_app.test_request_context("/api/accounts/", method="POST", json={}):
        response, status = create_account()
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with accounts_route_app.test_request_context(
        "/api/accounts/",
        method="POST",
        json={"name": "新账户"},
    ):
        response, status = create_account()
        payload = response.get_json() or {}
        assert status == 201
        assert payload["result"]["name"] == "新账户"

    with accounts_route_app.test_request_context(
        "/api/accounts/1",
        method="PUT",
        data="null",
        content_type="application/json",
    ):
        response, status = update_account(1)
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    with accounts_route_app.test_request_context("/api/accounts/999", method="PUT", json={"name": "missing"}):
        response, status = update_account(999)
        assert status == 404
        assert response.get_json()["error"] == "Account not found"

    with accounts_route_app.test_request_context(
        "/api/accounts/1",
        method="PUT",
        json={
            "name": "主账户-更新",
            "subAccounts": [
                {"id": 2, "name": "旧子账户-更新"},
                {"name": "新增子账户"},
            ],
        },
    ):
        payload = update_account(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["name"] == "主账户-更新"
        assert db.accounts[2]["name"] == "旧子账户-更新"
        assert 3 not in db.accounts

    with accounts_route_app.test_request_context("/api/accounts/999", method="DELETE"):
        response, status = delete_account(999)
        assert status == 404
        assert response.get_json()["error"] == "Account not found"

    with accounts_route_app.test_request_context("/api/accounts/1", method="DELETE"):
        payload = delete_account(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True

    with accounts_route_app.test_request_context("/api/accounts/display-orders", method="PUT", json={}):
        response, status = update_display_orders()
        assert status == 400
        assert response.get_json()["error"] == "Missing newDisplayOrders parameter"

    with accounts_route_app.test_request_context(
        "/api/accounts/display-orders",
        method="PUT",
        json={"newDisplayOrders": "bad"},
    ):
        response, status = update_display_orders()
        assert status == 400
        assert response.get_json()["error"] == "newDisplayOrders must be a list"

    with accounts_route_app.test_request_context(
        "/api/accounts/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "1"}]},
    ):
        response, status = update_display_orders()
        assert status == 400
        assert "displayOrder" in response.get_json()["error"]

    with accounts_route_app.test_request_context(
        "/api/accounts/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "2", "displayOrder": "5"}]},
    ):
        payload = update_display_orders().get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True

    with accounts_route_app.test_request_context("/api/accounts/sync-balances", method="POST"):
        payload = sync_all_balances().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["synced_accounts"] == 2

    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = move_all_transactions(1)
        assert status == 400
        assert response.get_json()["error"] == "toAccountId is required"

    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "2"},
    ):
        response, status = move_all_transactions(1)
        assert status == 400
        assert response.get_json()["error"] == "password is required"

    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "oops", "password": "ok"},
    ):
        response, status = move_all_transactions(1)
        assert status == 400
        assert "valid integers" in response.get_json()["error"]

    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "1", "password": "ok"},
    ):
        response, status = move_all_transactions(1)
        assert status == 400
        assert "must be different" in response.get_json()["error"]

    db.password_valid = False
    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "2", "password": "bad"},
    ):
        response, status = move_all_transactions(1)
        assert status == 401
        assert response.get_json()["error"] == "Invalid password"

    db.password_valid = True
    db.move_result = {"success": False, "message": "move failed"}
    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "2", "password": "ok"},
    ):
        response, status = move_all_transactions(1)
        assert status == 500
        assert response.get_json()["error"] == "move failed"

    db.move_result = {"success": True, "moved_count": 4}
    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "2", "password": "ok"},
    ):
        payload = move_all_transactions(1).get_json() or {}
        assert payload["success"] is True
        assert payload["moved_count"] == 4

    with accounts_route_app.test_request_context(
        "/api/accounts/2/transactions/clear",
        method="POST",
        json={},
    ):
        response, status = clear_transactions(2)
        assert status == 400
        assert response.get_json()["error"] == "password is required"

    db.password_valid = False
    with accounts_route_app.test_request_context(
        "/api/accounts/2/transactions/clear",
        method="POST",
        json={"password": "bad"},
    ):
        response, status = clear_transactions(2)
        assert status == 401
        assert response.get_json()["error"] == "Invalid password"

    db.password_valid = True
    db.clear_result = {"success": False, "message": "clear failed"}
    with accounts_route_app.test_request_context(
        "/api/accounts/2/transactions/clear",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = clear_transactions(2)
        assert status == 500
        assert response.get_json()["error"] == "clear failed"

    db.clear_result = {"success": True, "deleted_count": 5}
    with accounts_route_app.test_request_context(
        "/api/accounts/2/transactions/clear",
        method="POST",
        json={"password": "ok"},
    ):
        payload = clear_transactions(2).get_json() or {}
        assert payload["success"] is True
        assert payload["deleted_count"] == 5
