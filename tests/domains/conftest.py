"""Domain-level shared fixtures for integration tests."""

# pylint: disable=import-outside-toplevel,redefined-outer-name

from __future__ import annotations

import asyncio
import time
import uuid
from pathlib import Path

import pytest

from tests.runtime_paths import get_test_db_path, remove_test_database_family
from tests.user_cleanup_support import (
    begin_test_user_cleanup_tracking,
    cleanup_registered_test_users,
    register_test_user_for_cleanup,
)

_DOMAIN_TEST_DB_PATH: Path | None = None


@pytest.fixture(scope="session")
def app():
    """Provide a configured Flask app instance for domain integration tests."""
    from bill_analyser.api.app import app as flask_app
    from bill_analyser.api.app import db, initialize

    global _DOMAIN_TEST_DB_PATH  # pylint: disable=global-statement

    _DOMAIN_TEST_DB_PATH = get_test_db_path(f"test_domains_{int(time.time() * 1000)}.db")
    remove_test_database_family(_DOMAIN_TEST_DB_PATH)
    asyncio.run(initialize(db_path=str(_DOMAIN_TEST_DB_PATH)))

    from bill_analyser.api import app as app_module

    flask_app.config["TESTING"] = True
    flask_app.config["DEBUG"] = False
    flask_app.config["_PYTEST_DOMAINS_DB_INSTANCE"] = app_module.db
    flask_app.config["_PYTEST_DOMAINS_CATEGORY_ENGINE_INSTANCE"] = app_module.category_engine
    flask_app.config["_PYTEST_DOMAINS_BILL_SERVICE_INSTANCE"] = app_module.bill_service

    yield flask_app

    if db:
        asyncio.run(db.close())

    if _DOMAIN_TEST_DB_PATH is not None:
        remove_test_database_family(_DOMAIN_TEST_DB_PATH)


@pytest.fixture(autouse=True)
def _restore_domain_app_context(app):
    """Restore domain integration app context after route unit tests rewire globals."""
    from bill_analyser.api import app as app_module

    db_instance = app.config.get("_PYTEST_DOMAINS_DB_INSTANCE")
    category_engine_instance = app.config.get("_PYTEST_DOMAINS_CATEGORY_ENGINE_INSTANCE")
    bill_service_instance = app.config.get("_PYTEST_DOMAINS_BILL_SERVICE_INSTANCE")

    if db_instance is not None:
        app.config["DB_INSTANCE"] = db_instance
        app_module.DB_INSTANCE = db_instance
        app_module.db = db_instance
    if category_engine_instance is not None:
        app.config["CATEGORY_ENGINE_INSTANCE"] = category_engine_instance
        app_module.CATEGORY_ENGINE_INSTANCE = category_engine_instance
        app_module.category_engine = category_engine_instance
    if bill_service_instance is not None:
        app.config["BILL_SERVICE_INSTANCE"] = bill_service_instance
        app_module.BILL_SERVICE_INSTANCE = bill_service_instance
        app_module.bill_service = bill_service_instance


@pytest.fixture(scope="session")
def client(app):
    """Create a reusable Flask test client."""
    return app.test_client()


@pytest.fixture(autouse=True)
def _tracked_test_user_cleanup(db_instance):
    """Clean test-created users after each domain integration test."""
    begin_test_user_cleanup_tracking()
    yield
    cleanup_registered_test_users(db_instance)


@pytest.fixture
def auth_identity(client, db_instance):
    """Register an isolated test user for each integration test."""
    suffix = f"{int(time.time() * 1000)}_{uuid.uuid4().hex[:8]}"
    username = f"test_domains_{suffix}"
    password = "Test123456!"

    register_response = client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": password,
            "nickname": username,
        },
    )
    assert register_response.status_code in (200, 201), (
        register_response.get_data(as_text=True)
    )
    register_test_user_for_cleanup(db_instance, username)

    return {"username": username, "password": password}


@pytest.fixture
def auth_context(client, auth_identity):
    """Log in with the isolated user and expose token + user payload."""
    login_response = client.post(
        "/api/auth/login",
        json={"loginName": auth_identity["username"], "password": auth_identity["password"]},
    )
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    payload = login_response.get_json() or {}
    result = payload.get("result") or {}
    token = result.get("token")
    assert token, f"登录响应缺少 token: {payload}"

    return {
        "username": auth_identity["username"],
        "password": auth_identity["password"],
        "token": token,
        "refresh_token": result.get("refreshToken"),
        "headers": {"Authorization": f"Bearer {token}"},
        "user": result.get("user") or {},
    }


@pytest.fixture
def auth_headers(auth_context):
    """Return Authorization headers for authenticated requests."""
    return auth_context["headers"]


@pytest.fixture
def db_instance(app):
    """Expose the initialized database instance."""
    return app.config["DB_INSTANCE"]
