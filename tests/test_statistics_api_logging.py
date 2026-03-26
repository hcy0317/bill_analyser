"""统计分析API日志验证测试。"""

# pylint: disable=import-outside-toplevel,wrong-import-position,redefined-outer-name

import asyncio
from datetime import datetime, timedelta
import os
import sys
import pytest

# 添加项目根目录到Python路径
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from src.api.app import app, initialize


@pytest.fixture
def client():
    """创建Flask测试客户端"""
    asyncio.run(initialize())
    app.config['TESTING'] = True
    with app.test_client() as test_client:
        yield test_client


@pytest.fixture(name='auth_headers')
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    login_response = client.post('/api/auth/login', json={
        'loginName': 'admin',
        'password': 'admin123'
    })

    if login_response.status_code != 200:
        suffix = int(datetime.now().timestamp())
        username = f'test_stats_log_{suffix}'
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


def test_categorical_analysis_api_logging(client, auth_headers):
    """测试分类分析API的详细日志输出"""
    # 查询2025年11月数据
    response = client.get(
        '/api/statistics/category-statistics',
        query_string={
            'start_time': 1761926400,  # 2025-11-01
            'end_time': 1764518399      # 2025-11-30
        },
        headers=auth_headers
    )

    # 打印响应以便调试
    if response.status_code != 200:
        print(f"错误响应: {response.status_code}")
        print(f"响应内容: {response.get_json()}")

    assert response.status_code == 200
    data = response.get_json()
    assert data['success'] is True
    assert 'result' in data
    assert 'items' in data['result']

    # 验证返回的数据结构
    result = data['result']
    assert 'startTime' in result
    assert 'endTime' in result
    assert isinstance(result['items'], list)

    # 预期日志内容（在logs/bill_analyser_*.log中）:
    # - [分类分析] API调用
    # - [分类分析] 完整请求
    # - [分类分析] 查询到 X 条账单
    # - [分类分析] 账单示例1/2/3
    # - [分类分析] 分类列表
    # - [分类分析] 账户列表
    # - [分类分析] 开始处理账单
    # - [分类分析] 处理账单1/2/3
    # - [分类分析] 统计结果1/2/3
    # - [分类分析] ✅ 返回结果


def test_trend_analysis_api_logging(client, auth_headers):
    """测试趋势分析API的详细日志输出"""
    # 查询2025年11月趋势
    response = client.get(
        '/api/statistics/category-statistics/trends',
        query_string={
            'start_year_month': '2025-11',
            'end_year_month': '2025-11'
        },
        headers=auth_headers
    )

    assert response.status_code == 200
    data = response.get_json()
    assert data['success'] is True
    assert 'result' in data
    assert isinstance(data['result'], list)

    # 验证返回的数据结构
    if len(data['result']) > 0:
        month_data = data['result'][0]
        assert 'year' in month_data
        assert 'month' in month_data
        assert 'items' in month_data

    # 预期日志内容:
    # - [趋势分析] 查询到 X 条账单
    # - [趋势分析] 账单日期范围
    # - [趋势分析] 账单示例1/2/3
    # - [趋势分析] 月度统计
    # - [趋势分析] 月份1/2/3
    # - [趋势分析] ✅ 返回结果


def test_asset_trends_api_logging(client, auth_headers):
    """测试资产趋势API的详细日志输出"""
    # 查询7天资产趋势（2025-11-15 到 2025-11-22）
    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'start_time': 1763209934,  # 2025-11-15
            'end_time': 1763814734      # 2025-11-22
        },
        headers=auth_headers
    )

    assert response.status_code == 200
    data = response.get_json()
    assert data['success'] is True
    assert 'result' in data
    assert isinstance(data['result'], list)

    # 验证返回的数据结构
    if len(data['result']) > 0:
        day_data = data['result'][0]
        assert 'year' in day_data
        assert 'month' in day_data
        assert 'day' in day_data
        assert 'items' in day_data

        # 验证账户数据结构
        if len(day_data['items']) > 0:
            account_data = day_data['items'][0]
            assert 'accountId' in account_data
            assert 'accountOpeningBalance' in account_data
            assert 'accountClosingBalance' in account_data

    # 预期日志内容:
    # - [资产趋势] 查询到 X 个账户
    # - [资产趋势] 账户1/2/3
    # - [资产趋势] 计算第1/2/3天
    # - [资产趋势] 结果统计
    # - [资产趋势] 第一天前3个账户
    # - [资产趋势] ✅ 返回结果


def test_asset_trends_time_limit(client, auth_headers):
    """测试资产趋势API的时间范围限制（365天）。"""
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

    assert response.status_code == 400
    data = response.get_json()
    assert data['success'] is False
    assert 'error' in data or 'errorMessage' in data

    # 预期日志内容:
    # - [资产趋势] 时间跨度超过365天限制，拒绝请求


def test_statistics_api_error_handling(client, auth_headers):
    """测试统计分析API的参数错误处理。"""
    # v6.84后，缺少时间参数会默认使用本月，因此这里改测无效时间戳
    response = client.get(
        '/api/statistics/asset-trends',
        query_string={
            'startTime': 'invalid',
            'endTime': 'invalid'
        },
        headers=auth_headers
    )
    assert response.status_code == 400
    data = response.get_json() or {}
    assert data.get('success') is False
    assert 'error' in data


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
