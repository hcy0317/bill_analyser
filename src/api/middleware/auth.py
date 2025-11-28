"""
Authentication Middleware - 认证中间件

提供JWT令牌验证装饰器，用于保护需要认证的API端点
"""

import hashlib
import json
from datetime import datetime
from functools import wraps
from flask import request, jsonify
import jwt

from src.utils.logger import get_logger, log_method

logger = get_logger('AuthMiddleware')


def get_auth_config():
    """获取认证配置"""
    from pathlib import Path
    config_path = Path(__file__).parent.parent.parent.parent / "config" / "server_config.json"

    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
            return config
    except Exception as e:
        logger.error(f"加载配置文件失败: {e}")
        # 返回默认配置
        return {
            'jwt_secret': 'default_secret_key_change_in_production',
            'jwt_algorithm': 'HS256'
        }


def get_db():
    """获取数据库实例"""
    from flask import current_app
    return current_app.config.get('DB_INSTANCE')


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


def require_auth(f):
    """
    认证装饰器 - 验证JWT令牌并注入用户信息

    使用方法:
        @bp.route('/protected')
        @require_auth
        def protected_endpoint():
            user_id = request.user_id
            username = request.username
            return {'message': f'Hello, {username}'}
    """
    @wraps(f)
    @log_method
    def decorated(*args, **kwargs):
        logger.info(f"认证检查: {request.method} {request.path}")

        # 获取Authorization头
        auth_header = request.headers.get('Authorization', '')

        if not auth_header:
            logger.warning("缺少Authorization头")
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Missing authorization header'
            }), 401

        # 解析Bearer令牌
        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != 'bearer':
            logger.warning(f"无效的Authorization格式: {auth_header}")
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Invalid authorization header format'
            }), 401

        token = parts[1]

        try:
            # 获取配置
            config = get_auth_config()
            jwt_secret = config.get('jwt_secret')
            jwt_algorithm = config.get('jwt_algorithm', 'HS256')

            # 验证JWT令牌(检查签名和过期时间)
            jwt.decode(token, jwt_secret, algorithms=[jwt_algorithm])

            # 计算令牌哈希
            token_hash = calculate_token_hash(token)

            # 从数据库验证会话
            import asyncio
            db_instance = get_db()
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

            session = loop.run_until_complete(
                db_instance.get_session_by_token_hash(token_hash)
            )

            if not session:
                logger.warning(f"会话不存在或已失效: token_hash={token_hash[:16]}...")
                loop.close()
                return jsonify({
                    'success': False,
                    'error': 'Unauthorized',
                    'message': 'Invalid or expired session'
                }), 401

            # 检查会话是否过期
            expires_at = datetime.fromisoformat(session['expires_at'])
            if datetime.now() > expires_at:
                logger.warning(f"会话已过期: session_id={session['id']}")
                loop.close()
                return jsonify({
                    'success': False,
                    'error': 'Unauthorized',
                    'message': 'Session expired'
                }), 401

            # 检查用户是否激活
            if not session.get('user_is_active'):
                logger.warning(f"用户账户未激活: user_id={session['user_id']}")
                loop.close()
                return jsonify({
                    'success': False,
                    'error': 'Unauthorized',
                    'message': 'User account is not active'
                }), 401

            # 更新会话活动时间
            loop.run_until_complete(
                db_instance.update_session_activity(session['id'])
            )

            loop.close()

            # 将用户信息注入到request对象
            request.user_id = session['user_id']
            request.username = session['username']
            request.user_email = session['email']
            request.session_id = session['id']

            logger.info(f"认证成功: user_id={request.user_id}, username={request.username}")

            # 调用原始函数
            return f(*args, **kwargs)

        except jwt.ExpiredSignatureError:
            logger.warning("JWT令牌已过期")
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Token expired'
            }), 401

        except jwt.InvalidTokenError as e:
            logger.warning(f"无效的JWT令牌: {e}")
            return jsonify({
                'success': False,
                'error': 'Unauthorized',
                'message': 'Invalid token'
            }), 401

        except Exception as e:
            logger.error(f"认证过程发生错误: {e}", exc_info=True)
            return jsonify({
                'success': False,
                'error': 'Internal Server Error',
                'message': 'Authentication failed'
            }), 500

    return decorated


def optional_auth(f):
    """
    可选认证装饰器 - 尝试验证令牌，但不强制要求

    如果提供了有效令牌，则注入用户信息；否则继续执行，不会返回401错误
    """
    @wraps(f)
    @log_method
    def decorated(*args, **kwargs):
        auth_header = request.headers.get('Authorization', '')

        if auth_header:
            parts = auth_header.split()
            if len(parts) == 2 and parts[0].lower() == 'bearer':
                token = parts[1]

                try:
                    config = get_auth_config()
                    jwt_secret = config.get('jwt_secret')
                    jwt_algorithm = config.get('jwt_algorithm', 'HS256')

                    # 验证JWT令牌(检查签名和过期时间)
                    jwt.decode(token, jwt_secret, algorithms=[jwt_algorithm])
                    token_hash = calculate_token_hash(token)

                    import asyncio
                    db_instance = get_db()
                    loop = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop)

                    session = loop.run_until_complete(
                        db_instance.get_session_by_token_hash(token_hash)
                    )
                    loop.close()

                    if session and session.get('user_is_active'):
                        expires_at = datetime.fromisoformat(session['expires_at'])
                        if datetime.now() <= expires_at:
                            request.user_id = session['user_id']
                            request.username = session['username']
                            request.user_email = session['email']
                            request.session_id = session['id']
                            logger.info(f"可选认证成功: username={request.username}")

                except Exception as e:
                    logger.debug(f"可选认证失败: {e}")
                    # 忽略错误，继续执行

        # 如果没有认证信息，设置默认值
        if not hasattr(request, 'user_id'):
            request.user_id = None
            request.username = None
            request.user_email = None
            request.session_id = None

        return f(*args, **kwargs)

    return decorated
