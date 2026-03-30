# pyright: reportPrivateUsage=false
from __future__ import annotations

import asyncio
import base64
import re
from datetime import UTC, datetime
from typing import Any, cast

import bcrypt
import jwt
import pytest
from flask import Flask

from bill_analyser.api.routes import auth as auth_module

auth_module = cast("Any", auth_module)


@pytest.fixture(name="auth_helper_app")
def auth_helper_app_fixture() -> Flask:
    """Create a tiny Flask app for auth helper tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeUploadedFile:
    """Minimal upload stub for avatar helper tests."""

    def __init__(self, content: bytes, mimetype: str = "text/plain", filename: str = "avatar.txt") -> None:
        self._content = content
        self.mimetype = mimetype
        self.filename = filename

    def read(self) -> bytes:
        return self._content


class FakeQRCode:
    """Minimal QR image stub."""

    def save(self, buffer, fmt: str) -> None:
        assert fmt == "PNG"
        buffer.write(b"fake-png-bytes")


class FakeLoop:
    """Simple loop adapter for helper tests."""

    def run_until_complete(self, coroutine):
        return asyncio.run(coroutine)


class FakeCloudSettingsDB:
    """Minimal DB stub for cloud settings helpers."""

    async def get_user_application_cloud_settings(self, user_id: int) -> list[dict[str, Any]]:
        return [
            {"setting_key": f"setting-{user_id}", "setting_value": "true"},
        ]


class FakeSessionDB:
    """Minimal DB stub for new session helper."""

    def __init__(self) -> None:
        self.session_payloads: list[dict[str, Any]] = []

    async def create_session(self, payload: dict[str, Any]) -> None:
        self.session_payloads.append(payload)



def test_load_auth_config_and_get_app_context_cover_primary_fallback_and_error(
    auth_helper_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证配置和 DB 上下文 helper 应覆盖主路径、回退路径与错误路径。"""
    sentinel_db = object()
    monkeypatch.setattr(auth_module, "load_server_auth_settings", lambda: {"jwt_secret": "configured"})
    assert auth_module.load_auth_config() == {"jwt_secret": "configured"}

    with auth_helper_app.app_context():
        auth_helper_app.config["DB_INSTANCE"] = sentinel_db
        assert auth_module.get_app_context() is sentinel_db

        import bill_analyser.api.app as app_module

        auth_helper_app.config["DB_INSTANCE"] = None
        monkeypatch.setattr(app_module, "db", sentinel_db, raising=False)
        assert auth_module.get_app_context() is sentinel_db

        monkeypatch.setattr(app_module, "db", None, raising=False)
        with pytest.raises(RuntimeError, match="Database not initialized"):
            auth_module.get_app_context()



def test_request_helpers_cover_ip_bearer_url_and_token_type_variants(auth_helper_app: Flask) -> None:
    """请求相关 helper 应覆盖 IP、Bearer、URL 与 token type 推断。"""
    with auth_helper_app.test_request_context(
        "/api/profile",
        base_url="http://localhost:5000",
        headers={
            "X-Forwarded-For": "10.0.0.1, 10.0.0.2",
            "X-Real-IP": "10.0.0.9",
            "Authorization": "Bearer demo-token",
            "User-Agent": "Browser Agent",
        },
        environ_overrides={"REMOTE_ADDR": "127.0.0.1"},
    ):
        assert auth_module.get_client_ip() == "10.0.0.1"
        assert auth_module._extract_bearer_token() == "demo-token"
        assert auth_module._build_api_base_url() == "http://localhost:5000/api"
        assert auth_module._build_mcp_url() == "http://localhost:5000/mcp"
        assert auth_module._get_token_user_agent("session") == "Browser Agent"

    with auth_helper_app.test_request_context(
        "/",
        headers={"X-Real-IP": "10.0.0.9", "Authorization": "Token nope"},
        environ_overrides={"REMOTE_ADDR": "127.0.0.1"},
    ):
        assert auth_module.get_client_ip() == "10.0.0.9"
        assert auth_module._extract_bearer_token() == ""

    with auth_helper_app.test_request_context("/", environ_overrides={"REMOTE_ADDR": "127.0.0.1"}):
        assert auth_module.get_client_ip() == "127.0.0.1"

    monkeypatch = pytest.MonkeyPatch()
    monkeypatch.setattr(auth_module, "get_required_request_int", lambda name: 12 if name == "user_id" else 34)
    monkeypatch.setattr(
        auth_module,
        "get_required_request_str",
        lambda name: "alice" if name == "username" else "",
    )
    try:
        assert auth_module._get_request_user_id() == 12
        assert auth_module._get_request_username() == "alice"
        assert auth_module._get_request_session_id() == 34
    finally:
        monkeypatch.undo()

    assert auth_module._get_token_user_agent("api") == "Bill Analyser API Token"
    assert auth_module._get_token_user_agent("mcp") == "Bill Analyser MCP Token"
    assert auth_module._infer_token_type("Bill Analyser API Token") == auth_module.TOKEN_TYPE_API
    assert auth_module._infer_token_type("Bill Analyser MCP Token") == auth_module.TOKEN_TYPE_MCP
    assert auth_module._infer_token_type("browser") == auth_module.TOKEN_TYPE_DEFAULT



def test_token_generation_password_validation_and_action_token_helpers() -> None:
    """JWT、动作 token、密码校验和密码哈希 helper 应覆盖关键分支。"""
    config = {
        "jwt_secret": "test-secret",
        "jwt_algorithm": "HS256",
        "jwt_expiration_days": 1,
        "refresh_token_expiration_days": 3,
        "password_min_length": 8,
        "password_require_uppercase": True,
        "password_require_lowercase": True,
        "password_require_digit": True,
        "password_require_special": True,
    }

    access_payload = auth_module.generate_access_token(1, "alice", config, expires_in_seconds=90, token_kind="api")
    decoded_access = jwt.decode(
        access_payload["access_token"],
        config["jwt_secret"],
        algorithms=[config["jwt_algorithm"]],
    )
    assert decoded_access["user_id"] == 1
    assert decoded_access["username"] == "alice"
    assert decoded_access["token_kind"] == "api"
    assert decoded_access["type"] == "access"
    assert decoded_access["exp"] > decoded_access["iat"]

    long_lived_access = auth_module.generate_access_token(1, "alice", config)
    decoded_long_lived = jwt.decode(
        long_lived_access["access_token"],
        config["jwt_secret"],
        algorithms=[config["jwt_algorithm"]],
    )
    assert decoded_long_lived["token_kind"] == "session"
    assert decoded_long_lived["exp"] - decoded_long_lived["iat"] > 60 * 60 * 24 * 365

    session_tokens = auth_module.generate_jwt_token(2, "bob", config)
    decoded_session_access = jwt.decode(
        session_tokens["access_token"],
        config["jwt_secret"],
        algorithms=[config["jwt_algorithm"]],
    )
    decoded_session_refresh = jwt.decode(
        session_tokens["refresh_token"],
        config["jwt_secret"],
        algorithms=[config["jwt_algorithm"]],
    )
    assert decoded_session_access["type"] == "access"
    assert decoded_session_refresh["type"] == "refresh"

    action_token = auth_module.generate_action_token(3, "carol", "carol@example.com", config, "verify_email")
    decoded_action = auth_module.decode_action_token(action_token, config, "verify_email")
    assert decoded_action is not None
    assert decoded_action["email"] == "carol@example.com"
    assert auth_module.decode_action_token(action_token, config, "reset_password") is None
    assert auth_module.decode_action_token("broken-token", config, "verify_email") is None

    hashed_password = bcrypt.hashpw(b"Str0ng!Pass", bcrypt.gensalt()).decode("utf-8")
    assert auth_module._verify_user_password({"password_hash": hashed_password}, "Str0ng!Pass") is True
    assert auth_module._verify_user_password({"password_hash": hashed_password}, "wrong") is False
    assert auth_module._verify_user_password({}, "anything") is False

    assert auth_module.validate_password("short", config) == (False, "Password must be at least 8 characters long")
    assert auth_module.validate_password("lower123!", config) == (
        False,
        "Password must contain at least one uppercase letter",
    )
    assert auth_module.validate_password("UPPER123!", config) == (
        False,
        "Password must contain at least one lowercase letter",
    )
    assert auth_module.validate_password("NoDigits!!", config) == (False, "Password must contain at least one digit")
    assert auth_module.validate_password("NoSpecial1", config) == (
        False,
        "Password must contain at least one special character",
    )
    assert auth_module.validate_password("Valid1!A", config) == (True, "")


@pytest.mark.parametrize(
    ("setting", "expected_error"),
    [
        ({"settingKey": "", "settingValue": ""}, "settingKey is required"),
        ({"settingKey": "unknown", "settingValue": "1"}, "Unsupported setting key: unknown"),
        (
            {"settingKey": "autoSaveTransactionDraft", "settingValue": 123},
            "Invalid setting value for autoSaveTransactionDraft",
        ),
        (
            {"settingKey": "timezoneUsedForStatisticsInHomePage", "settingValue": "not-a-number"},
            "Invalid number value for timezoneUsedForStatisticsInHomePage",
        ),
        (
            {"settingKey": "showAmountInHomePage", "settingValue": "yes"},
            "Invalid boolean value for showAmountInHomePage",
        ),
        (
            {"settingKey": "overviewAccountFilterInHomePage", "settingValue": '{"a":"true"}'},
            "Invalid map value for overviewAccountFilterInHomePage",
        ),
        (
            {"settingKey": "overviewAccountFilterInHomePage", "settingValue": "[1,2,3]"},
            "Invalid map value for overviewAccountFilterInHomePage",
        ),
        ({"settingKey": "autoSaveTransactionDraft", "settingValue": "manual"}, ""),
        ({"settingKey": "timezoneUsedForStatisticsInHomePage", "settingValue": "1.5"}, ""),
        ({"settingKey": "showAmountInHomePage", "settingValue": "true"}, ""),
        ({"settingKey": "overviewAccountFilterInHomePage", "settingValue": '{"a":true}'}, ""),
    ],
)
def test_application_cloud_setting_helpers_and_profile_builders(
    auth_helper_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
    setting: dict[str, Any],
    expected_error: str,
) -> None:
    """应用云设置、用户资料和统一响应 helper 应覆盖正反分支。"""
    assert auth_module._validate_application_cloud_setting(setting) == expected_error

    monkeypatch.setattr(
        auth_module,
        "build_user_investment_keyword_settings",
        lambda _user: {
            "platform_keywords": ["基金"],
            "product_keywords": ["ETF"],
            "exclude_keywords": ["体验金"],
        },
    )

    profile = auth_module._build_user_profile_info(
        {
            "username": "alice",
            "email": "alice@example.com",
            "nickname": "",
            "avatar": "avatar-data",
            "default_account_id": 11,
            "transaction_edit_scope": 2,
            "language": "zh_Hans",
            "default_currency": "USD",
            "first_day_of_week": 7,
            "cash_account_id": 12,
            "cash_transfer_category_id": 13,
            "import_learning_enabled": 0,
            "email_verified": 1,
        }
    )
    assert profile["username"] == "alice"
    assert profile["nickname"] == "alice"
    assert profile["defaultAccountId"] == "11"
    assert profile["cashAccountId"] == "12"
    assert profile["cashTransferCategoryId"] == "13"
    assert profile["importLearningEnabled"] is False
    assert profile["investmentPlatformKeywords"] == ["基金"]
    assert profile["emailVerified"] is True

    assert auth_module._build_external_auth_info(
        {
            "external_auth_category": "oauth2",
            "external_auth_type": "github",
            "linked": True,
            "external_username": "octocat",
            "created_at": "2026-03-01T12:00:00",
        }
    )["externalAuthType"] == "github"
    assert auth_module._build_application_cloud_setting_info({"setting_key": "k", "setting_value": "v"}) == {
        "settingKey": "k",
        "settingValue": "v",
    }
    assert auth_module._normalize_application_cloud_settings(
        [{"settingKey": "  key ", "settingValue": 42}]
    ) == [{"setting_key": "key", "setting_value": "42"}]
    assert auth_module._build_auth_success_result({"username": "alice"}, {"access_token": "token"}) == {
        "token": "token",
        "refreshToken": None,
        "need2FA": False,
        "user": {"username": "alice"},
        "applicationCloudSettings": [],
    }
    assert auth_module._load_application_cloud_settings(FakeCloudSettingsDB(), 5, FakeLoop()) == [
        {"settingKey": "setting-5", "settingValue": "true"}
    ]

    auth_module._set_recovery_codes(99, ["abcd-1234"])
    assert auth_module._consume_recovery_code(99, "ABCD-1234") is True
    assert auth_module._consume_recovery_code(99, "ABCD-1234") is False

    with auth_helper_app.test_request_context("/"):
        api_session_payload = auth_module._build_auth_success_result(
            {"username": "alice"},
            {"access_token": "token", "refresh_token": "refresh-token"},
            [{"settingKey": "k", "settingValue": "v"}],
        )
        assert api_session_payload["refreshToken"] == "refresh-token"
        assert api_session_payload["applicationCloudSettings"] == [{"settingKey": "k", "settingValue": "v"}]



def test_avatar_qrcode_export_and_misc_parsing_helpers(
    auth_helper_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """头像、二维码、导出筛选、CSV 渲染和杂项解析 helper 应保持稳定契约。"""
    avatar_data_url = auth_module._build_avatar_data_url(FakeUploadedFile(b"avatar-bytes", mimetype="image/png"))
    assert avatar_data_url.startswith("data:image/png;base64,")
    assert base64.b64decode(avatar_data_url.split(",", 1)[1]) == b"avatar-bytes"

    with pytest.raises(ValueError, match="Avatar file is empty"):
        auth_module._build_avatar_data_url(FakeUploadedFile(b""))

    monkeypatch.setattr(auth_module.qrcode, "make", lambda _uri: FakeQRCode())
    qrcode_url = auth_module._generate_2fa_qrcode_data_url("alice", "secret")
    assert qrcode_url.startswith("data:image/png;base64,")
    assert base64.b64decode(qrcode_url.split(",", 1)[1]) == b"fake-png-bytes"

    recovery_codes = auth_module._generate_recovery_codes()
    assert len(recovery_codes) == 8
    assert all(re.fullmatch(r"[0-9A-F]{4}-[0-9A-F]{4}", code) for code in recovery_codes)

    assert auth_module.calculate_token_hash("demo-token") == (
        "7c43ef5ae21d43ce2743f770c68e24def1a43ee2f416d2438410c8af7af2ff2c"
    )
    assert auth_module._datetime_to_unix_millis("") == 0
    assert auth_module._datetime_to_unix_millis("bad-date") == 0
    assert auth_module._datetime_to_unix_millis("2026-03-01T00:00:00") > 0
    assert auth_module._parse_comma_separated_ints("1, 2, bad, , 3") == [1, 2, 3]
    assert auth_module._parse_comma_separated_ints("") == []
    assert auth_module._parse_export_datetime("0") is None
    assert auth_module._parse_export_datetime("bad") is None

    min_time = int(datetime(2026, 3, 1, 0, 0, 0, tzinfo=UTC).timestamp() * 1000)
    max_time = int(datetime(2026, 3, 31, 23, 59, 59, tzinfo=UTC).timestamp() * 1000)
    categories = [
        {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
        {"id": 11, "main_category": "交通", "sub_category": "地铁"},
    ]
    with auth_helper_app.test_request_context(
        f"/export?min_time={min_time}&max_time={max_time}&type=3&keyword=早餐&amount_filter=lt:0"
        "&account_ids=1, bad, 2&tag_ids=7,8&category_ids=10,11"
    ):
        filters = auth_module._build_export_filters(categories)
        assert filters["keyword"] == "早餐"
        assert filters["amount_filter"] == "lt:0"
        assert filters["account_ids"] == [1, 2]
        assert filters["tag_ids"] == [7, 8]
        assert filters["categories"] == [
            {"main": "餐饮", "sub": "早餐"},
            {"main": "交通", "sub": "地铁"},
        ]
        assert filters["type"]
        session_db = FakeSessionDB()
        loop = FakeLoop()
        monkeypatch.setattr(
            auth_module,
            "generate_jwt_token",
            lambda *_args, **_kwargs: {
                "access_token": "access-token",
                "refresh_token": "refresh-token",
                "expires_at": "2026-03-10T10:00:00",
                "refresh_expires_at": "2026-04-10T10:00:00",
            },
        )
        created_tokens = auth_module._create_new_session_payload(1, "alice", {"jwt_secret": "secret"}, session_db, loop)
        assert created_tokens["access_token"] == "access-token"
        assert session_db.session_payloads[0]["token_hash"] == auth_module.calculate_token_hash("access-token")
        assert session_db.session_payloads[0]["refresh_token_hash"] == auth_module.calculate_token_hash("refresh-token")

    export_text = auth_module._render_bills_export(
        bills=[
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
        ],
        accounts=[{"id": 1, "name": "现金"}, {"id": 2, "name": "支付宝"}],
        tags_map={1: [{"name": "早餐"}, {"name": "工作日"}]},
        delimiter=",",
    )
    assert "source_account,destination_account" in export_text
    assert "现金,支付宝" in export_text
    assert "早餐|工作日" in export_text

    default_accounts_zh = auth_module._build_default_accounts("zh_Hans")
    default_accounts_en = auth_module._build_default_accounts("en_US")
    assert default_accounts_zh[0]["name"] == "现金"
    assert default_accounts_en[0]["name"] == "Cash"
