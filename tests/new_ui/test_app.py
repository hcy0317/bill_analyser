"""
测试Flask应用的基本功能
"""
import json
from uuid import uuid4

import pytest

from bill_analyser import __version__


class TestApp:
    """应用基本功能测试"""

    def test_app_exists(self, app):
        """测试应用实例存在"""
        assert app is not None

    def test_app_is_testing(self, app):
        """测试应用处于测试模式"""
        assert app.config["TESTING"] is True

    def test_health_check(self, client):
        """测试健康检查端点"""
        response = client.get("/api/health")
        assert response.status_code == 200

        data = json.loads(response.data)
        assert data["success"] is True
        assert data["status"] == "healthy"
        assert data["version"] == __version__

    def test_cors_headers(self, client):
        """测试CORS头部"""
        response = client.get("/api/health", headers={"Origin": "http://localhost:8081"})
        assert response.headers.get("Access-Control-Allow-Origin") == "http://localhost:8081"

    def test_404_handling(self, client):
        """测试404错误处理"""
        response = client.get("/api/nonexistent-endpoint")
        assert response.status_code == 404

    def test_db_initialized(self, db):
        """测试数据库已初始化"""
        assert db is not None

    def test_category_engine_initialized(self, category_engine):
        """测试分类引擎已初始化"""
        assert category_engine is not None

    @pytest.mark.asyncio
    async def test_category_engine_has_rules(self, category_engine, db):
        """测试分类引擎从 category_rules canonical source 加载规则。"""
        suffix = uuid4().hex[:8]
        category_id = await db.create_category({
            "main_category": f"TestMain-{suffix}",
            "sub_category": f"TestSub-{suffix}",
            "keywords": "test_keyword",
        })
        assert category_id is not None

        rule_id = await db.create_category_rule(
            {
                "category_id": category_id,
                "name": "test canonical rule",
                "priority": 1,
                "rule_expression": "OR={test_keyword}",
                "regex_enabled": False,
                "enabled": True,
            }
        )
        assert rule_id is not None

        # 重新加载规则
        await category_engine.load_rules_from_db(db)

        assert hasattr(category_engine, "rules")
        assert isinstance(category_engine.rules, list)
        assert len(category_engine.rules) > 0


class TestAPIBlueprints:
    """测试API蓝图注册"""

    def test_bills_blueprint_registered(self, client):
        """测试bills import sidecar 蓝图仍保留"""
        response = client.get("/api/bills/import/parsers")
        # 应该返回200或其他有效响应，而不是404
        assert response.status_code != 404

    def test_categories_blueprint_removed_from_flask_sidecar(self, client):
        """分类 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
        response = client.get("/api/categories/")
        assert response.status_code == 404

    def test_category_rules_blueprint_removed_from_flask_sidecar(self, client):
        """分类规则 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
        response = client.get("/api/category-rules/")
        assert response.status_code == 404

    def test_tags_blueprint_removed_from_flask_sidecar(self, client):
        """标签 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
        response = client.get("/api/tags/")
        assert response.status_code == 404

    def test_templates_blueprint_removed_from_flask_sidecar(self, client):
        """模板 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
        response = client.get("/api/templates/")
        assert response.status_code == 404

    def test_settings_bundle_blueprint_removed_from_flask_sidecar(self, client):
        """设置包 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
        response = client.get("/api/settings/bundle/export")
        assert response.status_code == 404

    def test_bills_category_actions_removed_from_flask_sidecar(self, client):
        """账单非 import route shell 已由 Rust runtime 接管，不再注册 Flask sidecar。"""
        cases = [
            ("get", "/api/bills/"),
            ("post", "/api/bills/"),
            ("get", "/api/bills/by-month?year=2026&month=1&type=0"),
            ("get", "/api/bills/get?id=1"),
            ("get", "/api/bills/1"),
            ("put", "/api/bills/1"),
            ("delete", "/api/bills/1"),
            ("post", "/api/bills/modify"),
            ("post", "/api/bills/delete"),
            ("post", "/api/bills/batch"),
            ("get", "/api/bills/export"),
            ("post", "/api/bills/pictures"),
            ("post", "/api/bills/pictures/unused"),
            ("get", "/api/bills/1/recurring-candidates"),
            ("put", "/api/bills/1/recurring-match"),
            ("delete", "/api/bills/1/recurring-match"),
            ("post", "/api/bills/category/quick-add-keyword"),
            ("post", "/api/bills/category/refresh"),
            ("get", "/api/bills/reconciliation_statements?account_id=1&start_time=0&end_time=0"),
            ("put", "/api/bills/batch/update"),
            ("delete", "/api/bills/batch/delete"),
        ]

        for method, url in cases:
            response = getattr(client, method)(url, json={})
            assert response.status_code in (404, 405), (
                f"{method.upper()} {url} should not be registered in Flask sidecar"
            )

    def test_statistics_analyzer_removed_from_flask_sidecar(self, client):
        """统计 Analyzer 路由已由 Rust runtime 接管，不再注册 Flask sidecar。"""
        response = client.get("/api/statistics/overview")
        assert response.status_code == 404


class TestErrorHandling:
    """测试错误处理"""

    def test_invalid_json(self, client, auth_headers):
        """测试无效的JSON请求"""
        response = client.post(
            "/api/bills/import/batch",
            data="invalid json",
            content_type="application/json",
            headers=auth_headers
        )
        # 应该返回400或500，而不是崩溃
        assert response.status_code in (400, 415, 500)

    def test_method_not_allowed(self, client):
        """测试不允许的HTTP方法"""
        response = client.patch("/api/health")
        assert response.status_code == 405  # Method Not Allowed
