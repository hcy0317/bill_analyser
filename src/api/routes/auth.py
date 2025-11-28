"""
Authentication Routes - 认证相关API端点

提供用户登录、注册、登出等认证功能
"""

import asyncio
import hashlib
import json
import secrets
from datetime import datetime, timedelta
from pathlib import Path
from flask import Blueprint, request, jsonify
import bcrypt
import jwt

from src.utils.logger import get_logger, log_method
from src.api.middleware.auth import require_auth

logger = get_logger('AuthAPI')

bp = Blueprint('auth', __name__)


def load_auth_config():
    """加载认证配置"""
    config_path = Path(__file__).parent.parent.parent.parent / "config" / "server_config.json"

    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
            logger.info("认证配置加载成功")
            return config
    except Exception as e:
        logger.error(f"加载配置文件失败: {e}")
        # 返回默认配置
        return {
            'jwt_secret': 'default_secret_key_change_in_production',
            'jwt_algorithm': 'HS256',
            'jwt_expiration_days': 7,
            'refresh_token_expiration_days': 30,
            'password_min_length': 8,
            'enable_user_registration': True,
            'max_login_attempts': 5,
            'lockout_duration_minutes': 15
        }


def get_app_context():
    """获取应用上下文"""
    from flask import current_app
    db = current_app.config.get('DB_INSTANCE')
    if db is None:
        # 回退方案：尝试从模块导入
        import src.api.app as app_module
        db = app_module.db
    if db is None:
        raise RuntimeError("Database not initialized. Please ensure server is properly started.")
    return db


def get_client_ip():
    """获取客户端IP地址"""
    if request.headers.get('X-Forwarded-For'):
        return request.headers.get('X-Forwarded-For').split(',')[0].strip()
    if request.headers.get('X-Real-IP'):
        return request.headers.get('X-Real-IP')
    return request.remote_addr


def calculate_token_hash(token: str) -> str:
    """计算令牌哈希值"""
    return hashlib.sha256(token.encode('utf-8')).hexdigest()


def generate_jwt_token(user_id: int, username: str, config: dict) -> dict:
    """
    生成JWT令牌和刷新令牌

    Returns:
        dict: {
            'access_token': str,
            'refresh_token': str,
            'expires_at': str,
            'refresh_expires_at': str
        }
    """
    jwt_secret = config.get('jwt_secret')
    jwt_algorithm = config.get('jwt_algorithm', 'HS256')
    access_exp_days = config.get('jwt_expiration_days', 7)
    refresh_exp_days = config.get('refresh_token_expiration_days', 30)

    now = datetime.now()
    access_expires_at = now + timedelta(days=access_exp_days)
    refresh_expires_at = now + timedelta(days=refresh_exp_days)

    # 生成访问令牌（添加nonce确保唯一性）
    access_payload = {
        'user_id': user_id,
        'username': username,
        'type': 'access',
        'iat': int(now.timestamp()),
        'exp': int(access_expires_at.timestamp()),
        'nonce': secrets.token_hex(16)  # 添加32字符随机值
    }
    access_token = jwt.encode(access_payload, jwt_secret, algorithm=jwt_algorithm)

    # 生成刷新令牌（添加nonce确保唯一性）
    refresh_payload = {
        'user_id': user_id,
        'username': username,
        'type': 'refresh',
        'iat': int(now.timestamp()),
        'exp': int(refresh_expires_at.timestamp()),
        'nonce': secrets.token_hex(16)  # 添加32字符随机值
    }
    refresh_token_str = jwt.encode(refresh_payload, jwt_secret, algorithm=jwt_algorithm)

    return {
        'access_token': access_token,
        'refresh_token': refresh_token_str,
        'expires_at': access_expires_at.isoformat(),
        'refresh_expires_at': refresh_expires_at.isoformat()
    }


def validate_password(password: str, config: dict) -> tuple:
    """
    验证密码强度

    Returns:
        tuple: (is_valid: bool, error_message: str)
    """
    min_length = config.get('password_min_length', 8)

    if len(password) < min_length:
        return False, f'Password must be at least {min_length} characters long'

    if config.get('password_require_uppercase', False):
        if not any(c.isupper() for c in password):
            return False, 'Password must contain at least one uppercase letter'

    if config.get('password_require_lowercase', False):
        if not any(c.islower() for c in password):
            return False, 'Password must contain at least one lowercase letter'

    if config.get('password_require_digit', False):
        if not any(c.isdigit() for c in password):
            return False, 'Password must contain at least one digit'

    if config.get('password_require_special', False):
        special_chars = '!@#$%^&*()_+-=[]{}|;:,.<>?'
        if not any(c in special_chars for c in password):
            return False, 'Password must contain at least one special character'

    return True, ''


@bp.route('/authorize.json', methods=['POST'])
@log_method
def login():
    """
    用户登录端点

    Request Body:
        {
            "loginName": "username or email",
            "password": "password"
        }

    Response:
        {
            "success": true,
            "result": {
                "token": "jwt_access_token",
                "user": {
                    "username": "...",
                    "email": "...",
                    ...
                }
            }
        }
    """
    try:
        data = request.json
        login_name = data.get('loginName', '').strip()
        password = data.get('password', '')

        if not login_name or not password:
            logger.warning("登录失败: 缺少用户名或密码")
            return jsonify({
                'success': False,
                'error': 'Invalid request',
                'message': 'Username and password are required'
            }), 400

        # 获取配置和数据库
        config = load_auth_config()
        db = get_app_context()

        # 获取客户端信息
        ip_address = get_client_ip()
        user_agent = request.headers.get('User-Agent', '')

        # 查找用户（支持用户名或邮箱登录）
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        logger.info(f"查找用户: login_name={login_name}")
        user = loop.run_until_complete(db.get_user_by_username(login_name))
        if not user:
            user = loop.run_until_complete(db.get_user_by_email(login_name))

        if not user:
            logger.warning(f"登录失败: 用户不存在 - {login_name}")

            # 记录失败日志
            loop.run_until_complete(db.create_auth_log({
                'username': login_name,
                'event_type': 'login_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'User not found'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Invalid credentials',
                'message': 'Invalid username or password'
            }), 401

        # 检查账户是否被锁定
        logger.info(f"检查账户锁定状态: user_id={user['id']}")
        is_locked = loop.run_until_complete(db.is_user_locked(user['id']))
        if is_locked:
            logger.warning(f"登录失败: 账户被锁定 - user_id={user['id']}")

            loop.run_until_complete(db.create_auth_log({
                'user_id': user['id'],
                'username': user['username'],
                'event_type': 'login_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'Account locked'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Account locked',
                'message': 'Account is temporarily locked due to multiple failed login attempts'
            }), 403

        # 验证密码
        logger.info(f"验证密码: user_id={user['id']}")
        password_hash = user['password_hash']
        if not bcrypt.checkpw(password.encode('utf-8'), password_hash.encode('utf-8')):
            logger.warning(f"登录失败: 密码错误 - user_id={user['id']}")

            # 增加失败次数
            lockout_minutes = config.get('lockout_duration_minutes', 15)
            loop.run_until_complete(db.increment_failed_login(user['id'], lockout_minutes))

            loop.run_until_complete(db.create_auth_log({
                'user_id': user['id'],
                'username': user['username'],
                'event_type': 'login_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'Invalid password'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Invalid credentials',
                'message': 'Invalid username or password'
            }), 401

        # 检查账户是否激活
        if not user.get('is_active'):
            logger.warning(f"登录失败: 账户未激活 - user_id={user['id']}")

            loop.run_until_complete(db.create_auth_log({
                'user_id': user['id'],
                'username': user['username'],
                'event_type': 'login_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'Account not active'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Account not active',
                'message': 'Your account has been deactivated'
            }), 403

        # 生成JWT令牌
        logger.info(f"生成JWT令牌: user_id={user['id']}")
        tokens = generate_jwt_token(user['id'], user['username'], config)

        # 计算令牌哈希
        token_hash = calculate_token_hash(tokens['access_token'])
        refresh_token_hash = calculate_token_hash(tokens['refresh_token'])

        # 创建会话
        logger.info(f"创建会话: user_id={user['id']}")
        session_id = loop.run_until_complete(db.create_session({
            'user_id': user['id'],
            'token_hash': token_hash,
            'refresh_token_hash': refresh_token_hash,
            'expires_at': tokens['expires_at'],
            'refresh_expires_at': tokens['refresh_expires_at'],
            'user_agent': user_agent,
            'ip_address': ip_address
        }))

        # 更新最后登录时间
        loop.run_until_complete(db.update_user_last_login(user['id'], ip_address))

        # 记录成功日志
        loop.run_until_complete(db.create_auth_log({
            'user_id': user['id'],
            'username': user['username'],
            'event_type': 'login_success',
            'ip_address': ip_address,
            'user_agent': user_agent,
            'success': True,
            'metadata': json.dumps({'session_id': session_id})
        }))

        loop.close()

        # 构建用户信息响应
        logger.info(
            f"[login] 数据库用户记录: id={user['id']}, username={user['username']}, "
            f"fiscal_year_start={user.get('fiscal_year_start', 'NOT_SET')}"
        )

        user_info = {
            'id': user['id'],  # 🆕 添加用户ID字段（前端UserBasicInfo需要）
            'username': user['username'],
            'email': user['email'],
            'nickname': user.get('nickname', user['username']),
            'avatar': user.get('avatar', ''),
            'defaultAccountId': user.get('default_account_id', ''),
            'transactionEditScope': user.get('transaction_edit_scope', 0),
            'language': user.get('language', 'zh_Hans'),
            'defaultCurrency': user.get('default_currency', 'CNY'),
            'firstDayOfWeek': user.get('first_day_of_week', 1),
            'fiscalYearStart': user.get('fiscal_year_start', 1),
            'calendarDisplayType': user.get('calendar_display_type', 0),
            'dateDisplayType': user.get('date_display_type', 0),
            'longDateFormat': user.get('long_date_format', 0),
            'shortDateFormat': user.get('short_date_format', 0),
            'longTimeFormat': user.get('long_time_format', 0),
            'shortTimeFormat': user.get('short_time_format', 0),
            'fiscalYearFormat': user.get('fiscal_year_format', 0),
            'currencyDisplayType': user.get('currency_display_type', 0),
            'numeralSystem': user.get('numeral_system', 0),
            'decimalSeparator': user.get('decimal_separator', 0),
            'digitGroupingSymbol': user.get('digit_grouping_symbol', 0),
            'digitGrouping': user.get('digit_grouping', 0),
            'coordinateDisplayType': user.get('coordinate_display_type', 0),
            'expenseAmountColor': user.get('expense_amount_color', 0),
            'incomeAmountColor': user.get('income_amount_color', 0),
            'emailVerified': user.get('email_verified', False)
        }

        logger.info(f"登录成功: user_id={user['id']}, username={user['username']}, ip={ip_address}")
        logger.info(
            f"[login] 返回给前端的user_info: id={user_info['id']}, "
            f"username={user_info['username']}, fiscalYearStart={user_info['fiscalYearStart']} "
            f"(0x{user_info['fiscalYearStart']:x})"
        )

        return jsonify({
            'success': True,
            'result': {
                'token': tokens['access_token'],
                'refreshToken': tokens['refresh_token'],
                'user': user_info
            }
        })

    except Exception as e:
        logger.error(f"登录过程发生错误: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/register.json', methods=['POST'])
@log_method
def register():
    """
    用户注册端点

    Request Body:
        {
            "username": "username",
            "email": "email@example.com",
            "password": "password",
            "nickname": "nickname",
            "language": "zh_Hans",
            "defaultCurrency": "CNY",
            "firstDayOfWeek": 1
        }
    """
    try:
        data = request.json
        username = data.get('username', '').strip()
        email = data.get('email', '').strip()
        password = data.get('password', '')
        nickname = data.get('nickname', '').strip() or username

        # 验证必填字段
        if not username or not email or not password:
            logger.warning("注册失败: 缺少必填字段")
            return jsonify({
                'success': False,
                'error': 'Invalid request',
                'message': 'Username, email and password are required'
            }), 400

        # 加载配置
        config = load_auth_config()

        # 检查是否允许注册
        if not config.get('enable_user_registration', True):
            logger.warning("注册失败: 注册功能已禁用")
            return jsonify({
                'success': False,
                'error': 'Registration disabled',
                'message': 'User registration is currently disabled'
            }), 403

        # 验证密码强度
        logger.info(f"验证密码强度: username={username}")
        is_valid, error_msg = validate_password(password, config)
        if not is_valid:
            logger.warning(f"注册失败: 密码不符合要求 - {error_msg}")
            return jsonify({
                'success': False,
                'error': 'Invalid password',
                'message': error_msg
            }), 400

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get('User-Agent', '')

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 检查用户名是否已存在
        logger.info(f"检查用户名: username={username}")
        existing_user = loop.run_until_complete(db.get_user_by_username(username))
        if existing_user:
            logger.warning(f"注册失败: 用户名已存在 - {username}")

            loop.run_until_complete(db.create_auth_log({
                'username': username,
                'event_type': 'register_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'Username already exists'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Username exists',
                'message': 'Username already exists'
            }), 409

        # 检查邮箱是否已存在
        logger.info(f"检查邮箱: email={email}")
        existing_email = loop.run_until_complete(db.get_user_by_email(email))
        if existing_email:
            logger.warning(f"注册失败: 邮箱已存在 - {email}")

            loop.run_until_complete(db.create_auth_log({
                'username': username,
                'event_type': 'register_failed',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': False,
                'error_message': 'Email already exists'
            }))
            loop.close()

            return jsonify({
                'success': False,
                'error': 'Email exists',
                'message': 'Email already exists'
            }), 409

        # 哈希密码
        logger.info("正在哈希密码...")
        password_hash = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt()).decode('utf-8')

        # 创建用户
        logger.info(f"创建用户: username={username}, email={email}")
        user_id = loop.run_until_complete(db.create_user({
            'username': username,
            'email': email,
            'password_hash': password_hash,
            'nickname': nickname,
            'language': data.get('language', 'zh_Hans'),
            'default_currency': data.get('defaultCurrency', 'CNY'),
            'first_day_of_week': data.get('firstDayOfWeek', 1),
            'is_active': 1,
            'email_verified': 0 if config.get('require_email_verification') else 1
        }))

        # 记录成功日志
        loop.run_until_complete(db.create_auth_log({
            'user_id': user_id,
            'username': username,
            'event_type': 'register_success',
            'ip_address': ip_address,
            'user_agent': user_agent,
            'success': True
        }))

        loop.close()

        logger.info(f"注册成功: user_id={user_id}, username={username}, email={email}")

        return jsonify({
            'success': True,
            'result': {
                'user_id': user_id,
                'username': username,
                'email': email,
                'message': 'Registration successful'
            }
        })

    except Exception as e:
        logger.error(f"注册过程发生错误: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/logout.json', methods=['GET', 'POST'])
@log_method
def logout():
    """
    用户登出端点

    需要Authorization头
    """
    try:
        auth_header = request.headers.get('Authorization', '')

        if not auth_header:
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Missing authorization header'
            }), 401

        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != 'bearer':
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Invalid authorization header'
            }), 401

        token = parts[1]
        token_hash = calculate_token_hash(token)

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get('User-Agent', '')

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取会话信息
        session = loop.run_until_complete(db.get_session_by_token_hash(token_hash))

        if session:
            # 使会话失效
            logger.info(f"使会话失效: session_id={session['id']}")
            loop.run_until_complete(db.invalidate_session(token_hash))

            # 记录登出日志
            loop.run_until_complete(db.create_auth_log({
                'user_id': session['user_id'],
                'username': session['username'],
                'event_type': 'logout',
                'ip_address': ip_address,
                'user_agent': user_agent,
                'success': True
            }))

            logger.info(f"登出成功: user_id={session['id']}, username={session['username']}")
        else:
            logger.warning(f"登出时未找到会话: token_hash={token_hash[:16]}...")

        loop.close()

        # 🆕 返回result字段以匹配前端期望 (stores/index.ts Line 428)
        return jsonify({
            'success': True,
            'result': True,  # ← 前端检查这个字段！
            'message': 'Logged out successfully'
        })

    except Exception as e:
        logger.error(f"登出过程发生错误: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/v1/tokens/refresh.json', methods=['POST'])
@log_method
def refresh_token():
    """
    刷新令牌端点

    Request Body:
        {
            "refreshToken": "refresh_token_string"
        }
    """
    try:
        data = request.json
        refresh_token_str = data.get('refreshToken', '')

        if not refresh_token_str:
            return jsonify({
                'success': False,
                'error': 'Invalid request',
                'message': 'Refresh token is required'
            }), 400

        config = load_auth_config()
        jwt_secret = config.get('jwt_secret')
        jwt_algorithm = config.get('jwt_algorithm', 'HS256')

        # 验证刷新令牌
        try:
            payload = jwt.decode(refresh_token_str, jwt_secret, algorithms=[jwt_algorithm])

            if payload.get('type') != 'refresh':
                return jsonify({
                    'success': False,
                    'error': 'Invalid token',
                    'message': 'Not a refresh token'
                }), 400

            user_id = payload.get('user_id')
            username = payload.get('username')

        except jwt.ExpiredSignatureError:
            return jsonify({
                'success': False,
                'error': 'Token expired',
                'message': 'Refresh token has expired'
            }), 401
        except jwt.InvalidTokenError:
            return jsonify({
                'success': False,
                'error': 'Invalid token',
                'message': 'Invalid refresh token'
            }), 401

        # 生成新的访问令牌
        logger.info(f"生成新令牌: user_id={user_id}")
        tokens = generate_jwt_token(user_id, username, config)

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 查找旧会话并更新
        # old_refresh_hash = calculate_token_hash(refresh_token_str)  # 保留以供将来使用
        new_token_hash = calculate_token_hash(tokens['access_token'])
        new_refresh_hash = calculate_token_hash(tokens['refresh_token'])

        # 这里简化处理：创建新会话
        ip_address = get_client_ip()
        user_agent = request.headers.get('User-Agent', '')

        loop.run_until_complete(db.create_session({
            'user_id': user_id,
            'token_hash': new_token_hash,
            'refresh_token_hash': new_refresh_hash,
            'expires_at': tokens['expires_at'],
            'refresh_expires_at': tokens['refresh_expires_at'],
            'user_agent': user_agent,
            'ip_address': ip_address
        }))

        loop.close()

        logger.info(f"令牌刷新成功: user_id={user_id}, username={username}")

        return jsonify({
            'success': True,
            'result': {
                'token': tokens['access_token'],
                'refreshToken': tokens['refresh_token'],
                'newToken': tokens['access_token']  # 兼容前端期望的字段名
            }
        })

    except Exception as e:
        logger.error(f"刷新令牌过程发生错误: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/v1/users/profile.json', methods=['GET', 'POST'])
@log_method
@require_auth
def profile():
    """
    获取或更新用户资料端点（需要认证）

    GET: 获取用户资料
    POST: 更新用户资料
    """
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        if request.method == 'GET':
            # 获取用户资料
            user = loop.run_until_complete(db.get_user_by_id(request.user_id))
            loop.close()

            if not user:
                return jsonify({
                    'success': False,
                    'error': 'User not found'
                }), 404

            # 确保关键字段不为None
            username = user.get('username', '')
            email = user.get('email', '')
            nickname = user.get('nickname') or username  # 如果nickname为空则使用username

            user_info = {
                'username': username,
                'email': email,
                'nickname': nickname,
                'avatar': user.get('avatar') or '',
                'defaultAccountId': user.get('default_account_id') or '',
                'transactionEditScope': user.get('transaction_edit_scope', 0),
                'language': user.get('language') or 'zh_Hans',
                'defaultCurrency': user.get('default_currency') or 'CNY',
                'firstDayOfWeek': user.get('first_day_of_week', 1),
                'fiscalYearStart': user.get('fiscal_year_start', 1),
                'calendarDisplayType': user.get('calendar_display_type', 0),
                'dateDisplayType': user.get('date_display_type', 0),
                'longDateFormat': user.get('long_date_format', 0),
                'shortDateFormat': user.get('short_date_format', 0),
                'longTimeFormat': user.get('long_time_format', 0),
                'shortTimeFormat': user.get('short_time_format', 0),
                'fiscalYearFormat': user.get('fiscal_year_format', 0),
                'currencyDisplayType': user.get('currency_display_type', 0),
                'numeralSystem': user.get('numeral_system', 0),
                'decimalSeparator': user.get('decimal_separator', 0),
                'digitGroupingSymbol': user.get('digit_grouping_symbol', 0),
                'digitGrouping': user.get('digit_grouping', 0),
                'coordinateDisplayType': user.get('coordinate_display_type', 0),
                'expenseAmountColor': user.get('expense_amount_color', 0),
                'incomeAmountColor': user.get('income_amount_color', 0),
                'emailVerified': bool(user.get('email_verified', False))
            }

            logger.info(f"返回用户资料: user_id={request.user_id}, username={username}, "
                       f"calendarDisplayType={user_info['calendarDisplayType']}, "
                       f"fiscalYearStart={user_info['fiscalYearStart']}")
            logger.debug(f"完整用户资料数据: {user_info}")


            return jsonify({
                'success': True,
                'result': user_info
            })

        # POST - 更新用户资料
        data = request.json
        if not data:
            return jsonify({
                'success': False,
                'error': 'Bad Request',
                'message': 'Request body is required'
            }), 400

        update_data = {}

        # 允许更新的字段（基本信息）
        if 'nickname' in data:
            update_data['nickname'] = data['nickname']
        if 'email' in data:
            update_data['email'] = data['email']
        if 'avatar' in data:
            update_data['avatar'] = data['avatar']
        if 'language' in data:
            update_data['language'] = data['language']
        if 'defaultCurrency' in data:
            update_data['default_currency'] = data['defaultCurrency']
        if 'firstDayOfWeek' in data:
            update_data['first_day_of_week'] = data['firstDayOfWeek']
        if 'defaultAccountId' in data:
            update_data['default_account_id'] = data['defaultAccountId']
        if 'transactionEditScope' in data:
            update_data['transaction_edit_scope'] = data['transactionEditScope']

        # 允许更新的字段（显示配置）
        if 'fiscalYearStart' in data:
            update_data['fiscal_year_start'] = data['fiscalYearStart']
        if 'calendarDisplayType' in data:
            update_data['calendar_display_type'] = data['calendarDisplayType']
        if 'dateDisplayType' in data:
            update_data['date_display_type'] = data['dateDisplayType']
        if 'longDateFormat' in data:
            update_data['long_date_format'] = data['longDateFormat']
        if 'shortDateFormat' in data:
            update_data['short_date_format'] = data['shortDateFormat']
        if 'longTimeFormat' in data:
            update_data['long_time_format'] = data['longTimeFormat']
        if 'shortTimeFormat' in data:
            update_data['short_time_format'] = data['shortTimeFormat']
        if 'fiscalYearFormat' in data:
            update_data['fiscal_year_format'] = data['fiscalYearFormat']
        if 'currencyDisplayType' in data:
            update_data['currency_display_type'] = data['currencyDisplayType']
        if 'numeralSystem' in data:
            update_data['numeral_system'] = data['numeralSystem']
        if 'decimalSeparator' in data:
            update_data['decimal_separator'] = data['decimalSeparator']
        if 'digitGroupingSymbol' in data:
            update_data['digit_grouping_symbol'] = data['digitGroupingSymbol']
        if 'digitGrouping' in data:
            update_data['digit_grouping'] = data['digitGrouping']
        if 'coordinateDisplayType' in data:
            update_data['coordinate_display_type'] = data['coordinateDisplayType']
        if 'expenseAmountColor' in data:
            update_data['expense_amount_color'] = data['expenseAmountColor']
        if 'incomeAmountColor' in data:
            update_data['income_amount_color'] = data['incomeAmountColor']

        logger.info(f"将更新用户资料: user_id={request.user_id}, fields={list(update_data.keys())}")
        logger.debug(f"更新数据详情: {update_data}")

        if update_data:
            success = loop.run_until_complete(db.update_user(request.user_id, update_data))
            if not success:
                loop.close()
                return jsonify({
                    'success': False,
                    'error': 'Update failed',
                    'message': 'Failed to update user profile'
                }), 500
            logger.info(f"用户资料更新成功: user_id={request.user_id}")

        # 返回更新后的用户信息
        user = loop.run_until_complete(db.get_user_by_id(request.user_id))
        loop.close()

        if not user:
            return jsonify({
                'success': False,
                'error': 'User not found after update'
            }), 404

        username = user.get('username', '')
        email = user.get('email', '')
        nickname = user.get('nickname') or username

        user_info = {
            'username': username,
            'email': email,
            'nickname': nickname,
            'avatar': user.get('avatar') or '',
            'defaultAccountId': user.get('default_account_id') or '',
            'transactionEditScope': user.get('transaction_edit_scope', 0),
            'language': user.get('language') or 'zh_Hans',
            'defaultCurrency': user.get('default_currency') or 'CNY',
            'firstDayOfWeek': user.get('first_day_of_week', 1),
            'fiscalYearStart': user.get('fiscal_year_start', 1),
            'calendarDisplayType': user.get('calendar_display_type', 0),
            'dateDisplayType': user.get('date_display_type', 0),
            'longDateFormat': user.get('long_date_format', 0),
            'shortDateFormat': user.get('short_date_format', 0),
            'longTimeFormat': user.get('long_time_format', 0),
            'shortTimeFormat': user.get('short_time_format', 0),
            'fiscalYearFormat': user.get('fiscal_year_format', 0),
            'currencyDisplayType': user.get('currency_display_type', 0),
            'numeralSystem': user.get('numeral_system', 0),
            'decimalSeparator': user.get('decimal_separator', 0),
            'digitGroupingSymbol': user.get('digit_grouping_symbol', 0),
            'digitGrouping': user.get('digit_grouping', 0),
            'coordinateDisplayType': user.get('coordinate_display_type', 0),
            'expenseAmountColor': user.get('expense_amount_color', 0),
            'incomeAmountColor': user.get('income_amount_color', 0),
            'emailVerified': bool(user.get('email_verified', False))
        }

        logger.info(f"用户资料更新并返回: user_id={request.user_id}, updated_fields={len(update_data)}")
        logger.debug(f"返回的用户资料: {user_info}")

        return jsonify({
            'success': True,
            'result': user_info
        })

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error(f"处理用户资料失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/v1/tokens/list.json', methods=['GET'])
@log_method
@require_auth
def list_tokens():
    """获取用户所有令牌（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 先清理过期会话
        loop.run_until_complete(db.cleanup_expired_sessions())

        # 获取所有活跃会话
        sessions = loop.run_until_complete(db.get_user_sessions(request.user_id))
        loop.close()

        # 转换为前端期望的格式
        tokens = []
        # seen_devices = set()  # Unused

        for session in sessions:
            user_agent = session.get('user_agent', '')
            ip_address = session.get('ip_address', '')

            # 简单的设备标识: IP + UA的前50个字符
            # device_key = f"{ip_address}_{user_agent[:50]}" # Unused

            # 解析User-Agent提取设备信息
            device_name = parse_user_agent(user_agent)

            tokens.append({
                'tokenId': str(session['id']),
                'userAgent': user_agent,
                'deviceName': device_name,
                'ipAddress': ip_address,
                'createdAt': session.get('created_at', ''),
                'expiresAt': session.get('expires_at', ''),
                'lastActivityAt': session.get('last_activity_at', ''),
                'isCurrentToken': session['id'] == request.session_id
            })

        logger.info(f"返回会话列表: user_id={request.user_id}, count={len(tokens)}")

        return jsonify({
            'success': True,
            'result': tokens
        })

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error(f"获取令牌列表失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


def parse_user_agent(user_agent: str) -> str:
    """解析User-Agent字符串,返回友好的设备名称"""
    if not user_agent:
        return '未知设备'

    ua_lower = user_agent.lower()

    # 检测操作系统
    if 'windows' in ua_lower:
        os_name = 'Windows'
        if 'windows nt 10' in ua_lower:
            os_name = 'Windows 10'
        elif 'windows nt 11' in ua_lower:
            os_name = 'Windows 11'
    elif 'mac os' in ua_lower or 'macos' in ua_lower:
        os_name = 'macOS'
    elif 'linux' in ua_lower:
        os_name = 'Linux'
    elif 'android' in ua_lower:
        os_name = 'Android'
    elif 'iphone' in ua_lower or 'ipad' in ua_lower:
        os_name = 'iOS'
    else:
        os_name = '其他系统'

    # 检测浏览器
    if 'edg/' in ua_lower or 'edge/' in ua_lower:
        browser = 'Edge'
    elif 'chrome/' in ua_lower and 'edg/' not in ua_lower:
        browser = 'Chrome'
    elif 'firefox/' in ua_lower:
        browser = 'Firefox'
    elif 'safari/' in ua_lower and 'chrome' not in ua_lower:
        browser = 'Safari'
    else:
        browser = '其他浏览器'

    return f"{os_name} ({browser})"


@bp.route('/v1/users/2fa/status.json', methods=['GET'])
@log_method
@require_auth
def get_2fa_status():
    """获取用户2FA状态（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_id(request.user_id))
        loop.close()

        if not user:
            return jsonify({
                'success': False,
                'error': 'User not found'
            }), 404

        # 返回2FA状态
        is_enabled = user.get('two_factor_enabled', False)

        return jsonify({
            'success': True,
            'result': {
                'enable': bool(is_enabled),
                'isEnabled': bool(is_enabled)
            }
        })

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error(f"获取2FA状态失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500


@bp.route('/v1/data/statistics.json', methods=['GET'])
@log_method
@require_auth
def get_user_data_statistics():
    """获取用户数据统计（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取用户的账单数量（注意：get_bills可能需要user_id参数）
        bills = loop.run_until_complete(db.get_bills())

        # 获取所有账户
        accounts = loop.run_until_complete(db.get_all_accounts())

        # 获取所有分类
        categories = loop.run_until_complete(db.get_all_categories())

        loop.close()

        # 返回统计数据 - 确保所有数值都是有效整数
        bill_count = len(bills) if bills and isinstance(bills, list) else 0
        account_count = len(accounts) if accounts and isinstance(accounts, list) else 0
        category_count = len(categories) if categories and isinstance(categories, list) else 0

        statistics = {
            'billCount': int(bill_count),
            'accountCount': int(account_count),
            'categoryCount': int(category_count),
            'tagCount': 0,
            'templateCount': 0
        }

        logger.info(f"返回用户数据统计: user_id={request.user_id}, bills={bill_count}, accounts={account_count}, categories={category_count}")

        return jsonify({
            'success': True,
            'result': statistics
        })

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error(f"获取用户数据统计失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': 'Internal Server Error',
            'message': str(e)
        }), 500