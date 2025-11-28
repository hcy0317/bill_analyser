"""
Accounts API Routes - 账户相关API端点

重构后使用V1AccountAdapter统一处理账户数据格式转换。
"""

import asyncio
from flask import Blueprint, request, jsonify

from src.utils.logger import get_logger, log_method
from src.api.adapters.v1_account_adapter import V1AccountAdapter
from src.api.middleware.auth import require_auth

logger = get_logger('AccountsAPI')

bp = Blueprint('accounts', __name__)
account_adapter = V1AccountAdapter()


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app  # pylint: disable=import-outside-toplevel
    return current_app.config.get('DB_INSTANCE')




@bp.route('/', methods=['GET'])
@log_method
@require_auth
def get_accounts():
    """获取账户列表(需要认证) - 返回层级结构"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))
        loop.close()

        # 确保返回的是列表
        if accounts is None:
            accounts = []

        logger.info(f"查询到 {len(accounts)} 个账户")
        
        # 使用adapter构建层级并格式化
        response = account_adapter.format_list_response(accounts, build_hierarchy_flag=True)

        logger.info(
            "返回账户列表: user_id=%s, 总账户=%s",
            request.user_id, len(accounts)
        )

        return jsonify(response)

    except Exception as e:
        logger.error("获取账户列表失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:account_id>', methods=['GET'])
@log_method
@require_auth
def get_account(account_id: int):
    """获取账户详情"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))

        if account:
            sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
            if sub_accounts:
                account['subAccounts'] = sub_accounts

        loop.close()

        if not account:
            return jsonify({
                'success': False,
                'error': 'Account not found'
            }), 404

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        return jsonify({
            'success': True,
            'result': formatted_account
        })

    except Exception as e:
        logger.error("获取账户详情失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/', methods=['POST'])
@log_method
@require_auth
def create_account():
    """创建账户(需要认证)"""
    logger.info("="*50)
    logger.info("创建账户请求开始")
    logger.info(f"user_id={request.user_id}")

    try:
        data = request.get_json()
        if not data:
            logger.error("未提供数据")
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.debug(f"请求数据: {data}")
        
        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account_id = loop.run_until_complete(db.create_account(data, user_id=request.user_id))
        loop.close()

        logger.info(f"账户创建成功: account_id={account_id}")

        # 获取完整账户信息
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))

        # 获取子账户并附加
        sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
        if sub_accounts:
            account['subAccounts'] = sub_accounts

        loop.close()

        # 使用adapter格式化账户响应数据
        formatted_account = account_adapter.backend_to_frontend(account)

        logger.info("="*50)

        return jsonify({
            'success': True,
            'result': formatted_account
        }), 201

    except Exception as e:
        logger.error(f"创建账户失败: {e}", exc_info=True)
        logger.info("="*50)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:account_id>', methods=['PUT'])
@log_method
@require_auth
def update_account(account_id: int):
    """更新账户 - RESTful API（支持父账户和子账户更新）"""
    try:
        logger.info(f"[账户更新] 开始: account_id={account_id}, user_id={getattr(request, 'user_id', 'unknown')}")

        data = request.get_json()
        if not data:
            logger.warning(f"[账户更新] 未提供数据: account_id={account_id}")
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.debug(f"[账户更新] 请求数据字段: {list(data.keys())}")
        
        # 使用adapter转换前端数据格式
        data = account_adapter.frontend_to_backend(data)

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 处理子账户更新
        sub_accounts_data = data.pop('subAccounts', None)
        if sub_accounts_data and isinstance(sub_accounts_data, list):
            logger.info(f"[账户更新] 处理 {len(sub_accounts_data)} 个子账户")

            # 获取现有子账户
            existing_subs = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
            existing_sub_ids = {sub['id'] for sub in existing_subs}
            updated_sub_ids = set()

            # 更新或创建子账户
            for sub_data in sub_accounts_data:
                sub_id = sub_data.get('id')
                
                # 使用adapter转换子账户数据格式
                sub_data = account_adapter.frontend_to_backend(sub_data)

                if sub_id and sub_id in existing_sub_ids:
                    # 更新现有子账户
                    logger.info(f"[账户更新] 更新子账户: sub_id={sub_id}, name={sub_data.get('name')}")
                    loop.run_until_complete(db.update_account(sub_id, sub_data, user_id=request.user_id))
                    updated_sub_ids.add(sub_id)
                else:
                    # 创建新子账户
                    sub_data['parent_id'] = account_id
                    logger.info(f"[账户更新] 创建新子账户: name={sub_data.get('name')}")
                    new_sub_id = loop.run_until_complete(db.create_account(sub_data, user_id=request.user_id))
                    logger.info(f"[账户更新] 新子账户已创建: sub_id={new_sub_id}")

            # 删除不再存在的子账户
            deleted_sub_ids = existing_sub_ids - updated_sub_ids
            for old_sub_id in deleted_sub_ids:
                logger.info(f"[账户更新] 删除旧子账户: sub_id={old_sub_id}")
                loop.run_until_complete(db.delete_account(old_sub_id, user_id=request.user_id))

        # 更新父账户
        result = loop.run_until_complete(db.update_account(account_id, data, user_id=request.user_id))
        loop.close()

        if result:
            logger.info(f"[账户更新] 成功: account_id={account_id}")
            # 获取更新后的账户详情
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            updated_account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))
            if updated_account:
                sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
                if sub_accounts:
                    updated_account['subAccounts'] = sub_accounts
                formatted_account = account_adapter.backend_to_frontend(updated_account)
            loop.close()

            return jsonify({
                'success': True,
                'result': formatted_account if updated_account else {}
            })

        logger.warning(f"[账户更新] 账户不存在: account_id={account_id}")
        return jsonify({
            'success': False,
            'error': 'Account not found'
        }), 404

    except Exception as e:
        logger.error(f"[账户更新] 失败: account_id={account_id}, error={e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:account_id>', methods=['DELETE'])
@log_method
@require_auth
def delete_account(account_id: int):
    """删除账户 - RESTful API（支持级联删除子账户）"""
    try:
        logger.info(f"[账户删除] 开始: account_id={account_id}, user_id={getattr(request, 'user_id', 'unknown')}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 检查是否有子账户
        sub_accounts = loop.run_until_complete(db.get_sub_accounts(account_id, user_id=request.user_id))
        if sub_accounts and len(sub_accounts) > 0:
            logger.info(
                "[账户删除] 账户包含 %s 个子账户，将进行级联删除: account_id=%s",
                len(sub_accounts), account_id
            )

            # 先删除所有子账户
            for sub_account in sub_accounts:
                sub_id = sub_account['id']
                logger.info(f"[账户删除] 删除子账户: sub_account_id={sub_id}, name={sub_account.get('name')}")
                loop.run_until_complete(db.delete_account(sub_id, user_id=request.user_id))

            logger.info(f"[账户删除] 已删除 {len(sub_accounts)} 个子账户")

        # 删除父账户
        result = loop.run_until_complete(db.delete_account(account_id, user_id=request.user_id))
        loop.close()

        if result:
            logger.info(f"[账户删除] 成功: account_id={account_id}")
            return jsonify({
                'success': True,
                'result': True
            })

        logger.warning(f"[账户删除] 账户不存在: account_id={account_id}")
        return jsonify({
            'success': False,
            'error': 'Account not found'
        }), 404

    except Exception as e:
        logger.error(f"[账户删除] 失败: account_id={account_id}, error={e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


# ==========================================
# V1 API 兼容接口 (通过URLRewriteMiddleware映射)
# ==========================================

@bp.route('/get', methods=['GET'])
@log_method
@require_auth
def get_account_v1():
    """获取账户详情 (V1兼容)"""
    try:
        account_id = request.args.get('id')
        if not account_id:
            return jsonify({
                'success': False,
                'error': 'Missing id parameter'
            }), 400

        return get_account(int(account_id))
    except ValueError:
        return jsonify({
            'success': False,
            'error': 'Invalid id parameter'
        }), 400
    except Exception as e:
        logger.error(f"V1获取账户失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/modify', methods=['POST'])
@log_method
@require_auth
def modify_account_v1():
    """修改账户 (V1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing id in request body'
            }), 400

        account_id = int(data['id'])
        # 移除id字段，剩下的就是更新数据
        update_data = {k: v for k, v in data.items() if k != 'id'}

        # 调用现有的更新逻辑
        # 注意：update_account 是路由函数，我们需要直接调用逻辑或重构
        # 这里为了简单，直接复制逻辑，或者提取公共逻辑
        # 更好的方式是提取 service 层，但现在直接操作 DB

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 特殊处理：V1 API可能会发送 subAccounts，目前后端可能不支持直接更新子账户结构
        # 这里暂时忽略 subAccounts，只更新当前账户属性
        if 'subAccounts' in update_data:
            del update_data['subAccounts']

        result = loop.run_until_complete(db.update_account(account_id, update_data, user_id=request.user_id))

        # 如果成功，返回更新后的账户信息
        if result:
            account = loop.run_until_complete(db.get_account_by_id(account_id, user_id=request.user_id))
            loop.close()

            formatted_account = account_adapter.backend_to_frontend(account)
            return jsonify({
                'success': True,
                'result': formatted_account
            })

        loop.close()
        return jsonify({
            'success': False,
            'error': 'Account not found'
        }), 404

    except Exception as e:
        logger.error(f"V1修改账户失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/hide', methods=['POST'])
@log_method
@require_auth
def hide_account_v1():
    """隐藏/显示账户 (V1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data or 'hidden' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing id or hidden parameter'
            }), 400

        account_id = int(data['id'])
        hidden = data['hidden']

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(db.update_account(account_id, {'hidden': 1 if hidden else 0}, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'result': True
            })
        return jsonify({
            'success': False,
            'error': 'Account not found'
        }), 404

    except Exception as e:
        logger.error(f"V1隐藏账户失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/delete', methods=['POST'])
@log_method
@require_auth
def delete_account_v1():
    """删除账户 (V1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing id parameter'
            }), 400

        account_id = int(data['id'])

        # 复用 delete_account 逻辑
        # 注意：delete_account 是路由函数，返回的是 Response 对象
        # 这里我们直接调用 DB

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.delete_account(account_id, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'result': True
            })
        return jsonify({
            'success': False,
            'error': 'Account not found'
        }), 404

    except Exception as e:
        logger.error(f"V1删除账户失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/move', methods=['POST'])
@log_method
@require_auth
def move_account_v1():
    """移动账户/排序 (V1兼容)"""
    try:
        data = request.get_json()
        if not data or 'newDisplayOrders' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing newDisplayOrders parameter'
            }), 400

        new_orders = data['newDisplayOrders']
        if not isinstance(new_orders, list):
            return jsonify({
                'success': False,
                'error': 'newDisplayOrders must be a list'
            }), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        success_count = 0
        for item in new_orders:
            if 'id' in item and 'displayOrder' in item:
                acc_id = int(item['id'])
                order = int(item['displayOrder'])
                if loop.run_until_complete(db.update_account(acc_id, {'display_order': order}, user_id=request.user_id)):
                    success_count += 1

        loop.close()

        return jsonify({
            'success': True,
            'result': True
        })

    except Exception as e:
        logger.error(f"V1移动账户失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500
