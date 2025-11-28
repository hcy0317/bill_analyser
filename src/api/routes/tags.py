"""
Tags API Routes - 标签相关API端点
"""

import asyncio
from flask import Blueprint, request, jsonify

from src.utils.logger import get_logger, log_method
from src.api.middleware.auth import require_auth

logger = get_logger('TagsAPI')

bp = Blueprint('tags', __name__)
bp_v1 = Blueprint('tags_v1', __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app
    return current_app.config.get('DB_INSTANCE')


@bp.route('/', methods=['GET'])
@log_method
@require_auth
def get_tags():
    """获取标签列表"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tags = loop.run_until_complete(db.get_all_tags(user_id=request.user_id))
        loop.close()

        return jsonify({
            'success': True,
            'result': tags
        })

    except Exception as e:
        logger.error("获取标签列表失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:tag_id>', methods=['GET'])
@log_method
@require_auth
def get_tag(tag_id: int):
    """获取标签详情"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tag = loop.run_until_complete(db.get_tag_by_id(tag_id, user_id=request.user_id))
        loop.close()

        if not tag:
            return jsonify({
                'success': False,
                'error': 'Tag not found'
            }), 404

        return jsonify({
            'success': True,
            'result': tag
        })

    except Exception as e:
        logger.error("获取标签详情失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/', methods=['POST'])
@log_method
@require_auth
def create_tag():
    """创建标签"""
    try:
        data = request.get_json()
        if not data or 'name' not in data:
            return jsonify({
                'success': False,
                'error': 'name is required'
            }), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        tag_id = loop.run_until_complete(db.create_tag(data, user_id=request.user_id))
        loop.close()

        return jsonify({
            'success': True,
            'result': {'id': tag_id}
        }), 201

    except Exception as e:
        logger.error("创建标签失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:tag_id>', methods=['PUT'])
@log_method
@require_auth
def update_tag(tag_id: int):
    """更新标签"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.update_tag(tag_id, data, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'message': 'Tag updated successfully'
            })
        return jsonify({
            'success': False,
            'error': 'Tag not found'
        }), 404

    except Exception as e:
        logger.error("更新标签失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:tag_id>', methods=['DELETE'])
@log_method
@require_auth
def delete_tag(tag_id: int):
    """删除标签"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.delete_tag(tag_id, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'message': 'Tag deleted successfully'
            })
        return jsonify({
            'success': False,
            'error': 'Tag not found'
        }), 404

    except Exception as e:
        logger.error(f"删除标签失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/transaction/tags/list.json', methods=['GET'])
@log_method
@require_auth
def get_tags_v1():
    """获取标签列表 (v1兼容)"""
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            tags = loop.run_until_complete(db.get_all_tags(user_id=request.user_id))
            # 转换ID为字符串，符合v1规范
            for tag in tags:
                tag['id'] = str(tag['id'])
        finally:
            loop.close()

        return jsonify({
            'success': True,
            'result': tags
        })
    except Exception as e:
        logger.error("获取标签列表失败: %s", e)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/add.json', methods=['POST'])
@log_method
@require_auth
def add_tag_v1():
    """创建标签 (v1兼容)"""
    try:
        data = request.get_json()
        logger.info(f"[创建标签] 收到请求数据: {data}")

        if not data or 'name' not in data:
            logger.error(f"[创建标签] 缺少name字段: {data}")
            return jsonify({'success': False, 'error': 'name is required'}), 400

        logger.info(f"[创建标签] 标签名称: {data['name']}")

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            tag_id = loop.run_until_complete(db.create_tag(data, user_id=request.user_id))
            logger.info(f"[创建标签] 成功创建标签，ID: {tag_id}")
        finally:
            loop.close()

        result = {
            'id': str(tag_id),
            'name': data['name'],
            'color': data.get('color', '#000000'),
            'icon': data.get('icon', ''),
            'hidden': False
        }
        logger.info(f"[创建标签] 返回结果: {result}")

        return jsonify({
            'success': True,
            'result': result
        })
    except Exception as e:
        logger.error("创建标签失败: %s", e, exc_info=True)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/modify.json', methods=['POST'])
@log_method
@require_auth
def modify_tag_v1():
    """更新标签 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({'success': False, 'error': 'id is required'}), 400

        tag_id = int(data.pop('id'))

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_tag(tag_id, data, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            return jsonify({'success': True, 'result': {}})
        return jsonify({'success': False, 'error': 'Tag not found'}), 404
    except Exception as e:
        logger.error("更新标签失败: %s", e)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/hide.json', methods=['POST'])
@log_method
@require_auth
def hide_tag_v1():
    """隐藏标签 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({'success': False, 'error': 'id is required'}), 400

        tag_id = int(data['id'])

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_tag(tag_id, {'hidden': True}, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            return jsonify({'success': True, 'result': {}})
        return jsonify({'success': False, 'error': 'Tag not found'}), 404
    except Exception as e:
        logger.error("隐藏标签失败: %s", e)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/show.json', methods=['POST'])
@log_method
@require_auth
def show_tag_v1():
    """显示标签 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({'success': False, 'error': 'id is required'}), 400

        tag_id = int(data['id'])

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_tag(tag_id, {'hidden': False}, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            return jsonify({'success': True, 'result': {}})
        return jsonify({'success': False, 'error': 'Tag not found'}), 404
    except Exception as e:
        logger.error("显示标签失败: %s", e)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/delete.json', methods=['POST'])
@log_method
@require_auth
def delete_tag_v1():
    """删除标签 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({'success': False, 'error': 'id is required'}), 400

        tag_id = int(data['id'])

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.delete_tag(tag_id, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            return jsonify({'success': True, 'result': {}})
        return jsonify({'success': False, 'error': 'Tag not found'}), 404
    except Exception as e:
        logger.error("删除标签失败: %s", e)
        return jsonify({'success': False, 'error': str(e)}), 500


@bp_v1.route('/v1/transaction/tags/move.json', methods=['POST'])
@log_method
@require_auth
def move_tags_v1():
    """批量更新标签显示顺序 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'newDisplayOrders' not in data:
            return jsonify({'success': False, 'error': 'newDisplayOrders is required'}), 400

        new_orders = data['newDisplayOrders']
        if not isinstance(new_orders, list):
            return jsonify({'success': False, 'error': 'newDisplayOrders must be an array'}), 400

        # 转换为数据库格式: [(tag_id, display_order), ...]
        orders = []
        for item in new_orders:
            if not isinstance(item, dict) or 'id' not in item or 'displayOrder' not in item:
                return jsonify({
                    'success': False,
                    'error': 'Each item must have id and displayOrder'
                }), 400

            try:
                tag_id = int(item['id'])
                display_order = int(item['displayOrder'])
                orders.append((tag_id, display_order))
            except (ValueError, TypeError) as e:
                return jsonify({
                    'success': False,
                    'error': f'Invalid id or displayOrder: {e}'
                }), 400

        logger.info(f"[move_tags_v1] 更新{len(orders)}个标签的显示顺序")

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_tag_display_orders(orders, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            return jsonify({'success': True, 'result': True})
        return jsonify({'success': False, 'error': 'Failed to update display orders'}), 500
    except Exception as e:
        logger.error("更新标签显示顺序失败: %s", e, exc_info=True)
        return jsonify({'success': False, 'error': str(e)}), 500
