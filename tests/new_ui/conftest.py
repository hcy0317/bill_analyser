"""
测试配置和Fixtures
"""

# pylint: disable=deprecated-class,duplicate-code,import-outside-toplevel
# pylint: disable=redefined-outer-name,unused-argument

import asyncio
import os
import sys
import time
import uuid
from collections import defaultdict
from pathlib import Path

import pytest
import pytest_asyncio

from tests.real_sample_support import extract_real_sample_family
from tests.runtime_paths import get_test_db_path, remove_test_database_family
from tests.user_cleanup_support import (
    begin_test_user_cleanup_tracking,
    cleanup_registered_test_users,
    register_test_user_for_cleanup,
)

# 添加项目根目录到路径
PROJECT_ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

# 设置Windows事件循环策略
if sys.platform == "win32":
    asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())


def _restore_new_ui_runtime_context(flask_app):
    """Restore this package's initialized app context if another test rewired it."""
    from bill_analyser.api import app as app_module

    db_instance = flask_app.config.get("_PYTEST_NEW_UI_DB_INSTANCE")
    category_engine_instance = flask_app.config.get("_PYTEST_NEW_UI_CATEGORY_ENGINE_INSTANCE")
    bill_service_instance = flask_app.config.get("_PYTEST_NEW_UI_BILL_SERVICE_INSTANCE")

    if db_instance is not None:
        flask_app.config["DB_INSTANCE"] = db_instance
        app_module.DB_INSTANCE = db_instance
        app_module.db = db_instance
    if category_engine_instance is not None:
        flask_app.config["CATEGORY_ENGINE_INSTANCE"] = category_engine_instance
        app_module.CATEGORY_ENGINE_INSTANCE = category_engine_instance
        app_module.category_engine = category_engine_instance
    if bill_service_instance is not None:
        flask_app.config["BILL_SERVICE_INSTANCE"] = bill_service_instance
        app_module.BILL_SERVICE_INSTANCE = bill_service_instance
        app_module.bill_service = bill_service_instance


def _ensure_new_ui_auth_schema(flask_app):
    """Ensure auth routes see an initialized user/session schema before login setup."""
    db_instance = flask_app.config.get("DB_INSTANCE")
    if db_instance is not None:
        asyncio.run(db_instance.init_db())


@pytest_asyncio.fixture(scope="session")
async def initialize_app():
    """异步初始化Flask应用"""
    from bill_analyser.api.app import initialize

    # 使用测试数据库
    test_db_path = get_test_db_path("test_bills.db")
    remove_test_database_family(test_db_path)

    await initialize(db_path=str(test_db_path))


@pytest_asyncio.fixture(scope="session")
async def app(initialize_app):
    """创建Flask应用实例（依赖异步初始化）"""
    from bill_analyser.api.app import app, bill_service, category_engine, db
    from bill_analyser.utils.logger import _logger_instance

    # 配置测试模式
    app.config["TESTING"] = True
    app.config["DEBUG"] = False
    app.config["_PYTEST_NEW_UI_DB_INSTANCE"] = db
    app.config["_PYTEST_NEW_UI_CATEGORY_ENGINE_INSTANCE"] = category_engine
    app.config["_PYTEST_NEW_UI_BILL_SERVICE_INSTANCE"] = bill_service

    yield app

    # 清理：确保所有资源正确释放
    if db:
        await db.close()

    remove_test_database_family(get_test_db_path("test_bills.db"))

    # 停止日志监听器
    _logger_instance.stop()


@pytest.fixture(autouse=True)
def _restore_new_ui_app_context(app):
    """Restore this package's initialized app context if another test rewired it."""
    _restore_new_ui_runtime_context(app)
    yield
    _restore_new_ui_runtime_context(app)


@pytest.fixture
def client(app):
    """创建测试客户端"""
    _restore_new_ui_runtime_context(app)
    return app.test_client()


@pytest.fixture(autouse=True)
def _tracked_test_user_cleanup(db):
    """按测试回收共享测试库里新建的测试用户。"""
    begin_test_user_cleanup_tracking()
    yield
    cleanup_registered_test_users(db)


@pytest.fixture
def auth_identity(app, client, db):
    """为每个测试返回独立的测试认证身份信息。"""
    _restore_new_ui_runtime_context(app)
    _ensure_new_ui_auth_schema(app)
    suffix = f"{int(time.time() * 1000)}_{uuid.uuid4().hex[:8]}"
    username = f"test_new_ui_{suffix}"
    password = "Test123456!"

    register_response = client.post("/api/auth/register", json={
        "username": username,
        "email": f"{username}@example.com",
        "password": password,
        "nickname": username
    })
    assert register_response.status_code in [200, 201], (
        f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
    )
    register_test_user_for_cleanup(db, username)

    return {
        "username": username,
        "password": password,
    }


@pytest.fixture
def auth_context(app, client, auth_identity):
    """为每个测试生成新的认证上下文，避免共享 token 被其他用例作废。"""
    _restore_new_ui_runtime_context(app)
    login_response = client.post("/api/auth/login", json={
        "loginName": auth_identity["username"],
        "password": auth_identity["password"]
    })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    result = data.get("result") or {}
    token = result.get("token")
    assert token, f"登录响应缺少token: {data}"

    return {
        "username": auth_identity["username"],
        "password": auth_identity["password"],
        "token": token,
        "refresh_token": result.get("refreshToken"),
        "headers": {"Authorization": f"Bearer {token}"},
        "user": result.get("user") or {}
    }


@pytest.fixture
def auth_headers(auth_context):
    """返回可复用的认证请求头。"""
    return auth_context["headers"]


@pytest.fixture(scope="session")
def operation_password(app):
    """返回与运行时密码校验逻辑一致的操作密码。"""
    env_password = os.getenv("BILL_ANALYSER_OPERATION_PASSWORD")
    if env_password:
        return env_password

    db_instance = app.config["DB_INSTANCE"]
    stored_password = asyncio.run(db_instance.get_app_setting("operation_password"))
    return stored_password or "admin123"


@pytest.fixture
def db(app):
    """获取数据库实例"""
    _restore_new_ui_runtime_context(app)
    return app.config["DB_INSTANCE"]


@pytest.fixture(scope="session")
def category_engine(initialize_app):
    """获取分类引擎实例"""
    from bill_analyser.api.app import category_engine
    return category_engine


def pytest_terminal_summary(terminalreporter, exitstatus, config):
    """输出真实样本回归按文件族统计的通过率。"""
    family_stats: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))

    for outcome in ("passed", "failed", "skipped", "xfailed", "xpassed"):
        for report in terminalreporter.stats.get(outcome, []):
            if getattr(report, "when", "call") != "call":
                continue
            family = extract_real_sample_family(report.nodeid)
            if not family:
                continue
            family_stats[family][outcome] += 1

    if not family_stats:
        return

    terminalreporter.section("真实样本文件族通过率")
    for family in sorted(family_stats):
        stats = family_stats[family]
        passed = stats.get("passed", 0) + stats.get("xpassed", 0)
        failed = stats.get("failed", 0)
        skipped = stats.get("skipped", 0) + stats.get("xfailed", 0)
        total = passed + failed
        pass_rate = 100.0 if total == 0 else (passed / total) * 100.0
        terminalreporter.write_line(
            f"{family}: {passed}/{total} 通过 ({pass_rate:.1f}%), 失败={failed}, 跳过={skipped}"
        )
