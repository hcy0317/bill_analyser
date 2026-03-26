"""
Flask Web API Server - 账单分析系统Web API服务器

提供RESTful API接口供Vue前端调用
"""

import asyncio
import sys

from flask import Flask
from flask_cors import CORS

from bill_analyser.constants import PROJECT_ROOT

# 验证Python解释器路径
print(f"\n{'=' * 60}")
print(f"Python解释器: {sys.executable}")
print(f"Python版本: {sys.version}")
if ".venv" in sys.executable or "venv" in sys.executable:
    print("[OK] 正在使用虚拟环境")
else:
    print("[WARN] 未使用虚拟环境!")
    print(f"当前路径: {sys.executable}")
    print(f"应该使用: {PROJECT_ROOT / '.venv' / 'Scripts' / 'python.exe'}")
print(f"{'=' * 60}\n")


# pylint: disable=wrong-import-position,import-error
# 导入蓝图
from bill_analyser.api.routes import accounts, auth, backup, bills, budgets, categories, statistics, tags, templates
from bill_analyser.core.bill_service import BillService
from bill_analyser.core.category_engine import CategoryEngine
from bill_analyser.core.db import Database
from bill_analyser.utils.logger import get_logger

try:
    from bill_analyser.api.routes import ml
except ImportError:  # pylint: disable=import-error
    ml = None

logger = get_logger("WebAPI")

# 全局实例
db: Database = None
category_engine: CategoryEngine = None
bill_service: BillService = None


def create_app():
    """创建Flask应用"""
    flask_app = Flask(__name__)

    # 【关键修复】禁用严格斜杠模式，避免308重定向丢失Authorization头
    # Flask默认会将 /api/accounts 重定向到 /api/accounts/
    # 浏览器在跟随308重定向时会丢弃Authorization头（安全机制）
    flask_app.url_map.strict_slashes = False

    # 配置
    flask_app.config["JSON_AS_ASCII"] = False
    flask_app.config["JSON_SORT_KEYS"] = False

    # 启用CORS - 修改为特定源，包含自定义请求头
    CORS(
        flask_app,
        resources={
            r"/api/*": {
                "origins": ["http://localhost:8081", "http://127.0.0.1:8081"],
                "methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
                "allow_headers": [
                    "Content-Type",
                    "Authorization",  # 允许Authorization请求头（关键！）
                    "X-Timezone-Offset",  # 前端时区偏移量（注意大小写）
                    "X-Language",  # 前端语言设置
                    "Accept",  # 允许Accept头
                    "Accept-Language",  # 允许Accept-Language头
                ],
                "expose_headers": ["Content-Type", "Authorization"],  # 暴露响应头给前端
                "supports_credentials": True,
                "max_age": 3600,  # 预检请求缓存1小时
                "send_wildcard": False,  # 不使用通配符，明确指定源
                "always_send": True,  # 总是发送CORS头，即使没有预检请求
            }
        },
    )

    # 调试日志 - 记录所有请求的关键信息
    @flask_app.before_request
    def log_request_details():
        """记录所有请求的详细信息，特别是OPTIONS预检和Authorization头"""
        from flask import request as flask_request  # pylint: disable=import-outside-toplevel

        # v6.72: OPTIONS预检请求改为DEBUG级别，减少日志输出
        if flask_request.method == "OPTIONS":
            logger.debug(f"[CORS Preflight] {flask_request.path}")
            logger.debug(f"[CORS Preflight] Origin: {flask_request.headers.get('Origin', 'N/A')}")
            access_control_header = flask_request.headers.get("Access-Control-Request-Headers", "N/A")
            logger.debug(f"[CORS Preflight] Access-Control-Request-Headers: {access_control_header}")

        # v6.72: 账户请求调试日志改为DEBUG级别，仅在Authorization头缺失时用WARNING
        if "/api/accounts" in flask_request.path:
            auth_header = flask_request.headers.get("Authorization", None)
            logger.debug(f"[Request Debug] {flask_request.method} {flask_request.path}")
            logger.debug(f"[Request Debug] Has Authorization: {bool(auth_header)}")
            if auth_header:
                logger.debug(f"[Request Debug] Authorization: {auth_header[:30]}...")
            else:
                # 仅当缺少Authorization头时记录WARNING（实际问题）
                logger.warning("[Request Debug] Missing Authorization header!")
                logger.debug(f"[Request Debug] All headers: {dict(flask_request.headers)}")

        # v6.72: 记录v1路径请求改为DEBUG级别
        if "/v1/" in flask_request.path:
            logger.debug(f"v1请求: {flask_request.method} {flask_request.path}")

    # 注册认证蓝图（在/api路径下，以匹配前端axios的baseURL配置）
    flask_app.register_blueprint(auth.bp, url_prefix="/api")

    # 注册其他业务蓝图
    flask_app.register_blueprint(bills.bp, url_prefix="/api/bills")
    flask_app.register_blueprint(categories.bp, url_prefix="/api/categories")
    flask_app.register_blueprint(statistics.bp, url_prefix="/api/statistics")
    flask_app.register_blueprint(accounts.bp, url_prefix="/api/accounts")
    flask_app.register_blueprint(tags.bp, url_prefix="/api/tags")
    flask_app.register_blueprint(templates.bp, url_prefix="/api/templates")
    flask_app.register_blueprint(budgets.bp, url_prefix="/api/budgets")
    flask_app.register_blueprint(backup.bp, url_prefix="/api/backup")
    if ml is not None:
        flask_app.register_blueprint(ml.bp, url_prefix="/api/ml")  # v6.88: ML分类器

    # 健康检查端点
    @flask_app.route("/api/health", methods=["GET"])
    def health_check():
        """健康检查"""
        return {"success": True, "status": "healthy", "version": "1.0.0"}

    # 错误处理
    @flask_app.errorhandler(404)
    def not_found(error):
        """404错误处理"""
        return {"success": False, "error": "Not Found", "message": str(error)}, 404

    @flask_app.errorhandler(500)
    def internal_error(error):
        """500错误处理"""
        logger.error("Internal Server Error: %s", error)
        return {"success": False, "error": "Internal Server Error", "message": str(error)}, 500

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

        # 测试场景下 initialize() 可能被重复调用。
        # 先关闭旧的数据库连接，避免遗留 aiosqlite 线程导致进程无法退出。
        if db is not None:
            try:
                await db.close()
                logger.info("[OK] 已关闭旧数据库连接")
            except Exception as close_err:  # pylint: disable=broad-except
                logger.warning(f"关闭旧数据库连接失败: {close_err}")

        # 初始化数据库
        db = Database(db_path=db_path)
        await db.init_db()
        # 将数据库实例存储到Flask app配置中
        app.config["DB_INSTANCE"] = db
        logger.info(f"[OK] 数据库初始化完成 (路径: {db.db_path})")

        # 创建默认管理员用户（如果不存在）
        await create_default_admin_user(db)
        logger.info("[OK] 默认用户检查完成")

        # 初始化分类引擎
        category_engine = CategoryEngine()
        await category_engine.load_rules_from_db(db)
        app.config["CATEGORY_ENGINE_INSTANCE"] = category_engine
        logger.info("[OK] 分类引擎初始化完成")

        # 初始化账单服务
        bill_service = BillService(db=db)
        await bill_service.initialize()
        app.config["BILL_SERVICE_INSTANCE"] = bill_service
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

    config_path = PROJECT_ROOT / "config" / "server_config.json"

    try:
        with open(config_path, encoding="utf-8") as f:
            config = json.load(f)

        default_user_config = config.get("default_user", {})
        username = default_user_config.get("username", "admin")

        # 检查用户是否已存在
        existing_user = await database.get_user_by_username(username)

        if not existing_user:
            logger.info(f"创建默认管理员用户: {username}")

            password = default_user_config.get("password", "admin123")
            password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")

            await database.create_user(
                {
                    "username": username,
                    "email": default_user_config.get("email", "admin@bill-analyser.local"),
                    "password_hash": password_hash,
                    "nickname": default_user_config.get("nickname", "管理员"),
                    "language": default_user_config.get("language", "zh_Hans"),
                    "default_currency": default_user_config.get("default_currency", "CNY"),
                    "first_day_of_week": default_user_config.get("first_day_of_week", 1),
                    "is_active": 1,
                    "email_verified": 1,
                }
            )

            logger.info(f"[OK] 默认管理员用户创建成功: {username}")
            logger.warning(f"⚠️  默认密码: {password} - 请首次登录后立即修改!")
        else:
            logger.info(f"管理员用户已存在: {username}")

    except Exception as e:
        logger.warning(f"创建默认用户失败: {e}")


app = create_app()


def main():
    # 初始化服务
    asyncio.run(initialize())

    # 启动服务器
    logger.info("=" * 50)
    logger.info("启动Flask服务器")
    logger.info("监听地址: http://127.0.0.1:5000")
    logger.info("API文档: http://127.0.0.1:5000/api/")
    logger.info("=" * 50)

    app.run(host="127.0.0.1", port=5000, debug=False, threaded=True)
