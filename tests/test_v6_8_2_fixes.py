"""Current-contract regression checks for asset trends and frontend service guards."""

import asyncio
import os
import time
from datetime import datetime, timedelta

import pytest

from src.api.app import app, initialize
from tests.runtime_paths import get_test_db_path


@pytest.fixture(scope='module', name='client')
def _client_fixture():
    """Create a Flask test client bound to an initialized test database."""

    async def init_services() -> None:
        await initialize(db_path=str(get_test_db_path('test_v682_fixes.db')))

    asyncio.run(init_services())
    app.config['TESTING'] = True

    with app.test_client() as test_client:
        yield test_client


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """Get authenticated headers for statistics endpoints."""
    login_response = client.post('/api/auth/login', json={
        'loginName': 'admin',
        'password': 'admin123',
    })

    if login_response.status_code != 200:
        suffix = int(time.time())
        username = f'test_v682_{suffix}'
        register_response = client.post('/api/auth/register', json={
            'username': username,
            'email': f'{username}@example.com',
            'password': 'Test123456!',
            'nickname': username,
        })
        assert register_response.status_code in [200, 409]
        login_response = client.post('/api/auth/login', json={
            'loginName': username,
            'password': 'Test123456!',
        })

    assert login_response.status_code == 200
    data = login_response.get_json() or {}
    token = (data.get('result') or {}).get('token')
    assert token
    return {'Authorization': f'Bearer {token}'}


class TestAssetTrendsAPILimits:
    """Asset trends should follow the current 365-day REST contract."""

    def test_reject_over_365_days(self, client, auth_headers):
        """Requests beyond 365 days should be rejected."""
        now = datetime.now()
        start_time = int((now - timedelta(days=366)).timestamp())
        end_time = int(now.timestamp())

        response = client.get(
            '/api/statistics/asset-trends',
            query_string={'startTime': start_time, 'endTime': end_time},
            headers=auth_headers,
        )

        data = response.get_json() or {}
        assert response.status_code == 400, f"期望400，实际{response.status_code}"
        assert data['success'] is False
        assert '365天' in data.get('error', '') or '365天' in data.get('errorMessage', '')

    def test_accept_365_days_or_less(self, client, auth_headers):
        """Requests within 365 days should succeed."""
        now = datetime.now()
        start_time = int((now - timedelta(days=365)).timestamp())
        end_time = int(now.timestamp())

        response = client.get(
            '/api/statistics/asset-trends',
            query_string={'startTime': start_time, 'endTime': end_time},
            headers=auth_headers,
        )

        data = response.get_json() or {}
        assert response.status_code == 200, f"期望200，实际{response.status_code}"
        assert data['success'] is True
        assert 'result' in data

    def test_accept_short_window(self, client, auth_headers):
        """Shorter windows remain valid."""
        now = datetime.now()
        start_time = int((now - timedelta(days=7)).timestamp())
        end_time = int(now.timestamp())

        response = client.get(
            '/api/statistics/asset-trends',
            query_string={'startTime': start_time, 'endTime': end_time},
            headers=auth_headers,
        )

        data = response.get_json() or {}
        assert response.status_code == 200, f"期望200，实际{response.status_code}"
        assert data['success'] is True
        assert 'result' in data


class TestFrontendServicesTypeScript:
    """Static checks for the current frontend service implementation."""

    def test_services_ts_has_response_guard(self):
        """The cancelable response branch should guard error.response before access."""
        services_path = os.path.join(
            os.path.dirname(__file__),
            '..',
            'src',
            'web',
            'src',
            'lib',
            'services.ts',
        )

        assert os.path.exists(services_path), f"services.ts不存在: {services_path}"

        with open(services_path, 'r', encoding='utf-8') as file_obj:
            content = file_obj.read()

        assert "if (error.response?.config && 'cancelableUuid' in error.response.config" in content
        assert "if (error.response && !error.response.config.ignoreError)" in content
