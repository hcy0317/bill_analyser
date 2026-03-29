from __future__ import annotations

import pytest


def test_login_null_json_returns_bad_request(client) -> None:
    """`null` JSON body should be treated as an invalid login request, not a 500."""
    response = client.post(
        "/api/auth/login",
        data="null",
        content_type="application/json",
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload == {
        "success": False,
        "error": "Invalid request",
        "message": "Username and password are required",
    }


@pytest.mark.parametrize(
    ("path", "raw_body", "expected_message"),
    [
        ("/api/auth/login", '"plain-string"', "Username and password are required"),
        ("/api/tokens/refresh", "123", "Refresh token is required"),
    ],
)
def test_auth_endpoints_reject_non_object_json_bodies(
    client,
    path: str,
    raw_body: str,
    expected_message: str,
) -> None:
    """能被 JSON 解析但不是对象的请求体也应稳定返回 400。"""
    response = client.post(path, data=raw_body, content_type="application/json")

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload == {
        "success": False,
        "error": "Invalid request",
        "message": expected_message,
    }



def test_refresh_token_null_json_returns_bad_request(client) -> None:
    """Refresh-token endpoint should reject `null` JSON bodies with a stable 400 contract."""
    response = client.post(
        "/api/tokens/refresh",
        data="null",
        content_type="application/json",
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload == {
        "success": False,
        "error": "Invalid request",
        "message": "Refresh token is required",
    }



def test_login_unknown_user_and_wrong_password_share_same_error_contract(client, auth_identity) -> None:
    """Unknown-user and wrong-password flows should not leak whether a user exists."""
    missing_user_response = client.post(
        "/api/auth/login",
        json={
            "loginName": f"missing_{auth_identity['username']}",
            "password": auth_identity["password"],
        },
    )
    wrong_password_response = client.post(
        "/api/auth/login",
        json={
            "loginName": auth_identity["username"],
            "password": "DefinitelyWrongPassword!",
        },
    )

    assert missing_user_response.status_code == 401
    assert wrong_password_response.status_code == 401

    missing_payload = missing_user_response.get_json() or {}
    wrong_password_payload = wrong_password_response.get_json() or {}

    assert missing_payload == wrong_password_payload == {
        "success": False,
        "error": "Invalid credentials",
        "message": "Invalid username or password",
    }



def test_logout_rejects_non_bearer_authorization_header(client) -> None:
    """Logout should reject non-Bearer Authorization headers with a 401."""
    response = client.post(
        "/api/auth/logout",
        headers={"Authorization": "Token demo-token"},
    )

    assert response.status_code == 401
    payload = response.get_json() or {}
    assert payload == {
        "success": False,
        "error": "Unauthorized",
        "message": "Invalid authorization header",
    }
