"""
测试Flask应用的基本功能
"""
import json

import pytest

from bill_analyser import __version__


class TestApp:
    """应用基本功能测试"""
    
    def test_app_exists(self, app):
        """测试应用实例存在"""
        assert app is not None
        
    def test_app_is_testing(self, app):
        """测试应用处于测试模式"""
        assert app.config['TESTING'] is True
        
    def test_health_check(self, client):
        """测试健康检查端点"""
        response = client.get('/api/health')
        assert response.status_code == 200
        
        data = json.loads(response.data)
        assert data['success'] is True
        assert data['status'] == 'healthy'
        assert data['version'] == __version__
        
    def test_cors_headers(self, client):
        """测试CORS头部"""
        response = client.get('/api/health', headers={'Origin': 'http://localhost:8081'})
        assert response.headers.get('Access-Control-Allow-Origin') == 'http://localhost:8081'
        
    def test_404_handling(self, client):
        """测试404错误处理"""
        response = client.get('/api/nonexistent-endpoint')
        assert response.status_code == 404
        
    def test_db_initialized(self, db):
        """测试数据库已初始化"""
        assert db is not None
        
    def test_category_engine_initialized(self, category_engine):
        """测试分类引擎已初始化"""
        assert category_engine is not None
        
    @pytest.mark.asyncio
    async def test_category_engine_has_rules(self, category_engine, db):
        """测试分类引擎已加载规则"""
        # Ensure there is at least one rule
        await db.create_category({
            'main_category': 'TestMain',
            'sub_category': 'TestSub',
            'keywords': 'test_keyword'
        })
        
        # Reload rules
        await category_engine.load_rules_from_db(db)
        
        assert hasattr(category_engine, 'rules')
        assert isinstance(category_engine.rules, list)
        assert len(category_engine.rules) > 0


class TestAPIBlueprints:
    """测试API蓝图注册"""
    
    def test_bills_blueprint_registered(self, client):
        """测试bills蓝图已注册"""
        response = client.get('/api/bills/')
        # 应该返回200或其他有效响应，而不是404
        assert response.status_code != 404
        
    def test_categories_blueprint_registered(self, client):
        """测试categories蓝图已注册"""
        response = client.get('/api/categories/')
        assert response.status_code != 404
        
    def test_statistics_blueprint_registered(self, client):
        """测试statistics蓝图已注册"""
        response = client.get('/api/statistics/overview')
        assert response.status_code != 404


class TestErrorHandling:
    """测试错误处理"""
    
    def test_invalid_json(self, client, auth_headers):
        """测试无效的JSON请求"""
        response = client.post(
            '/api/bills/batch',
            data='invalid json',
            content_type='application/json',
            headers=auth_headers
        )
        # 应该返回400或500，而不是崩溃
        assert response.status_code in (400, 500)
        
    def test_method_not_allowed(self, client):
        """测试不允许的HTTP方法"""
        response = client.patch('/api/health')
        assert response.status_code == 405  # Method Not Allowed
