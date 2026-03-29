"""
测试statistics API的所有端点
"""
import json
import asyncio
import time
import pytest
from datetime import datetime, timedelta

from tests.runtime_paths import get_test_db_path


@pytest.fixture(scope='module')
def client():
    """为统计测试创建独立应用与数据库，避免全量运行中的全局状态串扰。"""
    from bill_analyser.api.app import app, initialize

    test_db_path = get_test_db_path(f'test_statistics_api_{int(time.time() * 1000)}.db')
    if test_db_path.exists():
        test_db_path.unlink()

    asyncio.run(initialize(db_path=str(test_db_path)))
    app.config['TESTING'] = True
    app.config['DEBUG'] = False
    return app.test_client()


@pytest.fixture(scope='module')
def auth_headers(client):
    """为统计测试创建隔离用户，避免被前序模块写入的数据污染。"""
    username = f'test_statistics_{int(time.time() * 1000)}'
    password = 'Test123456!'

    register_response = client.post(
        '/api/auth/register',
        json={
            'username': username,
            'email': f'{username}@example.com',
            'password': password,
            'nickname': username,
        },
    )
    assert register_response.status_code in (200, 201, 409), register_response.get_data(as_text=True)

    login_response = client.post('/api/auth/login', json={'loginName': username, 'password': password})
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    result = (login_response.get_json() or {}).get('result') or {}
    token = result.get('token')
    assert token, login_response.get_data(as_text=True)
    return {'Authorization': f'Bearer {token}'}


class TestStatisticsAPI:
    """统计API测试类"""
    
    def test_get_overview(self, client, auth_headers):
        """测试获取总览统计"""
        response = client.get('/api/statistics/overview', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert 'result' in data
        
        # 验证统计数据结构
        stats = data['result']
        assert 'total_income' in stats
        assert 'total_expense' in stats
        assert 'net_income' in stats
        assert 'bill_count' in stats
        
        # 验证数据类型
        assert isinstance(stats['total_income'], (int, float))
        assert isinstance(stats['total_expense'], (int, float))
        assert isinstance(stats['net_income'], (int, float))
        assert isinstance(stats['bill_count'], int)
        
    def test_get_overview_with_date_range(self, client, auth_headers):
        """测试带日期范围的总览统计"""
        end_date = datetime.now().strftime('%Y-%m-%d')
        start_date = (datetime.now() - timedelta(days=30)).strftime('%Y-%m-%d')
        
        response = client.get(
            f'/api/statistics/overview?start_date={start_date}&end_date={end_date}',
            headers=auth_headers
        )
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        
    def test_get_trend_monthly(self, client, auth_headers):
        """测试获取月度趋势"""
        response = client.get('/api/statistics/trend?granularity=month', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert isinstance(data['data'], list)
        
        # 验证趋势数据结构
        if data['data']:
            first_point = data['data'][0]
            assert 'date' in first_point
            assert 'income' in first_point
            assert 'expense' in first_point
            assert 'net' in first_point
            
    def test_get_trend_weekly(self, client, auth_headers):
        """测试获取周度趋势"""
        response = client.get('/api/statistics/trend?granularity=week', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert isinstance(data['data'], list)
        
    def test_get_trend_daily(self, client, auth_headers):
        """测试获取日度趋势"""
        response = client.get('/api/statistics/trend?granularity=day', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert isinstance(data['data'], list)
        
    def test_get_category_pie(self, client, auth_headers):
        """测试获取分类饼图数据"""
        response = client.get('/api/statistics/category-pie', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert isinstance(data['data'], list)
        
        # 验证饼图数据结构
        if data['data']:
            first_item = data['data'][0]
            assert 'name' in first_item
            assert 'value' in first_item
            
    def test_get_category_pie_with_type(self, client, auth_headers):
        """测试按类型获取分类饼图"""
        response = client.get('/api/statistics/category-pie?type=支出', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        
    def test_get_top_merchants_default(self, client, auth_headers):
        """测试获取TOP商家（默认限制）"""
        response = client.get('/api/statistics/top-merchants', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert isinstance(data['data'], list)
        
        # 默认返回不超过10个
        assert len(data['data']) <= 10
        
        # 验证商家数据结构
        if data['data']:
            first_merchant = data['data'][0]
            assert 'name' in first_merchant
            assert 'amount' in first_merchant
            assert 'count' in first_merchant
            
    def test_get_top_merchants_with_limit(self, client, auth_headers):
        """测试带限制数量的TOP商家"""
        response = client.get('/api/statistics/top-merchants?limit=5', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert len(data['data']) <= 5
        
    def test_get_top_merchants_sorted(self, client, auth_headers):
        """测试TOP商家按金额排序"""
        response = client.get('/api/statistics/top-merchants?limit=5', headers=auth_headers)
        assert response.status_code == 200
        
        data = json.loads(response.data)
        if len(data['data']) >= 2:
            # 验证按金额降序排列
            for i in range(len(data['data']) - 1):
                assert data['data'][i]['amount'] >= data['data'][i + 1]['amount']


class TestStatisticsIntegration:
    """统计API集成测试"""
    
    def test_overview_consistency(self, client, auth_headers):
        """测试总览统计的一致性"""
        response = client.get('/api/statistics/overview', headers=auth_headers)
        data = json.loads(response.data)
        
        if response.status_code == 200:
            stats = data['result']
            # 净收入 = 总收入 - 总支出
            expected_net = round(stats['total_income'] - stats['total_expense'], 2)
            assert stats['net_income'] == expected_net
            
    def test_trend_data_ordered(self, client, auth_headers):
        """测试趋势数据按日期排序"""
        response = client.get('/api/statistics/trend?granularity=month', headers=auth_headers)
        data = json.loads(response.data)
        
        if len(data['data']) >= 2:
            dates = [item['date'] for item in data['data']]
            # 验证日期升序排列
            assert dates == sorted(dates)
            
    def test_category_pie_sum(self, client, auth_headers):
        """测试分类饼图金额总和与总览一致"""
        # 获取总览
        overview_response = client.get('/api/statistics/overview', headers=auth_headers)
        overview_data = json.loads(overview_response.data)
        
        # 获取支出分类饼图
        pie_response = client.get('/api/statistics/category-pie?type=支出', headers=auth_headers)
        pie_data = json.loads(pie_response.data)
        
        if overview_response.status_code == 200 and pie_response.status_code == 200:
            pie_sum = sum(item['value'] for item in pie_data['data'])
            # 允许浮点数误差
            assert abs(pie_sum - overview_data['result']['total_expense']) < 0.01


class TestStatisticsPerformance:
    """统计API性能测试"""
    
    @pytest.mark.timeout(5)
    def test_overview_performance(self, client, auth_headers):
        """测试总览统计响应时间（应在5秒内）"""
        response = client.get('/api/statistics/overview', headers=auth_headers)
        assert response.status_code == 200
        
    @pytest.mark.timeout(5)
    def test_trend_performance(self, client, auth_headers):
        """测试趋势统计响应时间（应在5秒内）"""
        response = client.get('/api/statistics/trend', headers=auth_headers)
        assert response.status_code == 200
