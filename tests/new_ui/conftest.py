"""
测试配置和Fixtures
"""
import pytest
import pytest_asyncio
import sys
import asyncio
import time
from pathlib import Path

# 添加项目根目录到路径
PROJECT_ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

# 设置Windows事件循环策略
if sys.platform == 'win32':
    asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())


@pytest_asyncio.fixture(scope="session")
async def initialize_app():
    """异步初始化Flask应用"""
    from src.api.app import initialize
    
    # 使用测试数据库
    test_db_path = Path(__file__).parent.parent.parent / "data" / "test_bills.db"
    # 如果存在则删除，确保干净的环境
    if test_db_path.exists():
        try:
            test_db_path.unlink()
        except PermissionError:
            pass # 如果被占用则忽略，可能导致测试失败但比直接崩溃好
            
    await initialize(db_path=str(test_db_path))


@pytest.fixture(scope="session")
async def app(initialize_app):
    """创建Flask应用实例（依赖异步初始化）"""
    from src.api.app import app, db
    from src.utils.logger import _logger_instance
    
    # 配置测试模式
    app.config['TESTING'] = True
    app.config['DEBUG'] = False
    
    yield app
    
    # Cleanup: 确保所有资源正确释放
    if db:
        await db.close()
        
    # 停止日志监听器
    _logger_instance.stop()


@pytest.fixture(scope="session")
def client(app):
    """创建测试客户端"""
    return app.test_client()


@pytest.fixture(scope="session")
def auth_context(client):
    """返回可复用的测试认证上下文。"""
    username = 'admin'
    password = 'admin123'

    login_response = client.post('/api/auth/login', json={
        'loginName': username,
        'password': password
    })

    if login_response.status_code != 200:
        suffix = int(time.time())
        username = f'test_new_ui_{suffix}'
        password = 'Test123456!'
        register_response = client.post('/api/auth/register', json={
            'username': username,
            'email': f'{username}@example.com',
            'password': password,
            'nickname': username
        })
        assert register_response.status_code in [200, 409], (
            f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
        )

        login_response = client.post('/api/auth/login', json={
            'loginName': username,
            'password': password
        })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    result = data.get('result') or {}
    token = result.get('token')
    assert token, f"登录响应缺少token: {data}"

    return {
        'username': username,
        'password': password,
        'token': token,
        'refresh_token': result.get('refreshToken'),
        'headers': {'Authorization': f'Bearer {token}'},
        'user': result.get('user') or {}
    }


@pytest.fixture(scope="session")
def auth_headers(auth_context):
    """返回可复用的认证请求头。"""
    return auth_context['headers']


@pytest.fixture(scope="session")
def db():
    """获取数据库实例"""
    from src.api.app import db
    return db


@pytest.fixture(scope="session")
def category_engine(initialize_app):
    """获取分类引擎实例"""
    from src.api.app import category_engine
    return category_engine
