"""快速测试脚本 - 验证当前运行态已无任何 /api/v1 路由。"""

import sys
from pathlib import Path

# 添加项目根目录到路径
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from src.api.app import create_app

def test_v1_routes():
    """测试当前运行态所有 /api/v1 路由已不再注册。"""
    print("=" * 60)
    print("测试当前运行态 /api/v1 路由移除情况")
    print("=" * 60)
    
    # 创建Flask应用（不初始化数据库）
    app = create_app()
    
    # 获取所有路由
    routes = []
    for rule in app.url_map.iter_rules():
        routes.append({
            'endpoint': rule.endpoint,
            'methods': sorted(rule.methods - {'HEAD', 'OPTIONS'}),
            'path': str(rule)
        })
    
    v1_routes = [route for route in routes if '/api/v1/' in route['path']]

    print("\n当前检测到的 /api/v1 路由:")
    routes_removed = not v1_routes

    if v1_routes:
        for route in v1_routes:
            methods_str = ', '.join(route['methods'])
            print(f"  ❌ {route['path']} [{methods_str}] -> {route['endpoint']}")
    else:
        print("  ✓ 未检测到任何 /api/v1 路由")
    
    print("=" * 60)
    if routes_removed:
        print("✓ 当前运行态所有 /api/v1 路由已全部移除")
    else:
        print("❌ 当前运行态仍存在未移除的 /api/v1 路由")
    print("=" * 60)
    
    return routes_removed

if __name__ == '__main__':
    try:
        success = test_v1_routes()
        sys.exit(0 if success else 1)
    except Exception as e:
        print(f"❌ 测试失败: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
