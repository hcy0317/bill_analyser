from __future__ import annotations

from datetime import datetime, timedelta
from typing import Any

from flask import Flask, request
import pytest
from jwt.exceptions import ExpiredSignatureError, InvalidTokenError

from bill_analyser.api.middleware import auth as auth_module


class FakeDB:
    """Minimal async DB stub for auth middleware tests."""

    def __init__(
        self,
        session: dict[str, Any] | None = None,
        lookup_error: Exception | None = None,
        update_error: Exception | None = None,
    ) -> None:
        self.session = session
        self.lookup_error = lookup_error
        self.update_error = update_error
        self.token_hashes: list[str] = []
        self.updated_session_ids: list[int] = []

    async def get_session_by_token_hash(self, token_hash: str) -> dict[str, Any] | None:
        self.token_hashes.append(token_hash)
        if self.lookup_error is not None:
            raise self.lookup_error
        return self.session

    async def update_session_activity(self, session_id: int) -> None:
        self.updated_session_ids.append(session_id)
        if self.update_error is not None:
            raise self.update_error


@pytest.fixture(name="auth_app")
def auth_app_fixture() -> Flask:
    """Create a tiny Flask app for auth middleware tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


@pytest.fixture(name="configured_auth")
def configured_auth_fixture(monkeypatch: pytest.MonkeyPatch) -> None:
    """Provide a stable auth configuration for middleware tests."""
    monkeypatch.setattr(
        auth_module,
        "get_auth_config",
        lambda: {"jwt_secret": "secret", "jwt_algorithm": "HS256"},
    )



def _active_session() -> dict[str, Any]:
    return {
        "id": 7,
        "user_id": 42,
        "username": "alice",
        "email": "alice@example.com",
        "expires_at": (datetime.now() + timedelta(hours=1)).isoformat(),
        "user_is_active": True,
    }



def _expired_session() -> dict[str, Any]:
    session = _active_session()
    session["expires_at"] = (datetime.now() - timedelta(hours=1)).isoformat()
    return session



def _make_protected_endpoint() -> Any:
    @auth_module.require_auth
    def protected() -> dict[str, Any]:
        return {
            "user_id": getattr(request, "user_id"),
            "username": getattr(request, "username"),
            "email": getattr(request, "user_email"),
            "session_id": getattr(request, "session_id"),
        }

    return protected



def _make_optional_endpoint() -> Any:
    @auth_module.optional_auth
    def endpoint() -> dict[str, Any]:
        return {
            "user_id": getattr(request, "user_id"),
            "username": getattr(request, "username"),
            "email": getattr(request, "user_email"),
            "session_id": getattr(request, "session_id"),
        }

    return endpoint



def _unwrap_error_result(result: Any) -> tuple[dict[str, Any], int]:
    response, status = result
    return response.get_json(), status



def test_get_auth_config_delegates_to_load_auth_settings(monkeypatch: pytest.MonkeyPatch) -> None:
    """get_auth_config 应直接委托给配置加载函数。"""
    monkeypatch.setattr(auth_module, "load_auth_settings", lambda: {"jwt_secret": "configured"})

    assert auth_module.get_auth_config() == {"jwt_secret": "configured"}



def test_get_db_reads_current_app_config(auth_app: Flask) -> None:
    """get_db 应返回当前 Flask app 中的数据库实例。"""
    sentinel_db = object()
    auth_app.config["DB_INSTANCE"] = sentinel_db

    with auth_app.app_context():
        assert auth_module.get_db() is sentinel_db



def test_get_client_ip_prefers_forwarded_and_real_ip_headers(auth_app: Flask) -> None:
    """客户端 IP 提取顺序应为 X-Forwarded-For > X-Real-IP > remote_addr。"""
    with auth_app.test_request_context(
        "/",
        headers={"X-Forwarded-For": "10.0.0.1, 10.0.0.2", "X-Real-IP": "10.0.0.9"},
        environ_overrides={"REMOTE_ADDR": "127.0.0.1"},
    ):
        assert auth_module.get_client_ip() == "10.0.0.1"

    with auth_app.test_request_context(
        "/",
        headers={"X-Real-IP": "10.0.0.9"},
        environ_overrides={"REMOTE_ADDR": "127.0.0.1"},
    ):
        assert auth_module.get_client_ip() == "10.0.0.9"

    with auth_app.test_request_context("/", environ_overrides={"REMOTE_ADDR": "127.0.0.1"}):
        assert auth_module.get_client_ip() == "127.0.0.1"



def test_calculate_token_hash_matches_sha256_contract() -> None:
    """令牌哈希应是稳定的 SHA256 十六进制字符串。"""
    assert (
        auth_module.calculate_token_hash("demo-token")
        == "7c43ef5ae21d43ce2743f770c68e24def1a43ee2f416d2438410c8af7af2ff2c"
    )



def test_require_auth_rejects_missing_and_malformed_authorization_headers(
    auth_app: Flask,
) -> None:
    """必选认证应拒绝缺失头和错误的 Bearer 格式。"""
    protected = _make_protected_endpoint()

    with auth_app.test_request_context("/"):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Missing authorization header"

    with auth_app.test_request_context("/", headers={"Authorization": "Token abc"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Invalid authorization header format"



@pytest.mark.usefixtures("configured_auth")
def test_require_auth_rejects_token_errors(auth_app: Flask, monkeypatch: pytest.MonkeyPatch) -> None:
    """JWT 过期和无效 token 都应返回 401。"""
    protected = _make_protected_endpoint()

    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: (_ for _ in ()).throw(ExpiredSignatureError()))
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer expired"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Token expired"

    monkeypatch.setattr(
        auth_module.jwt,
        "decode",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(InvalidTokenError("bad token")),
    )
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer invalid"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Invalid token"



@pytest.mark.usefixtures("configured_auth")
def test_require_auth_handles_missing_db_invalid_session_expired_and_inactive_user(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """数据库缺失、会话不存在、过期和停用用户都应被拒绝。"""
    protected = _make_protected_endpoint()
    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})

    monkeypatch.setattr(auth_module, "get_db", lambda: None)
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer token"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 500
        assert payload["message"] == "Database not initialized"

    monkeypatch.setattr(auth_module, "get_db", lambda: FakeDB(session=None))
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer token"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Invalid or expired session"

    monkeypatch.setattr(auth_module, "get_db", lambda: FakeDB(session=_expired_session()))
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer token"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "Session expired"

    inactive_session = _active_session()
    inactive_session["user_is_active"] = False
    monkeypatch.setattr(auth_module, "get_db", lambda: FakeDB(session=inactive_session))
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer token"}):
        payload, status = _unwrap_error_result(protected())
        assert status == 401
        assert payload["message"] == "User account is not active"



@pytest.mark.usefixtures("configured_auth")
def test_require_auth_injects_request_user_context_on_success(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证成功时应更新会话活动时间并把用户信息注入 request。"""
    protected = _make_protected_endpoint()
    fake_db = FakeDB(session=_active_session())
    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})
    monkeypatch.setattr(auth_module, "get_db", lambda: fake_db)

    with auth_app.test_request_context("/secure", headers={"Authorization": "Bearer valid-token"}):
        result = protected()

    assert result == {
        "user_id": 42,
        "username": "alice",
        "email": "alice@example.com",
        "session_id": 7,
    }
    assert fake_db.token_hashes == [auth_module.calculate_token_hash("valid-token")]
    assert fake_db.updated_session_ids == [7]



@pytest.mark.usefixtures("configured_auth")
def test_require_auth_returns_500_when_session_lookup_raises_expected_errors(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证流程中的运行时错误应被转换为 500。"""
    protected = _make_protected_endpoint()
    fake_db = FakeDB(lookup_error=RuntimeError("db down"))
    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})
    monkeypatch.setattr(auth_module, "get_db", lambda: fake_db)

    with auth_app.test_request_context("/secure", headers={"Authorization": "Bearer valid-token"}):
        payload, status = _unwrap_error_result(protected())

    assert status == 500
    assert payload["message"] == "Authentication failed"



def test_optional_auth_sets_default_context_without_header(auth_app: Flask) -> None:
    """可选认证在没有凭据时应继续执行，并填充空上下文。"""
    endpoint = _make_optional_endpoint()

    with auth_app.test_request_context("/"):
        result = endpoint()

    assert result == {"user_id": None, "username": None, "email": None, "session_id": None}



@pytest.mark.usefixtures("configured_auth")
def test_optional_auth_ignores_malformed_headers_and_missing_db(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """格式错误的头应返回 401；鉴权基础设施缺失时应显式失败。"""
    endpoint = _make_optional_endpoint()
    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})

    with auth_app.test_request_context("/", headers={"Authorization": "Token abc"}):
        payload, status = _unwrap_error_result(endpoint())
        assert status == 401
        assert payload["message"] == "Invalid authorization header format"

    monkeypatch.setattr(auth_module, "get_db", lambda: None)
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer abc"}):
        payload, status = _unwrap_error_result(endpoint())
        assert status == 503
        assert payload["message"] == "Authentication temporarily unavailable"



@pytest.mark.usefixtures("configured_auth")
def test_optional_auth_injects_context_for_active_session(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """可选认证遇到有效会话时应注入用户上下文。"""
    endpoint = _make_optional_endpoint()
    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})
    monkeypatch.setattr(auth_module, "get_db", lambda: FakeDB(session=_active_session()))

    with auth_app.test_request_context("/", headers={"Authorization": "Bearer valid-token"}):
        result = endpoint()

    assert result == {
        "user_id": 42,
        "username": "alice",
        "email": "alice@example.com",
        "session_id": 7,
    }



@pytest.mark.usefixtures("configured_auth")
def test_optional_auth_ignores_invalid_tokens_and_runtime_errors(
    auth_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """可选认证对无效 token 返回 401，对内部运行时错误返回 503。"""
    endpoint = _make_optional_endpoint()

    monkeypatch.setattr(
        auth_module.jwt,
        "decode",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(InvalidTokenError("bad token")),
    )
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer bad-token"}):
        payload, status = _unwrap_error_result(endpoint())
        assert status == 401
        assert payload["message"] == "Invalid token"

    monkeypatch.setattr(
        auth_module.jwt,
        "decode",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(ExpiredSignatureError()),
    )
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer expired-token"}):
        payload, status = _unwrap_error_result(endpoint())
        assert status == 401
        assert payload["message"] == "Token expired"

    monkeypatch.setattr(auth_module.jwt, "decode", lambda *_args, **_kwargs: {"sub": 42})
    monkeypatch.setattr(auth_module, "get_db", lambda: FakeDB(lookup_error=RuntimeError("boom")))
    with auth_app.test_request_context("/", headers={"Authorization": "Bearer maybe-token"}):
        payload, status = _unwrap_error_result(endpoint())
        assert status == 503
        assert payload["message"] == "Authentication temporarily unavailable"
