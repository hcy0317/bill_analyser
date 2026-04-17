"""认证域 2FA 写接口 REST 收口回归测试。"""

from datetime import datetime

import pytest

from bill_analyser.api.routes import auth as auth_module
from tests.user_cleanup_support import register_test_user_for_cleanup


@pytest.fixture(name="user_credentials")
def _user_credentials_fixture(client):
    """注册测试用户并返回凭据。"""
    suffix = int(datetime.now().timestamp() * 1000)
    username = f"test_2fa_write_{suffix}"
    password = "Test123456!"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]

    register_response = client.post("/api/auth/register", json={
        "username": username,
        "email": f"{username}@example.com",
        "password": password,
        "nickname": username
    })
    assert register_response.status_code in [200, 201]
    register_test_user_for_cleanup(db, username)

    return {
        "username": username,
        "password": password
    }


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client, user_credentials):
    """登录并返回鉴权头。"""
    login_response = client.post("/api/auth/login", json={
        "loginName": user_credentials["username"],
        "password": user_credentials["password"]
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    login_data = login_response.get_json() or {}
    token = (login_data.get("result") or {}).get("token")
    assert token
    return {"Authorization": f"Bearer {token}"}



def test_2fa_write_rest_flow(client, auth_headers, user_credentials):
    """2FA 写接口应完整走 REST 主链。"""
    import pyotp

    enable_request_response = client.post("/api/2fa/enable/request", headers=auth_headers)
    assert enable_request_response.status_code == 200, enable_request_response.get_data(as_text=True)

    enable_request_data = enable_request_response.get_json() or {}
    assert enable_request_data.get("success") is True
    enable_request_result = enable_request_data.get("result") or {}
    secret = enable_request_result.get("secret")
    qrcode = enable_request_result.get("qrcode")
    assert secret
    assert qrcode and qrcode.startswith("data:image/png;base64,")

    passcode = pyotp.TOTP(secret).now()
    confirm_response = client.post("/api/2fa/enable/confirm", json={
        "secret": secret,
        "passcode": passcode
    }, headers=auth_headers)
    assert confirm_response.status_code == 200, confirm_response.get_data(as_text=True)

    confirm_data = confirm_response.get_json() or {}
    assert confirm_data.get("success") is True
    confirm_result = confirm_data.get("result") or {}
    assert confirm_result.get("token")
    assert confirm_result.get("refreshToken")
    assert len(confirm_result.get("recoveryCodes") or []) == 8

    refreshed_headers = {"Authorization": f"Bearer {confirm_result['token']}"}

    status_response = client.get("/api/2fa/status", headers=refreshed_headers)
    assert status_response.status_code == 200
    status_data = status_response.get_json() or {}
    assert (status_data.get("result") or {}).get("enable") is True

    regenerate_response = client.post("/api/2fa/recovery/regenerate", json={
        "password": user_credentials["password"]
    }, headers=refreshed_headers)
    assert regenerate_response.status_code == 200, regenerate_response.get_data(as_text=True)
    regenerate_data = regenerate_response.get_json() or {}
    assert regenerate_data.get("success") is True
    assert len((regenerate_data.get("result") or {}).get("recoveryCodes") or []) == 8

    disable_response = client.post("/api/2fa/disable", json={
        "password": user_credentials["password"]
    }, headers=refreshed_headers)
    assert disable_response.status_code == 200, disable_response.get_data(as_text=True)
    disable_data = disable_response.get_json() or {}
    assert disable_data.get("success") is True
    assert disable_data.get("result") is True

    status_after_disable = client.get("/api/2fa/status", headers=refreshed_headers)
    assert status_after_disable.status_code == 200
    status_after_disable_data = status_after_disable.get_json() or {}
    assert (status_after_disable_data.get("result") or {}).get("enable") is False


def test_2fa_login_verify_and_recovery_rest_flow(client, auth_headers, user_credentials):
    """登录场景下的 2FA passcode / recovery 验证应走新的 REST 主链。"""
    import pyotp

    enable_request_response = client.post("/api/2fa/enable/request", headers=auth_headers)
    assert enable_request_response.status_code == 200, enable_request_response.get_data(as_text=True)
    enable_request_data = enable_request_response.get_json() or {}
    secret = ((enable_request_data.get("result") or {}).get("secret"))
    assert secret

    confirm_response = client.post("/api/2fa/enable/confirm", json={
        "secret": secret,
        "passcode": pyotp.TOTP(secret).now()
    }, headers=auth_headers)
    assert confirm_response.status_code == 200, confirm_response.get_data(as_text=True)
    confirm_data = confirm_response.get_json() or {}
    recovery_codes = (confirm_data.get("result") or {}).get("recoveryCodes") or []
    assert len(recovery_codes) == 8

    login_response = client.post("/api/auth/login", json={
        "loginName": user_credentials["username"],
        "password": user_credentials["password"]
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)
    login_data = login_response.get_json() or {}
    login_result = login_data.get("result") or {}
    assert login_result.get("need2FA") is True
    pending_token = login_result.get("token")
    assert pending_token

    verify_response = client.post("/api/2fa/verify", json={
        "passcode": pyotp.TOTP(secret).now()
    }, headers={"Authorization": f"Bearer {pending_token}"})
    assert verify_response.status_code == 200, verify_response.get_data(as_text=True)
    verify_data = verify_response.get_json() or {}
    verify_result = verify_data.get("result") or {}
    assert verify_result.get("token")
    assert verify_result.get("need2FA") is False

    login_response_2 = client.post("/api/auth/login", json={
        "loginName": user_credentials["username"],
        "password": user_credentials["password"]
    })
    assert login_response_2.status_code == 200, login_response_2.get_data(as_text=True)
    login_result_2 = (login_response_2.get_json() or {}).get("result") or {}
    pending_token_2 = login_result_2.get("token")
    assert login_result_2.get("need2FA") is True
    assert pending_token_2

    auth_module.TWO_FACTOR_RECOVERY_CODES.clear()

    recovery_response = client.post("/api/2fa/recovery/verify", json={
        "recoveryCode": recovery_codes[0]
    }, headers={"Authorization": f"Bearer {pending_token_2}"})
    assert recovery_response.status_code == 200, recovery_response.get_data(as_text=True)
    recovery_data = recovery_response.get_json() or {}
    recovery_result = recovery_data.get("result") or {}
    assert recovery_result.get("token")
    assert recovery_result.get("need2FA") is False

    login_response_3 = client.post("/api/auth/login", json={
        "loginName": user_credentials["username"],
        "password": user_credentials["password"]
    })
    assert login_response_3.status_code == 200, login_response_3.get_data(as_text=True)
    login_result_3 = (login_response_3.get_json() or {}).get("result") or {}
    pending_token_3 = login_result_3.get("token")
    assert login_result_3.get("need2FA") is True
    assert pending_token_3

    auth_module.TWO_FACTOR_RECOVERY_CODES.clear()

    reused_recovery_response = client.post("/api/2fa/recovery/verify", json={
        "recoveryCode": recovery_codes[0]
    }, headers={"Authorization": f"Bearer {pending_token_3}"})
    assert reused_recovery_response.status_code == 401, reused_recovery_response.get_data(as_text=True)
    reused_recovery_data = reused_recovery_response.get_json() or {}
    assert reused_recovery_data.get("error") == "Invalid recovery code"

    disable_response = client.post("/api/2fa/disable", json={
        "password": user_credentials["password"]
    }, headers={"Authorization": f"Bearer {recovery_result['token']}"})
    assert disable_response.status_code == 200, disable_response.get_data(as_text=True)


@pytest.mark.parametrize("legacy_path,payload", [
    ("/api/v1/users/2fa/enable/request.json", None),
    ("/api/v1/users/2fa/enable/confirm.json", {"secret": "legacy", "passcode": "000000"}),
    ("/api/v1/users/2fa/disable.json", {"password": "legacy"}),
    ("/api/v1/users/2fa/recovery/regenerate.json", {"password": "legacy"}),
    ("/api/2fa/authorize.json", {"passcode": "000000"}),
    ("/api/2fa/recovery.json", {"recoveryCode": "AAAA-BBBB"})
])
def test_2fa_write_legacy_routes_removed(client, auth_headers, legacy_path, payload):
    """旧 2FA 写接口路径应已移除。"""
    response = client.post(legacy_path, json=payload, headers=auth_headers)
    assert response.status_code == 404
