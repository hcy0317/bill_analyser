"""Runtime-focused coverage tests for bill_analyser.api.app."""

from __future__ import annotations

import inspect
from typing import TYPE_CHECKING, Any

import pytest

import bill_analyser.api.app as app_module
from bill_analyser.utils.config import ConfigValidationError

# pylint: disable=line-too-long,missing-module-docstring,missing-function-docstring,too-few-public-methods,protected-access,use-implicit-booleaness-not-comparison,unnecessary-lambda

if TYPE_CHECKING:
    from collections.abc import Iterator
    from pathlib import Path


class StubAdminDatabase:
    """Lightweight async DB stub for default-admin tests."""

    def __init__(
        self,
        *,
        existing_user: dict[str, Any] | None = None,
        lookup_error: Exception | None = None,
        create_error: Exception | None = None,
    ) -> None:
        self.existing_user = existing_user
        self.lookup_error = lookup_error
        self.create_error = create_error
        self.created_payloads: list[dict[str, Any]] = []

    async def get_user_by_username(self, username: str) -> dict[str, Any] | None:
        _ = username
        if self.lookup_error is not None:
            raise self.lookup_error
        return self.existing_user

    async def create_user(self, payload: dict[str, Any]) -> None:
        self.created_payloads.append(dict(payload))
        if self.create_error is not None:
            raise self.create_error


class ExistingFailingDatabase:
    """Existing DB instance whose close path fails once."""

    async def close(self) -> None:
        raise OSError("close boom")


class ExistingHealthyDatabase:
    """Existing DB instance whose close path succeeds."""

    def __init__(self) -> None:
        self.closed = False

    async def close(self) -> None:
        self.closed = True


class FakeDatabase:
    """Minimal database implementation for initialize() tests."""

    def __init__(self, db_path: str | None = None) -> None:
        self.db_path = db_path or "default-test.db"
        self.init_calls = 0

    async def init_db(self) -> None:
        self.init_calls += 1


class ExplodingDatabase(FakeDatabase):
    """Database stub whose init_db fails."""

    async def init_db(self) -> None:
        raise RuntimeError("init boom")


class FakeCategoryEngine:
    """Minimal category engine stub."""

    def __init__(self) -> None:
        self.loaded_with: list[Any] = []

    async def load_rules_from_db(self, database: Any) -> None:
        self.loaded_with.append(database)


class FakeBillService:
    """Minimal bill service stub."""

    def __init__(self, db: Any) -> None:
        self.db = db
        self.initialized = False

    async def initialize(self) -> None:
        self.initialized = True


@pytest.fixture(autouse=True)
def restore_app_module_state() -> Iterator[None]:
    """Restore module-level globals and app config after each test."""
    original_globals = {
        "DB_INSTANCE": app_module.DB_INSTANCE,
        "CATEGORY_ENGINE_INSTANCE": app_module.CATEGORY_ENGINE_INSTANCE,
        "BILL_SERVICE_INSTANCE": app_module.BILL_SERVICE_INSTANCE,
        "db": app_module.db,
        "category_engine": app_module.category_engine,
        "bill_service": app_module.bill_service,
    }
    original_config = dict(app_module.app.config)

    yield

    for name, value in original_globals.items():
        setattr(app_module, name, value)

    app_module.app.config.clear()
    app_module.app.config.update(original_config)


def _runtime_config() -> dict[str, Any]:
    return {
        "host": "127.0.0.1",
        "port": 5010,
        "debug": False,
        "threaded": True,
        "cors": {
            "origins": ["http://localhost:8081"],
            "methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
            "allow_headers": ["Authorization", "Content-Type"],
            "expose_headers": ["Authorization"],
            "supports_credentials": True,
            "max_age_seconds": 600,
            "send_wildcard": False,
            "always_send": True,
        },
    }


def _build_fresh_app(monkeypatch: pytest.MonkeyPatch, *, static_dir: Path | None = None):
    monkeypatch.setattr(app_module, "load_api_runtime_settings", lambda: _runtime_config())
    if static_dir is not None:
        monkeypatch.setattr(app_module, "STATIC_DIR", static_dir)

    flask_app = app_module.create_app()
    flask_app.config.update(TESTING=True, PROPAGATE_EXCEPTIONS=False)
    return flask_app


def test_log_python_runtime_details_covers_virtualenv_and_warning_branches(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """运行时日志应覆盖虚拟环境与非虚拟环境两条分支。"""
    info_calls: list[tuple[Any, ...]] = []
    warning_calls: list[tuple[Any, ...]] = []

    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module.logger, "warning", lambda *args: warning_calls.append(args))

    monkeypatch.setattr(app_module.sys, "executable", str(app_module.PROJECT_ROOT / ".venv" / "Scripts" / "python.exe"))
    app_module._log_python_runtime_details()
    assert any(call[0] == "[OK] 正在使用虚拟环境" for call in info_calls)

    info_calls.clear()
    warning_calls.clear()
    monkeypatch.setattr(app_module.sys, "executable", "C:/Python314/python.exe")
    app_module._log_python_runtime_details()

    assert any(call[0] == "[WARN] 未使用虚拟环境" for call in warning_calls)
    assert any(call[0] == "当前路径: %s" for call in warning_calls)
    assert any(call[0] == "建议使用: %s" for call in warning_calls)


def test_create_app_logs_preflight_and_missing_authorization(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """before_request 应记录 OPTIONS 预检和账户请求缺少 Authorization 的分支。"""
    debug_calls: list[tuple[Any, ...]] = []
    warning_calls: list[tuple[Any, ...]] = []

    monkeypatch.setattr(app_module.logger, "debug", lambda *args: debug_calls.append(args))
    monkeypatch.setattr(app_module.logger, "warning", lambda *args: warning_calls.append(args))

    flask_app = _build_fresh_app(monkeypatch)
    client = flask_app.test_client()

    response = client.options(
        "/api/accounts/",
        headers={
            "Origin": "http://localhost:8081",
            "Access-Control-Request-Headers": "Authorization, Content-Type",
        },
    )
    assert response.status_code in (200, 204)

    assert any(call[0] == "[CORS Preflight] %s" and call[1] == "/api/accounts/" for call in debug_calls)
    assert any(call[0] == "[CORS Preflight] Origin: %s" and call[1] == "http://localhost:8081" for call in debug_calls)
    assert any(
        call[0] == "[CORS Preflight] Access-Control-Request-Headers: %s"
        and call[1] == "Authorization, Content-Type"
        for call in debug_calls
    )
    assert any(call[0] == "[Request Debug] Missing Authorization header!" for call in warning_calls)
    assert any(call[0] == "[Request Debug] All headers: %s" for call in debug_calls)


def test_create_app_logs_authorized_accounts_and_v1_requests(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """accounts 授权分支、v1 调试分支和 API 前端拦截分支都应被覆盖。"""
    debug_calls: list[tuple[Any, ...]] = []

    (tmp_path / "index.html").write_text("INDEX-CONTENT", encoding="utf-8")
    monkeypatch.setattr(app_module.logger, "debug", lambda *args: debug_calls.append(args))

    flask_app = _build_fresh_app(monkeypatch, static_dir=tmp_path)
    client = flask_app.test_client()

    accounts_response = client.get("/api/accounts/", headers={"Authorization": "Bearer token-1234567890"})
    assert accounts_response.status_code in (200, 401, 500)

    legacy_response = client.get("/api/v1/legacy.json")
    assert legacy_response.status_code == 404

    api_root_response = client.get("/api")
    assert api_root_response.status_code == 404

    assert any(call[0] == "[Request Debug] Authorization: %s..." for call in debug_calls)
    assert any(call[0] == "v1请求: %s %s" and call[2] == "/api/v1/legacy.json" for call in debug_calls)


def test_create_app_serves_static_file_and_frontend_index(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """前端路由应覆盖静态文件命中与 SPA index fallback。"""
    (tmp_path / "index.html").write_text("INDEX-CONTENT", encoding="utf-8")
    (tmp_path / "app.js").write_text("console.log('asset');", encoding="utf-8")

    flask_app = _build_fresh_app(monkeypatch, static_dir=tmp_path)
    client = flask_app.test_client()

    static_response = client.get("/app.js")
    assert static_response.status_code == 200
    assert "asset" in static_response.get_data(as_text=True)

    spa_response = client.get("/dashboard")
    assert spa_response.status_code == 200
    assert "INDEX-CONTENT" in spa_response.get_data(as_text=True)


def test_create_app_favicon_404_returns_204(monkeypatch: pytest.MonkeyPatch) -> None:
    """favicon 404 应走 204 兜底分支。"""
    flask_app = _build_fresh_app(monkeypatch)

    @flask_app.route("/_raise_favicon_404")
    def _raise_favicon_404():
        from flask import abort  # pylint: disable=import-outside-toplevel

        abort(404, description="missing favicon.ico")

    client = flask_app.test_client()
    response = client.get("/_raise_favicon_404")

    assert response.status_code == 204
    assert response.get_data(as_text=True) == ""


def test_create_app_internal_error_handler_logs_and_returns_json(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """500 handler 应记录错误日志并返回统一 JSON。"""
    error_calls: list[tuple[tuple[Any, ...], dict[str, Any]]] = []
    monkeypatch.setattr(app_module.logger, "error", lambda *args, **kwargs: error_calls.append((args, kwargs)))

    flask_app = _build_fresh_app(monkeypatch)

    @flask_app.route("/_boom")
    def _boom():
        raise RuntimeError("boom")

    flask_app.config.update(TESTING=False, PROPAGATE_EXCEPTIONS=False)
    client = flask_app.test_client()
    response = client.get("/_boom")
    payload = response.get_json() or {}

    assert response.status_code == 500
    assert payload["success"] is False
    assert payload["error"] == "Internal Server Error"
    assert any(args[0] == "Internal Server Error: %s" for args, _ in error_calls)


def test_create_app_method_not_allowed_distinguishes_legacy_and_current_endpoints(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """405 handler 应对 legacy .json 路径降级为 404，并保留普通 405 语义。"""
    flask_app = _build_fresh_app(monkeypatch)

    @flask_app.route("/api/current.json", methods=["GET"])
    def _current_json():
        return {"success": True}

    client = flask_app.test_client()

    legacy_like = client.patch("/api/current.json")
    legacy_payload = legacy_like.get_json() or {}
    assert legacy_like.status_code == 404
    assert legacy_payload["message"] == "Legacy endpoint not found"

    standard = client.patch("/api/health")
    standard_payload = standard.get_json() or {}
    assert standard.status_code == 405
    assert standard_payload["error"] == "Method Not Allowed"


@pytest.mark.asyncio
async def test_create_default_admin_user_handles_disabled_config_and_runtime_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """默认管理员初始化应覆盖 no-config 早退和运行时失败 warning 分支。"""
    info_calls: list[tuple[Any, ...]] = []
    warning_calls: list[tuple[Any, ...]] = []
    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module.logger, "warning", lambda *args: warning_calls.append(args))

    monkeypatch.setattr(app_module, "load_default_user_settings", lambda: None)
    disabled_db = StubAdminDatabase()
    disabled_database: Any = disabled_db
    await app_module.create_default_admin_user(disabled_database)
    assert disabled_db.created_payloads == []
    assert any(call[0] == "未启用默认管理员自动创建，跳过初始化" for call in info_calls)

    config = {
        "username": "admin",
        "password": "Secret123!",
        "email": "admin@example.com",
        "nickname": "管理员",
        "language": "zh-CN",
        "default_currency": "CNY",
        "first_day_of_week": 1,
    }
    monkeypatch.setattr(app_module, "load_default_user_settings", lambda: config)
    failing_db = StubAdminDatabase(lookup_error=OSError("lookup boom"))
    failing_database: Any = failing_db
    await app_module.create_default_admin_user(failing_database)
    assert any(call[0] == "创建默认用户失败: %s" for call in warning_calls)


@pytest.mark.asyncio
async def test_create_default_admin_user_reraises_config_validation_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """配置校验失败应记录 error 并继续向上抛出。"""
    error_calls: list[tuple[Any, ...]] = []
    monkeypatch.setattr(app_module.logger, "error", lambda *args: error_calls.append(args))

    def _raise_config_error() -> dict[str, Any]:
        raise ConfigValidationError("bad config")

    monkeypatch.setattr(app_module, "load_default_user_settings", _raise_config_error)

    invalid_database: Any = StubAdminDatabase()
    with pytest.raises(ConfigValidationError, match="bad config"):
        await app_module.create_default_admin_user(invalid_database)

    assert any(call[0] == "默认管理员配置无效: %s" for call in error_calls)


@pytest.mark.asyncio
async def test_create_default_admin_user_creates_missing_user_and_warns_password(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """默认管理员不存在时应创建用户并提示修改默认密码。"""
    config = {
        "username": "admin",
        "password": "Secret123!",
        "email": "admin@example.com",
        "nickname": "管理员",
        "language": "zh-CN",
        "default_currency": "CNY",
        "first_day_of_week": 1,
    }
    info_calls: list[tuple[Any, ...]] = []
    warning_calls: list[tuple[Any, ...]] = []

    monkeypatch.setattr(app_module, "load_default_user_settings", lambda: config)
    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module.logger, "warning", lambda *args: warning_calls.append(args))

    created_db = StubAdminDatabase(existing_user=None)
    created_database: Any = created_db
    await app_module.create_default_admin_user(created_database)

    assert len(created_db.created_payloads) == 1
    created_payload = created_db.created_payloads[0]
    assert created_payload["username"] == "admin"
    assert created_payload["email"] == config["email"]
    assert created_payload["password_hash"] != config["password"]
    assert any(call[0] == "创建默认管理员用户: %s" for call in info_calls)
    assert any(call[0] == "[OK] 默认管理员用户创建成功: %s" for call in info_calls)
    assert any(call[0] == "⚠️  默认密码: %s - 请首次登录后立即修改!" for call in warning_calls)


@pytest.mark.asyncio
async def test_create_default_admin_user_logs_when_admin_already_exists(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """默认管理员已存在时不应重复创建。"""
    config = {
        "username": "admin",
        "password": "Secret123!",
        "email": "admin@example.com",
        "nickname": "管理员",
        "language": "zh-CN",
        "default_currency": "CNY",
        "first_day_of_week": 1,
    }
    info_calls: list[tuple[Any, ...]] = []

    monkeypatch.setattr(app_module, "load_default_user_settings", lambda: config)
    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))

    existing_db = StubAdminDatabase(existing_user={"username": "admin"})
    existing_database: Any = existing_db
    await app_module.create_default_admin_user(existing_database)

    assert existing_db.created_payloads == []
    assert any(call[0] == "管理员用户已存在: %s" for call in info_calls)


@pytest.mark.asyncio
async def test_initialize_handles_close_warning_and_sets_runtime_instances(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """initialize 应覆盖旧连接关闭失败 warning，并完成依赖注入。"""
    info_calls: list[tuple[Any, ...]] = []
    warning_calls: list[tuple[Any, ...]] = []
    created_admin_calls: list[Any] = []

    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module.logger, "warning", lambda *args: warning_calls.append(args))
    monkeypatch.setattr(app_module, "DB_INSTANCE", ExistingFailingDatabase())
    monkeypatch.setattr(app_module, "Database", FakeDatabase)
    monkeypatch.setattr(app_module, "CategoryEngine", FakeCategoryEngine)
    monkeypatch.setattr(app_module, "BillService", FakeBillService)

    async def _fake_create_default_admin_user(database: Any) -> None:
        created_admin_calls.append(database)

    monkeypatch.setattr(app_module, "create_default_admin_user", _fake_create_default_admin_user)

    await app_module.initialize(db_path="custom-test.db")

    assert any(call[0] == "关闭旧数据库连接失败: %s" for call in warning_calls)
    assert isinstance(app_module.DB_INSTANCE, FakeDatabase)
    assert app_module.DB_INSTANCE.db_path == "custom-test.db"
    assert created_admin_calls == [app_module.DB_INSTANCE]
    assert app_module.app.config["DB_INSTANCE"] is app_module.DB_INSTANCE
    assert app_module.app.config["CATEGORY_ENGINE_INSTANCE"] is app_module.CATEGORY_ENGINE_INSTANCE
    assert app_module.app.config["BILL_SERVICE_INSTANCE"] is app_module.BILL_SERVICE_INSTANCE
    assert any(call[0] == "Web API服务器初始化完成" for call in info_calls)


@pytest.mark.asyncio
async def test_initialize_closes_previous_db_and_uses_default_database_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """initialize 应覆盖旧连接成功关闭与默认 Database() 分支。"""
    info_calls: list[tuple[Any, ...]] = []
    previous_db = ExistingHealthyDatabase()

    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module, "DB_INSTANCE", previous_db)
    monkeypatch.setattr(app_module, "Database", FakeDatabase)
    monkeypatch.setattr(app_module, "CategoryEngine", FakeCategoryEngine)
    monkeypatch.setattr(app_module, "BillService", FakeBillService)

    async def _fake_create_default_admin_user(_database: Any) -> None:
        return None

    monkeypatch.setattr(app_module, "create_default_admin_user", _fake_create_default_admin_user)

    await app_module.initialize()

    assert previous_db.closed is True
    assert isinstance(app_module.DB_INSTANCE, FakeDatabase)
    assert app_module.DB_INSTANCE.db_path == "default-test.db"
    assert any(call[0] == "[OK] 已关闭旧数据库连接" for call in info_calls)


@pytest.mark.asyncio
async def test_initialize_logs_critical_and_exits_on_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """initialize 在关键初始化失败时应记录 critical 并退出。"""
    critical_calls: list[tuple[tuple[Any, ...], dict[str, Any]]] = []

    monkeypatch.setattr(app_module, "DB_INSTANCE", None)
    monkeypatch.setattr(app_module, "Database", ExplodingDatabase)
    monkeypatch.setattr(app_module.logger, "critical", lambda *args, **kwargs: critical_calls.append((args, kwargs)))
    monkeypatch.setattr(app_module.sys, "exit", lambda code: (_ for _ in ()).throw(SystemExit(code)))

    with pytest.raises(SystemExit) as exc_info:
        await app_module.initialize(db_path="explode.db")

    assert exc_info.value.code == 1
    assert any(args[0] == "服务器初始化失败: %s" for args, _ in critical_calls)


def test_main_runs_initialize_and_starts_flask(monkeypatch: pytest.MonkeyPatch) -> None:
    """main 应读取运行时配置、初始化依赖并调用 app.run。"""
    runtime_config = _runtime_config()
    info_calls: list[tuple[Any, ...]] = []
    run_calls: list[dict[str, Any]] = []
    asyncio_run_args: list[Any] = []
    runtime_log_calls: list[str] = []

    monkeypatch.setattr(app_module, "load_api_runtime_settings", lambda: runtime_config)
    monkeypatch.setattr(app_module, "_log_python_runtime_details", lambda: runtime_log_calls.append("called"))
    monkeypatch.setattr(app_module.logger, "info", lambda *args: info_calls.append(args))
    monkeypatch.setattr(app_module.app, "run", lambda **kwargs: run_calls.append(kwargs))

    async def _fake_initialize() -> None:
        return None

    monkeypatch.setattr(app_module, "initialize", _fake_initialize)

    def _fake_asyncio_run(coro: Any) -> None:
        asyncio_run_args.append(coro)
        if inspect.iscoroutine(coro):
            coro.close()

    monkeypatch.setattr(app_module.asyncio, "run", _fake_asyncio_run)

    app_module.main()

    assert runtime_log_calls == ["called"]
    assert len(asyncio_run_args) == 1
    assert run_calls == [
        {
            "host": runtime_config["host"],
            "port": runtime_config["port"],
            "debug": runtime_config["debug"],
            "threaded": runtime_config["threaded"],
        }
    ]
    assert any(call[0] == "启动Flask服务器" for call in info_calls)
    assert any(call[0] == "监听地址: http://%s:%s" for call in info_calls)
    assert any(call[0] == "API文档: http://%s:%s/api/" for call in info_calls)
