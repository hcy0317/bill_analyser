"""Domain-level shared fixtures for integration tests."""

from __future__ import annotations

import asyncio
import time
import uuid
from pathlib import Path

import pytest

from tests.runtime_paths import get_test_db_path, remove_test_database_family

_DOMAIN_TEST_DB_PATH: Path | None = None


@pytest.fixture(scope="session")
def app():
    """Provide a configured Flask app instance for domain integration tests."""
    from bill_analyser.api.app import app as flask_app
    from bill_analyser.api.app import initialize
    from bill_analyser.api.app import db

    global _DOMAIN_TEST_DB_PATH  # pylint: disable=global-statement

    _DOMAIN_TEST_DB_PATH = get_test_db_path(f"test_domains_{int(time.time() * 1000)}.db")
    remove_test_database_family(_DOMAIN_TEST_DB_PATH)
    asyncio.run(initialize(db_path=str(_DOMAIN_TEST_DB_PATH)))

    flask_app.config["TESTING"] = True
    flask_app.config["DEBUG"] = False

    yield flask_app

    if db:
        asyncio.run(db.close())

    if _DOMAIN_TEST_DB_PATH is not None:
        remove_test_database_family(_DOMAIN_TEST_DB_PATH)


@pytest.fixture(scope="session")
def client(app):
    """Create a reusable Flask test client."""
    return app.test_client()


@pytest.fixture
def auth_identity(client):
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
    assert register_response.status_code in (200, 201, 409), register_response.get_data(as_text=True)

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


@pytest.fixture(scope="session")
def db_instance():
    """Expose the initialized database instance."""
    from bill_analyser.api.app import db

    return db
