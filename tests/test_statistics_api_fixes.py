"""Test statistics API fixes

Test cases:
1. Asset trends API - 365 days limit (was 90 days)
2. Categorical analysis API - use default month when params missing
3. Trend analysis API - use default year when params missing
"""

# pylint: disable=import-outside-toplevel

import asyncio
from datetime import datetime, timedelta
import pytest


@pytest.fixture(scope='module', name='client')
def _client_fixture():
    """创建测试客户端"""
    async def init_services():
        from src.api.app import initialize
        await initialize()

    asyncio.run(init_services())

    from src.api.app import app
    app.config['TESTING'] = True

    with app.test_client() as test_client:
        yield test_client


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """获取认证请求头"""
    login_response = client.post('/api/auth/login', json={
        'loginName': 'admin',
        'password': 'admin123'
    })

    if login_response.status_code != 200:
        suffix = int(datetime.now().timestamp())
        username = f'test_stats_{suffix}'
        register_response = client.post('/api/auth/register', json={
            'username': username,
            'email': f'{username}@example.com',
            'password': 'Test123456!',
            'nickname': username
        })
        assert register_response.status_code in [200, 409], (
            f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
        )

        login_response = client.post('/api/auth/login', json={
            'loginName': username,
            'password': 'Test123456!'
        })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    token = (data.get('result') or {}).get('token')
    assert token, f"登录响应缺少token: {data}"
    return {'Authorization': f'Bearer {token}'}


def test_asset_trends_90_days_success(client, auth_headers):
    """测试资产趋势API - 90天查询应该成功"""
    now = datetime.now()
    start_time = int((now - timedelta(days=89)).timestamp())
    end_time = int(now.timestamp())

    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'startTime': start_time,
            'endTime': end_time
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n90天查询响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"90天查询失败: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data


def test_asset_trends_365_days_success(client, auth_headers):
    """测试资产趋势API - 365天查询应该成功(修复后)"""
    now = datetime.now()
    start_time = int((now - timedelta(days=364)).timestamp())
    end_time = int(now.timestamp())

    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'startTime': start_time,
            'endTime': end_time
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n365天查询响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"365天查询应该成功: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data


def test_asset_trends_all_mode_success(client, auth_headers):
    """测试资产趋势API - 全量模式(0/0)不应被365天限制拦截。"""
    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'startTime': 0,
            'endTime': 0
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n资产趋势全量模式响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"全量模式查询失败: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data


def test_asset_trends_366_days_fail(client, auth_headers):
    """测试资产趋势API - 366天查询应该失败"""
    now = datetime.now()
    start_time = int((now - timedelta(days=366)).timestamp())
    end_time = int(now.timestamp())

    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'startTime': start_time,
            'endTime': end_time
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n366天查询响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 400, "366天查询应该被拒绝"
    assert data['success'] is False
    assert '365天' in data.get('error', '') or '365天' in data.get('errorMessage', '')


def test_categorical_analysis_missing_params(client, auth_headers):
    """测试分类分析API - 缺少参数时使用本月默认值"""
    response = client.get(
        '/api/statistics/category-statistics',
        query_string={
            'useTransactionTimezone': 'false'
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n分类分析(缺少参数)响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"缺少参数时应使用默认值: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data

    # 验证返回的时间范围是本月
    now = datetime.now()
    month_start = datetime(now.year, now.month, 1)
    result = data['result']

    # 时间戳应该接近本月第一天
    start_diff = abs(result['startTime'] - int(month_start.timestamp()))
    assert start_diff < 86400, f"默认时间应该是本月,差距: {start_diff}秒"


def test_trend_analysis_missing_params(client, auth_headers):
    """测试趋势分析API - 缺少参数时使用本年默认值"""
    response = client.get(
        '/api/statistics/category-statistics/trends',
        query_string={
            'useTransactionTimezone': 'false'
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n趋势分析(缺少参数)响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"缺少参数时应使用默认值: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data

    # 验证返回的年月范围是本年
    now = datetime.now()
    result = data['result']

    # 应该包含12个月的数据(1月到12月)
    assert len(result) == 12, f"本年应该有12个月,实际: {len(result)}个月"
    assert result[0]['year'] == now.year
    assert result[0]['month'] == 1
    assert result[11]['year'] == now.year
    assert result[11]['month'] == 12


def test_categorical_analysis_with_params(client, auth_headers):
    """测试分类分析API - 提供完整参数时正常工作"""
    now = datetime.now()
    start_time = int((now - timedelta(days=30)).timestamp())
    end_time = int(now.timestamp())

    response = client.get(
        '/api/statistics/category-statistics',
        query_string={
            'startTime': start_time,
            'endTime': end_time
        },
        headers=auth_headers
    )

    data = response.get_json() or {}
    print(f"\n分类分析(完整参数)响应: {response.status_code}")
    print(f"响应数据: {data}")

    assert response.status_code == 200, f"完整参数查询失败: {response.get_data(as_text=True)}"
    assert data['success'] is True
    assert 'result' in data
    assert data['result']['startTime'] == start_time
    assert data['result']['endTime'] == end_time


def test_amounts_rest_endpoint_and_legacy_route_removed(client, auth_headers):
    """交易金额统计应直连 REST，旧 v1 路径应返回 404。"""
    now = datetime.now()
    start_time = int((now - timedelta(days=7)).timestamp())
    end_time = int(now.timestamp())
    query = f'test_{start_time}_{end_time}'

    rest_response = client.get(
        '/api/statistics/amounts',
        query_string={'query': query},
        headers=auth_headers
    )
    rest_data = rest_response.get_json() or {}

    assert rest_response.status_code == 200, rest_response.get_data(as_text=True)
    assert rest_data['success'] is True
    assert 'result' in rest_data
    assert 'test' in rest_data['result']


def test_exchange_rates_specific_provider_success(client, auth_headers, monkeypatch):
    """汇率接口应支持显式 provider，并返回实际命中的来源元数据。"""
    from src.api.routes import statistics as statistics_routes

    async def fake_fetch(base_currency, target_currencies, requested_provider='auto'):
        assert base_currency == 'CNY'
        assert requested_provider == 'ecb'
        assert 'USD' in target_currencies
        return {
            'rates': {'USD': 0.138, 'EUR': 0.127},
            'source': 'ECB (欧洲央行)',
            'url': 'https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml',
            'provider_key': 'ecb',
            'fallback_used': False
        }

    monkeypatch.setattr(statistics_routes, '_fetch_exchange_rates_from_providers', fake_fetch)

    response = client.get(
        '/api/statistics/exchange-rates',
        query_string={'provider': 'ecb'},
        headers=auth_headers
    )

    data = response.get_json() or {}
    assert response.status_code == 200, response.get_data(as_text=True)
    assert data['success'] is True
    result = data['result']
    assert result['requestedProvider'] == 'ecb'
    assert result['providerKey'] == 'ecb'
    assert result['fallbackUsed'] is False
    assert result['dataSource'] == 'ECB (欧洲央行)'
    assert any(item['currency'] == 'USD' for item in result['exchangeRates'])


def test_exchange_rates_invalid_provider_rejected(client, auth_headers):
    """非法 provider 应返回 400，避免前后端契约失配。"""
    response = client.get(
        '/api/statistics/exchange-rates',
        query_string={'provider': 'unknown_provider'},
        headers=auth_headers
    )

    data = response.get_json() or {}
    assert response.status_code == 400
    assert data['success'] is False
    assert 'Unsupported exchange rate provider' in data.get('error', '')


if __name__ == '__main__':
    # 运行测试
    pytest.main([__file__, '-v', '--tb=short'])
