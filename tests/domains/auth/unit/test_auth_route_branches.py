from __future__ import annotations

import asyncio
import io
from typing import Any

import bcrypt
import pytest
from flask import Flask

from bill_analyser.api.routes import auth as auth_module


@pytest.fixture(name="auth_route_unit_app")
def auth_route_unit_app_fixture() -> Flask:
    """Create a tiny Flask app for direct auth route unit tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeLoop:
    """Minimal event-loop adapter for direct route invocation tests."""

    def __init__(self) -> None:
        self.closed = False

    def run_until_complete(self, coroutine):
        return asyncio.run(coroutine)

    def close(self) -> None:
        self.closed = True

    def is_closed(self) -> bool:
        return self.closed


class FakeAuthDB:
    """Minimal async DB stub for auth route branches."""

    def __init__(
        self,
        *,
        user_by_id: dict[str, Any] | None = None,
        user_by_email: dict[str, Any] | None = None,
        session: dict[str, Any] | None = None,
        cloud_settings: list[dict[str, Any]] | None = None,
    ) -> None:
        self.user_by_id = user_by_id
        self.user_by_email = user_by_email
        self.user_by_username = None if user_by_id is None else dict(user_by_id)
        self.session = session
        self.cloud_settings = cloud_settings or []
        self.operation_password_valid = True
        self.user_data_statistics = {
            "billCount": 3,
            "accountCount": 2,
            "categoryCount": 4,
            "tagCount": 1,
            "templateCount": 5,
        }
        self.export_categories: list[dict[str, Any]] = []
        self.export_bills: list[dict[str, Any]] = []
        self.export_accounts: list[dict[str, Any]] = []
        self.export_tags_map: dict[int, list[dict[str, Any]]] = {}
        self.clear_transactions_result: dict[str, Any] = {"success": True, "deleted_count": 2}
        self.clear_all_result: dict[str, Any] = {"success": True, "counts": {"bills": 2, "accounts": 1}}
        self.invalidate_session_by_id_result = True
        self.invalidate_other_count = 2
        self.user_sessions: list[dict[str, Any]] = []
        self.updated_users: list[tuple[int, dict[str, Any]]] = []
        self.updated_cloud_settings: list[tuple[int, list[dict[str, Any]], bool]] = []
        self.deleted_cloud_settings: list[int] = []
        self.auth_logs: list[dict[str, Any]] = []
        self.invalidated_tokens: list[str] = []
        self.created_sessions: list[dict[str, Any]] = []
        self.created_users: list[dict[str, Any]] = []
        self.failed_login_increments: list[tuple[int, int]] = []
        self.last_login_updates: list[tuple[int, str]] = []
        self.user_locked = False
        self.user_external_auths: list[dict[str, Any]] = []
        self.deleted_external_auths: list[tuple[int, str]] = []
        self.replaced_recovery_codes: list[tuple[int, list[str]]] = []
        self.cleared_recovery_codes: list[int] = []
        self.recovery_codes_by_user: dict[int, list[str]] = {}

    async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
        if self.user_by_id is None:
            return None
        if self.user_by_id.get("id") == user_id:
            return self.user_by_id
        return None

    async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
        self.updated_users.append((user_id, data))
        if self.user_by_id is not None and self.user_by_id.get("id") == user_id:
            self.user_by_id.update(data)
        return True

    async def create_auth_log(self, payload: dict[str, Any]) -> None:
        self.auth_logs.append(payload)

    async def get_user_by_email(self, email: str) -> dict[str, Any] | None:
        if self.user_by_email is None:
            return None
        if (self.user_by_email.get("email") or "").strip() == email:
            return self.user_by_email
        return None

    async def get_user_by_username(self, username: str) -> dict[str, Any] | None:
        if self.user_by_username is None:
            return None
        if (self.user_by_username.get("username") or "").strip() == username:
            return self.user_by_username
        return None

    async def get_user_application_cloud_settings(self, _user_id: int) -> list[dict[str, Any]]:
        return self.cloud_settings

    async def get_session_by_token_hash(self, token_hash: str) -> dict[str, Any] | None:
        self.invalidated_tokens.append(f"lookup:{token_hash}")
        return self.session

    async def invalidate_session(self, token_hash: str) -> None:
        self.invalidated_tokens.append(token_hash)

    async def create_session(self, payload: dict[str, Any]) -> int:
        self.created_sessions.append(payload)
        return 101

    async def create_user(self, payload: dict[str, Any]) -> int:
        self.created_users.append(payload)
        new_id = 77
        self.user_by_id = {"id": new_id, **payload}
        self.user_by_username = dict(self.user_by_id)
        self.user_by_email = dict(self.user_by_id)
        return new_id

    async def create_audit_log(self, **payload: Any) -> None:
        self.auth_logs.append(payload)

    async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
        self.replaced_recovery_codes.append((user_id, list(recovery_codes)))
        self.recovery_codes_by_user[user_id] = [str(code).strip().upper() for code in recovery_codes]
        return len(recovery_codes)

    async def consume_two_factor_recovery_code(self, user_id: int, recovery_code: str) -> bool:
        normalized_code = str(recovery_code).strip().upper()
        active_codes = self.recovery_codes_by_user.get(user_id, [])
        if normalized_code not in active_codes:
            return False

        active_codes.remove(normalized_code)
        self.recovery_codes_by_user[user_id] = active_codes
        return True

    async def clear_two_factor_recovery_codes(self, user_id: int) -> int:
        active_count = len(self.recovery_codes_by_user.get(user_id, []))
        self.cleared_recovery_codes.append(user_id)
        self.recovery_codes_by_user[user_id] = []
        return active_count

    async def count_active_two_factor_recovery_codes(self, user_id: int) -> int:
        return len(self.recovery_codes_by_user.get(user_id, []))

    async def get_user_data_statistics(self, user_id: int) -> dict[str, Any]:
        _ = user_id
        return dict(self.user_data_statistics)

    async def get_all_categories(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.export_categories]

    async def get_bills(self, filters: dict[str, Any], user_id: int) -> list[dict[str, Any]]:
        _ = (filters, user_id)
        return [dict(item) for item in self.export_bills]

    async def get_all_accounts(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.export_accounts]

    async def get_tags_for_bills(self, bill_ids: list[int], user_id: int) -> dict[int, list[dict[str, Any]]]:
        _ = (bill_ids, user_id)
        return {bill_id: [dict(tag) for tag in self.export_tags_map.get(bill_id, [])] for bill_id in bill_ids}

    async def verify_operation_password(self, password: str) -> bool:
        return self.operation_password_valid and password == "ok"

    async def clear_user_transactions(self, user_id: int) -> dict[str, Any]:
        _ = user_id
        return dict(self.clear_transactions_result)

    async def clear_user_data(self, user_id: int) -> dict[str, Any]:
        _ = user_id
        return dict(self.clear_all_result)

    async def is_user_locked(self, user_id: int) -> bool:
        _ = user_id
        return self.user_locked

    async def increment_failed_login(self, user_id: int, lockout_minutes: int) -> None:
        self.failed_login_increments.append((user_id, lockout_minutes))

    async def update_user_last_login(self, user_id: int, ip_address: str) -> None:
        self.last_login_updates.append((user_id, ip_address))

    async def invalidate_session_by_id(self, session_id: int, user_id: int) -> bool:
        _ = (session_id, user_id)
        return self.invalidate_session_by_id_result

    async def invalidate_other_user_sessions(self, user_id: int, session_id: int) -> int:
        _ = (user_id, session_id)
        return self.invalidate_other_count

    async def cleanup_expired_sessions(self) -> None:
        return None

    async def get_user_sessions(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.user_sessions]

    async def get_user_external_auths(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.user_external_auths]

    async def get_user_external_auth(self, user_id: int, external_auth_type: str) -> dict[str, Any] | None:
        _ = user_id
        for item in self.user_external_auths:
            if item.get("external_auth_type") == external_auth_type:
                return dict(item)
        return None

    async def delete_user_external_auth(self, user_id: int, external_auth_type: str) -> bool:
        _ = user_id
        self.deleted_external_auths.append((user_id, external_auth_type))
        self.user_external_auths = [
            item for item in self.user_external_auths if item.get("external_auth_type") != external_auth_type
        ]
        return True

    async def delete_user_application_cloud_settings(self, user_id: int) -> None:
        self.deleted_cloud_settings.append(user_id)
        self.cloud_settings = []

    async def update_user_application_cloud_settings(
        self,
        user_id: int,
        settings: list[dict[str, Any]],
        full_update: bool = False,
    ) -> None:
        self.updated_cloud_settings.append((user_id, settings, full_update))
        self.cloud_settings = settings



def _install_fake_loop(monkeypatch: pytest.MonkeyPatch, loop: FakeLoop) -> None:
    monkeypatch.setattr(auth_module.asyncio, "new_event_loop", lambda: loop)
    monkeypatch.setattr(auth_module.asyncio, "set_event_loop", lambda _loop: None)



def _unwrap_response(result: Any) -> tuple[Any, int]:
    if isinstance(result, tuple):
        response, status = result
        return response, status
    return result, result.status_code


def _unwrap_all(func):
    """Unwrap at most two decorator layers (log_method + require_auth when present)."""
    first = getattr(func, "__wrapped__", None)
    if first is None:
        return func

    second = getattr(first, "__wrapped__", None)
    return second or first



def test_verify_email_route_covers_missing_invalid_not_found_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """邮箱验证路由应覆盖缺参、非法 token、找不到用户和成功补发 token。"""
    with auth_route_unit_app.test_request_context("/api/auth/email/verify", method="POST", json={}):
        response, status = _unwrap_response(auth_module.verify_email_by_token())
        assert status == 400
        assert response.get_json()["message"] == "Verification token is required"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret"})
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: None)
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "invalid-token"},
    ):
        response, status = _unwrap_response(auth_module.verify_email_by_token())
        assert status == 400
        assert response.get_json()["error"] == "Invalid token"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": "bad"})
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "invalid-shape"},
    ):
        response, status = _unwrap_response(auth_module.verify_email_by_token())
        assert status == 400
        assert response.get_json()["error"] == "Invalid token"

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1})
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "missing-user"},
    ):
        response, status = _unwrap_response(auth_module.verify_email_by_token())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    verified_user_db = FakeAuthDB(
        user_by_id={"id": 1, "username": "alice", "email": "alice@example.com", "email_verified": 0}
    )
    verified_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verified_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verified_user_db)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1})
    monkeypatch.setattr(
        auth_module,
        "_create_new_session_payload",
        lambda *_args, **_kwargs: {"access_token": "new-access-token"},
    )
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "valid-token", "requestNewToken": True},
    ):
        response, status = _unwrap_response(auth_module.verify_email_by_token())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["success"] is True
    assert payload["result"]["newToken"] == "new-access-token"
    assert verified_user_db.updated_users == [(1, {"email_verified": 1})]
    assert verified_user_db.auth_logs



def test_resend_verification_and_password_reset_request_routes_cover_error_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """重发验证邮件与请求找回密码路由应覆盖缺参、鉴权失败、禁用与成功路径。"""
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/resend-verification",
        method="POST",
        json={},
    ):
        response, status = _unwrap_response(auth_module.resend_verification_email_unauthed())
        assert status == 400
        assert response.get_json()["message"] == "Email and password are required"

    hashed_password = bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")
    resend_db = FakeAuthDB(
        user_by_email={"id": 2, "username": "bob", "email": "bob@example.com", "password_hash": hashed_password}
    )
    resend_loop = FakeLoop()
    _install_fake_loop(monkeypatch, resend_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: resend_db)
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"})
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/resend-verification",
        method="POST",
        json={"email": "bob@example.com", "password": "Wrong123!"},
    ):
        response, status = _unwrap_response(auth_module.resend_verification_email_unauthed())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    with auth_route_unit_app.test_request_context(
        "/api/auth/email/resend-verification",
        method="POST",
        json={"email": "bob@example.com", "password": "Correct123!"},
    ):
        response, status = _unwrap_response(auth_module.resend_verification_email_unauthed())
        assert status == 200
        assert response.get_json() == {"success": True, "result": True}
        assert resend_db.auth_logs

    with auth_route_unit_app.test_request_context(
        "/api/auth/password/forgot",
        method="POST",
        json={},
    ):
        response, status = _unwrap_response(auth_module.request_password_reset())
        assert status == 400
        assert response.get_json()["message"] == "Email is required"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_user_forget_password": False})
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/forgot",
        method="POST",
        json={"email": "bob@example.com"},
    ):
        response, status = _unwrap_response(auth_module.request_password_reset())
        assert status == 403
        assert response.get_json()["error"] == "Forget password disabled"

    reset_request_db = FakeAuthDB(user_by_email={"id": 2, "username": "bob", "email": "bob@example.com"})
    reset_request_loop = FakeLoop()
    _install_fake_loop(monkeypatch, reset_request_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: reset_request_db)
    monkeypatch.setattr(
        auth_module,
        "load_auth_config",
        lambda: {"enable_user_forget_password": True, "jwt_secret": "secret", "jwt_algorithm": "HS256"},
    )
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/forgot",
        method="POST",
        json={"email": "bob@example.com"},
    ):
        response, status = _unwrap_response(auth_module.request_password_reset())
        assert status == 200
        assert response.get_json() == {"success": True, "result": True}
        assert reset_request_db.auth_logs



def test_reset_password_route_covers_validation_token_lookup_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """重置密码路由应覆盖缺参、配置禁用、无效 token、用户缺失和成功路径。"""
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 400
        assert response.get_json()["message"] == "Email, password and token are required"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_user_forget_password": False})
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "token"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 403
        assert response.get_json()["error"] == "Forget password disabled"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_user_forget_password": True})
    monkeypatch.setattr(auth_module, "validate_password", lambda *_args, **_kwargs: (False, "bad password"))
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "weak", "token": "token"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 400
        assert response.get_json()["message"] == "bad password"

    monkeypatch.setattr(auth_module, "validate_password", lambda *_args, **_kwargs: (True, ""))
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: None)
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "bad-token"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 400
        assert response.get_json()["error"] == "Invalid token"

    monkeypatch.setattr(
        auth_module,
        "decode_action_token",
        lambda *_args, **_kwargs: {"user_id": 1, "email": "other@example.com"},
    )
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "mismatch"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 400
        assert response.get_json()["message"] == "Reset password token does not match email"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": "bad", "email": "alice@example.com"})
    invalid_id_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    invalid_id_loop = FakeLoop()
    _install_fake_loop(monkeypatch, invalid_id_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: invalid_id_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "bad-id"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 400
        assert response.get_json()["error"] == "Invalid token"

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"})
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "missing-user"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_db)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"})
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "good-token"},
    ):
        response, status = _unwrap_response(auth_module.reset_password_by_token())
        payload = response.get_json() or {}

    assert status == 200
    assert payload == {"success": True, "result": True}
    assert success_db.updated_users
    assert success_db.auth_logs



def test_oauth_logout_and_refresh_routes_cover_disabled_invalid_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """OAuth2、登出和 refresh token 路由应覆盖常见坏路径与成功路径。"""
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_oauth2": False})
    with auth_route_unit_app.test_request_context("/api/auth/oauth2/authorize", method="POST"):
        response, status = _unwrap_response(auth_module.authorize_oauth2_callback())
        assert status == 403
        assert response.get_json()["error"] == "OAuth2 disabled"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_oauth2": True})
    with auth_route_unit_app.test_request_context("/api/auth/oauth2/authorize", method="POST"):
        response, status = _unwrap_response(auth_module.authorize_oauth2_callback())
        assert status == 501
        assert response.get_json()["error"] == "Not Implemented"

    with auth_route_unit_app.test_request_context("/api/auth/logout", method="POST"):
        response, status = _unwrap_response(auth_module.logout())
        assert status == 401
        assert response.get_json()["message"] == "Missing authorization header"

    logout_db = FakeAuthDB(session={"id": 11, "user_id": 1, "username": "alice"})
    logout_loop = FakeLoop()
    _install_fake_loop(monkeypatch, logout_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: logout_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/logout",
        method="POST",
        headers={"Authorization": "Bearer session-token"},
    ):
        response, status = _unwrap_response(auth_module.logout())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["result"] is True
    assert any(token != logout_db.invalidated_tokens[0] for token in logout_db.invalidated_tokens[1:])
    assert logout_db.auth_logs

    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "token"},
    ):
        monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"})
        monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"type": "access"})
        response, status = _unwrap_response(auth_module.refresh_token())
        assert status == 400
        assert response.get_json()["message"] == "Not a refresh token"

    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "token"},
    ):
        monkeypatch.setattr(
            auth_module.jwt,
            "decode",
            lambda *_args, **_kwargs: {"type": "refresh", "user_id": "bad", "username": ""},
        )
        response, status = _unwrap_response(auth_module.refresh_token())
        assert status == 401
        assert response.get_json()["message"] == "Invalid refresh token"

    missing_user_refresh_db = FakeAuthDB(user_by_id=None)
    missing_user_refresh_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_refresh_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_refresh_db)
    monkeypatch.setattr(
        auth_module.jwt,
        "decode",
        lambda *_args, **_kwargs: {"type": "refresh", "user_id": 1, "username": "alice"},
    )
    monkeypatch.setattr(
        auth_module,
        "generate_jwt_token",
        lambda *_args, **_kwargs: {
            "access_token": "fresh-access-token",
            "refresh_token": "fresh-refresh-token",
            "expires_at": "2026-03-10T10:00:00",
            "refresh_expires_at": "2026-04-10T10:00:00",
        },
    )
    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "token"},
    ):
        response, status = _unwrap_response(auth_module.refresh_token())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    success_refresh_db = FakeAuthDB(
        user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"},
        cloud_settings=[{"setting_key": "showAmountInHomePage", "setting_value": "true"}],
    )
    success_refresh_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_refresh_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_refresh_db)
    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "token"},
    ):
        response, status = _unwrap_response(auth_module.refresh_token())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["success"] is True
    assert payload["result"]["newToken"] == "fresh-access-token"
    assert success_refresh_db.created_sessions
    assert payload["result"]["applicationCloudSettings"] == [
        {"settingKey": "showAmountInHomePage", "settingValue": "true"}
    ]


def test_profile_route_covers_get_put_validation_failures_and_success(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """profile GET/PUT 应覆盖用户不存在、空 body、更新失败、更新后缺失与成功路径。"""
    route = _unwrap_all(auth_module.profile)
    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)

    def _serialize_keyword_list(items: list[str]) -> str:
        return "|".join(items)

    monkeypatch.setattr(auth_module, "serialize_keyword_list", _serialize_keyword_list)

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context("/api/profile", method="GET"):
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_db)
    with auth_route_unit_app.test_request_context("/api/profile", method="GET"):
        response, status = _unwrap_response(route())
        assert status == 200
        assert response.get_json()["result"]["username"] == "alice"

    with auth_route_unit_app.test_request_context("/api/profile", method="PUT", json={}):
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["message"] == "Request body is required"

    class UpdateFailDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            return False

    update_fail_db = UpdateFailDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    update_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, update_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: update_fail_db)
    with auth_route_unit_app.test_request_context("/api/profile", method="PUT", json={"nickname": "Alice"}):
        response, status = _unwrap_response(route())
        assert status == 500
        assert response.get_json()["error"] == "Update failed"

    class UpdateThenMissingDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            self.user_by_id = None
            return True

    update_missing_db = UpdateThenMissingDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    update_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, update_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: update_missing_db)
    with auth_route_unit_app.test_request_context("/api/profile", method="PUT", json={"nickname": "Alice"}):
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "User not found after update"

    updated_user_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    updated_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, updated_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: updated_user_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile",
        method="PUT",
        json={
            "nickname": "Alice",
            "language": "en_US",
            "defaultCurrency": "USD",
            "importLearningEnabled": True,
            "investmentPlatformKeywords": ["基金", "ETF"],
        },
    ):
        response, status = _unwrap_response(route())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["success"] is True
    assert payload["result"]["user"]["nickname"] == "Alice"
    assert updated_user_db.updated_users[0][1]["investment_platform_keywords"] == "基金|ETF"
    assert updated_user_db.updated_users[0][1]["import_learning_enabled"] == 1


def test_profile_avatar_routes_cover_missing_file_value_error_failures_and_success(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """头像增删接口应覆盖缺文件、空文件、更新失败、用户缺失与成功路径。"""
    upload_route = _unwrap_all(auth_module.update_profile_avatar)
    delete_route = _unwrap_all(auth_module.remove_profile_avatar)
    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)

    with auth_route_unit_app.test_request_context("/api/profile/avatar", method="POST"):
        response, status = _unwrap_response(upload_route())
        assert status == 400
        assert response.get_json()["message"] == "Avatar file is required"

    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b""), "avatar.png")},
    ):
        response, status = _unwrap_response(upload_route())
        assert status == 400
        assert response.get_json()["message"] == "Avatar file is empty"

    class UpdateFailDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            return False

    avatar_fail_db = UpdateFailDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    avatar_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, avatar_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: avatar_fail_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b"avatar-bytes"), "avatar.png")},
    ):
        response, status = _unwrap_response(upload_route())
        assert status == 500
        assert response.get_json()["error"] == "Update failed"

    class AvatarUpdateThenMissingDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            self.user_by_id = None
            return True

    avatar_missing_db = AvatarUpdateThenMissingDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    avatar_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, avatar_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: avatar_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b"avatar-bytes"), "avatar.png")},
    ):
        response, status = _unwrap_response(upload_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    avatar_success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    avatar_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, avatar_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: avatar_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b"avatar-bytes"), "avatar.png")},
    ):
        response, status = _unwrap_response(upload_route())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["result"]["username"] == "alice"
    assert avatar_success_db.updated_users
    assert avatar_success_db.updated_users[0][1]["avatar"].startswith("data:image/png;base64,")

    class AvatarExplodingDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            _ = (user_id, data)
            raise RuntimeError("avatar boom")

    avatar_exploding_db = AvatarExplodingDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    avatar_exploding_loop = FakeLoop()
    _install_fake_loop(monkeypatch, avatar_exploding_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: avatar_exploding_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b"avatar-bytes"), "avatar.png")},
    ):
        response, status = _unwrap_response(upload_route())
        assert status == 500
        assert response.get_json()["message"] == "avatar boom"

    remove_fail_db = UpdateFailDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    remove_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, remove_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: remove_fail_db)
    with auth_route_unit_app.test_request_context("/api/profile/avatar", method="DELETE"):
        response, status = _unwrap_response(delete_route())
        assert status == 500
        assert response.get_json()["error"] == "Update failed"

    remove_missing_db = AvatarUpdateThenMissingDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    remove_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, remove_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: remove_missing_db)
    with auth_route_unit_app.test_request_context("/api/profile/avatar", method="DELETE"):
        response, status = _unwrap_response(delete_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    remove_success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com", "avatar": "set"})
    remove_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, remove_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: remove_success_db)
    with auth_route_unit_app.test_request_context("/api/profile/avatar", method="DELETE"):
        response, status = _unwrap_response(delete_route())
        payload = response.get_json() or {}

    assert status == 200
    assert payload["result"]["username"] == "alice"
    assert remove_success_db.updated_users[0][1] == {"avatar": ""}

    remove_exploding_db = AvatarExplodingDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    remove_exploding_loop = FakeLoop()
    _install_fake_loop(monkeypatch, remove_exploding_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: remove_exploding_db)
    with auth_route_unit_app.test_request_context("/api/profile/avatar", method="DELETE"):
        response, status = _unwrap_response(delete_route())
        assert status == 500
        assert response.get_json()["message"] == "avatar boom"


def test_profile_email_and_external_auth_routes_cover_error_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证资料相关的验证邮件与第三方登录接口应覆盖主要错误和成功分支。"""
    resend_route = _unwrap_all(auth_module.resend_profile_verification_email)
    list_route = _unwrap_all(auth_module.list_profile_external_auths)
    unlink_route = _unwrap_all(auth_module.unlink_profile_external_auth)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(
        auth_module,
        "load_auth_config",
        lambda: {"require_email_verification": True, "enable_oauth2": True, "oauth2_provider": "gitlab"},
    )

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context("/api/profile/email/resend-verification", method="POST"):
        response, status = _unwrap_response(resend_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    no_email_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "", "email_verified": 0})
    no_email_loop = FakeLoop()
    _install_fake_loop(monkeypatch, no_email_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: no_email_db)
    with auth_route_unit_app.test_request_context("/api/profile/email/resend-verification", method="POST"):
        response, status = _unwrap_response(resend_route())
        assert status == 400
        assert response.get_json()["message"] == "Email is required to resend verification email"

    resend_success_db = FakeAuthDB(
        user_by_id={"id": 1, "username": "alice", "email": "alice@example.com", "email_verified": 0}
    )
    resend_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, resend_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: resend_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/email/resend-verification",
        method="POST",
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(resend_route())
        assert status == 200
        assert response.get_json()["result"] is True
        assert resend_success_db.auth_logs

    class ExplodingGetUserDB(FakeAuthDB):
        async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
            _ = user_id
            raise RuntimeError("resend boom")

    resend_fail_db = ExplodingGetUserDB(user_by_id={"id": 1})
    resend_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, resend_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: resend_fail_db)
    with auth_route_unit_app.test_request_context("/api/profile/email/resend-verification", method="POST"):
        response, status = _unwrap_response(resend_route())
        assert status == 500
        assert response.get_json()["message"] == "resend boom"

    external_auths_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice"})
    external_auths_db.user_external_auths = [
        {
            "external_auth_category": "oauth2",
            "external_auth_type": "github",
            "linked": True,
            "external_username": "octocat",
            "created_at": "2026-03-01T12:00:00",
        }
    ]
    external_auths_loop = FakeLoop()
    _install_fake_loop(monkeypatch, external_auths_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: external_auths_db)
    with auth_route_unit_app.test_request_context("/api/profile/external-auths", method="GET"):
        response, status = _unwrap_response(list_route())
        payload = response.get_json() or {}
        assert status == 200
        assert [item["externalAuthType"] for item in payload["result"]] == ["github", "gitlab"]
        assert payload["result"][1]["linked"] is False

    class ExplodingExternalAuthsDB(FakeAuthDB):
        async def get_user_external_auths(self, user_id: int) -> list[dict[str, Any]]:
            _ = user_id
            raise RuntimeError("external auths boom")

    exploding_external_auths_db = ExplodingExternalAuthsDB(user_by_id={"id": 1})
    exploding_external_auths_loop = FakeLoop()
    _install_fake_loop(monkeypatch, exploding_external_auths_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: exploding_external_auths_db)
    with auth_route_unit_app.test_request_context("/api/profile/external-auths", method="GET"):
        response, status = _unwrap_response(list_route())
        assert status == 500
        assert response.get_json()["message"] == "external auths boom"

    with auth_route_unit_app.test_request_context("/api/profile/external-auths/unlink", method="POST", json={}):
        response, status = _unwrap_response(unlink_route())
        assert status == 400
        assert response.get_json()["message"] == "externalAuthType is required"

    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 400
        assert response.get_json()["message"] == "password is required"

    unlink_missing_user_db = FakeAuthDB(user_by_id=None)
    unlink_missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, unlink_missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: unlink_missing_user_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github", "password": "Correct123!"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    hashed_password = bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")
    unlink_user = {"id": 1, "username": "alice", "password_hash": hashed_password}
    invalid_password_db = FakeAuthDB(user_by_id=dict(unlink_user))
    invalid_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, invalid_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: invalid_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github", "password": "Wrong123!"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 400
        assert response.get_json()["message"] == "Invalid password"

    not_linked_db = FakeAuthDB(user_by_id=dict(unlink_user))
    not_linked_loop = FakeLoop()
    _install_fake_loop(monkeypatch, not_linked_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: not_linked_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github", "password": "Correct123!"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 404
        assert response.get_json()["message"] == "Third-party login is not linked"

    linked_db = FakeAuthDB(user_by_id=dict(unlink_user))
    linked_db.user_external_auths = [
        {
            "external_auth_category": "oauth2",
            "external_auth_type": "github",
            "linked": True,
            "external_username": "octocat",
            "created_at": "2026-03-01T12:00:00",
        }
    ]
    linked_loop = FakeLoop()
    _install_fake_loop(monkeypatch, linked_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: linked_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github", "password": "Correct123!"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 200
        assert response.get_json()["result"] is True
        assert linked_db.deleted_external_auths == [(1, "github")]
        assert linked_db.auth_logs

    class ExplodingDeleteExternalAuthDB(FakeAuthDB):
        async def delete_user_external_auth(self, user_id: int, external_auth_type: str) -> bool:
            _ = (user_id, external_auth_type)
            raise RuntimeError("unlink boom")

    exploding_unlink_db = ExplodingDeleteExternalAuthDB(user_by_id=dict(unlink_user))
    exploding_unlink_db.user_external_auths = [
        {
            "external_auth_category": "oauth2",
            "external_auth_type": "github",
            "linked": True,
            "external_username": "octocat",
            "created_at": "2026-03-01T12:00:00",
        }
    ]
    exploding_unlink_loop = FakeLoop()
    _install_fake_loop(monkeypatch, exploding_unlink_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: exploding_unlink_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile/external-auths/unlink",
        method="POST",
        json={"externalAuthType": "github", "password": "Correct123!"},
    ):
        response, status = _unwrap_response(unlink_route())
        assert status == 500
        assert response.get_json()["message"] == "unlink boom"


def test_2fa_status_request_confirm_disable_recovery_and_cloud_settings_routes(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """2FA 基础写路径与云设置路由应覆盖主要错误和成功分支。"""
    status_route = _unwrap_all(auth_module.get_2fa_status)
    request_route = _unwrap_all(auth_module.enable_2fa_request)
    confirm_route = _unwrap_all(auth_module.enable_2fa_confirm)
    disable_route = _unwrap_all(auth_module.disable_2fa)
    regenerate_route = _unwrap_all(auth_module.regenerate_2fa_recovery_codes)
    cloud_settings_route = _unwrap_all(auth_module.profile_cloud_settings)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/status", method="GET"):
        response, status = _unwrap_response(status_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    enabled_user_db = FakeAuthDB(user_by_id={"id": 1, "two_factor_enabled": 1})
    enabled_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, enabled_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: enabled_user_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/status", method="GET"):
        response, status = _unwrap_response(status_route())
        assert status == 200
        assert response.get_json()["result"] == {"enable": True, "isEnabled": True}

    monkeypatch.setattr(auth_module.pyotp, "random_base32", lambda: "SECRET123")
    monkeypatch.setattr(auth_module, "_generate_2fa_qrcode_data_url", lambda username, secret: f"qr:{username}:{secret}")
    with auth_route_unit_app.test_request_context("/api/auth/2fa/enable/request", method="POST"):
        response, status = _unwrap_response(request_route())
        assert status == 200
        assert response.get_json()["result"] == {"secret": "SECRET123", "qrcode": "qr:alice:SECRET123"}

    monkeypatch.setattr(
        auth_module,
        "_generate_2fa_qrcode_data_url",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("qrcode boom")),
    )
    with auth_route_unit_app.test_request_context("/api/auth/2fa/enable/request", method="POST"):
        response, status = _unwrap_response(request_route())
        assert status == 500
        assert response.get_json()["message"] == "qrcode boom"

    cloud_db = FakeAuthDB(user_by_id={"id": 1}, cloud_settings=[{"setting_key": "showAmountInHomePage", "setting_value": "true"}])
    cloud_loop = FakeLoop()
    _install_fake_loop(monkeypatch, cloud_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: cloud_db)
    monkeypatch.setattr(auth_module, "_load_application_cloud_settings", lambda db, user_id, loop: [{"settingKey": "showAmountInHomePage", "settingValue": "true"}])
    monkeypatch.setattr(auth_module, "_validate_application_cloud_setting", lambda setting: "" if setting.get("settingKey") != "bad" else "bad setting")
    monkeypatch.setattr(auth_module, "_normalize_application_cloud_settings", lambda settings: [{"setting_key": s["settingKey"], "setting_value": str(s.get("settingValue", ""))} for s in settings])

    with auth_route_unit_app.test_request_context("/api/profile/cloud-settings", method="GET"):
        response, status = _unwrap_response(cloud_settings_route())
        assert status == 200
        assert response.get_json()["result"] == [{"settingKey": "showAmountInHomePage", "settingValue": "true"}]

    with auth_route_unit_app.test_request_context("/api/profile/cloud-settings", method="PUT", json={"settings": {}}):
        response, status = _unwrap_response(cloud_settings_route())
        assert status == 400
        assert response.get_json()["message"] == "settings must be an array"

    with auth_route_unit_app.test_request_context(
        "/api/profile/cloud-settings",
        method="PUT",
        json={"settings": [{"settingKey": "bad", "settingValue": "1"}]},
    ):
        response, status = _unwrap_response(cloud_settings_route())
        assert status == 400
        assert response.get_json()["message"] == "bad setting"

    with auth_route_unit_app.test_request_context(
        "/api/profile/cloud-settings",
        method="PUT",
        json={"settings": [{"settingKey": "showAmountInHomePage", "settingValue": "true"}], "fullUpdate": True},
    ):
        response, status = _unwrap_response(cloud_settings_route())
        assert status == 200
        assert response.get_json()["result"] is True
        assert cloud_db.updated_cloud_settings == [
            (1, [{"setting_key": "showAmountInHomePage", "setting_value": "true"}], True)
        ]

    with auth_route_unit_app.test_request_context("/api/profile/cloud-settings", method="DELETE"):
        response, status = _unwrap_response(cloud_settings_route())
        assert status == 200
        assert response.get_json()["result"] is True
        assert cloud_db.deleted_cloud_settings == [1]

    with auth_route_unit_app.test_request_context("/api/auth/2fa/enable/confirm", method="POST", json={}):
        response, status = _unwrap_response(confirm_route())
        assert status == 400
        assert response.get_json()["message"] == "Secret and passcode are required"

    class RejectingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return False

    monkeypatch.setattr(auth_module.pyotp, "TOTP", RejectingTOTP)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "000000"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid passcode"

    class AcceptingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return True

    class ConfirmFailDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            return False

    monkeypatch.setattr(auth_module.pyotp, "TOTP", AcceptingTOTP)
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret"})
    monkeypatch.setattr(auth_module, "_create_new_session_payload", lambda *_args, **_kwargs: {"access_token": "access", "refresh_token": "refresh"})
    monkeypatch.setattr(auth_module, "_generate_recovery_codes", lambda: ["ABCD-1234"])
    confirm_fail_db = ConfirmFailDB(user_by_id={"id": 1, "username": "alice"})
    confirm_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_fail_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["error"] == "Update failed"

    confirm_success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice"})
    confirm_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        payload = response.get_json() or {}
    assert status == 200
    assert payload["result"]["token"] == "access"
    assert payload["result"]["recoveryCodes"] == ["ABCD-1234"]

    class ConfirmPersistErrorDB(FakeAuthDB):
        async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
            self.replaced_recovery_codes.append((user_id, list(recovery_codes)))
            raise RuntimeError("persist boom")

    confirm_persist_error_db = ConfirmPersistErrorDB(user_by_id={"id": 1, "username": "alice"})
    confirm_persist_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_persist_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_persist_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["message"] == "persist boom"
        assert confirm_persist_error_db.updated_users[-1] == (1, {"two_factor_enabled": 0, "two_factor_secret": ""})
        assert confirm_persist_error_db.cleared_recovery_codes == [1]

    class ConfirmPersistClearErrorDB(ConfirmPersistErrorDB):
        async def clear_two_factor_recovery_codes(self, user_id: int) -> int:
            _ = user_id
            raise RuntimeError("clear boom")

    confirm_persist_clear_error_db = ConfirmPersistClearErrorDB(user_by_id={"id": 1, "username": "alice"})
    confirm_persist_clear_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_persist_clear_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_persist_clear_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["message"] == "persist boom"
        assert confirm_persist_clear_error_db.updated_users[-1] == (1, {"two_factor_enabled": 0, "two_factor_secret": ""})

    class ConfirmPersistMismatchDB(FakeAuthDB):
        async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
            self.replaced_recovery_codes.append((user_id, list(recovery_codes)))
            return 0

    confirm_persist_mismatch_db = ConfirmPersistMismatchDB(user_by_id={"id": 1, "username": "alice"})
    confirm_persist_mismatch_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_persist_mismatch_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_persist_mismatch_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["message"] == "Failed to persist two-factor recovery codes"
        assert confirm_persist_mismatch_db.updated_users[-1] == (1, {"two_factor_enabled": 0, "two_factor_secret": ""})

    with auth_route_unit_app.test_request_context("/api/auth/2fa/disable", method="POST", json={}):
        response, status = _unwrap_response(disable_route())
        assert status == 400
        assert response.get_json()["message"] == "Current password or stepUpToken is required"

    disable_missing_db = FakeAuthDB(user_by_id=None)
    disable_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_missing_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/disable", method="POST", json={"password": "ok"}):
        response, status = _unwrap_response(disable_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    disable_user = {"id": 1, "username": "alice", "password_hash": bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")}
    disable_invalid_db = FakeAuthDB(user_by_id=dict(disable_user))
    disable_invalid_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_invalid_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_invalid_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/disable", method="POST", json={"password": "Wrong123!"}):
        response, status = _unwrap_response(disable_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    class DisableFailDB(FakeAuthDB):
        async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
            self.updated_users.append((user_id, data))
            return False

    disable_fail_db = DisableFailDB(user_by_id=dict(disable_user))
    disable_fail_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_fail_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_fail_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/disable", method="POST", json={"password": "Correct123!"}):
        response, status = _unwrap_response(disable_route())
        assert status == 500
        assert response.get_json()["error"] == "Update failed"

    disable_success_db = FakeAuthDB(user_by_id=dict(disable_user))
    disable_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_success_db)
    with auth_route_unit_app.test_request_context("/api/auth/2fa/disable", method="POST", json={"password": "Correct123!"}):
        response, status = _unwrap_response(disable_route())
        assert status == 200
        assert response.get_json()["result"] is True

    monkeypatch.setattr(
        auth_module,
        "decode_action_token",
        lambda token, _config, expected_type: {"user_id": 1, "type": expected_type} if token == "step-up-token" else None,
    )
    disable_step_up_db = FakeAuthDB(user_by_id=dict(disable_user))
    disable_step_up_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_step_up_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_step_up_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/disable",
        method="POST",
        json={"stepUpToken": "step-up-token"},
    ):
        response, status = _unwrap_response(disable_route())
        assert status == 200
        assert response.get_json()["result"] is True

    with auth_route_unit_app.test_request_context("/api/auth/2fa/recovery/regenerate", method="POST", json={}):
        response, status = _unwrap_response(regenerate_route())
        assert status == 400
        assert response.get_json()["message"] == "Current password or stepUpToken is required"

    regen_disabled_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": disable_user["password_hash"], "two_factor_enabled": 0})
    regen_disabled_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regen_disabled_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regen_disabled_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 400
        assert response.get_json()["message"] == "Two-factor authentication is not enabled"

    regen_success_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": disable_user["password_hash"], "two_factor_enabled": 1})
    regen_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regen_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regen_success_db)
    monkeypatch.setattr(auth_module, "_generate_recovery_codes", lambda: ["WXYZ-9999"])
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 200
        assert response.get_json()["result"]["recoveryCodes"] == ["WXYZ-9999"]

    class RegenPersistMismatchDB(FakeAuthDB):
        async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
            self.replaced_recovery_codes.append((user_id, list(recovery_codes)))
            return 0

    regen_mismatch_db = RegenPersistMismatchDB(
        user_by_id={"id": 1, "password_hash": disable_user["password_hash"], "two_factor_enabled": 1}
    )
    regen_mismatch_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regen_mismatch_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regen_mismatch_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 500
        assert response.get_json()["message"] == "Failed to persist two-factor recovery codes"

    regen_step_up_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": disable_user["password_hash"], "two_factor_enabled": 1})
    regen_step_up_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regen_step_up_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regen_step_up_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/regenerate",
        method="POST",
        json={"stepUpToken": "step-up-token"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 200
        assert response.get_json()["result"]["recoveryCodes"] == ["WXYZ-9999"]


def test_2fa_verify_and_recovery_verify_routes_cover_error_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """2FA 登录校验与恢复码校验应覆盖主要错误和成功路径。"""
    verify_route = _unwrap_all(auth_module.verify_2fa_login)
    recovery_route = _unwrap_all(auth_module.verify_2fa_login_by_recovery_code)

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret"})
    monkeypatch.setattr(auth_module, "_load_application_cloud_settings", lambda *_args, **_kwargs: [{"settingKey": "k", "settingValue": "v"}])
    monkeypatch.setattr(auth_module, "_build_user_profile_info", lambda user: {"username": user["username"]})
    monkeypatch.setattr(auth_module, "_build_auth_success_result", lambda user, tokens, settings=None: {"token": tokens["access_token"], "user": user, "applicationCloudSettings": settings or []})
    monkeypatch.setattr(auth_module, "_create_new_session_payload", lambda *_args, **_kwargs: {"access_token": "session-token"})
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")

    with auth_route_unit_app.test_request_context("/api/auth/2fa/verify", method="POST", json={}):
        response, status = _unwrap_response(verify_route())
        assert status == 401
        assert response.get_json()["message"] == "Missing authorization header"

    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 400
        assert response.get_json()["message"] == "Passcode is required"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: None)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 401
        assert response.get_json()["message"] == "Invalid or expired 2FA token"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1})
    verify_missing_db = FakeAuthDB(user_by_id=None)
    verify_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    verify_disabled_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "two_factor_enabled": 0, "two_factor_secret": ""})
    verify_disabled_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_disabled_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_disabled_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 400
        assert response.get_json()["message"] == "Two-factor authentication is not enabled"

    class RejectingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return False

    monkeypatch.setattr(auth_module.pyotp, "TOTP", RejectingTOTP)
    verify_user = {"id": 1, "username": "alice", "two_factor_enabled": 1, "two_factor_secret": "SECRET123"}
    verify_invalid_db = FakeAuthDB(user_by_id=dict(verify_user))
    verify_invalid_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_invalid_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_invalid_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token", "User-Agent": "Browser"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid passcode"

    class AcceptingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return True

    monkeypatch.setattr(auth_module.pyotp, "TOTP", AcceptingTOTP)
    verify_success_db = FakeAuthDB(user_by_id=dict(verify_user))
    verify_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token", "User-Agent": "Browser"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 200
        assert response.get_json()["result"]["token"] == "session-token"
        assert verify_success_db.auth_logs

    with auth_route_unit_app.test_request_context("/api/auth/2fa/recovery/verify", method="POST", json={}):
        response, status = _unwrap_response(recovery_route())
        assert status == 401
        assert response.get_json()["message"] == "Missing authorization header"

    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 400
        assert response.get_json()["message"] == "Recovery code is required"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: None)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 401
        assert response.get_json()["message"] == "Invalid or expired 2FA token"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1})
    recovery_invalid_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "two_factor_enabled": 1})
    recovery_invalid_loop = FakeLoop()
    _install_fake_loop(monkeypatch, recovery_invalid_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: recovery_invalid_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid recovery code"

    recovery_missing_db = FakeAuthDB(user_by_id=None)
    recovery_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, recovery_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: recovery_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    recovery_success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "two_factor_enabled": 1})
    recovery_success_db.recovery_codes_by_user[1] = ["ABCD-1234"]
    recovery_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, recovery_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: recovery_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token", "User-Agent": "Browser"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 200
        assert response.get_json()["result"]["token"] == "session-token"
        assert recovery_success_db.auth_logs


def test_user_data_routes_cover_statistics_export_clear_and_version(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """用户数据统计/导出/清理与版本端点应覆盖主要正反路径。"""
    stats_route = _unwrap_all(auth_module.get_user_data_statistics)
    export_route = _unwrap_all(auth_module.export_user_data)
    clear_transactions_route = _unwrap_all(auth_module.clear_user_transactions)
    clear_all_route = _unwrap_all(auth_module.clear_all_user_data)
    version_route = _unwrap_all(auth_module.get_system_version)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")

    stats_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice"})
    stats_loop = FakeLoop()
    _install_fake_loop(monkeypatch, stats_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: stats_db)
    with auth_route_unit_app.test_request_context("/api/auth/data/statistics", method="GET"):
        response, status = _unwrap_response(stats_route())
        assert status == 200
        assert response.get_json()["result"]["billCount"] == 3

    class ExplodingStatsDB(FakeAuthDB):
        async def get_user_data_statistics(self, user_id: int) -> dict[str, Any]:
            _ = user_id
            raise RuntimeError("stats boom")

    exploding_stats_db = ExplodingStatsDB(user_by_id={"id": 1})
    exploding_stats_loop = FakeLoop()
    _install_fake_loop(monkeypatch, exploding_stats_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: exploding_stats_db)
    with auth_route_unit_app.test_request_context("/api/auth/data/statistics", method="GET"):
        response, status = _unwrap_response(stats_route())
        assert status == 500
        assert response.get_json()["message"] == "stats boom"

    with auth_route_unit_app.test_request_context("/api/auth/data/export.json", method="GET"):
        response, status = _unwrap_response(export_route("json"))
        assert status == 400
        assert response.get_json()["message"] == "Unsupported export file type"

    export_db = FakeAuthDB(user_by_id={"id": 1})
    export_db.export_categories = [{"id": 10, "main_category": "餐饮", "sub_category": "早餐"}]
    export_db.export_bills = [
        {
            "id": 1,
            "date": "2026-03-05 12:00:00",
            "type": "支出",
            "amount": -18.8,
            "main_category": "餐饮",
            "sub_category": "早餐",
            "source_account_id": 1,
            "destination_account_id": 2,
            "counterparty": "早餐店",
            "payment_method": "支付宝",
            "description": "豆浆油条",
            "comment": "测试备注",
            "created_at": "2026-03-05T12:00:00",
            "updated_at": "2026-03-05T12:00:00",
        }
    ]
    export_db.export_accounts = [{"id": 1, "name": "现金"}, {"id": 2, "name": "支付宝"}]
    export_db.export_tags_map = {1: [{"name": "早餐"}]}
    export_loop = FakeLoop()
    _install_fake_loop(monkeypatch, export_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: export_db)
    monkeypatch.setattr(auth_module, "_build_export_filters", lambda _categories: {"keyword": "早餐"})
    with auth_route_unit_app.test_request_context("/api/auth/data/export.csv", method="GET"):
        response, status = _unwrap_response(export_route("csv"))
        assert status == 200
        assert "attachment; filename=bill_analyser_export_" in response.headers["Content-Disposition"]
        assert response.get_data(as_text=True).startswith("\ufeff")

    class ExplodingExportDB(FakeAuthDB):
        async def get_bills(self, filters: dict[str, Any], user_id: int) -> list[dict[str, Any]]:
            _ = (filters, user_id)
            raise RuntimeError("export boom")

    exploding_export_db = ExplodingExportDB(user_by_id={"id": 1})
    exploding_export_loop = FakeLoop()
    _install_fake_loop(monkeypatch, exploding_export_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: exploding_export_db)
    monkeypatch.setattr(auth_module, "_build_export_filters", lambda _categories: {})
    with auth_route_unit_app.test_request_context("/api/auth/data/export.csv", method="GET"):
        response, status = _unwrap_response(export_route("csv"))
        assert status == 500
        assert response.get_json()["message"] == "export boom"

    clear_db = FakeAuthDB(user_by_id={"id": 1})
    clear_loop = FakeLoop()
    _install_fake_loop(monkeypatch, clear_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_db)
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"})
    monkeypatch.setattr(
        auth_module,
        "decode_action_token",
        lambda token, _config, expected_type: {"user_id": 1, "type": expected_type} if token == "step-up-token" else None,
    )

    with auth_route_unit_app.test_request_context("/api/auth/data/clear/transactions", method="POST", json={}):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 400
        assert response.get_json()["message"] == "Current password or stepUpToken is required"

    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/transactions",
        method="POST",
        json={"password": "bad"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    clear_db.clear_transactions_result = {"success": False, "message": "clear failed", "deleted_count": 0}
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 500
        assert response.get_json()["message"] == "clear failed"

    clear_db.clear_transactions_result = {"success": True, "deleted_count": 7}
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 200
        assert response.get_json()["deletedCount"] == 7

    step_up_clear_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": "not-a-valid-bcrypt-hash"})
    step_up_clear_db.clear_transactions_result = {"success": True, "deleted_count": 7}
    step_up_clear_loop = FakeLoop()
    _install_fake_loop(monkeypatch, step_up_clear_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: step_up_clear_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/transactions",
        method="POST",
        json={"stepUpToken": "step-up-token"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 200
        assert response.get_json()["deletedCount"] == 7

    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_db)

    _install_fake_loop(monkeypatch, clear_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_db)

    with auth_route_unit_app.test_request_context("/api/auth/data/clear/all", method="POST", json={}):
        response, status = _unwrap_response(clear_all_route())
        assert status == 400
        assert response.get_json()["message"] == "Current password or stepUpToken is required"

    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/all",
        method="POST",
        json={"password": "bad"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    clear_db.clear_all_result = {"success": False, "message": "clear all failed", "counts": {}}
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/all",
        method="POST",
        json={"password": "ok"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 500
        assert response.get_json()["message"] == "clear all failed"

    clear_db.clear_all_result = {"success": True, "counts": {"bills": 3, "accounts": 1}}
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/all",
        method="POST",
        json={"password": "ok"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 200
        assert response.get_json()["counts"] == {"bills": 3, "accounts": 1}

    step_up_clear_all_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": "not-a-valid-bcrypt-hash"})
    step_up_clear_all_db.clear_all_result = {"success": True, "counts": {"bills": 2}}
    step_up_clear_all_loop = FakeLoop()
    _install_fake_loop(monkeypatch, step_up_clear_all_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: step_up_clear_all_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/data/clear/all",
        method="POST",
        json={"stepUpToken": "step-up-token"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 200
        assert response.get_json()["counts"] == {"bills": 2}

    monkeypatch.setattr(auth_module, "__version__", "9.9.9")
    with auth_route_unit_app.test_request_context("/api/auth/system/version", method="GET"):
        response, status = _unwrap_response(version_route())
        assert status == 200
        assert response.get_json()["result"]["version"] == "9.9.9"


def test_security_step_up_verify_route_covers_password_and_passcode_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """step-up 验证路由应支持密码与 2FA passcode，并返回短期 token。"""
    route = _unwrap_all(auth_module.verify_security_step_up)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"})
    monkeypatch.setattr(auth_module, "generate_action_token", lambda *_args, **_kwargs: "step-up-token")

    with auth_route_unit_app.test_request_context("/api/security/step-up/verify", method="POST", json={}):
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["message"] == "password or passcode is required"

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    hashed_password = bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")
    invalid_password_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password})
    invalid_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, invalid_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: invalid_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"password": "Wrong123!"},
    ):
        response, status = _unwrap_response(route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    no_2fa_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password, "two_factor_enabled": 0})
    no_2fa_loop = FakeLoop()
    _install_fake_loop(monkeypatch, no_2fa_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: no_2fa_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["message"] == "Two-factor authentication is not enabled"

    class RejectingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return False

    monkeypatch.setattr(auth_module.pyotp, "TOTP", RejectingTOTP)
    invalid_passcode_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password, "two_factor_enabled": 1, "two_factor_secret": "SECRET123"})
    invalid_passcode_loop = FakeLoop()
    _install_fake_loop(monkeypatch, invalid_passcode_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: invalid_passcode_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid passcode"

    class AcceptingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return True

    monkeypatch.setattr(auth_module.pyotp, "TOTP", AcceptingTOTP)
    success_password_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password, "email": "alice@example.com"})
    success_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"password": "Correct123!"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(route())
        assert status == 200
        payload = response.get_json() or {}
        assert payload["result"]["stepUpToken"] == "step-up-token"
        assert payload["result"]["verifiedVia"] == "password"
        assert success_password_db.auth_logs

    success_passcode_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password, "email": "alice@example.com", "two_factor_enabled": 1, "two_factor_secret": "SECRET123"})
    success_passcode_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_passcode_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_passcode_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"passcode": "123456"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(route())
        assert status == 200
        payload = response.get_json() or {}
        assert payload["result"]["stepUpToken"] == "step-up-token"
        assert payload["result"]["verifiedVia"] == "passcode"

    class ExplodingStepUpDB(FakeAuthDB):
        async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
            _ = user_id
            raise RuntimeError("step up boom")

    exploding_step_up_db = ExplodingStepUpDB(user_by_id={"id": 1, "username": "alice"})
    exploding_step_up_loop = FakeLoop()
    _install_fake_loop(monkeypatch, exploding_step_up_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: exploding_step_up_db)
    with auth_route_unit_app.test_request_context(
        "/api/security/step-up/verify",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(route())
        assert status == 500
        assert response.get_json()["message"] == "step up boom"


def test_personal_token_routes_cover_validation_failures_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """API/MCP token 生成、撤销和列举路由应覆盖主要正反路径。"""
    generate_api_route = _unwrap_all(auth_module.generate_api_token)
    generate_mcp_route = _unwrap_all(auth_module.generate_mcp_token)
    revoke_route = _unwrap_all(auth_module.revoke_token)
    list_tokens_route = _unwrap_all(auth_module.list_tokens)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")
    monkeypatch.setattr(auth_module, "_get_request_session_id", lambda: 101)
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret"})
    monkeypatch.setattr(
        auth_module,
        "generate_access_token",
        lambda *_args, **_kwargs: {"access_token": "token-abc", "expires_at": "2026-03-10T10:00:00"},
    )
    monkeypatch.setattr(auth_module, "_build_api_base_url", lambda: "http://localhost:5000/api")
    monkeypatch.setattr(auth_module, "_build_mcp_url", lambda: "http://localhost:5000/mcp")

    with auth_route_unit_app.test_request_context("/api/auth/tokens/api", method="POST", json={}):
        response, status = _unwrap_response(generate_api_route())
        assert status == 400
        assert response.get_json()["message"] == "Current password is required"

    with auth_route_unit_app.test_request_context(
        "/api/auth/tokens/api",
        method="POST",
        json={"password": "ok", "expiresInSeconds": "bad"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 400
        assert response.get_json()["message"] == "expiresInSeconds must be a valid integer"

    missing_user_db = FakeAuthDB(user_by_id=None)
    missing_user_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_user_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/tokens/api",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    hashed_password = bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")
    invalid_password_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password})
    invalid_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, invalid_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: invalid_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/tokens/api",
        method="POST",
        json={"password": "Wrong123!"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"
        assert invalid_password_db.auth_logs

    success_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password})
    success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/tokens/api",
        method="POST",
        json={"password": "Correct123!", "expiresInSeconds": 600},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 200
        assert response.get_json()["result"] == {"token": "token-abc", "apiBaseUrl": "http://localhost:5000/api"}
        assert success_db.created_sessions

    with auth_route_unit_app.test_request_context(
        "/api/auth/tokens/mcp",
        method="POST",
        json={"password": "Correct123!"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(generate_mcp_route())
        assert status == 200
        assert response.get_json()["result"] == {"token": "token-abc", "mcpUrl": "http://localhost:5000/mcp"}

    with auth_route_unit_app.test_request_context("/api/auth/tokens/not-an-int", method="DELETE"):
        response, status = _unwrap_response(revoke_route("not-an-int"))
        assert status == 400
        assert response.get_json()["message"] == "tokenId must be a valid integer"

    revoke_db = FakeAuthDB(user_by_id={"id": 1})
    revoke_db.invalidate_session_by_id_result = False
    revoke_loop = FakeLoop()
    _install_fake_loop(monkeypatch, revoke_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: revoke_db)
    with auth_route_unit_app.test_request_context("/api/auth/tokens/42", method="DELETE"):
        response, status = _unwrap_response(revoke_route("42"))
        assert status == 404
        assert response.get_json()["message"] == "Token not found"

    revoke_db.invalidate_session_by_id_result = True
    with auth_route_unit_app.test_request_context("/api/auth/tokens/42", method="DELETE"):
        response, status = _unwrap_response(revoke_route("42"))
        assert status == 200
        assert response.get_json()["result"] is True

    list_db = FakeAuthDB(user_by_id={"id": 1})
    list_db.user_sessions = [
        {
            "id": 101,
            "user_agent": "Bill Analyser API Token",
            "ip_address": "127.0.0.1",
            "created_at": "2026-03-01T10:00:00",
            "expires_at": "2026-03-10T10:00:00",
            "last_activity_at": "2026-03-02T10:00:00",
        },
        {
            "id": 102,
            "user_agent": "Mozilla/5.0 (Windows NT 10.0) Chrome/123.0",
            "ip_address": "10.0.0.2",
            "created_at": "2026-03-01T10:00:00",
            "expires_at": "2026-03-11T10:00:00",
            "last_activity_at": "2026-03-03T10:00:00",
        },
    ]
    list_db.invalidate_other_count = 5
    list_loop = FakeLoop()
    _install_fake_loop(monkeypatch, list_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: list_db)
    with auth_route_unit_app.test_request_context("/api/auth/tokens", method="GET"):
        response, status = _unwrap_response(list_tokens_route())
        payload = response.get_json() or {}
        assert status == 200
        assert payload["success"] is True
        assert payload["result"][0]["isCurrent"] is True
        assert payload["result"][0]["tokenType"] == auth_module.TOKEN_TYPE_API
        assert payload["result"][1]["deviceName"].startswith("Windows")

    with auth_route_unit_app.test_request_context("/api/auth/tokens", method="DELETE"):
        response, status = _unwrap_response(list_tokens_route())
        assert status == 200
        assert response.get_json()["revokedCount"] == 5


def test_login_and_register_routes_cover_error_pending_2fa_and_success_paths(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """login/register 主链应覆盖主要正反路径。"""
    login_route = _unwrap_all(auth_module.login)
    register_route = _unwrap_all(auth_module.register)

    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"lockout_duration_minutes": 15, "enable_user_registration": True, "require_email_verification": True})
    monkeypatch.setattr(auth_module, "_load_application_cloud_settings", lambda *_args, **_kwargs: [{"settingKey": "k", "settingValue": "v"}])
    monkeypatch.setattr(auth_module, "_build_user_profile_info", lambda user: {"username": user["username"], "fiscalYearStart": 1})
    monkeypatch.setattr(auth_module, "_build_auth_success_result", lambda user, tokens, settings=None: {"token": tokens["access_token"], "refreshToken": tokens.get("refresh_token"), "user": user, "applicationCloudSettings": settings or []})
    monkeypatch.setattr(auth_module, "generate_jwt_token", lambda *_args, **_kwargs: {"access_token": "access-token", "refresh_token": "refresh-token", "expires_at": "2026-03-10T10:00:00", "refresh_expires_at": "2026-04-10T10:00:00"})
    monkeypatch.setattr(auth_module, "generate_action_token", lambda *_args, **_kwargs: "pending-2fa-token")

    async def _save_register_categories_stub(*_args, **_kwargs) -> bool:
        return True

    async def _create_register_default_accounts_stub(*_args, **_kwargs) -> dict[str, Any]:
        return {"success": True}

    seeded_user_ids: list[int] = []

    async def _ensure_default_category_seed_stub(_db, *, user_id: int) -> dict[str, Any]:
        seeded_user_ids.append(user_id)
        return {"categories": {"created": 0, "skipped": 0}, "rules": {"created": 0, "skipped": 0}}

    monkeypatch.setattr(auth_module, "_save_register_categories", _save_register_categories_stub)
    monkeypatch.setattr(auth_module, "_create_register_default_accounts", _create_register_default_accounts_stub)
    monkeypatch.setattr(auth_module, "ensure_default_category_seed", _ensure_default_category_seed_stub)

    with auth_route_unit_app.test_request_context("/api/auth/login", method="POST", json={}):
        response, status = _unwrap_response(login_route())
        assert status == 400
        assert response.get_json()["message"] == "Username and password are required"

    missing_user_db = FakeAuthDB(user_by_id=None, user_by_email=None)
    missing_user_db.user_by_username = None
    missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: missing_user_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"
        assert missing_user_db.auth_logs

    hashed_password = bcrypt.hashpw(b"Valid1!A", bcrypt.gensalt()).decode("utf-8")
    locked_user = {"id": 1, "username": "alice", "email": "alice@example.com", "password_hash": hashed_password, "is_active": 1, "two_factor_enabled": 0}
    locked_db = FakeAuthDB(user_by_id=dict(locked_user), user_by_email=dict(locked_user))
    locked_db.user_locked = True
    locked_loop = FakeLoop()
    _install_fake_loop(monkeypatch, locked_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: locked_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        assert status == 403
        assert response.get_json()["error"] == "Account locked"

    wrong_password_db = FakeAuthDB(user_by_id=dict(locked_user), user_by_email=dict(locked_user))
    wrong_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, wrong_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: wrong_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Wrong123!"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"
        assert wrong_password_db.failed_login_increments == [(1, 15)]

    inactive_user = dict(locked_user)
    inactive_user["is_active"] = 0
    inactive_db = FakeAuthDB(user_by_id=inactive_user, user_by_email=inactive_user)
    inactive_loop = FakeLoop()
    _install_fake_loop(monkeypatch, inactive_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: inactive_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        assert status == 403
        assert response.get_json()["error"] == "Account not active"

    two_factor_user = dict(locked_user)
    two_factor_user["two_factor_enabled"] = 1
    two_factor_db = FakeAuthDB(user_by_id=two_factor_user, user_by_email=two_factor_user)
    two_factor_loop = FakeLoop()
    _install_fake_loop(monkeypatch, two_factor_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: two_factor_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        assert status == 200
        assert response.get_json()["result"] == {"token": "pending-2fa-token", "need2FA": True}

    success_db = FakeAuthDB(user_by_id=dict(locked_user), user_by_email=dict(locked_user))
    success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice@example.com", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(login_route())
        payload = response.get_json() or {}
    assert status == 200
    assert payload["result"]["token"] == "access-token"
    assert success_db.created_sessions
    assert success_db.last_login_updates == [(1, "127.0.0.1")]

    with auth_route_unit_app.test_request_context("/api/auth/register", method="POST", json={}):
        response, status = _unwrap_response(register_route())
        assert status == 400
        assert response.get_json()["message"] == "Username, email and password are required"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_user_registration": False})
    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={"username": "alice", "email": "alice@example.com", "password": "Valid1!A"},
    ):
        response, status = _unwrap_response(register_route())
        assert status == 403
        assert response.get_json()["error"] == "Registration disabled"

    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"enable_user_registration": True, "require_email_verification": True})
    monkeypatch.setattr(auth_module, "validate_password", lambda *_args, **_kwargs: (False, "weak password"))
    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={"username": "alice", "email": "alice@example.com", "password": "weak"},
    ):
        response, status = _unwrap_response(register_route())
        assert status == 400
        assert response.get_json()["message"] == "weak password"

    monkeypatch.setattr(auth_module, "validate_password", lambda *_args, **_kwargs: (True, ""))
    existing_username_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "old@example.com"})
    existing_username_db.user_by_email = None
    existing_username_loop = FakeLoop()
    _install_fake_loop(monkeypatch, existing_username_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: existing_username_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={"username": "alice", "email": "alice@example.com", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(register_route())
        assert status == 409
        assert response.get_json()["error"] == "Username exists"

    existing_email_db = FakeAuthDB(user_by_id=None, user_by_email={"id": 2, "username": "other", "email": "alice@example.com"})
    existing_email_db.user_by_username = None
    existing_email_loop = FakeLoop()
    _install_fake_loop(monkeypatch, existing_email_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: existing_email_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={"username": "alice", "email": "alice@example.com", "password": "Valid1!A"},
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(register_route())
        assert status == 409
        assert response.get_json()["error"] == "Email exists"

    register_success_db = FakeAuthDB(user_by_id=None, user_by_email=None)
    register_success_db.user_by_username = None
    register_success_loop = FakeLoop()
    _install_fake_loop(monkeypatch, register_success_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: register_success_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={
            "username": "alice",
            "email": "alice@example.com",
            "password": "Valid1!A",
            "language": "zh_Hans",
            "categories": [{"main_category": "餐饮"}],
        },
        headers={"User-Agent": "Browser"},
    ):
        response, status = _unwrap_response(register_route())
        payload = response.get_json() or {}
    assert status == 200
    assert payload["result"]["user_id"] == 77
    assert payload["result"]["needVerifyEmail"] is True
    assert payload["result"]["presetCategoriesSaved"] is True
    assert payload["result"]["presetAccountsSaved"] is True
    assert seeded_user_ids == [77]


def test_auth_entry_and_refresh_routes_cover_remaining_generic_exceptions(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """登录/注册/邮箱/密码/登出/refresh 剩余兜底异常与坏路径应被锁定。"""
    login_route = _unwrap_all(auth_module.login)
    register_route = _unwrap_all(auth_module.register)
    verify_email_route = _unwrap_all(auth_module.verify_email_by_token)
    resend_guest_route = _unwrap_all(auth_module.resend_verification_email_unauthed)
    forgot_route = _unwrap_all(auth_module.request_password_reset)
    reset_route = _unwrap_all(auth_module.reset_password_by_token)
    oauth_route = _unwrap_all(auth_module.authorize_oauth2_callback)
    logout_route = _unwrap_all(auth_module.logout)
    refresh_route = _unwrap_all(auth_module.refresh_token)

    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "validate_password", lambda *_args, **_kwargs: (True, ""))
    monkeypatch.setattr(
        auth_module,
        "load_auth_config",
        lambda: {
            "enable_user_registration": True,
            "require_email_verification": True,
            "enable_user_forget_password": True,
            "jwt_secret": "secret",
            "jwt_algorithm": "HS256",
        },
    )
    monkeypatch.setattr(
        auth_module,
        "decode_action_token",
        lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"},
    )

    def _raise_runtime_error(message: str):
        def _raiser():
            raise RuntimeError(message)

        return _raiser

    with auth_route_unit_app.test_request_context(
        "/api/auth/login",
        method="POST",
        json={"loginName": "alice", "password": "Valid1!A"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("login boom"))
        response, status = _unwrap_response(login_route())
        assert status == 500
        assert response.get_json()["message"] == "login boom"

    with auth_route_unit_app.test_request_context(
        "/api/auth/register",
        method="POST",
        json={"username": "alice", "email": "alice@example.com", "password": "Valid1!A"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("register boom"))
        response, status = _unwrap_response(register_route())
        assert status == 500
        assert response.get_json()["message"] == "register boom"

    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "ok-token"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("verify boom"))
        response, status = _unwrap_response(verify_email_route())
        assert status == 500
        assert response.get_json()["message"] == "verify boom"

    with auth_route_unit_app.test_request_context(
        "/api/auth/email/resend-verification",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("guest resend boom"))
        response, status = _unwrap_response(resend_guest_route())
        assert status == 500
        assert response.get_json()["message"] == "guest resend boom"

    with auth_route_unit_app.test_request_context(
        "/api/auth/password/forgot",
        method="POST",
        json={"email": "alice@example.com"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("forgot boom"))
        response, status = _unwrap_response(forgot_route())
        assert status == 500
        assert response.get_json()["message"] == "forgot boom"

    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "ok-token"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("reset boom"))
        response, status = _unwrap_response(reset_route())
        assert status == 500
        assert response.get_json()["message"] == "reset boom"

    monkeypatch.setattr(auth_module, "load_auth_config", _raise_runtime_error("oauth boom"))
    with auth_route_unit_app.test_request_context("/api/auth/oauth2/authorize", method="POST"):
        response, status = _unwrap_response(oauth_route())
        assert status == 500
        assert response.get_json()["message"] == "oauth boom"

    monkeypatch.setattr(
        auth_module,
        "load_auth_config",
        lambda: {
            "enable_user_registration": True,
            "require_email_verification": True,
            "enable_user_forget_password": True,
            "jwt_secret": "secret",
            "jwt_algorithm": "HS256",
        },
    )
    with auth_route_unit_app.test_request_context(
        "/api/auth/logout",
        method="POST",
        headers={"Authorization": "Token nope"},
    ):
        response, status = _unwrap_response(logout_route())
        assert status == 401
        assert response.get_json()["message"] == "Invalid authorization header"

    with auth_route_unit_app.test_request_context(
        "/api/auth/logout",
        method="POST",
        headers={"Authorization": "Bearer token"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("logout boom"))
        response, status = _unwrap_response(logout_route())
        assert status == 500
        assert response.get_json()["message"] == "logout boom"

    with auth_route_unit_app.test_request_context("/api/tokens/refresh", method="POST", json={}):
        response, status = _unwrap_response(refresh_route())
        assert status == 400
        assert response.get_json()["message"] == "Refresh token is required"

    def _raise_expired_signature(*_args, **_kwargs):
        raise auth_module.jwt.ExpiredSignatureError("expired")

    monkeypatch.setattr(auth_module.jwt, "decode", _raise_expired_signature)
    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "expired-token"},
    ):
        response, status = _unwrap_response(refresh_route())
        assert status == 401
        assert response.get_json()["error"] == "Token expired"

    def _raise_invalid_token(*_args, **_kwargs):
        raise auth_module.jwt.InvalidTokenError("invalid")

    monkeypatch.setattr(auth_module.jwt, "decode", _raise_invalid_token)
    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "invalid-token"},
    ):
        response, status = _unwrap_response(refresh_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid token"

    monkeypatch.setattr(
        auth_module.jwt,
        "decode",
        lambda *_args, **_kwargs: {"type": "refresh", "user_id": 1, "username": "alice"},
    )
    with auth_route_unit_app.test_request_context(
        "/api/tokens/refresh",
        method="POST",
        json={"refreshToken": "good-token"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("refresh boom"))
        response, status = _unwrap_response(refresh_route())
        assert status == 500
        assert response.get_json()["message"] == "refresh boom"


def test_profile_token_2fa_and_clear_routes_cover_remaining_generic_exceptions(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """profile / token / 2FA / clear 路由剩余兜底异常分支应可回归。"""
    profile_route = _unwrap_all(auth_module.profile)
    generate_api_route = _unwrap_all(auth_module.generate_api_token)
    revoke_route = _unwrap_all(auth_module.revoke_token)
    list_tokens_route = _unwrap_all(auth_module.list_tokens)
    status_route = _unwrap_all(auth_module.get_2fa_status)
    cloud_route = _unwrap_all(auth_module.profile_cloud_settings)
    confirm_route = _unwrap_all(auth_module.enable_2fa_confirm)
    disable_route = _unwrap_all(auth_module.disable_2fa)
    regenerate_route = _unwrap_all(auth_module.regenerate_2fa_recovery_codes)
    verify_route = _unwrap_all(auth_module.verify_2fa_login)
    recovery_route = _unwrap_all(auth_module.verify_2fa_login_by_recovery_code)
    clear_transactions_route = _unwrap_all(auth_module.clear_user_transactions)
    clear_all_route = _unwrap_all(auth_module.clear_all_user_data)

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")
    monkeypatch.setattr(auth_module, "_get_request_session_id", lambda: 101)
    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "load_auth_config", lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"})
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1})
    monkeypatch.setattr(auth_module, "_consume_recovery_code", lambda *_args, **_kwargs: True)

    class AcceptingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return True

    monkeypatch.setattr(auth_module.pyotp, "TOTP", AcceptingTOTP)

    def _raise_runtime_error(message: str):
        def _raiser():
            raise RuntimeError(message)

        return _raiser

    with auth_route_unit_app.test_request_context("/api/profile", method="GET"):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("profile boom"))
        response, status = _unwrap_response(profile_route())
        assert status == 500
        assert response.get_json()["message"] == "profile boom"

    with auth_route_unit_app.test_request_context(
        "/api/tokens/api",
        method="POST",
        json={"password": "Correct123!"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("personal token boom"))
        response, status = _unwrap_response(generate_api_route())
        assert status == 500
        assert response.get_json()["message"] == "personal token boom"

    class ExplodingRevokeDB(FakeAuthDB):
        async def invalidate_session_by_id(self, session_id: int, user_id: int) -> bool:
            _ = (session_id, user_id)
            raise RuntimeError("revoke boom")

    revoke_db = ExplodingRevokeDB(user_by_id={"id": 1})
    revoke_loop = FakeLoop()
    _install_fake_loop(monkeypatch, revoke_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: revoke_db)
    with auth_route_unit_app.test_request_context("/api/tokens/42", method="DELETE"):
        response, status = _unwrap_response(revoke_route("42"))
        assert status == 500
        assert response.get_json()["message"] == "revoke boom"

    with auth_route_unit_app.test_request_context("/api/tokens", method="GET"):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("list tokens boom"))
        response, status = _unwrap_response(list_tokens_route())
        assert status == 500
        assert response.get_json()["message"] == "list tokens boom"

    with auth_route_unit_app.test_request_context("/api/2fa/status", method="GET"):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa status boom"))
        response, status = _unwrap_response(status_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa status boom"

    with auth_route_unit_app.test_request_context("/api/profile/cloud-settings", method="GET"):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("cloud settings boom"))
        response, status = _unwrap_response(cloud_route())
        assert status == 500
        assert response.get_json()["message"] == "cloud settings boom"

    with auth_route_unit_app.test_request_context(
        "/api/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa confirm boom"))
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa confirm boom"

    with auth_route_unit_app.test_request_context(
        "/api/2fa/disable",
        method="POST",
        json={"password": "Correct123!"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa disable boom"))
        response, status = _unwrap_response(disable_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa disable boom"

    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa regenerate boom"))
        response, status = _unwrap_response(regenerate_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa regenerate boom"

    with auth_route_unit_app.test_request_context(
        "/api/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa verify boom"))
        response, status = _unwrap_response(verify_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa verify boom"

    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("2fa recovery boom"))
        response, status = _unwrap_response(recovery_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa recovery boom"

    with auth_route_unit_app.test_request_context(
        "/api/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("clear transactions boom"))
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 500
        assert response.get_json()["message"] == "clear transactions boom"

    with auth_route_unit_app.test_request_context(
        "/api/data/clear/all",
        method="POST",
        json={"password": "ok"},
    ):
        monkeypatch.setattr(auth_module, "get_app_context", _raise_runtime_error("clear all boom"))
        response, status = _unwrap_response(clear_all_route())
        assert status == 500
        assert response.get_json()["message"] == "clear all boom"


def test_auth_route_remaining_branch_closures_and_field_mapping(
    auth_route_unit_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """收尾分支：loop 关闭路径、字段映射和剩余 2FA/token 坏路径应保持稳定。"""
    verify_email_route = _unwrap_all(auth_module.verify_email_by_token)
    resend_guest_route = _unwrap_all(auth_module.resend_verification_email_unauthed)
    forgot_route = _unwrap_all(auth_module.request_password_reset)
    reset_route = _unwrap_all(auth_module.reset_password_by_token)
    logout_route = _unwrap_all(auth_module.logout)
    profile_route = _unwrap_all(auth_module.profile)
    generate_api_route = _unwrap_all(auth_module.generate_api_token)
    revoke_route = _unwrap_all(auth_module.revoke_token)
    list_tokens_route = _unwrap_all(auth_module.list_tokens)
    status_route = _unwrap_all(auth_module.get_2fa_status)
    cloud_route = _unwrap_all(auth_module.profile_cloud_settings)
    confirm_route = _unwrap_all(auth_module.enable_2fa_confirm)
    disable_route = _unwrap_all(auth_module.disable_2fa)
    regenerate_route = _unwrap_all(auth_module.regenerate_2fa_recovery_codes)
    verify_route = _unwrap_all(auth_module.verify_2fa_login)
    recovery_route = _unwrap_all(auth_module.verify_2fa_login_by_recovery_code)
    clear_transactions_route = _unwrap_all(auth_module.clear_user_transactions)
    clear_all_route = _unwrap_all(auth_module.clear_all_user_data)

    monkeypatch.setattr(auth_module, "get_client_ip", lambda: "127.0.0.1")
    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(auth_module, "_get_request_username", lambda: "alice")
    monkeypatch.setattr(auth_module, "_get_request_session_id", lambda: 101)
    monkeypatch.setattr(
        auth_module,
        "load_auth_config",
        lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256", "enable_user_forget_password": True},
    )
    monkeypatch.setattr(
        auth_module,
        "decode_action_token",
        lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"},
    )
    monkeypatch.setattr(auth_module, "serialize_keyword_list", lambda values: "|".join(values))

    class AcceptingTOTP:
        def __init__(self, _secret: str) -> None:
            pass

        def verify(self, _passcode: str, valid_window: int = 1) -> bool:
            _ = valid_window
            return True

    monkeypatch.setattr(auth_module.pyotp, "TOTP", AcceptingTOTP)
    monkeypatch.setattr(
        auth_module,
        "_create_new_session_payload",
        lambda *_args, **_kwargs: {"access_token": "session", "refresh_token": "refresh"},
    )
    monkeypatch.setattr(auth_module, "_generate_recovery_codes", lambda: ["ABCD-1234"])
    monkeypatch.setattr(auth_module, "_build_auth_success_result", lambda user, tokens, settings=None: {"token": tokens["access_token"], "user": user, "applicationCloudSettings": settings or []})
    monkeypatch.setattr(auth_module, "_build_user_profile_info", lambda user: {"username": user.get("username", "alice")})
    monkeypatch.setattr(auth_module, "_load_application_cloud_settings", lambda *_args, **_kwargs: [{"settingKey": "k", "settingValue": "v"}])
    monkeypatch.setattr(auth_module, "_consume_recovery_code", lambda *_args, **_kwargs: True)

    def _async_runtime_error(message: str):
        async def _raiser(*_args, **_kwargs):
            raise RuntimeError(message)

        return _raiser

    def _async_value_error(message: str):
        async def _raiser(*_args, **_kwargs):
            raise ValueError(message)

        return _raiser

    class VerifyEmailUpdateThenMissingDB(FakeAuthDB):
        def __init__(self) -> None:
            super().__init__(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
            self.lookup_count = 0

        async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
            self.lookup_count += 1
            if self.lookup_count == 1:
                return await super().get_user_by_id(user_id)
            return None

    verify_missing_db = VerifyEmailUpdateThenMissingDB()
    verify_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "ok-token"},
    ):
        response, status = _unwrap_response(verify_email_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    verify_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    verify_error_db.get_user_by_id = _async_runtime_error("verify lookup boom")  # type: ignore[method-assign]
    verify_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/verify",
        method="POST",
        json={"token": "ok-token"},
    ):
        response, status = _unwrap_response(verify_email_route())
        assert status == 500
        assert response.get_json()["message"] == "verify lookup boom"

    forgot_error_db = FakeAuthDB(user_by_email={"id": 1, "username": "alice", "email": "alice@example.com"})
    forgot_error_db.get_user_by_email = _async_runtime_error("forgot lookup boom")  # type: ignore[method-assign]
    forgot_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, forgot_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: forgot_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/forgot",
        method="POST",
        json={"email": "alice@example.com"},
    ):
        response, status = _unwrap_response(forgot_route())
        assert status == 500
        assert response.get_json()["message"] == "forgot lookup boom"

    reset_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    reset_error_db.get_user_by_id = _async_runtime_error("reset lookup boom")  # type: ignore[method-assign]
    reset_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, reset_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: reset_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/password/reset",
        method="POST",
        json={"email": "alice@example.com", "password": "Valid1!A", "token": "ok-token"},
    ):
        response, status = _unwrap_response(reset_route())
        assert status == 500
        assert response.get_json()["message"] == "reset lookup boom"

    logout_missing_session_db = FakeAuthDB(session=None)
    logout_missing_session_loop = FakeLoop()
    _install_fake_loop(monkeypatch, logout_missing_session_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: logout_missing_session_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/logout",
        method="POST",
        headers={"Authorization": "Bearer token"},
    ):
        response, status = _unwrap_response(logout_route())
        assert status == 200
        assert response.get_json()["result"] is True

    profile_fields_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    profile_fields_loop = FakeLoop()
    _install_fake_loop(monkeypatch, profile_fields_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: profile_fields_db)
    with auth_route_unit_app.test_request_context(
        "/api/profile",
        method="PUT",
        json={
            "email": "new@example.com",
            "avatar": "data:image/png;base64,abc",
            "firstDayOfWeek": 1,
            "defaultAccountId": "11",
            "transactionEditScope": 2,
            "fiscalYearStart": 99,
            "calendarDisplayType": 1,
            "dateDisplayType": 2,
            "longDateFormat": 3,
            "shortDateFormat": 4,
            "longTimeFormat": 5,
            "shortTimeFormat": 6,
            "fiscalYearFormat": 7,
            "currencyDisplayType": 8,
            "numeralSystem": 9,
            "decimalSeparator": 10,
            "digitGroupingSymbol": 11,
            "digitGrouping": 12,
            "coordinateDisplayType": 13,
            "expenseAmountColor": 14,
            "incomeAmountColor": 15,
            "cashAccountId": "16",
            "cashTransferCategoryId": "17",
            "investmentProductKeywords": ["ETF", "债券"],
            "investmentExcludeKeywords": ["体验金"],
        },
    ):
        response, status = _unwrap_response(profile_route())
        assert status == 200
        updated_fields = profile_fields_db.updated_users[0][1]
        assert updated_fields["email"] == "new@example.com"
        assert updated_fields["avatar"] == "data:image/png;base64,abc"
        assert updated_fields["first_day_of_week"] == 1
        assert updated_fields["default_account_id"] == "11"
        assert updated_fields["transaction_edit_scope"] == 2
        assert updated_fields["fiscal_year_start"] == 99
        assert updated_fields["calendar_display_type"] == 1
        assert updated_fields["date_display_type"] == 2
        assert updated_fields["long_date_format"] == 3
        assert updated_fields["short_date_format"] == 4
        assert updated_fields["long_time_format"] == 5
        assert updated_fields["short_time_format"] == 6
        assert updated_fields["fiscal_year_format"] == 7
        assert updated_fields["currency_display_type"] == 8
        assert updated_fields["numeral_system"] == 9
        assert updated_fields["decimal_separator"] == 10
        assert updated_fields["digit_grouping_symbol"] == 11
        assert updated_fields["digit_grouping"] == 12
        assert updated_fields["coordinate_display_type"] == 13
        assert updated_fields["expense_amount_color"] == 14
        assert updated_fields["income_amount_color"] == 15
        assert updated_fields["cash_account_id"] == "16"
        assert updated_fields["cash_transfer_category_id"] == "17"
        assert updated_fields["investment_product_keywords"] == "ETF|债券"
        assert updated_fields["investment_exclude_keywords"] == "体验金"

    profile_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    profile_error_db.get_user_by_id = _async_runtime_error("profile lookup boom")  # type: ignore[method-assign]
    profile_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, profile_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: profile_error_db)
    with auth_route_unit_app.test_request_context("/api/profile", method="GET"):
        response, status = _unwrap_response(profile_route())
        assert status == 500
        assert response.get_json()["message"] == "profile lookup boom"

    avatar_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "email": "alice@example.com"})
    avatar_loop = FakeLoop()
    _install_fake_loop(monkeypatch, avatar_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: avatar_db)
    monkeypatch.setattr(auth_module, "_build_avatar_data_url", lambda _file: "data:image/png;base64,abc")
    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: (_ for _ in ()).throw(ValueError("avatar id boom")))
    with auth_route_unit_app.test_request_context(
        "/api/profile/avatar",
        method="POST",
        data={"avatar": (io.BytesIO(b"avatar-bytes"), "avatar.png")},
    ):
        response, status = _unwrap_response(_unwrap_all(auth_module.update_profile_avatar)())
        assert status == 400
        assert response.get_json()["message"] == "avatar id boom"

    monkeypatch.setattr(auth_module, "_get_request_user_id", lambda: 1)

    hashed_password = bcrypt.hashpw(b"Correct123!", bcrypt.gensalt()).decode("utf-8")

    resend_guest_error_db = FakeAuthDB(
        user_by_email={"id": 1, "username": "alice", "email": "alice@example.com", "password_hash": hashed_password}
    )
    resend_guest_error_db.get_user_by_email = _async_runtime_error("resend guest lookup boom")  # type: ignore[method-assign]
    resend_guest_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, resend_guest_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: resend_guest_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/auth/email/resend-verification",
        method="POST",
        json={"email": "alice@example.com", "password": "Correct123!"},
    ):
        response, status = _unwrap_response(resend_guest_route())
        assert status == 500
        assert response.get_json()["message"] == "resend guest lookup boom"

    value_error_token_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password})
    value_error_token_loop = FakeLoop()
    _install_fake_loop(monkeypatch, value_error_token_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: value_error_token_db)

    def _raise_generate_access_token(*_args, **_kwargs):
        raise ValueError("generate token boom")

    monkeypatch.setattr(auth_module, "generate_access_token", _raise_generate_access_token)
    with auth_route_unit_app.test_request_context(
        "/api/tokens/api",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    runtime_error_token_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "password_hash": hashed_password})
    runtime_error_token_db.create_session = _async_runtime_error("session boom")  # type: ignore[method-assign]
    runtime_error_token_loop = FakeLoop()
    _install_fake_loop(monkeypatch, runtime_error_token_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: runtime_error_token_db)
    monkeypatch.setattr(
        auth_module,
        "generate_access_token",
        lambda *_args, **_kwargs: {"access_token": "token-abc", "expires_at": "2026-03-10T10:00:00"},
    )
    with auth_route_unit_app.test_request_context(
        "/api/tokens/api",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(generate_api_route())
        assert status == 500
        assert response.get_json()["message"] == "session boom"

    revoke_value_error_db = FakeAuthDB(user_by_id={"id": 1})
    revoke_value_error_db.invalidate_session_by_id = _async_value_error("revoke value boom")  # type: ignore[method-assign]
    revoke_value_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, revoke_value_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: revoke_value_error_db)
    with auth_route_unit_app.test_request_context("/api/tokens/42", method="DELETE"):
        response, status = _unwrap_response(revoke_route("42"))
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    list_error_db = FakeAuthDB(user_by_id={"id": 1})
    list_error_db.cleanup_expired_sessions = _async_runtime_error("cleanup boom")  # type: ignore[method-assign]
    list_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, list_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: list_error_db)
    with auth_route_unit_app.test_request_context("/api/tokens", method="GET"):
        response, status = _unwrap_response(list_tokens_route())
        assert status == 500
        assert response.get_json()["message"] == "cleanup boom"

    status_error_db = FakeAuthDB(user_by_id={"id": 1})
    status_error_db.get_user_by_id = _async_runtime_error("status lookup boom")  # type: ignore[method-assign]
    status_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, status_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: status_error_db)
    with auth_route_unit_app.test_request_context("/api/2fa/status", method="GET"):
        response, status = _unwrap_response(status_route())
        assert status == 500
        assert response.get_json()["message"] == "status lookup boom"

    cloud_error_db = FakeAuthDB(user_by_id={"id": 1})
    cloud_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, cloud_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: cloud_error_db)
    monkeypatch.setattr(auth_module, "_load_application_cloud_settings", lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("cloud lookup boom")))
    with auth_route_unit_app.test_request_context("/api/profile/cloud-settings", method="GET"):
        response, status = _unwrap_response(cloud_route())
        assert status == 500
        assert response.get_json()["message"] == "cloud lookup boom"

    confirm_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice"})
    confirm_error_db.update_user = _async_runtime_error("confirm update boom")  # type: ignore[method-assign]
    confirm_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, confirm_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: confirm_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/enable/confirm",
        method="POST",
        json={"secret": "SECRET123", "passcode": "111111"},
    ):
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["message"] == "confirm update boom"

    disable_error_user = {"id": 1, "username": "alice", "password_hash": hashed_password}
    disable_error_db = FakeAuthDB(user_by_id=dict(disable_error_user))
    disable_error_db.update_user = _async_runtime_error("disable update boom")  # type: ignore[method-assign]
    disable_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, disable_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: disable_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/disable",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(disable_route())
        assert status == 500
        assert response.get_json()["message"] == "disable update boom"

    regenerate_missing_db = FakeAuthDB(user_by_id=None)
    regenerate_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regenerate_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regenerate_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    regenerate_invalid_password_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": hashed_password, "two_factor_enabled": 1})
    regenerate_invalid_password_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regenerate_invalid_password_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regenerate_invalid_password_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Wrong123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 401
        assert response.get_json()["error"] == "Invalid credentials"

    regenerate_error_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": hashed_password, "two_factor_enabled": 1})
    regenerate_error_db.get_user_by_id = _async_runtime_error("regenerate lookup boom")  # type: ignore[method-assign]
    regenerate_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, regenerate_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: regenerate_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/regenerate",
        method="POST",
        json={"password": "Correct123!"},
    ):
        response, status = _unwrap_response(regenerate_route())
        assert status == 500
        assert response.get_json()["message"] == "regenerate lookup boom"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": "bad", "email": "alice@example.com"})
    with auth_route_unit_app.test_request_context(
        "/api/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 401
        assert response.get_json()["error"] == "Unauthorized"

    verify_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "two_factor_enabled": 1, "two_factor_secret": "SECRET123"})
    verify_error_db.get_user_by_id = _async_runtime_error("2fa verify lookup boom")  # type: ignore[method-assign]
    verify_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, verify_error_loop)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"})
    monkeypatch.setattr(auth_module, "get_app_context", lambda: verify_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"passcode": "123456"},
    ):
        response, status = _unwrap_response(verify_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa verify lookup boom"

    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": "bad", "email": "alice@example.com"})
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 401
        assert response.get_json()["error"] == "Unauthorized"

    recovery_error_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice"})
    recovery_error_db.get_user_by_id = _async_runtime_error("2fa recovery lookup boom")  # type: ignore[method-assign]
    recovery_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, recovery_error_loop)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"})
    monkeypatch.setattr(auth_module, "get_app_context", lambda: recovery_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 500
        assert response.get_json()["message"] == "2fa recovery lookup boom"

    recovery_not_enabled_db = FakeAuthDB(user_by_id={"id": 1, "username": "alice", "two_factor_enabled": 0})
    recovery_not_enabled_loop = FakeLoop()
    _install_fake_loop(monkeypatch, recovery_not_enabled_loop)
    monkeypatch.setattr(auth_module, "decode_action_token", lambda *_args, **_kwargs: {"user_id": 1, "email": "alice@example.com"})
    monkeypatch.setattr(auth_module, "get_app_context", lambda: recovery_not_enabled_db)
    with auth_route_unit_app.test_request_context(
        "/api/2fa/recovery/verify",
        method="POST",
        headers={"Authorization": "Bearer token"},
        json={"recoveryCode": "ABCD-1234"},
    ):
        response, status = _unwrap_response(recovery_route())
        assert status == 400
        assert response.get_json()["message"] == "Two-factor authentication is not enabled"

    clear_transactions_missing_db = FakeAuthDB(user_by_id=None)
    clear_transactions_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, clear_transactions_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_transactions_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    clear_all_missing_db = FakeAuthDB(user_by_id=None)
    clear_all_missing_loop = FakeLoop()
    _install_fake_loop(monkeypatch, clear_all_missing_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_all_missing_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/all",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    clear_transactions_error_db = FakeAuthDB(user_by_id={"id": 1})
    clear_transactions_error_db.verify_operation_password = _async_runtime_error("clear transactions verify boom")  # type: ignore[method-assign]
    clear_transactions_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, clear_transactions_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_transactions_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 500
        assert response.get_json()["message"] == "clear transactions verify boom"

    clear_all_error_db = FakeAuthDB(user_by_id={"id": 1})
    clear_all_error_db.verify_operation_password = _async_runtime_error("clear all verify boom")  # type: ignore[method-assign]
    clear_all_error_loop = FakeLoop()
    _install_fake_loop(monkeypatch, clear_all_error_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: clear_all_error_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/all",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 500
        assert response.get_json()["message"] == "clear all verify boom"

    bad_hash_clear_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": "not-a-valid-bcrypt-hash"})
    bad_hash_clear_loop = FakeLoop()
    _install_fake_loop(monkeypatch, bad_hash_clear_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: bad_hash_clear_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/transactions",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_transactions_route())
        assert status == 200
        assert response.get_json()["success"] is True

    bad_hash_clear_all_db = FakeAuthDB(user_by_id={"id": 1, "password_hash": "not-a-valid-bcrypt-hash"})
    bad_hash_clear_all_loop = FakeLoop()
    _install_fake_loop(monkeypatch, bad_hash_clear_all_loop)
    monkeypatch.setattr(auth_module, "get_app_context", lambda: bad_hash_clear_all_db)
    with auth_route_unit_app.test_request_context(
        "/api/data/clear/all",
        method="POST",
        json={"password": "ok"},
    ):
        response, status = _unwrap_response(clear_all_route())
        assert status == 200
        assert response.get_json()["success"] is True
