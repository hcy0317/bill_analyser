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
        self.session = session
        self.cloud_settings = cloud_settings or []
        self.updated_users: list[tuple[int, dict[str, Any]]] = []
        self.auth_logs: list[dict[str, Any]] = []
        self.invalidated_tokens: list[str] = []
        self.created_sessions: list[dict[str, Any]] = []

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

    async def get_user_application_cloud_settings(self, _user_id: int) -> list[dict[str, Any]]:
        return self.cloud_settings

    async def get_session_by_token_hash(self, token_hash: str) -> dict[str, Any] | None:
        self.invalidated_tokens.append(f"lookup:{token_hash}")
        return self.session

    async def invalidate_session(self, token_hash: str) -> None:
        self.invalidated_tokens.append(token_hash)

    async def create_session(self, payload: dict[str, Any]) -> None:
        self.created_sessions.append(payload)



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
    monkeypatch.setattr(auth_module, "serialize_keyword_list", lambda items: "|".join(items))

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
