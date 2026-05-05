from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from collections.abc import Callable

import bcrypt
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
        self.last_display_orders: list[tuple[int, int]] = []

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
        return [
            dict(account)
            for account in self.accounts.values()
            if int(account.get("parent_id") or 0) == int(account_id)
        ]

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

    def update_account_display_orders(self, orders: list[tuple[int, int]], user_id: int = 0) -> bool:
        _ = user_id
        self.last_display_orders = list(orders)
        for account_id, display_order in orders:
            account = self.accounts.get(int(account_id))
            if account is not None:
                account["display_order"] = int(display_order)
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
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


def _identity_mapping(payload: dict[str, Any]) -> dict[str, Any]:
    return dict(payload)


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
    monkeypatch.setattr(accounts_module.account_adapter, "frontend_to_backend", _identity_mapping)
    monkeypatch.setattr(accounts_module.account_adapter, "backend_to_frontend", _identity_mapping)
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
        assert db.last_display_orders == [(2, 5)]

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


def test_accounts_routes_cover_helpers_and_error_handlers(
    accounts_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """账户路由应覆盖 helper 与主要 500 异常兜底。"""
    run_async_helper = getattr(accounts_module, "_run_async")
    get_request_user_id_helper = getattr(accounts_module, "_get_request_user_id")

    async def _sample_coroutine() -> str:
        return "ok"

    assert run_async_helper(_sample_coroutine()) == "ok"

    db = FakeAccountsDB()
    monkeypatch.setattr(accounts_module, "_run_async", lambda value: value)
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: db)
    monkeypatch.setattr(accounts_module, "_get_request_user_id", lambda: 9)
    monkeypatch.setattr(accounts_module.account_adapter, "frontend_to_backend", _identity_mapping)
    monkeypatch.setattr(accounts_module.account_adapter, "backend_to_frontend", _identity_mapping)
    monkeypatch.setattr(
        accounts_module.account_adapter,
        "format_list_response",
        lambda accounts, build_hierarchy_flag=True: {"success": True, "result": list(accounts)},
    )

    get_accounts = _unwrap(accounts_module.get_accounts)
    get_account = _unwrap(accounts_module.get_account)
    create_account = _unwrap(accounts_module.create_account)
    update_account = _unwrap(accounts_module.update_account)
    delete_account = _unwrap(accounts_module.delete_account)
    sync_all_balances = _unwrap(accounts_module.sync_all_balances)
    move_all_transactions = _unwrap(accounts_module.move_all_transactions_rest)
    clear_transactions = _unwrap(accounts_module.clear_all_transactions_by_account_rest)

    with accounts_route_app.test_request_context("/api/accounts/"):
        cast("Any", accounts_module.request).user_id = 12
        assert get_request_user_id_helper() == 12

    with accounts_route_app.app_context():
        accounts_route_app.config["DB_INSTANCE"] = db
        assert accounts_module.get_app_context() is db

    list_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        list_fail_db,
        "get_all_accounts",
        lambda *args, **kwargs: _raise_runtime_error("list boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: list_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/"):
        response, status = get_accounts()
        assert status == 500
        assert response.get_json()["error"] == "list boom"

    detail_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        detail_fail_db,
        "get_account_by_id",
        lambda *args, **kwargs: _raise_runtime_error("detail boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: detail_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/1"):
        response, status = get_account(1)
        assert status == 500
        assert response.get_json()["error"] == "detail boom"

    create_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        create_fail_db,
        "create_account",
        lambda *args, **kwargs: _raise_runtime_error("create boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: create_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/", method="POST", json={"name": "异常账户"}):
        response, status = create_account()
        assert status == 500
        assert response.get_json()["error"] == "create boom"

    update_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        update_fail_db,
        "update_account",
        lambda *args, **kwargs: _raise_runtime_error("update boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: update_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/1", method="PUT", json={"name": "异常更新"}):
        response, status = update_account(1)
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    delete_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        delete_fail_db,
        "delete_account",
        lambda *args, **kwargs: _raise_runtime_error("delete boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: delete_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/1", method="DELETE"):
        response, status = delete_account(1)
        assert status == 500
        assert response.get_json()["error"] == "delete boom"

    sync_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        sync_fail_db,
        "sync_all_account_balances",
        lambda *args, **kwargs: _raise_runtime_error("sync boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: sync_fail_db)
    with accounts_route_app.test_request_context("/api/accounts/sync-balances", method="POST"):
        response, status = sync_all_balances()
        assert status == 500
        assert response.get_json()["error"] == "sync boom"

    move_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        move_fail_db,
        "move_all_transactions",
        lambda *args, **kwargs: _raise_runtime_error("move boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: move_fail_db)
    with accounts_route_app.test_request_context(
        "/api/accounts/1/transactions/move",
        method="POST",
        json={"toAccountId": "2", "password": "ok"},
    ):
        response, status = move_all_transactions(1)
        assert status == 500
        assert response.get_json()["error"] == "move boom"

    clear_fail_db = FakeAccountsDB()
    monkeypatch.setattr(
        clear_fail_db,
        "delete_all_transactions_by_account",
        lambda *args, **kwargs: _raise_runtime_error("clear boom"),
    )
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: clear_fail_db)
    with accounts_route_app.test_request_context(
        "/api/accounts/2/transactions/clear",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = clear_transactions(2)
        assert status == 500
        assert response.get_json()["error"] == "clear boom"


def test_accounts_routes_cover_remaining_success_shapes_and_password_paths(
    accounts_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """账户路由应覆盖剩余成功分支形态与敏感密码校验回退路径。"""
    db = FakeAccountsDB()

    monkeypatch.setattr(accounts_module, "_run_async", lambda value: value)
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: db)
    monkeypatch.setattr(accounts_module, "_get_request_user_id", lambda: 11)
    monkeypatch.setattr(accounts_module.account_adapter, "frontend_to_backend", _identity_mapping)
    monkeypatch.setattr(accounts_module.account_adapter, "backend_to_frontend", _identity_mapping)
    monkeypatch.setattr(
        accounts_module.account_adapter,
        "format_list_response",
        lambda accounts, build_hierarchy_flag=True: {
            "success": True,
            "result": list(accounts),
            "build_hierarchy": build_hierarchy_flag,
        },
    )

    get_accounts = _unwrap(accounts_module.get_accounts)
    update_account = _unwrap(accounts_module.update_account)
    update_display_orders = _unwrap(accounts_module.update_account_display_orders)

    with accounts_route_app.test_request_context("/api/accounts/"):
        payload = get_accounts().get_json() or {}
        assert payload["success"] is True
        assert payload["build_hierarchy"] is True
        assert payload["result"][0]["name"] == "主账户"

    original_get_sub_accounts = db.get_sub_accounts
    monkeypatch.setattr(
        db,
        "get_sub_accounts",
        lambda account_id, user_id=0: [] if int(account_id) == 1 else original_get_sub_accounts(account_id, user_id),
    )

    with accounts_route_app.test_request_context(
        "/api/accounts/1",
        method="PUT",
        json={"name": "主账户-别名更新", "aliases": ["钱包", "现金"]},
    ):
        payload = update_account(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["aliases"] == ["钱包", "现金"]
        assert "subAccounts" not in payload["result"]

    ghost_db = FakeAccountsDB()
    monkeypatch.setattr(accounts_module, "get_app_context", lambda: ghost_db)
    monkeypatch.setattr(ghost_db, "get_account_by_id", lambda account_id, user_id=0: None)

    with accounts_route_app.test_request_context(
        "/api/accounts/1",
        method="PUT",
        json={"name": "主账户-空响应"},
    ):
        payload = update_account(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"] == {}

    monkeypatch.setattr(accounts_module, "get_app_context", lambda: db)
    with accounts_route_app.test_request_context(
        "/api/accounts/display-orders",
        method="PUT",
        json={"newDisplayOrders": [{"id": "oops", "displayOrder": "5"}]},
    ):
        response, status = update_display_orders()
        assert status == 500
        assert "invalid literal" in response.get_json()["error"]

    bcrypt_db = FakeAccountsDB()
    monkeypatch.setattr(
        bcrypt_db,
        "get_user_by_id",
        lambda user_id: {
            "password_hash": bcrypt.hashpw(b"secret", bcrypt.gensalt()).decode("utf-8"),
        },
        raising=False,
    )
    assert accounts_module._verify_sensitive_operation_password(bcrypt_db, 11, "secret") is True

    fallback_db = FakeAccountsDB()
    monkeypatch.setattr(
        fallback_db,
        "get_user_by_id",
        lambda user_id: {
            "password_hash": bcrypt.hashpw(b"another-secret", bcrypt.gensalt()).decode("utf-8"),
        },
        raising=False,
    )
    assert accounts_module._verify_sensitive_operation_password(fallback_db, 11, "ok") is True

    invalid_hash_db = FakeAccountsDB()
    monkeypatch.setattr(
        invalid_hash_db,
        "get_user_by_id",
        lambda user_id: {"password_hash": "not-a-bcrypt-hash"},
        raising=False,
    )
    assert accounts_module._verify_sensitive_operation_password(invalid_hash_db, 11, "ok") is True
