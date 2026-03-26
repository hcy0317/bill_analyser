"""测试账单导入v2三阶段API."""
import os
import sys
import asyncio
import pytest
import aiosqlite

# 添加项目根目录到路径
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.api.app import create_app


@pytest.fixture
def client():
    """创建测试客户端."""
    app = create_app()
    app.config['TESTING'] = True
    with app.test_client() as test_client:
        yield test_client


class TestImportV2Endpoints:
    """测试导入v2三阶段API端点."""

    def test_parse_endpoint_exists(self, client):
        """测试阶段1解析端点存在."""
        response = client.post(
            '/api/bills/import/v2/parse',
            headers={'Authorization': 'Bearer test_token'}
        )
        assert response.status_code != 404, "阶段1解析端点应该存在"

    def test_dedup_endpoint_exists(self, client):
        """测试阶段2去重端点存在."""
        response = client.post(
            '/api/bills/import/v2/dedup',
            json={'session_id': 'test-session'},
            headers={'Authorization': 'Bearer test_token'}
        )
        assert response.status_code != 404, "阶段2去重端点应该存在"

    def test_confirm_endpoint_exists(self, client):
        """测试阶段3确认端点存在."""
        response = client.post(
            '/api/bills/import/v2/confirm',
            json={'session_id': 'test-session'},
            headers={'Authorization': 'Bearer test_token'}
        )
        assert response.status_code != 404, "阶段3确认端点应该存在"


class TestDatabaseTables:
    """测试导入相关数据库表."""

    def test_parser_template_table_exists(self, client):
        """验证bills_parser_template表已创建."""
        async def check_table():
            db_path = os.path.join(
                os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                'data', 'bills.db'
            )
            if not os.path.exists(db_path):
                return False

            async with aiosqlite.connect(db_path) as conn:
                cursor = await conn.execute(
                    "SELECT name FROM sqlite_master "
                    "WHERE type='table' AND name='bills_parser_template'"
                )
                row = await cursor.fetchone()
                return row is not None

        result = asyncio.run(check_table())
        assert result is True, "bills_parser_template表应该存在"

    def test_preview_table_exists(self, client):
        """验证bills_preview表已创建."""
        async def check_table():
            db_path = os.path.join(
                os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                'data', 'bills.db'
            )
            if not os.path.exists(db_path):
                return False

            async with aiosqlite.connect(db_path) as conn:
                cursor = await conn.execute(
                    "SELECT name FROM sqlite_master "
                    "WHERE type='table' AND name='bills_preview'"
                )
                row = await cursor.fetchone()
                return row is not None

        result = asyncio.run(check_table())
        assert result is True, "bills_preview表应该存在"

    def test_import_sessions_table_exists(self, client):
        """验证import_sessions表已创建."""
        async def check_table():
            db_path = os.path.join(
                os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                'data', 'bills.db'
            )
            if not os.path.exists(db_path):
                return False

            async with aiosqlite.connect(db_path) as conn:
                cursor = await conn.execute(
                    "SELECT name FROM sqlite_master "
                    "WHERE type='table' AND name='import_sessions'"
                )
                row = await cursor.fetchone()
                return row is not None

        result = asyncio.run(check_table())
        assert result is True, "import_sessions表应该存在"


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
