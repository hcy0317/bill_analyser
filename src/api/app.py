"""
Flask Web API Server - 账单分析系统Web API服务器

提供RESTful API接口供Vue前端调用
"""

import asyncio
import sys
from pathlib import Path
from flask import Flask
from flask_cors import CORS

# 验证Python解释器路径
print(f"\n{'='*60}")
print(f"Python解释器: {sys.executable}")
print(f"Python版本: {sys.version}")
if '.venv' in sys.executable or 'venv' in sys.executable:
    print("[OK] 正在使用虚拟环境")
else:
    print("[WARN] 未使用虚拟环境!")
    print(f"当前路径: {sys.executable}")
    print(f"应该使用: {Path(__file__).parent.parent.parent / '.venv' / 'Scripts' / 'python.exe'}")
print(f"{'='*60}\n")

# 添加项目根目录到路径
PROJECT_ROOT = Path(__file__).parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

# pylint: disable=wrong-import-position,import-error
from src.core.db import Database
from src.core.bill_service import BillService
from src.core.category_engine import CategoryEngine
from src.utils.logger import get_logger

# 导入蓝图
from src.api.routes import bills, categories, statistics, accounts, tags, templates, auth, budgets, backup

logger = get_logger('WebAPI')

# 全局实例
db: Database = None
category_engine: CategoryEngine = None
bill_service: BillService = None


class URLRewriteMiddleware:
    """WSGI中间件 - 在Flask处理之前重写v1旧路径"""

    def __init__(self, flask_app):
        self.app = flask_app
        self.logger = get_logger('URLRewrite')

    def __call__(self, environ, start_response):
        """WSGI应用接口"""
        import re  # pylint: disable=import-outside-toplevel

        path = environ.get('PATH_INFO', '')
        method = environ.get('REQUEST_METHOD', 'GET')

        # 记录所有v1请求
        if '/v1/' in path:
            self.logger.info(f"收到v1请求: {method} {path}")

        # v1路径映射规则
        simple_mappings = {
            # 账户相关
            r'^/api/v1/accounts/list\.json$': '/api/accounts/',
            r'^/api/v1/accounts/add\.json$': '/api/accounts/',
            r'^/api/v1/accounts/get\.json$': '/api/accounts/get',
            r'^/api/v1/accounts/modify\.json$': '/api/accounts/modify',
            r'^/api/v1/accounts/hide\.json$': '/api/accounts/hide',
            r'^/api/v1/accounts/delete\.json$': '/api/accounts/delete',
            r'^/api/v1/accounts/move\.json$': '/api/accounts/move',

            # 交易相关（保持v1路径，不重写，让bills.py中的v1路由直接处理）
            # r'^/api/v1/transactions/list\.json$': '/api/bills/',  # 禁用重写，使用bills.py中的get_transactions_v1
            # r'^/api/v1/transactions/list/by_month\.json$': '/api/bills/by_month',  # 禁用重写，使用bills.py中的get_bills_by_month
            r'^/api/v1/transactions/get\.json$': '/api/bills/get',
            r'^/api/v1/transactions/add\.json$': '/api/bills/',
            r'^/api/v1/transactions/modify\.json$': '/api/bills/modify',
            r'^/api/v1/transactions/delete\.json$': '/api/bills/delete',
            r'^/api/v1/transactions/import\.json$': '/api/bills/batch',
            r'^/api/v1/transactions/parse_import\.json$': '/api/bills/parse_import',
            r'^/api/v1/transactions/reconciliation_statements\.json$': '/api/bills/reconciliation_statements',

            # 分类相关
            r'^/api/v1/transaction/categories/list\.json$': '/api/categories/',
            r'^/api/v1/transaction/categories/add\.json$': '/api/categories/',
            r'^/api/v1/transaction/categories/add_batch\.json$': '/api/categories/batch',

            # 模板相关
            r'^/api/v1/transaction/templates/list\.json$': '/api/templates/',
            r'^/api/v1/transaction/templates/add\.json$': '/api/templates/',

            # 统计相关 - 保留amounts和exchange_rates重写，其他让statistics.py的bp_v1处理
            # r'^/api/v1/transactions/statistics\.json$': 禁用，使用statistics.py的bp_v1
            # r'^/api/v1/transactions/statistics/trends\.json$': 禁用，使用statistics.py的bp_v1
            # r'^/api/v1/transactions/statistics/asset_trends\.json$': 禁用，使用statistics.py的bp_v1
            r'^/api/v1/transactions/amounts\.json$': '/api/statistics/amounts',
            r'^/api/v1/exchange_rates/latest\.json$': '/api/statistics/exchange-rates',

            # 用户相关
            r'^/api/v1/users/profile/get\.json$': '/api/v1/users/profile.json',
            r'^/api/v1/users/profile/update\.json$': '/api/v1/users/profile.json',
            r'^/api/v1/users/login\.json$': '/api/authorize.json',

            # 令牌相关
            r'^/api/v1/tokens/list\.json$': '/api/v1/tokens/list.json',

            # 2FA相关
            r'^/api/v1/users/2fa/status\.json$': '/api/v1/users/2fa/status.json',

            # 用户数据统计
            r'^/api/v1/data/statistics\.json$': '/api/v1/data/statistics.json',
        }

        # 尝试简单映射
        for pattern, replacement in simple_mappings.items():
            if re.match(pattern, path):
                self.logger.info(f"URL重写: {path} -> {replacement}")
                environ['PATH_INFO'] = replacement
                break

        # 记录未匹配的v1请求
        if '/v1/' in path and environ.get('PATH_INFO', '') == path:
            self.logger.warning(f"未匹配的v1路径: {path}")

        return self.app(environ, start_response)


def create_app():
    """创建Flask应用"""
    flask_app = Flask(__name__)

    # 【关键修复】禁用严格斜杠模式，避免308重定向丢失Authorization头
    # Flask默认会将 /api/accounts 重定向到 /api/accounts/
    # 浏览器在跟随308重定向时会丢弃Authorization头（安全机制）
    flask_app.url_map.strict_slashes = False

    # 配置
    flask_app.config['JSON_AS_ASCII'] = False
    flask_app.config['JSON_SORT_KEYS'] = False

    # 启用CORS - 修改为特定源，包含自定义请求头
    CORS(flask_app, resources={
        r"/api/*": {
            "origins": ["http://localhost:8081", "http://127.0.0.1:8081"],
            "methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
            "allow_headers": [
                "Content-Type", 
                "Authorization",      # 允许Authorization请求头（关键！）
                "X-Timezone-Offset",  # 前端时区偏移量（注意大小写）
                "X-Language",         # 前端语言设置
                "Accept",             # 允许Accept头
                "Accept-Language"     # 允许Accept-Language头
            ],
            "expose_headers": ["Content-Type", "Authorization"],  # 暴露响应头给前端
            "supports_credentials": True,
            "max_age": 3600,  # 预检请求缓存1小时
            "send_wildcard": False,  # 不使用通配符，明确指定源
            "always_send": True      # 总是发送CORS头，即使没有预检请求
        }
    })

    # 应用URL重写中间件（WSGI级别）
    flask_app.wsgi_app = URLRewriteMiddleware(flask_app.wsgi_app)

    # 调试日志 - 记录所有请求的关键信息
    @flask_app.before_request
    def log_request_details():
        """记录所有请求的详细信息，特别是OPTIONS预检和Authorization头"""
        from flask import request as flask_request  # pylint: disable=import-outside-toplevel

        # 记录OPTIONS预检请求
        if flask_request.method == 'OPTIONS':
            logger.info(f"[CORS Preflight] {flask_request.path}")
            logger.info(f"[CORS Preflight] Origin: {flask_request.headers.get('Origin', 'N/A')}")
            access_control_header = flask_request.headers.get(
                'Access-Control-Request-Headers', 'N/A'
            )
            logger.info(f"[CORS Preflight] Access-Control-Request-Headers: {access_control_header}")

        # 记录所有/api/accounts请求，检查Authorization头
        if '/api/accounts' in flask_request.path:
            auth_header = flask_request.headers.get('Authorization', None)
            logger.info(f"[Request Debug] {flask_request.method} {flask_request.path}")
            logger.info(f"[Request Debug] Has Authorization: {bool(auth_header)}")
            if auth_header:
                logger.info(f"[Request Debug] Authorization: {auth_header[:30]}...")
            else:
                logger.warning("[Request Debug] Missing Authorization header!")
                logger.info(f"[Request Debug] All headers: {dict(flask_request.headers)}")

        # 记录v1路径请求
        if '/v1/' in flask_request.path:
            logger.debug(f"v1请求: {flask_request.method} {flask_request.path}")

    # 注册认证蓝图（在/api路径下，以匹配前端axios的baseURL配置）
    flask_app.register_blueprint(auth.bp, url_prefix='/api')

    # 注册v1 API兼容蓝图（直接在/api下，不加前缀，匹配 /api/v1/... 路径）
    flask_app.register_blueprint(bills.bp_v1, url_prefix='/api')
    flask_app.register_blueprint(statistics.bp_v1, url_prefix='/api')  # 统计分析v1兼容API
    flask_app.register_blueprint(tags.bp_v1, url_prefix='/api')  # 标签v1兼容API
    flask_app.register_blueprint(budgets.bp_v1, url_prefix='/api')  # 预算v1兼容API

    # 注册其他业务蓝图
    flask_app.register_blueprint(bills.bp, url_prefix='/api/bills')
    flask_app.register_blueprint(categories.bp, url_prefix='/api/categories')
    flask_app.register_blueprint(statistics.bp, url_prefix='/api/statistics')
    flask_app.register_blueprint(accounts.bp, url_prefix='/api/accounts')
    flask_app.register_blueprint(tags.bp, url_prefix='/api/tags')
    flask_app.register_blueprint(templates.bp, url_prefix='/api/templates')
    flask_app.register_blueprint(budgets.bp, url_prefix='/api/budgets')
    flask_app.register_blueprint(backup.bp, url_prefix='/api/backup')

    # 健康检查端点
    @flask_app.route('/api/health', methods=['GET'])
    def health_check():
        """健康检查"""
        return {
            'success': True,
            'status': 'healthy',
            'version': '1.0.0'
        }

    # 错误处理
    @flask_app.errorhandler(404)
    def not_found(error):
        """404错误处理"""
        return {
            'success': False,
            'error': 'Not Found',
            'message': str(error)
        }, 404

    @flask_app.errorhandler(500)
    def internal_error(error):
        """500错误处理"""
        logger.error("Internal Server Error: %s", error)
        return {
            'success': False,
            'error': 'Internal Server Error',
            'message': str(error)
        }, 500

    return flask_app


async def initialize(db_path: str = None):
    """
    初始化应用服务

    Args:
        db_path: 数据库路径，如果为None则使用默认路径
    """
    global db, category_engine, bill_service

    try:
        logger.info("=" * 50)
        logger.info("初始化Web API服务器")

        # 初始化数据库
        db = Database(db_path=db_path)
        await db.init_db()
        # 将数据库实例存储到Flask app配置中
        app.config['DB_INSTANCE'] = db
        logger.info(f"[OK] 数据库初始化完成 (路径: {db.db_path})")

        # 创建默认管理员用户（如果不存在）
        await create_default_admin_user(db)
        logger.info("[OK] 默认用户检查完成")

        # 初始化分类引擎
        category_engine = CategoryEngine()
        await category_engine.load_rules_from_db(db)
        app.config['CATEGORY_ENGINE_INSTANCE'] = category_engine
        logger.info("[OK] 分类引擎初始化完成")

        # 初始化账单服务
        bill_service = BillService(db=db)
        await bill_service.initialize()
        app.config['BILL_SERVICE_INSTANCE'] = bill_service
        logger.info("[OK] 账单服务初始化完成")

        logger.info("=" * 50)
        logger.info("Web API服务器初始化完成")
    except Exception as e:
        logger.critical(f"服务器初始化失败: {e}", exc_info=True)
        sys.exit(1)


async def create_default_admin_user(database: Database):
    """创建默认管理员用户"""
    import json  # pylint: disable=import-outside-toplevel
    import bcrypt  # pylint: disable=import-outside-toplevel

    config_path = Path(__file__).parent.parent.parent / "config" / "server_config.json"

    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)

        default_user_config = config.get('default_user', {})
        username = default_user_config.get('username', 'admin')

        # 检查用户是否已存在
        existing_user = await database.get_user_by_username(username)

        if not existing_user:
            logger.info(f"创建默认管理员用户: {username}")

            password = default_user_config.get('password', 'admin123')
            password_hash = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt()).decode('utf-8')

            await database.create_user({
                'username': username,
                'email': default_user_config.get('email', 'admin@bill-analyser.local'),
                'password_hash': password_hash,
                'nickname': default_user_config.get('nickname', '管理员'),
                'language': default_user_config.get('language', 'zh_Hans'),
                'default_currency': default_user_config.get('default_currency', 'CNY'),
                'first_day_of_week': default_user_config.get('first_day_of_week', 1),
                'is_active': 1,
                'email_verified': 1
            })

            logger.info(f"[OK] 默认管理员用户创建成功: {username}")
            logger.warning(f"⚠️  默认密码: {password} - 请首次登录后立即修改!")
        else:
            logger.info(f"管理员用户已存在: {username}")

    except Exception as e:
        logger.warning(f"创建默认用户失败: {e}")


app = create_app()


if __name__ == '__main__':
    # 初始化服务
    asyncio.run(initialize())

    # 启动服务器
    logger.info("=" * 50)
    logger.info("启动Flask服务器")
    logger.info("监听地址: http://127.0.0.1:5000")
    logger.info("API文档: http://127.0.0.1:5000/api/")
    logger.info("=" * 50)

    app.run(
        host='127.0.0.1',
        port=5000,
        debug=False,
        threaded=True
    )
