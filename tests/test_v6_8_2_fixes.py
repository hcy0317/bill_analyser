"""
v6.8.2 修复验证测试 - 前端响应拦截器null检查 + 资产趋势API时间范围限制

测试内容:
1. 前端services.ts - error.response null检查（TypeScript语法检查）
2. 资产趋势API - 拒绝超过90天的请求
3. 资产趋势API - 接受90天内的请求
"""

import pytest
import sys
import os

# 添加项目根目录到路径
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from src.api.app import create_app
from datetime import datetime, timedelta


@pytest.fixture
def client():
    """创建Flask测试客户端"""
    flask_app = create_app()
    flask_app.config['TESTING'] = True
    with flask_app.test_client() as client:
        yield client


class TestAssetTrendsAPILimits:
    """资产趋势API时间范围限制测试"""

    def test_reject_over_90_days(self, client):
        """测试：拒绝超过90天的请求"""
        # 计算时间范围：365天（整年）
        start_time = int(datetime(2025, 1, 1).timestamp())
        end_time = int(datetime(2025, 12, 31).timestamp())
        
        print(f"\n[测试] 发送365天时间范围请求")
        print(f"  start_time: {start_time} ({datetime.fromtimestamp(start_time)})")
        print(f"  end_time: {end_time} ({datetime.fromtimestamp(end_time)})")
        
        response = client.get(
            f'/api/v1/transactions/statistics/asset_trends.json?startTime={start_time}&endTime={end_time}'
        )
        
        print(f"[响应] 状态码: {response.status_code}")
        data = response.get_json()
        print(f"[响应] 数据: {data}")
        
        # 验证：应该返回400错误
        assert response.status_code == 400, f"期望400，实际{response.status_code}"
        assert data['success'] is False, "success应该为False"
        assert 'errorMessage' in data, "应包含errorMessage"
        assert '90天' in data['errorMessage'], "错误消息应提到90天限制"
        
        print("✅ 成功拒绝超过90天的请求")

    def test_accept_90_days_or_less(self, client):
        """测试：接受90天内的请求"""
        # 计算时间范围：7天
        now = datetime.now()
        start_time = int((now - timedelta(days=7)).timestamp())
        end_time = int(now.timestamp())
        
        print(f"\n[测试] 发送7天时间范围请求")
        print(f"  start_time: {start_time} ({datetime.fromtimestamp(start_time)})")
        print(f"  end_time: {end_time} ({datetime.fromtimestamp(end_time)})")
        
        response = client.get(
            f'/api/v1/transactions/statistics/asset_trends.json?startTime={start_time}&endTime={end_time}'
        )
        
        print(f"[响应] 状态码: {response.status_code}")
        data = response.get_json()
        print(f"[响应] success: {data.get('success')}")
        
        # 验证：应该返回200成功
        assert response.status_code == 200, f"期望200，实际{response.status_code}"
        assert data['success'] is True, f"success应该为True，实际{data}"
        assert 'result' in data, "应包含result字段"
        
        print(f"✅ 成功接受7天时间范围请求，返回 {len(data['result'])} 天的数据")

    def test_accept_exactly_90_days(self, client):
        """测试：边界条件 - 恰好90天应该被接受"""
        # 计算时间范围：恰好90天
        now = datetime.now()
        start_time = int((now - timedelta(days=90)).timestamp())
        end_time = int(now.timestamp())
        
        print(f"\n[测试] 发送恰好90天时间范围请求")
        print(f"  start_time: {start_time} ({datetime.fromtimestamp(start_time)})")
        print(f"  end_time: {end_time} ({datetime.fromtimestamp(end_time)})")
        
        response = client.get(
            f'/api/v1/transactions/statistics/asset_trends.json?startTime={start_time}&endTime={end_time}'
        )
        
        print(f"[响应] 状态码: {response.status_code}")
        data = response.get_json()
        
        # 验证：应该返回200成功
        assert response.status_code == 200, f"期望200，实际{response.status_code}"
        assert data['success'] is True, f"success应该为True"
        
        print("✅ 成功接受恰好90天的请求")

    def test_reject_91_days(self, client):
        """测试：边界条件 - 91天应该被拒绝"""
        # 计算时间范围：91天
        now = datetime.now()
        start_time = int((now - timedelta(days=91)).timestamp())
        end_time = int(now.timestamp())
        
        print(f"\n[测试] 发送91天时间范围请求")
        
        response = client.get(
            f'/api/v1/transactions/statistics/asset_trends.json?startTime={start_time}&endTime={end_time}'
        )
        
        print(f"[响应] 状态码: {response.status_code}")
        data = response.get_json()
        
        # 验证：应该返回400错误
        assert response.status_code == 400, f"期望400，实际{response.status_code}"
        assert data['success'] is False, "success应该为False"
        
        print("✅ 成功拒绝91天的请求")


class TestFrontendServicesTypeScript:
    """前端services.ts代码验证（静态检查）"""

    def test_services_ts_syntax(self):
        """测试：验证services.ts文件存在且可读"""
        services_path = os.path.join(
            os.path.dirname(__file__), 
            '..', 
            'ezbookkeeping', 
            'src', 
            'lib', 
            'services.ts'
        )
        
        assert os.path.exists(services_path), f"services.ts不存在: {services_path}"
        
        with open(services_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 验证：error.response访问前应该有null检查
        # 查找 Line 228附近的响应拦截器错误处理
        assert "}, error => {" in content, "应包含错误处理函数"
        
        # 验证：应该检查error.response存在性
        lines = content.split('\n')
        error_handler_found = False
        proper_check_found = False
        
        for i, line in enumerate(lines):
            if "}, error => {" in line:
                error_handler_found = True
                # 检查后续10行是否有正确的null检查
                for j in range(i, min(i+10, len(lines))):
                    # 应该先检查 error.response 存在
                    if "if (error.response &&" in lines[j] and "'cancelableUuid' in error.response.config" in lines[j]:
                        proper_check_found = True
                        print(f"✅ 在Line {j+1}找到正确的null检查")
                        break
                break
        
        assert error_handler_found, "未找到错误处理函数"
        assert proper_check_found, "未找到正确的error.response null检查"
        
        print("✅ services.ts包含正确的null检查逻辑")


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--timeout=30'])
