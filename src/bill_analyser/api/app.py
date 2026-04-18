"""
Flask Web API 服务器 - 账单分析系统

提供 REST API 接口供 Vue 前端调用。
"""

import asyncio
import sys

from flask import Flask, abort, send_from_directory
from flask_cors import CORS

from bill_analyser import __version__

# pylint: disable=wrong-import-position
# 导入蓝图
from bill_analyser.api.routes import (
    accounts,
    auth,
    backup,
    bills,
    budgets,
    categories,
    learning,
    matching,
    recurring,
    statistics,
    tags,
    templates,
)
from bill_analyser.constants import PROJECT_ROOT, STATIC_DIR
from bill_analyser.core.bill_service import BillService
from bill_analyser.core.category_engine import CategoryEngine
from bill_analyser.core.db import Database
from bill_analyser.utils.config import ConfigValidationError, load_api_runtime_settings, load_default_user_settings
from bill_analyser.utils.logger import get_logger

logger = get_logger("WebAPI")

# 全局实例
DB_INSTANCE: Database | None = None
CATEGORY_ENGINE_INSTANCE: CategoryEngine | None = None
BILL_SERVICE_INSTANCE: BillService | None = None
db: Database | None = None  # pylint: disable=invalid-name
category_engine: CategoryEngine | None = None  # pylint: disable=invalid-name
bill_service: BillService | None = None  # pylint: disable=invalid-name


def _log_python_runtime_details():
    """记录解释器与虚拟环境信息，避免在模块导入时直接 print。"""
    logger.info("%s", "=" * 60)
    logger.info("Python解释器: %s", sys.executable)
    logger.info("Python版本: %s", sys.version)
    if ".venv" in sys.executable or "venv" in sys.executable:
        logger.info("[OK] 正在使用虚拟环境")
    else:
        logger.warning("[WARN] 未使用虚拟环境")
        logger.warning("当前路径: %s", sys.executable)
        logger.warning("建议使用: %s", PROJECT_ROOT / ".venv" / "Scripts" / "python.exe")
    logger.info("%s", "=" * 60)


def create_app():
    """创建Flask应用"""
    flask_app = Flask(__name__)
    runtime_config = load_api_runtime_settings()
    cors_config = runtime_config["cors"]

    # 【关键修复】禁用严格斜杠模式，避免308重定向丢失Authorization头
    # Flask默认会将 /api/accounts 重定向到 /api/accounts/
    # 浏览器在跟随308重定向时会丢弃Authorization头（安全机制）
    flask_app.url_map.strict_slashes = False

    # 配置
    flask_app.config["JSON_AS_ASCII"] = False
    flask_app.config["JSON_SORT_KEYS"] = False
    flask_app.json.sort_keys = False

    # 启用CORS - 修改为特定源，包含自定义请求头
    CORS(
        flask_app,
        resources={
            r"/api/*": {
                "origins": cors_config["origins"],
                "methods": cors_config["methods"],
                "allow_headers": cors_config["allow_headers"],
                "expose_headers": cors_config["expose_headers"],
                "supports_credentials": cors_config["supports_credentials"],
                "max_age": cors_config["max_age_seconds"],
                "send_wildcard": cors_config["send_wildcard"],
                "always_send": cors_config["always_send"],
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
            logger.debug("[CORS Preflight] %s", flask_request.path)
            logger.debug(
                "[CORS Preflight] Origin: %s",
                flask_request.headers.get("Origin", "N/A"),
            )
            access_control_header = flask_request.headers.get(
                "Access-Control-Request-Headers",
                "N/A",
            )
            logger.debug(
                "[CORS Preflight] Access-Control-Request-Headers: %s",
                access_control_header,
            )

        # v6.72: 账户请求调试日志改为DEBUG级别，仅在Authorization头缺失时用WARNING
        if "/api/accounts" in flask_request.path:
            auth_header = flask_request.headers.get("Authorization", None)
            logger.debug("[Request Debug] %s %s", flask_request.method, flask_request.path)
            logger.debug("[Request Debug] Has Authorization: %s", bool(auth_header))
            if auth_header:
                logger.debug("[Request Debug] Authorization: %s...", auth_header[:30])
            else:
                # 仅当缺少Authorization头时记录WARNING（实际问题）
                logger.warning("[Request Debug] Missing Authorization header!")
                logger.debug("[Request Debug] All headers: %s", dict(flask_request.headers))

        # v6.72: 记录v1路径请求改为DEBUG级别
        if "/v1/" in flask_request.path:
            logger.debug("v1请求: %s %s", flask_request.method, flask_request.path)

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
    flask_app.register_blueprint(matching.bp, url_prefix="/api/matching")
    flask_app.register_blueprint(learning.bp, url_prefix="/api/learning")
    flask_app.register_blueprint(recurring.bp, url_prefix="/api/recurring")

    # 健康检查端点
    @flask_app.route("/api/health", methods=["GET"])
    def health_check():
        """健康检查"""
        return {"success": True, "status": "healthy", "version": __version__}

    @flask_app.route("/api/ml/receipt-recognition", methods=["POST"])
    def receipt_recognition_disabled_safe():
        """AI 小票识图当前保持 disabled-safe 501 占位语义。"""
        return {
            "success": False,
            "error": "Not Implemented",
            "errorMessage": "Receipt recognition not implemented",
            "message": "Receipt recognition not implemented",
        }, 501

    @flask_app.route("/", defaults={"path": ""})
    @flask_app.route("/<path:path>")
    def serve_frontend(path):
        normalized_path = str(path or "").lstrip("/")
        if normalized_path == "api" or normalized_path.startswith("api/"):
            abort(404)

        if path and (STATIC_DIR / path).is_file():
            return send_from_directory(STATIC_DIR, path)
        return send_from_directory(STATIC_DIR, "index.html")

    # 错误处理
    @flask_app.errorhandler(404)
    def not_found(error):
        if error.description and "favicon" in str(error.description):
            return "", 204
        return {"success": False, "error": "Not Found", "message": str(error)}, 404

    @flask_app.errorhandler(405)
    def method_not_allowed(error):
        from flask import request as flask_request  # pylint: disable=import-outside-toplevel

        if flask_request.path.startswith("/api/v1/") or (
            flask_request.path.startswith("/api/") and flask_request.path.endswith(".json")
        ):
            return {"success": False, "error": "Not Found", "message": "Legacy endpoint not found"}, 404

        return {
            "success": False,
            "error": "Method Not Allowed",
            "message": str(error),
        }, 405

    @flask_app.errorhandler(500)
    def internal_error(error):
        logger.error("Internal Server Error: %s", error)
        return {"success": False, "error": "Internal Server Error", "message": str(error)}, 500

    return flask_app


async def initialize(db_path: str | None = None):
    """
    初始化应用服务

    Args:
        db_path: 数据库路径，如果为None则使用默认路径
    """
    global DB_INSTANCE, CATEGORY_ENGINE_INSTANCE, BILL_SERVICE_INSTANCE  # pylint: disable=global-statement
    global db, category_engine, bill_service  # pylint: disable=global-statement

    try:
        logger.info("=" * 50)
        logger.info("初始化Web API服务器")

        # 测试场景下 initialize() 可能被重复调用。
        # 先关闭旧的数据库连接，避免遗留 aiosqlite 线程导致进程无法退出。
        if DB_INSTANCE is not None:
            try:
                await DB_INSTANCE.close()
                logger.info("[OK] 已关闭旧数据库连接")
            except OSError as close_err:
                logger.warning("关闭旧数据库连接失败: %s", close_err)

        # 初始化数据库
        if db_path is None:
            DB_INSTANCE = Database()
        else:
            DB_INSTANCE = Database(db_path=db_path)
        db = DB_INSTANCE
        await DB_INSTANCE.init_db()
        # 将数据库实例存储到Flask app配置中
        app.config["DB_INSTANCE"] = DB_INSTANCE
        logger.info("[OK] 数据库初始化完成 (路径: %s)", DB_INSTANCE.db_path)

        # 创建默认管理员用户（如果不存在）
        await create_default_admin_user(DB_INSTANCE)
        logger.info("[OK] 默认用户检查完成")

        # 初始化分类引擎
        CATEGORY_ENGINE_INSTANCE = CategoryEngine()
        category_engine = CATEGORY_ENGINE_INSTANCE
        await CATEGORY_ENGINE_INSTANCE.load_rules_from_db(DB_INSTANCE)
        app.config["CATEGORY_ENGINE_INSTANCE"] = CATEGORY_ENGINE_INSTANCE
        logger.info("[OK] 分类引擎初始化完成")

        # 初始化账单服务
        BILL_SERVICE_INSTANCE = BillService(db=DB_INSTANCE)
        bill_service = BILL_SERVICE_INSTANCE
        await BILL_SERVICE_INSTANCE.initialize()
        app.config["BILL_SERVICE_INSTANCE"] = BILL_SERVICE_INSTANCE
        logger.info("[OK] 账单服务初始化完成")

        logger.info("=" * 50)
        logger.info("Web API服务器初始化完成")
    except (OSError, ValueError, RuntimeError) as exc:
        logger.critical("服务器初始化失败: %s", exc, exc_info=True)
        sys.exit(1)


async def create_default_admin_user(database: Database):
    """创建默认管理员用户"""
    import bcrypt  # pylint: disable=import-outside-toplevel

    try:
        default_user_config = load_default_user_settings()
        if not default_user_config:
            logger.info("未启用默认管理员自动创建，跳过初始化")
            return

        username = default_user_config["username"]

        # 检查用户是否已存在
        existing_user = await database.get_user_by_username(username)

        if not existing_user:
            logger.info("创建默认管理员用户: %s", username)

            password = default_user_config["password"]
            password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")

            await database.create_user(
                {
                    "username": username,
                    "email": default_user_config["email"],
                    "password_hash": password_hash,
                    "nickname": default_user_config["nickname"],
                    "language": default_user_config["language"],
                    "default_currency": default_user_config["default_currency"],
                    "first_day_of_week": default_user_config["first_day_of_week"],
                    "is_active": 1,
                    "email_verified": 1,
                }
            )

            logger.info("[OK] 默认管理员用户创建成功: %s", username)
            logger.warning("⚠️  默认密码: %s - 请首次登录后立即修改!", password)
        else:
            logger.info("管理员用户已存在: %s", username)

    except ConfigValidationError as exc:
        logger.error("默认管理员配置无效: %s", exc)
        raise
    except (OSError, ValueError) as exc:
        logger.warning("创建默认用户失败: %s", exc)


app = create_app()


def main():
    """初始化依赖并启动 Flask 开发服务器。"""
    runtime_config = load_api_runtime_settings()
    host = runtime_config["host"]
    port = runtime_config["port"]

    _log_python_runtime_details()

    # 初始化服务
    asyncio.run(initialize())

    # 启动服务器
    logger.info("=" * 50)
    logger.info("启动Flask服务器")
    logger.info("监听地址: http://%s:%s", host, port)
    logger.info("API文档: http://%s:%s/api/", host, port)
    logger.info("=" * 50)

    app.run(host=host, port=port, debug=runtime_config["debug"], threaded=runtime_config["threaded"])


if __name__ == "__main__":
    main()
