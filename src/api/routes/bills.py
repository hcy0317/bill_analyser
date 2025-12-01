"""
Bills API Routes - 账单相关API端点

重构后使用统一的V1 Adapter进行数据格式转换，
消除冗余代码，提升性能和可维护性。
"""

import asyncio
import os
import uuid
from datetime import datetime
from pathlib import Path
from werkzeug.utils import secure_filename
from flask import Blueprint, request, jsonify

from src.utils.logger import get_logger, log_method
from src.utils.currency import yuan_to_cents  # 金额单位转换工具
from src.api.adapters.v1_adapter import V1TransactionAdapter
from src.utils.constants import (
    BACKEND_TO_FRONTEND_TYPE
)
from src.api.middleware.auth import require_auth

logger = get_logger('BillsAPI')

# 主蓝图 - 现代RESTful API
bp = Blueprint('bills', __name__)

# v1蓝图 - 兼容ezBookkeeping前端（无url_prefix，直接匹配/api/v1/...路径）
bp_v1 = Blueprint('bills_v1', __name__)

# 上传文件配置 - 指向项目根目录的 uploads 文件夹
UPLOAD_FOLDER = Path(__file__).parent.parent.parent.parent / "uploads"
ALLOWED_EXTENSIONS = {'csv', 'xlsx', 'xls', 'txt'}
MAX_FILE_SIZE = 10 * 1024 * 1024  # 10MB

# 确保上传目录存在
UPLOAD_FOLDER.mkdir(parents=True, exist_ok=True)


def allowed_file(filename):
    """检查文件扩展名是否允许"""
    return '.' in filename and \
           filename.rsplit('.', 1)[1].lower() in ALLOWED_EXTENSIONS


def get_app_context(user_id: int = None):
    """获取应用上下文中的服务实例
    
    Args:
        user_id: 用户ID (如果为None，自动从request获取)
    """
    from flask import current_app, request as flask_request
    db = current_app.config.get('DB_INSTANCE')
    bill_service = current_app.config.get('BILL_SERVICE_INSTANCE')
    category_engine = current_app.config.get('CATEGORY_ENGINE_INSTANCE')

    # 自动获取user_id
    if user_id is None:
        user_id = getattr(flask_request, 'user_id', 1)

    # 创建adapter实例(带数据库引用和用户ID)
    adapter = V1TransactionAdapter(db=db, user_id=user_id)

    return db, bill_service, category_engine, adapter


async def sync_balances_for_bill(db, bill_data):
    """同步账单相关账户的余额

    Args:
        db: 数据库实例
        bill_data: 账单数据字典 (包含 source_account_id, destination_account_id)
    """
    try:
        # 同步源账户
        source_id = bill_data.get('source_account_id')
        if source_id:
            await db.sync_account_balance(int(source_id))
            logger.info(f"已同步源账户余额: {source_id}")

        # 同步目标账户 (转账/投资)
        dest_id = bill_data.get('destination_account_id')
        if dest_id:
            await db.sync_account_balance(int(dest_id))
            logger.info(f"已同步目标账户余额: {dest_id}")

    except Exception as e:
        logger.error(f"同步账户余额失败: {e}", exc_info=True)


def get_category_id_from_names(main_category: str, sub_category: str, db, user_id: int = 1) -> str:
    """
    根据主分类和子分类名称查询分类ID

    Args:
        main_category: 主分类名称
        sub_category: 子分类名称
        db: 数据库实例
        user_id: 用户ID

    Returns:
        分类ID字符串，找不到返回'0'
    """
    if not main_category:
        return '0'

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
    loop.close()

    for cat in categories:
        if cat.get('main_category') == main_category and cat.get('sub_category', '') == sub_category:
            return str(cat['id'])

    return '0'


def parse_bill_date(date_str):
    """
    解析账单日期字段,支持多种格式

    Args:
        date_str: 日期字符串,可能是 '%Y-%m-%d' 或 '%Y-%m-%d %H:%M:%S'

    Returns:
        datetime对象
    """
    # 先尝试完整格式(包含时分秒)
    try:
        return datetime.strptime(date_str, '%Y-%m-%d %H:%M:%S')
    except ValueError:
        pass

    # 回退到只有日期的格式
    try:
        return datetime.strptime(date_str, '%Y-%m-%d')
    except ValueError as e:
        logger.error(f"无法解析日期字符串: {date_str}, 错误: {e}")
        return datetime.now()  # 返回当前时间作为默认值


@bp.route('/', methods=['GET'])
@log_method
@require_auth
def get_bills():
    """
    获取账单列表

    Query Parameters:
        - page: 页码（默认1）
        - page_size: 每页数量（默认20）
        - type: 类型过滤（数字或中文: 0=全部, 2=收入, 3=支出, 4=转账, 5=投资）
        - main_category: 主分类过滤
        - sub_category: 子分类过滤
        - start_date: 开始日期
        - end_date: 结束日期
        - keyword: 关键词搜索
    """
    try:
        # 获取查询参数
        page = int(request.args.get('page', 1))
        page_size = int(request.args.get('page_size', 20))

        # 构建过滤条件
        filters = {}
        # 交易类型映射：前端v1格式(数字) -> 后端格式(中文)
        # 0=全部(不过滤), 2=收入, 3=支出, 4=转账, 5=投资
        if request.args.get('type'):
            type_param = request.args.get('type')
            try:
                type_int = int(type_param)
                type_mapping = {
                    2: '收入',
                    3: '支出',
                    4: '转账',
                    5: '投资'
                }
                # type=0表示全部类型，不设置过滤条件
                if type_int in type_mapping:
                    filters['type'] = type_mapping[type_int]
                elif type_int != 0:
                    logger.warning(f"未知的type参数: {type_int}, 已忽略")
            except ValueError:
                # 如果不是数字，认为是中文类型名，直接使用
                filters['type'] = type_param
        if request.args.get('main_category'):
            filters['main_category'] = request.args.get('main_category')
        if request.args.get('sub_category'):
            filters['sub_category'] = request.args.get('sub_category')
        if request.args.get('start_date'):
            filters['start_date'] = request.args.get('start_date')
        if request.args.get('end_date'):
            filters['end_date'] = request.args.get('end_date')
        if request.args.get('keyword'):
            filters['keyword'] = request.args.get('keyword')

        # **新增：解析 accountIds (逗号分隔)**
        if request.args.get('accountIds'):
            try:
                account_ids = [int(x) for x in request.args.get('accountIds').split(',') if x]
                if account_ids:
                    filters['account_ids'] = account_ids
            except ValueError:
                logger.warning(f"无效的accountIds参数: {request.args.get('accountIds')}")

        # **新增：解析 categoryIds (逗号分隔)**
        # 需要先查询分类信息，转换为 (main, sub) 列表
        if request.args.get('categoryIds'):
            try:
                category_ids = [int(x) for x in request.args.get('categoryIds').split(',') if x]
                if category_ids:
                    # 注意：这里需要获取db实例，但get_app_context在下面才调用
                    # 为了避免重复获取，我们提前获取
                    if 'db' not in locals():
                        db, _, _, _ = get_app_context()

                    loop_cat = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop_cat)
                    all_categories = loop_cat.run_until_complete(db.get_all_categories())
                    loop_cat.close()

                    target_categories = []
                    for cat in all_categories:
                        if cat['id'] in category_ids:
                            target_categories.append({
                                'main': cat.get('main_category'),
                                'sub': cat.get('sub_category')
                            })

                    if target_categories:
                        filters['categories'] = target_categories
            except ValueError:
                logger.warning(f"无效的categoryIds参数: {request.args.get('categoryIds')}")

        # **新增：解析 amountFilter**
        if request.args.get('amountFilter'):
            filters['amount_filter'] = request.args.get('amountFilter')

        if 'db' not in locals():
            db, _, _, adapter = get_app_context()
        else:
            _, _, _, adapter = get_app_context()

        # 异步调用
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bills, total = loop.run_until_complete(
            db.query_bills(
                page=page,
                page_size=page_size,
                filters=filters,
                user_id=request.user_id
            )
        )

        # 使用adapter批量转换为v1格式
        response = loop.run_until_complete(
            adapter.backend_list_to_frontend(bills, total, page, page_size)
        )
        loop.close()

        # 添加额外的分页信息
        response['result']['total'] = total
        response['result']['page'] = page
        response['result']['page_size'] = page_size
        response['result']['total_pages'] = (total + page_size - 1) // page_size

        return jsonify(response)

    except Exception as e:
        logger.error("获取账单列表失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:bill_id>', methods=['GET'])
@log_method
@require_auth
def get_bill(bill_id: int):
    """获取单个账单详情"""
    try:
        db, _, _, adapter = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({
            'success': True,
            'result': v1_bill
        })

    except Exception as e:
        logger.error("获取账单详情失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/get', methods=['GET'])
@log_method
@require_auth
def get_bill_by_query():
    """通过查询参数获取单个账单详情 (v1兼容)"""
    try:
        bill_id = request.args.get('id', type=int)
        if not bill_id:
            return jsonify({
                'success': False,
                'error': 'Missing id parameter'
            }), 400

        db, _, _, adapter = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({
            'success': True,
            'result': v1_bill
        })

    except Exception as e:
        logger.error("获取账单详情失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/modify', methods=['POST'])
@log_method
@require_auth
def modify_bill():
    """修改账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing id parameter'
            }), 400

        bill_id = int(data['id'])
        db, _, _, adapter = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：先获取原账单类型**
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        # 转换前端格式
        backend_data, metadata = adapter.frontend_to_backend(data)

        # **关键修复：只更新传入的字段**
        # 如果frontend_data只包含部分字段（如只有remark），只更新那些字段
        if 'remark' in data and 'type' not in data:
            # 简单更新模式：只有备注等非关键字段
            backend_data = {}
            if 'remark' in data:
                backend_data['description'] = data['remark']
            if 'comment' in data:
                backend_data['description'] = data['comment']
            # 保持原类型
            backend_data['type'] = old_bill['type']
            logger.info(f"简单更新模式：只更新 description={backend_data.get('description')}")

        # **保持原类型：如果前端没有传type，使用原账单的类型**
        if 'type' not in backend_data or not backend_data['type']:
            backend_data['type'] = old_bill['type']
            logger.info(f"保持原类型: {old_bill['type']}")

        # source_account_id已经在adapter中设置，无需额外查询
        # 保持原有的source_account_id
        if 'source_account_id' not in backend_data and old_bill:
            backend_data['source_account_id'] = old_bill.get('source_account_id', 0)

        # 查询分类
        if metadata.get('category_id'):
            try:
                category = loop.run_until_complete(
                    db.get_category_by_id(int(metadata['category_id']), user_id=request.user_id)
                )
                if category:
                    backend_data['main_category'] = category.get('main_category')
                    backend_data['sub_category'] = category.get('sub_category')
            except ValueError:
                pass

        # **保持其他关键字段**
        for field in ['destination_account_id', 'destination_amount']:
            if field not in backend_data and field in old_bill:
                backend_data[field] = old_bill[field]
                logger.info(f"保持原字段 {field}: {old_bill[field]}")

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # 更新标签
            if 'tagIds' in data:
                loop.run_until_complete(db.update_bill_tags(bill_id, metadata['tag_ids'], user_id=request.user_id))

            # **使用全量同步更新余额**
            # 1. 同步旧账单相关的账户余额
            old_sync_data = {
                'source_account_id': old_bill.get('source_account_id'),
                'destination_account_id': old_bill.get('destination_account_id')
            }
            loop.run_until_complete(sync_balances_for_bill(db, old_sync_data))

            # 2. 同步新账单相关的账户余额 (如果账户发生了变化)
            new_sync_data = {
                'source_account_id': backend_data.get('source_account_id', old_bill.get('source_account_id')),
                'destination_account_id': backend_data.get('destination_account_id', old_bill.get('destination_account_id'))
            }
            loop.run_until_complete(sync_balances_for_bill(db, new_sync_data))

        loop.close()

        if result:
            return jsonify({
                'success': True,
                'result': {'id': str(bill_id)}
            })
        else:
            return jsonify({
                'success': False,
                'error': 'Failed to update bill'
            }), 500

    except Exception as e:
        logger.error("修改账单失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/delete', methods=['POST'])
@log_method
@require_auth
def delete_bill_by_query():
    """删除账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing id parameter'
            }), 400

        bill_id = int(data['id'])
        db, _, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：添加余额回滚逻辑**
        # 先获取账单信息
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            sync_data = {
                'source_account_id': bill.get('source_account_id'),
                'destination_account_id': bill.get('destination_account_id')
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data))

        loop.close()

        if result:
            return jsonify({
                'success': True
            })
        else:
            return jsonify({
                'success': False,
                'error': 'Failed to delete bill'
            }), 500

    except Exception as e:
        logger.error("删除账单失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/', methods=['POST'])
@log_method
@require_auth
def create_bill():
    """创建单条账单"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.info("收到前端数据: %s", frontend_data)

        db, _, category_engine, adapter = get_app_context()

        # 转换前端格式为后端格式
        backend_data, metadata = adapter.frontend_to_backend(frontend_data)
        logger.info("转换后的后端数据: %s", backend_data)
        logger.info("元数据: %s", metadata)

        # 异步查询账户和分类信息
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **处理投资类型的自动目标账户**
        if metadata.get('auto_invest_account', False) and backend_data.get('type') == '投资':
            # 查找或创建投资目标账户
            all_accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))
            investment_account = None

            # 优先查找"活期资产"账户
            for acc in all_accounts:
                if acc['name'] in ['活期资产', '投资账户', '中信建投证券']:
                    investment_account = acc
                    break

            if investment_account:
                backend_data['destination_account_id'] = investment_account['id']
                backend_data['destination_amount'] = backend_data['amount']  # 默认同金额
                logger.info(f"投资自动设置目标账户: {investment_account['name']} (ID={investment_account['id']})")
            else:
                logger.warning("未找到合适的投资目标账户，destination_account_id保持为0")

        # ⚠️ 关键修复: 验证source_account_id有效性
        source_account_id = backend_data.get('source_account_id')
        logger.info(f"[创建账单] 收到的source_account_id: {source_account_id} (类型: {type(source_account_id)})")
        logger.info(f"[创建账单] 收到的destination_account_id: {backend_data.get('destination_account_id')} (类型: {type(backend_data.get('destination_account_id'))})")

        # 验证source_account_id是否有效
        if not source_account_id or source_account_id == 0:
            # 使用默认账户
            all_accounts_fallback = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))
            if all_accounts_fallback:
                fallback_account = all_accounts_fallback[0]
                backend_data['source_account_id'] = fallback_account['id']
                logger.warning(f"⚠ source_account_id为0，使用默认账户: {fallback_account['name']} (ID={fallback_account['id']})")
            else:
                logger.error("✗✗✗ 没有可用账户，创建将失败！")
                loop.close()
                return jsonify({
                    'success': False,
                    'error': 'No account available'
                }), 400        # 查询分类名称
        if metadata.get('category_id'):
            try:
                category = loop.run_until_complete(
                    db.get_category_by_id(int(metadata['category_id']), user_id=request.user_id)
                )
                if category:
                    # categories表结构: main_category, sub_category
                    backend_data['main_category'] = category.get('main_category', '')
                    backend_data['sub_category'] = category.get('sub_category', '')
                    logger.info("查询到分类: %s - %s",
                              backend_data['main_category'], backend_data['sub_category'])
            except ValueError:
                logger.error("无效的分类ID: %s", metadata['category_id'])

        # **强化自动分类逻辑**
        if not backend_data.get('main_category'):
            # 先尝试使用分类引擎
            main_cat, sub_cat = category_engine.match_category(backend_data)
            if main_cat:
                backend_data['main_category'] = main_cat
                backend_data['sub_category'] = sub_cat
                logger.info("自动分类（规则匹配）: %s - %s", main_cat, sub_cat)
            else:
                # 如果没有匹配规则，使用类型默认分类
                default_categories = {
                    '收入': ('工资', ''),
                    '支出': ('其他', '日常支出'),
                    '转账': ('转账', ''),
                    '投资': ('投资理财', '证券投资')
                }
                bill_type = backend_data.get('type', '支出')
                if bill_type in default_categories:
                    backend_data['main_category'], backend_data['sub_category'] = default_categories[bill_type]
                    logger.info("自动分类（默认分类）: %s - %s", backend_data['main_category'], backend_data['sub_category'])
                else:
                    backend_data['main_category'] = '其他'
                    backend_data['sub_category'] = ''
                    logger.warning(f"未知账单类型: {bill_type}，使用默认分类'其他'")

        # 设置创建时间和更新时间
        now = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        backend_data['created_at'] = now
        backend_data['updated_at'] = now

        # 创建账单
        logger.info("="*60)
        logger.info("📝 准备插入数据库的完整数据:")
        logger.info(f"  - type: {backend_data.get('type')}")
        logger.info(f"  - amount: {backend_data.get('amount')}")
        logger.info(f"  - source_account_id: {backend_data.get('source_account_id')}")
        logger.info(f"  - destination_account_id: {backend_data.get('destination_account_id')}")
        logger.info(f"  - destination_amount: {backend_data.get('destination_amount')}")
        logger.info(f"  - date: {backend_data.get('date')}")
        logger.info(f"  - main_category: {backend_data.get('main_category')}")
        logger.info(f"  - sub_category: {backend_data.get('sub_category')}")
        logger.info("="*60)

        bill_id = loop.run_until_complete(db.create_bill(backend_data, user_id=request.user_id))

        if bill_id:
            # 保存标签
            if metadata.get('tag_ids'):
                logger.info(f"[创建账单] 保存标签: {metadata['tag_ids']}")
                loop.run_until_complete(db.add_tags_to_bill(bill_id, metadata['tag_ids'], user_id=request.user_id))

            logger.info("✅ 账单创建成功，ID: %s", bill_id)

            # **使用全量同步更新余额**
            # 构造包含所有相关账户ID的数据字典
            sync_data = {
                'source_account_id': backend_data.get('source_account_id'),
                'destination_account_id': backend_data.get('destination_account_id')
            }
            logger.info(f"🔄 同步账户余额: source={sync_data['source_account_id']}, dest={sync_data['destination_account_id']}")

            # 执行同步
            loop.run_until_complete(sync_balances_for_bill(db, sync_data))

            # 获取创建的账单详情
            bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

            # 获取标签
            tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

            # 使用adapter转换为v1格式
            v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
            loop.close()

            logger.info(f"返回创建的账单(v1格式): {v1_bill}")

            return jsonify({
                'success': True,
                'result': v1_bill
            }), 201

        loop.close()
        return jsonify({
            'success': False,
            'error': 'Failed to create bill'
        }), 500

    except Exception as e:
        logger.error("创建账单失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:bill_id>', methods=['PUT'])
@log_method
@require_auth
def update_bill(bill_id: int):
    """更新账单(支持v1格式)"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.info(f"更新账单 {bill_id}, 收到前端数据: {frontend_data}")

        db, _, category_engine, adapter = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取原账单，用于余额回滚
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        # 判断是前端格式还是后端格式
        if 'sourceAmount' in frontend_data or 'sourceAccountId' in frontend_data:
            # 前端v1格式,需要转换
            backend_data, metadata = adapter.frontend_to_backend(frontend_data)

            # source_account_id已在adapter中处理，无需额外查询
            # 验证source_account_id是否有效即可
            if metadata.get('source_account_id'):
                logger.info(f"源账户ID: {metadata['source_account_id']}")

            # 查询分类名称
            if metadata.get('category_id'):
                try:
                    category = loop.run_until_complete(
                        db.get_category_by_id(int(metadata['category_id']), user_id=request.user_id)
                    )
                    if category:
                        backend_data['main_category'] = category.get('main_category', '')
                        backend_data['sub_category'] = category.get('sub_category', '')
                        logger.info(f"查询到分类: {backend_data['main_category']} - {backend_data['sub_category']}")
                except ValueError:
                    logger.error(f"无效的分类ID: {metadata['category_id']}")
        else:
            # 后端格式,直接使用
            backend_data = frontend_data
            metadata = {}
            # 如果有描述或对方变更，重新分类
            if 'description' in backend_data or 'counterparty' in backend_data:
                # 合并旧数据
                old_bill.update(backend_data)
                # 重新分类
                main_cat, sub_cat = category_engine.match_category(old_bill)
                if main_cat:
                    backend_data['main_category'] = main_cat
                    backend_data['sub_category'] = sub_cat

        backend_data['updated_at'] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        logger.info(f"准备更新的后端数据: {backend_data}")

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # **处理标签更新**
            if 'tag_ids' in metadata:
                tag_ids = metadata['tag_ids']
                logger.info(f"[更新账单] 更新标签: {tag_ids}")
                loop.run_until_complete(db.update_bill_tags(bill_id, tag_ids, user_id=request.user_id))
            else:
                logger.debug(f"[更新账单] 未提供标签数据，保持现有标签")

            # 获取更新后的账单
            updated_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

            # 获取标签
            tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))
            logger.info(f"[更新账单] 获取到标签: {tags}")

            # **同步账户余额**
            # 1. 同步旧账单关联的账户 (回滚旧金额)
            sync_data_old = {
                'source_account_id': old_bill.get('source_account_id'),
                'destination_account_id': old_bill.get('destination_account_id')
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_old))

            # 2. 同步新账单关联的账户 (应用新金额)
            # 如果账户ID没变，其实会被同步两次，但这保证了数据的最终一致性
            sync_data_new = {
                'source_account_id': updated_bill.get('source_account_id'),
                'destination_account_id': updated_bill.get('destination_account_id')
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_new))

            # 使用adapter转换为v1格式
            v1_bill = loop.run_until_complete(adapter.backend_to_frontend(updated_bill, tags=tags))
            loop.close()

            return jsonify({
                'success': True,
                'result': v1_bill
            })

        loop.close()
        return jsonify({
            'success': False,
            'error': 'Bill not found or update failed'
        }), 404

    except Exception as e:
        logger.error(f"更新账单失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:bill_id>', methods=['DELETE'])
@log_method
@require_auth
def delete_bill(bill_id: int):
    """删除账单"""
    try:
        db, _, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 先获取账单信息，用于回滚余额
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({
                'success': False,
                'error': 'Bill not found'
            }), 404

        # 删除账单
        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            try:
                sync_data = {
                    'source_account_id': bill.get('source_account_id'),
                    'destination_account_id': bill.get('destination_account_id')
                }
                loop.run_until_complete(sync_balances_for_bill(db, sync_data))
            except Exception as e:
                logger.error(f"删除账单后同步余额失败: {e}", exc_info=True)
                # 不阻断删除成功的响应

            loop.close()
            return jsonify({
                'success': True,
                'result': True,
                'message': 'Bill deleted successfully'
            })

        loop.close()
        return jsonify({
            'success': False,
            'error': 'Failed to delete bill'
        }), 500
    except Exception as e:
        logger.error("删除账单失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/batch', methods=['POST'])
@log_method
@require_auth
def import_bills_batch():
    """批量导入账单"""
    try:
        data = request.get_json()
        if not data or 'file_path' not in data:
            return jsonify({
                'success': False,
                'error': 'file_path is required'
            }), 400

        _, bill_service, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            bill_service.import_bills(data['file_path'])
        )
        loop.close()

        return jsonify({
            'success': result['success'],
            'result': result
        })

    except Exception as e:
        logger.error(f"批量导入账单失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/upload', methods=['POST'])
@log_method
@require_auth
def upload_and_import():
    """
    上传文件并导入账单

    Request:
        - file: 上传的文件（支持csv, xlsx, xls, txt）
        - parser_type: 解析器类型（wechat/alipay/icbc/cmbc/abc/ccb）
        - preview_only: 是否仅预览（true/false）

    Response:
        {
            'success': true,
            'data': {
                'preview': [...],  # 预览数据
                'total': 100,      # 总记录数
                'imported': 95,    # 导入成功数
                'failed': 5,       # 导入失败数
                'duplicates': 10   # 重复记录数
            }
        }
    """
    try:
        # 检查文件是否在请求中
        if 'file' not in request.files:
            return jsonify({
                'success': False,
                'error': 'No file provided'
            }), 400

        file = request.files['file']

        # 检查文件名是否为空
        if file.filename == '':
            return jsonify({
                'success': False,
                'error': 'No file selected'
            }), 400

        # 检查文件扩展名
        if not allowed_file(file.filename):
            return jsonify({
                'success': False,
                'error': f'File type not allowed. Supported: {", ".join(ALLOWED_EXTENSIONS)}'
            }), 400

        # 获取解析器类型
        parser_type = request.form.get('parser_type', 'auto')
        preview_only = request.form.get('preview_only', 'false').lower() == 'true'

        # 保存文件
        if file.filename is None:
            return jsonify({
                'success': False,
                'error': 'Invalid filename'
            }), 400

        filename = secure_filename(file.filename)
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        unique_filename = f"{timestamp}_{filename}"
        file_path = UPLOAD_FOLDER / unique_filename

        file.save(str(file_path))
        logger.info(f"文件已保存: {file_path}")

        # 导入账单
        _, bill_service, _, _ = get_app_context()

        # 获取用户ID
        user_id = getattr(request, 'user_id', 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取解析结果
        result = loop.run_until_complete(
            bill_service.import_bills(
                str(file_path),
                parser_type=parser_type,
                preview_only=preview_only,
                user_id=user_id
            )
        )

        loop.close()

        # 如果不是预览模式且导入成功，删除临时文件
        if not preview_only and result.get('success'):
            try:
                os.remove(file_path)
                logger.info(f"临时文件已删除: {file_path}")
            except Exception as e:
                logger.warning(f"删除临时文件失败: {e}")

        # 日志记录返回数据
        preview_count = len(result.get('preview', []))
        logger.info(f"[导入API返回] success={result.get('success')}, "
                    f"preview_count={preview_count}, "
                    f"total={result.get('total')}, valid={result.get('valid')}")

        # 前端期望格式: { success, data: { preview: [...] } }
        return jsonify({
            'success': result.get('success', False),
            'data': {
                'preview': result.get('preview', []),
                'total': result.get('total', 0),
                'valid': result.get('valid', 0),
                'invalid': result.get('invalid', 0),
                'inserted': result.get('inserted', 0),
                'duplicates': result.get('duplicates', 0),
                'dedup_stats': result.get('dedup_stats'),
                'parser_type': result.get('parser_type', 'unknown'),
                'errors': result.get('errors', [])
            }
        })

    except Exception as e:
        logger.error(f"上传并导入账单失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/parsers', methods=['GET'])
@log_method
@require_auth
def get_available_parsers():
    """
    获取可用的解析器列表

    Response:
        {
            'success': true,
            'data': [
                {'id': 'wechat', 'name': '微信支付', 'description': '...'},
                {'id': 'alipay', 'name': '支付宝', 'description': '...'},
                ...
            ]
        }
    """
    try:
        parsers = [
            {
                'id': 'auto',
                'name': '自动识别',
                'description': '自动检测文件类型并选择合适的解析器',
                'supported_formats': ['csv']
            },
            {
                'id': 'wechat',
                'name': '微信支付',
                'description': '解析微信支付账单CSV文件',
                'supported_formats': ['csv']
            },
            {
                'id': 'alipay',
                'name': '支付宝',
                'description': '解析支付宝交易明细CSV文件',
                'supported_formats': ['csv']
            },
            {
                'id': 'icbc',
                'name': '工商银行',
                'description': '解析工商银行流水文件',
                'supported_formats': ['csv', 'xlsx', 'xls']
            },
            {
                'id': 'cmbc',
                'name': '招商银行',
                'description': '解析招商银行流水文件',
                'supported_formats': ['csv', 'xlsx', 'xls']
            },
            {
                'id': 'abc',
                'name': '农业银行',
                'description': '解析农业银行流水文件',
                'supported_formats': ['csv', 'xlsx', 'xls']
            },
            {
                'id': 'ccb',
                'name': '建设银行',
                'description': '解析建设银行流水文件',
                'supported_formats': ['csv', 'xlsx', 'xls']
            }
        ]

        return jsonify({
            'success': True,
            'result': parsers
        })

    except Exception as e:
        logger.error(f"获取解析器列表失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/reclassify', methods=['POST'])
@log_method
@require_auth
def reclassify_transactions():
    """
    重新分类导入预览中的交易

    使用后端分类引擎和账户匹配逻辑重新计算交易的分类和账户

    Request:
        {
            'transactions': [
                {
                    'description': '...',
                    'counterparty': '...',
                    'amount': 100.50,
                    'type': 3,  # TransactionType
                    'originalSourceAccountName': '...',
                    'originalDestinationAccountName': '...'
                },
                ...
            ]
        }

    Response:
        {
            'success': true,
            'result': [
                {
                    'index': 0,
                    'categoryId': '123',
                    'categoryName': '餐饮-外卖',
                    'sourceAccountId': '456',
                    'destinationAccountId': '789'  # 仅转账/投资类型
                },
                ...
            ]
        }
    """
    try:
        data = request.get_json()
        if not data or 'transactions' not in data:
            return jsonify({
                'success': False,
                'error': '缺少transactions字段'
            }), 400

        transactions = data['transactions']
        if not isinstance(transactions, list):
            return jsonify({
                'success': False,
                'error': 'transactions必须是数组'
            }), 400

        logger.info(f"[重新分类] 收到 {len(transactions)} 条交易")

        # 获取分类引擎和数据库
        db, _, category_engine, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 加载分类规则
            loop.run_until_complete(category_engine.load_rules_from_db(db, user_id=request.user_id))

            # 获取所有账户用于匹配
            all_accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            # 获取所有分类用于ID查询
            all_categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            category_map = {}
            for cat in all_categories:
                key = (cat.get('main_category', ''), cat.get('sub_category', ''))
                category_map[key] = cat

            results = []
            for idx, trans in enumerate(transactions):
                result = {'index': idx}

                # 构建用于分类匹配的bill结构
                bill = {
                    'description': trans.get('description', ''),
                    'counterparty': trans.get('counterparty', ''),
                    'amount': float(trans.get('amount', 0)),
                    'type': trans.get('type', '支出')
                }

                # 1. 分类匹配
                main_cat, sub_cat = category_engine.match_category(bill)
                if main_cat:
                    result['mainCategory'] = main_cat
                    result['subCategory'] = sub_cat or ''
                    # 查找分类ID
                    cat_info = category_map.get((main_cat, sub_cat or ''))
                    if cat_info:
                        result['categoryId'] = str(cat_info.get('id', ''))
                        result['categoryName'] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                    else:
                        result['categoryId'] = ''
                        result['categoryName'] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                else:
                    result['categoryId'] = ''
                    result['categoryName'] = ''

                # 2. 账户匹配（通过账户名称或别名）
                original_source = trans.get('originalSourceAccountName', '')
                original_dest = trans.get('originalDestinationAccountName', '')

                if original_source:
                    source_account = _match_account_by_name(all_accounts, original_source)
                    result['sourceAccountId'] = str(source_account['id']) if source_account else ''
                    result['sourceAccountName'] = source_account['name'] if source_account else ''

                if original_dest:
                    dest_account = _match_account_by_name(all_accounts, original_dest)
                    result['destinationAccountId'] = str(dest_account['id']) if dest_account else ''
                    result['destinationAccountName'] = dest_account['name'] if dest_account else ''

                results.append(result)

            logger.info(f"[重新分类] 完成 {len(results)} 条交易的重新分类")

            return jsonify({
                'success': True,
                'result': results
            })

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"重新分类失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


def _match_account_by_name(accounts: list, name: str) -> dict:
    """
    通过名称或别名匹配账户

    Args:
        accounts: 账户列表
        name: 要匹配的名称

    Returns:
        匹配到的账户字典，或None
    """
    if not name:
        return None

    name_lower = name.lower().strip()

    for account in accounts:
        # 精确匹配账户名称
        if account.get('name', '').lower() == name_lower:
            return account

        # 匹配别名（存储在comment字段或aliases字段）
        aliases_str = account.get('aliases', '') or account.get('comment', '')
        if aliases_str:
            aliases = [a.strip().lower() for a in aliases_str.split(',')]
            if name_lower in aliases:
                return account

    # 模糊匹配：名称包含关系
    for account in accounts:
        account_name = account.get('name', '').lower()
        if name_lower in account_name or account_name in name_lower:
            return account

    return None


@bp.route('/import/v2/reclassify/<session_id>', methods=['POST'])
@log_method
@require_auth
def reclassify_preview_session(session_id: str):
    """
    v6.55: 重新分类导入会话中的预览账单

    功能：
    1. 刷新分类规则（从数据库重新加载）
    2. 从 bills_preview 表读取所有账单
    3. 根据 dedup_type 使用不同类型的分类规则
    4. 重新执行账户匹配
    5. 更新 bills_preview 表
    6. 返回更新后的预览数据

    Request:
        POST /api/bills/import/v2/reclassify/<session_id>

    Response:
        {
            'success': true,
            'data': {
                'session_id': 'xxx',
                'total': 100,
                'categorized': 80,
                'account_matched': 90,
                'preview': [...]  // 更新后的预览数据
            }
        }
    """
    try:
        logger.info(f"[v2重新分类] session_id={session_id}, user_id={request.user_id}")

        db, bill_service, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 1. 执行重新分类
            reclassify_result = loop.run_until_complete(
                bill_service.reclassify_preview_bills(session_id, user_id=request.user_id)
            )

            if not reclassify_result.get('success'):
                return jsonify({
                    'success': False,
                    'error': reclassify_result.get('errors', ['未知错误'])[0] if reclassify_result.get('errors') else '重新分类失败'
                }), 500

            # 2. 获取更新后的预览数据
            preview_data = loop.run_until_complete(
                bill_service.get_import_preview(session_id)
            )

            logger.info(f"[v2重新分类] 完成 session={session_id}, "
                        f"total={reclassify_result.get('total')}, "
                        f"categorized={reclassify_result.get('categorized')}, "
                        f"account_matched={reclassify_result.get('account_matched')}")

            return jsonify({
                'success': True,
                'data': {
                    'session_id': session_id,
                    'total': reclassify_result.get('total', 0),
                    'categorized': reclassify_result.get('categorized', 0),
                    'account_matched': reclassify_result.get('account_matched', 0),
                    'preview': preview_data
                }
            })

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[v2重新分类] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/confirm', methods=['POST'])
@log_method
@require_auth
def confirm_import():
    """
    确认导入预览的账单

    用户在预览后可能修改了分类，然后确认导入

    Request:
        {
            'bills': [...]  # 经用户确认（可能修改）的账单列表
        }

    Response:
        {
            'success': true,
            'result': {
                'total': 100,
                'inserted': 95,
                'duplicates': 5
            }
        }
    """
    try:
        data = request.get_json()
        if not data or 'bills' not in data:
            return jsonify({
                'success': False,
                'error': 'bills is required'
            }), 400

        _, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(
            bill_service.import_preview_confirmed(
                data['bills'],
                user_id=user_id
            )
        )
        loop.close()

        return jsonify({
            'success': result.get('success', False),
            'result': result
        })

    except Exception as e:
        logger.error(f"确认导入失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/category/quick-add-keyword', methods=['POST'])
@log_method
@require_auth
def quick_add_category_keyword():
    """
    快速为分类添加关键词

    用户在手动分类时可以将交易的某个关键词添加到分类规则中

    Request:
        {
            'main_category': '餐饮',
            'sub_category': '外卖',  # 可选
            'keyword': '美团'
        }

    Response:
        {
            'success': true,
            'message': 'Keyword added successfully'
        }
    """
    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'Request body is required'
            }), 400

        main_category = data.get('main_category')
        sub_category = data.get('sub_category')
        keyword = data.get('keyword')

        if not main_category or not keyword:
            return jsonify({
                'success': False,
                'error': 'main_category and keyword are required'
            }), 400

        _, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        success = loop.run_until_complete(
            bill_service.add_category_keyword(
                main_category,
                sub_category,
                keyword,
                user_id=user_id
            )
        )
        loop.close()

        if success:
            return jsonify({
                'success': True,
                'message': 'Keyword added successfully'
            })
        else:
            return jsonify({
                'success': False,
                'error': 'Failed to add keyword'
            }), 400

    except Exception as e:
        logger.error(f"添加关键词失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/category/refresh', methods=['POST'])
@log_method
@require_auth
def refresh_bill_categories():
    """
    刷新账单分类

    使用最新的分类规则重新匹配账单

    Request:
        {
            'bill_ids': [1, 2, 3]  # 可选，不提供则刷新所有未分类账单
        }

    Response:
        {
            'success': true,
            'result': {
                'total': 50,
                'categorized': 45,
                'still_uncategorized': 5
            }
        }
    """
    try:
        data = request.get_json() or {}
        bill_ids = data.get('bill_ids')

        _, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(
            bill_service.refresh_category_for_bills(
                bill_ids,
                user_id=user_id
            )
        )
        loop.close()

        return jsonify({
            'success': result.get('success', False),
            'result': result
        })

    except Exception as e:
        logger.error(f"刷新分类失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/batch/update', methods=['PUT'])
@log_method
@require_auth
def batch_update_bills():
    """批量更新账单"""
    try:
        data = request.get_json()
        if not data or 'ids' not in data or 'updates' not in data:
            return jsonify({
                'success': False,
                'error': 'ids and updates are required'
            }), 400

        db, _, _, _ = get_app_context()

        ids = data['ids']
        updates = data['updates']
        updates['updated_at'] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            db.batch_update_bills(ids, updates, user_id=request.user_id)
        )
        loop.close()

        return jsonify({
            'success': True,
            'result': {
                'updated_count': result
            }
        })

    except Exception as e:
        logger.error(f"批量更新账单失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/batch/delete', methods=['DELETE'])
@log_method
@require_auth
def batch_delete_bills():
    """批量删除账单"""
    try:
        data = request.get_json()
        if not data or 'ids' not in data:
            return jsonify({
                'success': False,
                'error': 'ids are required'
            }), 400

        db, _, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **收集受影响的账户ID**
        affected_accounts = set()
        # 注意：如果批量删除数量很大，这里可能会慢。但通常批量删除是分页的。
        # 为了准确同步余额，我们需要知道哪些账户被影响了。
        for bill_id in data['ids']:
            try:
                bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
                if bill:
                    if bill.get('source_account_id'):
                        affected_accounts.add(int(bill['source_account_id']))
                    if bill.get('destination_account_id'):
                        affected_accounts.add(int(bill['destination_account_id']))
            except Exception as e:
                logger.warning(f"获取账单信息失败 (ID: {bill_id}): {e}")

        result = loop.run_until_complete(
            db.batch_delete_bills(data['ids'], user_id=request.user_id)
        )

        # **同步余额**
        if result > 0:
            logger.info(f"批量删除成功，开始同步 {len(affected_accounts)} 个账户的余额")
            for account_id in affected_accounts:
                try:
                    loop.run_until_complete(db.sync_account_balance(account_id))
                except Exception as e:
                    logger.error(f"同步账户余额失败 (ID: {account_id}): {e}")

        loop.close()

        return jsonify({
            'success': True,
            'result': {
                'deleted_count': result
            }
        })

    except Exception as e:
        logger.error(f"批量删除账单失败: {e}")
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/transactions/list/by_month.json', methods=['GET'])
@log_method
@require_auth
def get_bills_by_month():
    """
    按月查询交易记录（兼容v1 API）

    Query Parameters:
        - year: 年份
        - month: 月份
        - type: 交易类型（0=全部，1=支出，2=收入，3=转账）
        - category_ids: 分类ID过滤（逗号分隔）
        - account_ids: 账户ID过滤（逗号分隔）
        - tag_ids: 标签ID过滤（逗号分隔）
        - tag_filter_type: 标签过滤类型
        - amount_filter: 金额过滤
        - keyword: 关键词搜索
    """
    logger.info("=" * 50)
    logger.info("按月查询交易记录")

    try:
        # 获取查询参数
        year = int(request.args.get('year', datetime.now().year))
        month = int(request.args.get('month', datetime.now().month))
        transaction_type = request.args.get('type', '0')

        logger.info(f"📅 查询参数: year={year}, month={month}, type={transaction_type}")

        # 构建过滤条件
        filters = {}

        # 计算月份的起止日期
        start_date = f"{year:04d}-{month:02d}-01"
        if month == 12:
            end_date = f"{year+1:04d}-01-01"
        else:
            end_date = f"{year:04d}-{month+1:02d}-01"

        filters['start_date'] = start_date
        filters['end_date'] = end_date
        logger.info(f"⏰ 时间范围: {start_date} 至 {end_date}")

        # 交易类型映射：v1格式 -> 后端格式
        # 0=全部(不过滤), 1=修改(暂不支持), 2=收入, 3=支出, 4=转账, 5=投资
        if transaction_type and int(transaction_type) > 0:
            type_int = int(transaction_type)
            if type_int in BACKEND_TO_FRONTEND_TYPE.values():
                # 反向查找
                for chinese, v1_type in BACKEND_TO_FRONTEND_TYPE.items():
                    if v1_type == type_int:
                        filters['type'] = chinese
                        logger.info(f"🏷️ 类型筛选: {type_int} -> {chinese}")
                        break

        # 处理 category_ids 过滤（逗号分隔的ID列表）
        category_ids_str = request.args.get('category_ids', default='')
        if category_ids_str:
            try:
                category_ids = [int(x) for x in category_ids_str.split(',') if x.strip()]
                if category_ids:
                    logger.info(f"🗂️ 分类筛选: {category_ids}")
                    # 需要查询分类信息转换为 (main, sub) 列表
                    db, _, _, adapter = get_app_context()
                    loop_cat = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop_cat)
                    all_categories = loop_cat.run_until_complete(db.get_all_categories(user_id=request.user_id))
                    loop_cat.close()

                    target_categories = []
                    for cat in all_categories:
                        if cat['id'] in category_ids:
                            target_categories.append({
                                'main': cat.get('main_category'),
                                'sub': cat.get('sub_category')
                            })

                    if target_categories:
                        filters['categories'] = target_categories
                        logger.info(f"   转换为: {target_categories}")
            except ValueError as e:
                logger.warning(f"⚠️ 无效的category_ids参数: {category_ids_str}, 错误: {e}")

        # 处理 account_ids 过滤（逗号分隔的ID列表）
        account_ids_str = request.args.get('account_ids', default='')
        if account_ids_str:
            try:
                account_ids = [int(x) for x in account_ids_str.split(',') if x.strip()]
                if account_ids:
                    filters['account_ids'] = account_ids
                    logger.info(f"💳 账户筛选: {account_ids}")
            except ValueError as e:
                logger.warning(f"⚠️ 无效的account_ids参数: {account_ids_str}, 错误: {e}")

        # 处理 tag_ids 过滤
        tag_ids_str = request.args.get('tag_ids', default='')
        if tag_ids_str:
            try:
                tag_ids = [int(x) for x in tag_ids_str.split(',') if x.strip()]
                if tag_ids:
                    filters['tag_ids'] = tag_ids
                    logger.info(f"🏷️ 标签筛选: {tag_ids}")
            except ValueError as e:
                logger.warning(f"⚠️ 无效的tag_ids参数: {tag_ids_str}, 错误: {e}")

        # 关键词搜索
        keyword = request.args.get('keyword', default='')
        if keyword:
            from urllib.parse import unquote
            filters['keyword'] = unquote(keyword)
            logger.info(f"🔍 关键词搜索: {filters['keyword']}")

        # 处理 amount_filter 过滤
        amount_filter = request.args.get('amount_filter', default='')
        if amount_filter:
            filters['amount_filter'] = amount_filter
            logger.info(f"💰 金额筛选: {amount_filter}")

        logger.info(f"📋 最终过滤条件: {filters}")

        db, _, _, adapter = get_app_context()

        # 异步查询（获取所有数据，不分页）
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bills, total = loop.run_until_complete(
            db.query_bills(
                page=1,
                page_size=10000,  # 足够大的数字获取全部
                filters=filters,
                user_id=request.user_id
            )
        )

        # ⚠️ 关键修复: 使用adapter转换为v1格式
        logger.info(f"📦 数据库查询完成: 找到 {total} 条记录，开始转换为v1格式...")
        response = loop.run_until_complete(
            adapter.backend_list_to_frontend(bills, total, 1, 10000)
        )
        loop.close()

        logger.info(f"✅ 按月查询完成: 返回 {len(response['result']['items'])} 条v1格式记录")
        logger.info("=" * 50)

        return jsonify(response)

    except Exception as e:
        logger.error(f"按月查询交易失败: {e}", exc_info=True)
        logger.info("=" * 50)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/parse_import', methods=['POST'])
@log_method
@require_auth
def parse_import_file():
    """
    解析导入文件（兼容v1 API）

    Form Data:
        - file: 上传的文件
        - fileType: 解析器类型（auto/wechat/alipay/icbc/cmbc/abc/ccb）
        - fileEncoding: 文件编码（可选，默认utf-8）

    Response:
        {
            'success': true,
            'result': {
                'items': [...],  # 解析后的交易列表
                'totalCount': 100  # 总数
            }
        }
    """
    logger.info("=" * 50)
    logger.info("解析导入文件")

    try:
        # 检查文件
        if 'file' not in request.files:
            logger.error("未提供文件")
            return jsonify({
                'success': False,
                'error': 'No file provided'
            }), 400

        file = request.files['file']

        if file.filename == '':
            logger.error("文件名为空")
            return jsonify({
                'success': False,
                'error': 'No file selected'
            }), 400

        # 获取解析器类型
        parser_type = request.form.get('fileType', 'auto')
        if parser_type == 'auto':
            parser_type = None  # None表示自动检测

        logger.info(f"文件: {file.filename}, 解析器类型: {parser_type or '自动检测'}")

        # 检查文件扩展名
        if not allowed_file(file.filename):
            logger.error(f"不支持的文件类型: {file.filename}")
            return jsonify({
                'success': False,
                'error': f'File type not allowed. Supported: {", ".join(ALLOWED_EXTENSIONS)}'
            }), 400

        # 保存临时文件
        if file.filename is None:
            return jsonify({
                'success': False,
                'error': 'Invalid filename'
            }), 400

        filename = secure_filename(file.filename)
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        unique_filename = f"{timestamp}_{filename}"
        file_path = UPLOAD_FOLDER / unique_filename

        file.save(str(file_path))
        logger.info(f"临时文件已保存: {file_path}")

        # 使用ParserFactory解析
        from src.parsers.factory import ParserFactory
        parser_factory = ParserFactory()

        logger.debug("开始解析文件...")
        bills = parser_factory.parse(str(file_path), parser_type=parser_type)
        logger.info(f"解析完成: {len(bills)} 条记录")

        # 删除临时文件
        try:
            os.remove(file_path)
            logger.debug(f"临时文件已删除: {file_path}")
        except Exception as e:
            logger.warning(f"删除临时文件失败: {e}")

        # 转换为前端期望的格式
        # 解析器已经返回统一字段：trade_time, type, amount, account, description, counterparty,
        # main_category, sub_category, payment_method
        items = []
        for bill in bills:
            item = {
                'time': bill.get('trade_time', ''),  # 交易时间
                'type': bill.get('type', ''),  # 交易类型（收入/支出/转账/退款）
                'categoryName': bill.get('main_category', ''),  # 主分类
                'subCategoryName': bill.get('sub_category', ''),  # 子分类
                'accountName': bill.get('account', ''),  # 账户/来源
                'amount': bill.get('amount', 0),  # 金额
                'description': bill.get('description', ''),  # 描述/备注
                'counterparty': bill.get('counterparty', ''),  # 交易对方
                'paymentMethod': bill.get('payment_method', '')  # 支付方式
            }
            items.append(item)

        logger.info("=" * 50)

        return jsonify({
            'success': True,
            'result': {
                'items': items,
                'totalCount': len(items)
            }
        })

    except Exception as e:
        logger.error(f"解析导入文件失败: %s", e, exc_info=True)
        logger.info("=" * 50)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


# ============================================================================
# v1 API兼容层 - 兼容ezBookkeeping前端格式
# ============================================================================

@bp_v1.route('/v1/transactions/list.json', methods=['GET'])
@log_method
@require_auth
def get_transactions_v1():
    """
    v1版本的交易列表接口 - 兼容ezBookkeeping前端

    Query Parameters:
        - max_time: 最大时间(Unix timestamp ms)
        - min_time: 最小时间(Unix timestamp ms)
        - type: 类型过滤 (0=全部, 1=修改, 2=收入, 3=支出, 4=转账, 5=投资)
        - category_ids: 分类ID列表（逗号分隔）
        - account_ids: 账户ID列表（逗号分隔）
        - tag_ids: 标签ID列表（逗号分隔）
        - tag_filter_type: 标签过滤类型
        - amount_filter: 金额过滤
        - keyword: 关键词
        - count: 每页数量（默认50）
        - page: 页码（默认1）
        - with_count: 是否返回总数
        - trim_account: 是否精简账户信息
        - trim_category: 是否精简分类信息
        - trim_tag: 是否精简标签信息

    Returns:
        {
            "success": true,
            "result": {
                "items": [{"id": "1", "date": "2024-01-01", ...}],
                "totalCount": 100,
                "nextTimeSequenceId": 123456
            }
        }
    """
    try:
        # 解析查询参数
        max_time = request.args.get('max_time', type=int, default=0)
        min_time = request.args.get('min_time', type=int, default=0)
        transaction_type = request.args.get('type', type=int, default=0)
        count = request.args.get('count', type=int, default=50)
        page = request.args.get('page', type=int, default=1)
        # with_count用于是否返回总数，保留以保持API兼容性
        _ = request.args.get('with_count', default='false').lower() == 'true'
        keyword = request.args.get('keyword', default='')

        # 分类、账户、标签ID（逗号分隔字符串）
        category_ids_str = request.args.get('category_ids', default='')
        account_ids_str = request.args.get('account_ids', default='')
        amount_filter = request.args.get('amount_filter', default='')

        logger.info((
            f"v1交易列表查询: page={page}, count={count}, type={transaction_type}, "
            f"keyword={keyword}, category_ids={category_ids_str}, "
            f"account_ids={account_ids_str}, amount_filter={amount_filter}"
        ))

        # 构建过滤条件
        filters = {}

        # 时间范围（转换为日期字符串）
        if min_time > 0:
            filters['start_date'] = datetime.fromtimestamp(min_time / 1000).strftime('%Y-%m-%d')

        if max_time > 0:
            filters['end_date'] = datetime.fromtimestamp(max_time / 1000).strftime('%Y-%m-%d')

        # 交易类型映射：v1格式 -> 后端格式
        # 0=全部(不过滤), 1=修改(暂不支持), 2=收入, 3=支出, 4=转账, 5=投资
        if transaction_type > 0 and transaction_type in BACKEND_TO_FRONTEND_TYPE.values():
            # 反向查找
            for chinese, type_int in BACKEND_TO_FRONTEND_TYPE.items():
                if type_int == transaction_type:
                    filters['type'] = chinese
                    break
        # transaction_type=0表示查询全部类型,不设置filters['type']

        # 关键词搜索
        if keyword:
            from urllib.parse import unquote
            filters['keyword'] = unquote(keyword)

        db, _, _, adapter = get_app_context()

        # 处理 account_ids 过滤（逗号分隔的ID列表）
        if account_ids_str:
            try:
                account_ids = [int(x) for x in account_ids_str.split(',') if x.strip()]
                if account_ids:
                    filters['account_ids'] = account_ids
                    logger.info(f"account_ids筛选: {account_ids}")
            except ValueError:
                logger.warning(f"无效的account_ids参数: {account_ids_str}")

        # 处理 category_ids 过滤（需要查询分类信息转换为 (main, sub) 列表）
        if category_ids_str:
            try:
                category_ids = [int(x) for x in category_ids_str.split(',') if x.strip()]
                if category_ids:
                    loop_cat = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop_cat)
                    all_categories = loop_cat.run_until_complete(db.get_all_categories(user_id=request.user_id))
                    loop_cat.close()

                    target_categories = []
                    for cat in all_categories:
                        if cat['id'] in category_ids:
                            target_categories.append({
                                'main': cat.get('main_category'),
                                'sub': cat.get('sub_category')
                            })

                    if target_categories:
                        filters['categories'] = target_categories
                        logger.info(f"category_ids筛选: {category_ids} -> {target_categories}")
            except ValueError:
                logger.warning(f"无效的category_ids参数: {category_ids_str}")

        # 处理 amount_filter 过滤（格式：equals=100 或 gt=100 或 lt=100 或 between=100-200）
        if amount_filter:
            filters['amount_filter'] = amount_filter
            logger.info(f"amount_filter筛选: {amount_filter}")

        # TODO: 处理 tag_ids 过滤（暂未实现标签系统）

        # 查询账单
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bills, total = loop.run_until_complete(
            db.query_bills(
                page=page,
                page_size=count,
                filters=filters,
                user_id=request.user_id
            )
        )

        # 使用adapter批量转换为v1格式
        response = loop.run_until_complete(
            adapter.backend_list_to_frontend(bills, total, page, count)
        )
        loop.close()

        logger.info(f"v1交易列表返回: 共{len(response['result']['items'])}条记录, 总数={total}")

        return jsonify(response)

    except Exception as e:
        logger.error("v1获取交易列表失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e),
            'message': 'Unable to retrieve transaction list'
        }), 500


@bp.route('/reconciliation_statements', methods=['GET'])
@log_method
@require_auth
def get_reconciliation_statements():
    """
    获取账户对账单

    Query Parameters:
        - account_id: 账户ID (必需)
        - start_time: 开始时间（Unix时间戳，秒）(必需)
        - end_time: 结束时间（Unix时间戳，秒）(必需)
        - category_ids: 分类ID列表（逗号分隔，可选）
        - type: 交易类型（1=收入, 2=支出, 3=转账，可选）
        - keyword: 关键词搜索（可选）

    返回对账单数据，包含收入、支出、余额变动等统计信息
    """
    # 记录入口参数
    logger.info("[get_reconciliation_statements] 入口参数: account_id=%s, start_time=%s, end_time=%s, "
                "category_ids=%s, type=%s, keyword=%s",
                request.args.get('account_id'),
                request.args.get('start_time'),
                request.args.get('end_time'),
                request.args.get('category_ids'),
                request.args.get('type'),
                request.args.get('keyword'))

    try:
        # 获取必需参数
        account_id = request.args.get('account_id')
        start_time = request.args.get('start_time', type=int)
        end_time = request.args.get('end_time', type=int)

        # 注意：start_time=0, end_time=0 是有效值，表示查询全部
        if account_id is None or start_time is None or end_time is None:
            logger.error("[get_reconciliation_statements] 缺少必需参数")
            return jsonify({
                'success': False,
                'error': 'Missing required parameters: account_id, start_time, end_time'
            }), 400

        # 获取可选参数
        category_ids = request.args.get('category_ids')  # 逗号分隔的分类ID
        trans_type = request.args.get('type', type=int)  # 1=收入, 2=支出, 3=转账
        keyword = request.args.get('keyword')

        # 转换Unix时间戳为日期字符串
        # 注意：start_time=0 和 end_time=0 表示查询全部，不设置日期限制
        if start_time == 0 and end_time == 0:
            start_date = None
            end_date = None
            logger.info("[get_reconciliation_statements] 查询全部时间范围")
        else:
            start_date = datetime.fromtimestamp(start_time).strftime('%Y-%m-%d')
            end_date = datetime.fromtimestamp(end_time).strftime('%Y-%m-%d')
            logger.info("[get_reconciliation_statements] 时间范围: %s 至 %s", start_date, end_date)

        # 验证account_id是有效数字
        try:
            account_id_int = int(account_id)
        except (ValueError, TypeError):
            logger.error("[get_reconciliation_statements] 无效的账户ID: %s", account_id)
            return jsonify({
                'success': False,
                'error': f'Invalid account_id: {account_id}'
            }), 400

        db, _, _, adapter = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 获取账户信息
            account = loop.run_until_complete(db.get_account_by_id(account_id_int, user_id=request.user_id))
            if not account:
                logger.warning("[get_reconciliation_statements] 账户不存在: %s", account_id_int)
                return jsonify({
                    'success': False,
                    'error': 'Account not found'
                }), 404

            account_name = account['name']
            logger.info("[get_reconciliation_statements] 查询账户: %s (ID=%s)", account_name, account_id_int)

            # 构建查询筛选条件 - 使用账户ID而非名称，更精确
            filters = {
                'account_ids': [account_id_int]  # 使用账户ID列表精确匹配
            }
            
            # 只有在指定了时间范围时才添加日期过滤
            if start_date is not None and end_date is not None:
                filters['start_date'] = start_date
                filters['end_date'] = end_date

            # 添加可选筛选条件
            if category_ids:
                # 将分类ID字符串转换为列表
                category_id_list = [cid.strip() for cid in category_ids.split(',') if cid.strip()]
                if category_id_list:
                    filters['category_ids'] = category_id_list
                    logger.info("[get_reconciliation_statements] 分类筛选: %s", category_id_list)

            if trans_type:
                # 转换类型编号为中文名称
                type_map = {1: '收入', 2: '支出', 3: '转账', 4: '投资'}
                if trans_type in type_map:
                    filters['type'] = type_map[trans_type]
                    logger.info("[get_reconciliation_statements] 类型筛选: %s (%d)",
                                filters['type'], trans_type)

            if keyword:
                filters['keyword'] = keyword
                logger.info("[get_reconciliation_statements] 关键词筛选: %s", keyword)

            # 查询账单
            logger.info("[get_reconciliation_statements] 查询筛选条件: %s", filters)
            bills, total = loop.run_until_complete(db.query_bills(
                page=1,
                page_size=10000,  # 获取所有匹配记录
                filters=filters,
                user_id=request.user_id
            ))

            logger.info("[get_reconciliation_statements] 查询到账单数量: %d (total=%d)", len(bills), total)
            logger.info("[get_reconciliation_statements] 原始账单列表（前5条）:")
            for i, bill in enumerate(bills[:5], 1):
                logger.info(f"  #{i}: ID={bill['id']}, Date={bill.get('date')}, "
                            f"Type={bill.get('type')}, Amount={bill.get('amount')}, "
                            f"Source={bill.get('source_account_id')}, Dest={bill.get('destination_account_id')}")

            # 重要：数据库返回的是按时间倒序（最新在前），需要反转为正序进行余额计算
            bills_sorted = sorted(bills, key=lambda b: b.get('date', ''))

            # 记录排序后的顺序
            logger.info("[get_reconciliation_statements] 排序后的账单顺序（按时间正序）:")
            for i, bill in enumerate(bills_sorted, 1):
                logger.info(f"  #{i}: ID={bill['id']}, Date={bill.get('date')}, Amount={bill.get('amount')}")

            # 计算期初余额（查询开始时间之前的所有账单余额）
            opening_balance = 0.0
            try:
                # 如果指定了开始时间，查询开始时间之前最后一笔账单
                if start_date is not None:
                    pre_filters = {
                        'end_date': start_date,
                        'account_ids': [account_id_int]
                    }
                    pre_bills, _ = loop.run_until_complete(db.query_bills(
                        page=1,
                        page_size=1,
                        filters=pre_filters,
                        user_id=request.user_id
                    ))
                    # query_bills返回的是按date DESC排序，第一条就是最晚的
                    if pre_bills and len(pre_bills) > 0:
                        # 如果有历史账单，取最后一笔的账户余额
                        opening_balance = float(pre_bills[0].get('account_balance', 0))
                        logger.info("[get_reconciliation_statements] 期初余额: %.2f (基于历史账单)",
                                    opening_balance)
                    else:
                        opening_balance = 0.0
                        logger.info("[get_reconciliation_statements] 期初余额: 0.00 (无历史账单)")
                else:
                    # 如果是查询全部（start_time=0），期初余额使用账户初始余额
                    opening_balance = float(account.get('initial_balance', 0))
                    logger.info("[get_reconciliation_statements] 期初余额: %.2f (账户初始余额)",
                                opening_balance)
            except Exception as balance_err:
                logger.warning("[get_reconciliation_statements] 获取期初余额失败: %s", balance_err)
                opening_balance = 0.0

            # 在loop内获取映射数据
            account_map = loop.run_until_complete(db.get_account_mappings())
            
            # 获取并转换分类映射格式以适配 v1_adapter
            # v1_adapter 需要 {(main, sub): cat_dict}
            # db.get_category_mappings 返回 {'id_to_category': ..., 'name_to_id': ...}
            db_category_map = loop.run_until_complete(db.get_category_mappings())
            category_map = {}
            if db_category_map and 'id_to_category' in db_category_map:
                for cat in db_category_map['id_to_category'].values():
                    category_map[(cat['main_category'], cat['sub_category'])] = cat

            # 计算统计数据
            total_inflows = 0.0  # 总收入
            total_outflows = 0.0  # 总支出
            closing_balance = opening_balance  # 期末余额 = 期初余额 + 净流入
            transactions = []

            # 第一步：按时间正序遍历，计算每笔交易的余额（包含期初和期末）
            balance_history = {}  # {bill_id: {'opening': xxx, 'closing': xxx}}
            current_balance = opening_balance  # 当前余额初始值为期初余额

            for bill in bills_sorted:
                bill_type = bill.get('type', '')
                amount = float(bill.get('amount', 0))
                source_account_id_raw = bill.get('source_account_id')
                dest_account_id_raw = bill.get('destination_account_id')

                # 安全转换账户ID
                source_acc_id = int(source_account_id_raw) if source_account_id_raw else None
                dest_acc_id = int(dest_account_id_raw) if dest_account_id_raw else None

                # 记录该交易的期初余额（交易前的余额）
                transaction_opening_balance = current_balance

                # 分类收入/支出，并更新余额
                if bill_type == '收入':
                    total_inflows += amount
                    current_balance += amount
                elif bill_type == '支出':
                    total_outflows += amount
                    current_balance -= amount
                elif bill_type == '转账':
                    # 转账需要判断是转入还是转出 - 使用账户ID比较
                    if dest_acc_id and dest_acc_id == account_id_int:
                        total_inflows += amount
                        current_balance += amount
                    elif source_acc_id and source_acc_id == account_id_int:
                        total_outflows += amount
                        current_balance -= amount
                    else:
                        logger.warning("[get_reconciliation_statements] 转账账单但账户ID不匹配: bill_id=%s", bill.get('id'))
                        continue
                elif bill_type == '投资':
                    # 投资同样需要判断是转入还是转出 - 使用账户ID比较
                    if dest_acc_id and dest_acc_id == account_id_int:
                        total_inflows += amount
                        current_balance += amount
                    elif source_acc_id and source_acc_id == account_id_int:
                        total_outflows += amount
                        current_balance -= amount
                else:
                    logger.warning("[get_reconciliation_statements] 未知账单类型: %s", bill_type)
                    continue

                # 保存该交易的期初和期末余额快照
                balance_history[bill['id']] = {
                    'opening': transaction_opening_balance,  # 交易前余额
                    'closing': current_balance  # 交易后余额
                }
                logger.info(f"[get_reconciliation_statements] ID={bill['id']}, Date={bill.get('date')}, "
                           f"Type={bill_type}, Amount={amount} → Opening={transaction_opening_balance}, Closing={current_balance}")

            # 更新最终的期末余额
            closing_balance = current_balance

            # 第二步：转换所有交易为v1格式，并关联余额
            for bill in bills_sorted:  # 使用排序后的列表，保证balance_history的bill_id对应正确
                bill_id = bill['id']

                # 跳过无效交易
                if bill_id not in balance_history:
                    continue

                # 使用adapter转换为v1格式，确保所有字段完整
                v1_transaction = loop.run_until_complete(
                    adapter.backend_to_frontend(bill, account_map, category_map)
                )

                # 添加余额字段（转换为分，前端使用accountOpeningBalance和accountClosingBalance字段）
                v1_transaction['accountOpeningBalance'] = yuan_to_cents(balance_history[bill_id]['opening'])  # 期初余额（分）
                v1_transaction['accountClosingBalance'] = yuan_to_cents(balance_history[bill_id]['closing'])  # 期末余额（分）
                transactions.append(v1_transaction)

            # 按时间倒序排序（最新交易在前）
            transactions.sort(key=lambda x: x['time'], reverse=True)

            # 计算净流入
            net_flow = total_inflows - total_outflows

            logger.info("[get_reconciliation_statements] 统计结果: 期初=%.2f, 期末=%.2f, "
                        "收入=%.2f, 支出=%.2f, 净流入=%.2f, 交易数=%d",
                        opening_balance, closing_balance, total_inflows, total_outflows,
                        net_flow, len(transactions))

            # 构建返回结果 - 将所有金额从元转换为分（前端期望的单位）
            result = {
                'accountId': str(account_id),
                'accountName': account_name,
                'startTime': start_time,
                'endTime': end_time,
                'openingBalance': yuan_to_cents(opening_balance),  # 期初余额（分）
                'closingBalance': yuan_to_cents(closing_balance),  # 期末余额（分）
                'totalInflows': yuan_to_cents(total_inflows),  # 总收入（分）
                'totalOutflows': yuan_to_cents(total_outflows),  # 总支出（分）
                'netFlow': yuan_to_cents(net_flow),  # 净流入（分）
                'transactions': transactions,
                'itemCount': len(transactions)
            }

            logger.info("[get_reconciliation_statements] 返回成功, 交易数量: %d",
                       len(transactions))

            # 记录前3笔交易的详细信息，验证accountOpeningBalance字段
            if transactions and len(transactions) > 0:
                logger.info(
                    "[get_reconciliation_statements] 前3笔交易详情（验证accountOpeningBalance）:"
                )
                for i, txn in enumerate(transactions[:3], 1):
                    logger.info(
                        f"  #{i}: ID={txn.get('id')}, "
                        f"Date={txn.get('gregorianCalendarYearDashMonthDashDay')}, "
                        f"accountOpeningBalance={txn.get('accountOpeningBalance')}, "
                        f"accountClosingBalance={txn.get('accountClosingBalance')}, "
                        f"amount={txn.get('amount')}"
                    )

            return jsonify({
                'success': True,
                'result': result
            })

        finally:
            loop.close()

    except Exception as e:
        logger.error("[get_reconciliation_statements] 获取对账单失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e),
            'message': 'Failed to retrieve reconciliation statements'
        }), 500


@bp_v1.route('/v1/transactions/move/all.json', methods=['POST'])
@log_method
@require_auth
def move_all_transactions():
    """
    移动所有交易从一个账户到另一个账户（v1兼容API）
    
    Request Body:
        {
            "fromAccountId": "源账户ID",
            "toAccountId": "目标账户ID"
        }
    
    Returns:
        {
            "success": true,
            "result": true,  # v1格式兼容
            "moved_count": 10  # 移动的交易数量
        }
    """
    try:
        data = request.get_json()
        from_account_id = data.get('fromAccountId')
        to_account_id = data.get('toAccountId')
        password = data.get('password')  # 新增：密码验证
        
        if not from_account_id or not to_account_id:
            return jsonify({
                'success': False,
                'error': 'Missing required parameters',
                'message': 'fromAccountId and toAccountId are required'
            }), 400
        
        # 新增：密码验证
        if not password:
            return jsonify({
                'success': False,
                'error': 'Missing required parameter',
                'message': 'Password is required for security verification'
            }), 400
        
        if from_account_id == to_account_id:
            return jsonify({
                'success': False,
                'error': 'Invalid parameters',
                'message': 'Source and target accounts must be different'
            }), 400
        
        # 转换为整数
        try:
            from_account_id = int(from_account_id)
            to_account_id = int(to_account_id)
        except ValueError:
            return jsonify({
                'success': False,
                'error': 'Invalid parameter type',
                'message': 'Account IDs must be valid integers'
            }), 400
        
        db, _, _, _ = get_app_context()
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            # 新增：验证密码
            password_valid = loop.run_until_complete(
                db.verify_operation_password(password)
            )
            
            if not password_valid:
                # 记录失败的审计日志
                loop.run_until_complete(
                    db.create_audit_log(
                        operation_type='move_transactions',
                        operation_target='account',
                        target_id=from_account_id,
                        details={'from_account_id': from_account_id, 'to_account_id': to_account_id},
                        status='failed',
                        error_message='Invalid password',
                        ip_address=request.remote_addr,
                        user_agent=request.headers.get('User-Agent')
                    )
                )
                
                logger.warning(f"密码验证失败: 移动账户 {from_account_id} 的交易")
                return jsonify({
                    'success': False,
                    'error': 'Invalid password',
                    'message': 'Password verification failed'
                }), 401
            
            # 执行移动操作
            result = loop.run_until_complete(
                db.move_all_transactions(from_account_id, to_account_id, user_id=request.user_id)
            )
            
            if not result.get('success'):
                # 记录失败的审计日志
                loop.run_until_complete(
                    db.create_audit_log(
                        operation_type='move_transactions',
                        operation_target='account',
                        target_id=from_account_id,
                        details={'from_account_id': from_account_id, 'to_account_id': to_account_id},
                        status='failed',
                        error_message=result.get('message'),
                        ip_address=request.remote_addr,
                        user_agent=request.headers.get('User-Agent')
                    )
                )
                
                logger.error(f"移动交易失败: {result.get('message')}")
                return jsonify({
                    'success': False,
                    'error': result.get('message', 'Failed to move transactions'),
                    'message': result.get('message', 'Failed to move transactions')
                }), 500
            
            moved_count = result.get('moved_count', 0)
            
            # 新增：记录成功的审计日志
            loop.run_until_complete(
                db.create_audit_log(
                    operation_type='move_transactions',
                    operation_target='account',
                    target_id=from_account_id,
                    details={
                        'from_account_id': from_account_id,
                        'to_account_id': to_account_id,
                        'moved_count': moved_count
                    },
                    affected_count=moved_count,
                    status='success',
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get('User-Agent')
                )
            )
            
            logger.info(f"成功移动 {moved_count} 条交易从账户 {from_account_id} 到 {to_account_id}")
            
            return jsonify({
                'success': True,
                'result': True,  # v1格式兼容
                'moved_count': moved_count
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"移动所有交易失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e),
            'message': 'Failed to move transactions'
        }), 500


@bp_v1.route('/v1/data/clear/transactions/by_account.json', methods=['POST'])
@log_method
@require_auth
def clear_all_transactions_by_account():
    """
    删除指定账户的所有交易（v1兼容API）
    
    Request Body:
        {
            "accountId": "账户ID",
            "password": "用户密码"（用于安全验证）
        }
    
    Returns:
        {
            "success": true,
            "result": true,  # v1格式兼容
            "deleted_count": 10  # 删除的交易数量
        }
    
    Note:
        此操作需要密码验证以防误操作
    """
    try:
        data = request.get_json()
        account_id = data.get('accountId')
        password = data.get('password')
        
        if not account_id:
            return jsonify({
                'success': False,
                'error': 'Missing required parameter',
                'message': 'accountId is required'
            }), 400
        
        if not password:
            return jsonify({
                'success': False,
                'error': 'Missing required parameter',
                'message': 'password is required for security verification'
            }), 400
        
        # 转换为整数
        try:
            account_id = int(account_id)
        except ValueError:
            return jsonify({
                'success': False,
                'error': 'Invalid parameter type',
                'message': 'Account ID must be a valid integer'
            }), 400
        
        db, _, _, _ = get_app_context()
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            # 验证密码
            password_valid = loop.run_until_complete(
                db.verify_operation_password(password)
            )
            
            if not password_valid:
                # 记录失败的审计日志
                loop.run_until_complete(
                    db.create_audit_log(
                        operation_type='delete_transactions',
                        operation_target='account',
                        target_id=account_id,
                        details={'account_id': account_id},
                        status='failed',
                        error_message='Invalid password',
                        ip_address=request.remote_addr,
                        user_agent=request.headers.get('User-Agent')
                    )
                )
                
                logger.warning(f"密码验证失败: 删除账户 {account_id} 的交易")
                return jsonify({
                    'success': False,
                    'error': 'Invalid password',
                    'message': 'Password verification failed'
                }), 401
            
            # 执行删除操作
            result = loop.run_until_complete(
                db.delete_all_transactions_by_account(account_id, user_id=request.user_id)
            )
            
            if not result.get('success'):
                # 记录失败的审计日志
                loop.run_until_complete(
                    db.create_audit_log(
                        operation_type='delete_transactions',
                        operation_target='account',
                        target_id=account_id,
                        details={'account_id': account_id},
                        status='failed',
                        error_message=result.get('message'),
                        ip_address=request.remote_addr,
                        user_agent=request.headers.get('User-Agent')
                    )
                )
                
                logger.error(f"删除账户交易失败: {result.get('message')}")
                return jsonify({
                    'success': False,
                    'error': result.get('message', 'Failed to delete transactions'),
                    'message': result.get('message', 'Failed to delete transactions')
                }), 500
            
            deleted_count = result.get('deleted_count', 0)
            
            # 记录成功的审计日志
            loop.run_until_complete(
                db.create_audit_log(
                    operation_type='delete_transactions',
                    operation_target='account',
                    target_id=account_id,
                    details={
                        'account_id': account_id,
                        'deleted_count': deleted_count
                    },
                    affected_count=deleted_count,
                    status='success',
                    ip_address=request.remote_addr,
                    user_agent=request.headers.get('User-Agent')
                )
            )
            
            logger.info(f"成功删除账户 {account_id} 的 {deleted_count} 条交易")
            
            return jsonify({
                'success': True,
                'result': True,  # v1格式兼容
                'deleted_count': deleted_count
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"删除账户所有交易失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e),
            'message': 'Failed to delete transactions'
        }), 500


# ==================== v6.47 三阶段导入API ====================


@bp.route('/import/v2/parse', methods=['POST'])
@log_method
@require_auth
def import_stage1_parse():
    """
    三阶段导入 - 阶段1: 解析文件
    
    支持多文件并行上传解析，将解析结果写入 bills_parser_template 表。
    
    Request:
        FormData:
            - files: 多个账单文件（支持csv, xlsx, xls, txt）
            
    Response:
        {
            'success': true,
            'data': {
                'session_id': 'uuid',
                'parsed_count': 100,
                'files': [
                    {'filename': 'xxx.csv', 'parser_type': 'wechat', 'count': 50},
                    ...
                ],
                'errors': []
            }
        }
    """
    try:
        logger.info("[阶段1-解析] 开始处理上传文件")
        
        # 检查是否有文件
        if 'files' not in request.files and 'file' not in request.files:
            logger.warning("[阶段1-解析] 未找到上传文件")
            return jsonify({
                'success': False,
                'error': 'No files provided'
            }), 400
        
        # 兼容单文件和多文件上传
        files = request.files.getlist('files') or [request.files['file']]
        
        if not files or (len(files) == 1 and files[0].filename == ''):
            logger.warning("[阶段1-解析] 文件列表为空")
            return jsonify({
                'success': False,
                'error': 'No files selected'
            }), 400
        
        # 获取服务实例
        _, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        # 保存文件到临时目录
        saved_files = []
        for file in files:
            if file.filename and allowed_file(file.filename):
                filename = secure_filename(file.filename)
                timestamp = datetime.now().strftime('%Y%m%d_%H%M%S_%f')
                unique_filename = f"{timestamp}_{filename}"
                file_path = UPLOAD_FOLDER / unique_filename
                file.save(str(file_path))
                saved_files.append({
                    'path': str(file_path),
                    'original_name': file.filename
                })
                logger.info(f"[阶段1-解析] 文件已保存: {file_path}")
            else:
                logger.warning(f"[阶段1-解析] 跳过不支持的文件: {file.filename}")
        
        if not saved_files:
            logger.error("[阶段1-解析] 没有有效的文件")
            return jsonify({
                'success': False,
                'error': 'No valid files to process'
            }), 400
        
        # 生成session_id
        session_id = str(uuid.uuid4())
        logger.info(f"[阶段1-解析] 生成会话ID: {session_id}")
        
        # 调用阶段1解析
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            file_paths = [f['path'] for f in saved_files]
            result = loop.run_until_complete(
                bill_service.import_stage1_parse(file_paths, session_id, user_id)
            )
            
            logger.info(f"[阶段1-解析] 完成: session={result.get('session_id')}, "
                        f"总数={result.get('total_parsed', 0)}")
            
            return jsonify({
                'success': result.get('success', False),
                'data': {
                    'session_id': result.get('session_id'),
                    'parsed_count': result.get('total_parsed', 0),
                    'files': result.get('file_results', []),
                    'errors': result.get('errors', [])
                }
            })
            
        finally:
            loop.close()
            # 清理临时文件
            for f in saved_files:
                try:
                    os.remove(f['path'])
                    logger.debug(f"[阶段1-解析] 临时文件已删除: {f['path']}")
                except Exception as e:
                    logger.warning(f"[阶段1-解析] 删除临时文件失败: {e}")
    
    except Exception as e:
        logger.error(f"[阶段1-解析] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/dedup', methods=['POST'])
@log_method
@require_auth
def import_stage2_dedup():
    """
    三阶段导入 - 阶段2: 去重并预览
    
    对 bills_parser_template 表中的账单进行去重处理，
    结果写入 bills_preview 表供用户确认。
    
    Request:
        JSON:
            - session_id: 导入会话ID
            
    Response:
        {
            'success': true,
            'data': {
                'session_id': 'uuid',
                'preview': [...],         # 预览账单列表
                'total': 100,             # 原始总数
                'after_dedup': 80,        # 去重后数量
                'dedup_stats': {          # 去重统计
                    'transfer_pairs': 5,
                    'platform_bank': 10,
                    'similar': 3,
                    'split_merge': 2
                }
            }
        }
    """
    try:
        logger.info("[阶段2-去重] 开始处理")
        
        data = request.get_json()
        if not data or 'session_id' not in data:
            logger.warning("[阶段2-去重] 缺少session_id")
            return jsonify({
                'success': False,
                'error': 'Missing session_id'
            }), 400
        
        session_id = data['session_id']
        
        # 获取服务实例
        _, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            # 执行阶段2去重处理
            result = loop.run_until_complete(
                bill_service.import_stage2_dedup(session_id, user_id)
            )
            
            # 获取预览数据返回给前端
            preview_data = []
            if result.get('success'):
                preview_data = loop.run_until_complete(
                    bill_service.get_import_preview(session_id, selected_only=False)
                )
            
            logger.info(f"[阶段2-去重] 完成: session={session_id}, "
                        f"原始={result.get('template_count', 0)}, "
                        f"去重后={result.get('preview_count', 0)}, "
                        f"预览数据={len(preview_data)}条")
            
            return jsonify({
                'success': result.get('success', False),
                'data': {
                    'session_id': session_id,
                    'preview': preview_data,
                    'total': result.get('template_count', 0),
                    'after_dedup': result.get('preview_count', 0),
                    'dedup_stats': result.get('dedup_stats', {}),
                    'match_stats': result.get('match_stats', {})
                }
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[阶段2-去重] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/confirm', methods=['POST'])
@log_method
@require_auth
def import_stage3_confirm():
    """
    三阶段导入 - 阶段3: 确认导入
    
    将 bills_preview 表中的账单正式写入 bills 表，
    并清理临时表数据。
    
    Request:
        JSON:
            - session_id: 导入会话ID
            - selected_ids: 可选，用户选择的预览账单ID列表（不传则全部导入）
            - preview_updates: 可选，用户编辑后的预览数据列表
            
    Response:
        {
            'success': true,
            'data': {
                'imported_count': 80,     # 导入成功数量
                'skipped_count': 0,       # 跳过数量
                'errors': []
            }
        }
    """
    try:
        logger.info("[阶段3-确认] 开始处理")
        
        data = request.get_json()
        if not data or 'session_id' not in data:
            logger.warning("[阶段3-确认] 缺少session_id")
            return jsonify({
                'success': False,
                'error': 'Missing session_id'
            }), 400
        
        session_id = data['session_id']
        selected_ids = data.get('selected_ids')  # 可选：用户选择的账单ID
        preview_updates = data.get('preview_updates')  # 可选：用户编辑后的数据
        
        # 获取服务实例
        db, bill_service, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            # v6.61: 如果有preview_updates，先重置所有选中状态，再更新选中的账单
            # 这确保只有前端传入的选中账单才会被导入，未选中的不会被导入
            if preview_updates:
                # 第一步：重置该会话所有账单的选中状态为未选中
                reset_count = loop.run_until_complete(
                    db.reset_session_preview_selection(session_id)
                )
                logger.info(f"[阶段3-确认] 已重置 {reset_count} 条账单的选中状态")
                
                # 第二步：更新前端传入的选中账单
                logger.info(f"[阶段3-确认] 更新 {len(preview_updates)} 条预览数据")
                loop.run_until_complete(
                    db.update_preview_bills_batch(session_id, preview_updates, user_id)
                )
                # 从preview_updates中提取选中的ID
                selected_ids = [u['id'] for u in preview_updates if u.get('selected', True) and u.get('id')]
                logger.info(f"[阶段3-确认] 选中的账单ID数: {len(selected_ids)}")
            
            result = loop.run_until_complete(
                bill_service.import_stage3_confirm(session_id, user_id, selected_ids)
            )
            
            logger.info(f"[阶段3-确认] 完成: session={session_id}, "
                        f"导入={result.get('imported_count', 0)}")
            
            return jsonify({
                'success': result.get('success', False),
                'data': {
                    'imported_count': result.get('imported_count', 0),
                    'skipped_count': result.get('skipped_count', 0),
                    'errors': result.get('errors', [])
                }
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[阶段3-确认] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/session/<session_id>', methods=['GET'])
@log_method
@require_auth
def get_import_session(session_id: str):
    """
    获取导入会话状态
    
    Response:
        {
            'success': true,
            'data': {
                'session_id': 'uuid',
                'status': 'parsed|deduped|confirmed|expired',
                'created_at': '2025-11-30 10:00:00',
                'parsed_count': 100,
                'preview_count': 80
            }
        }
    """
    try:
        db, _, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            session = loop.run_until_complete(db.get_import_session(session_id, user_id))
            
            if not session:
                return jsonify({
                    'success': False,
                    'error': 'Session not found or expired'
                }), 404
            
            return jsonify({
                'success': True,
                'data': {
                    'session_id': session['session_id'],
                    'status': session['status'],
                    'created_at': session['created_at'],
                    'parsed_count': session.get('parsed_count', 0),
                    'preview_count': session.get('preview_count', 0),
                    'file_paths': session.get('file_paths', '')
                }
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[获取会话] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/session/<session_id>', methods=['DELETE'])
@log_method
@require_auth
def cancel_import_session(session_id: str):
    """
    取消/清理导入会话
    
    清理 bills_parser_template 和 bills_preview 中的临时数据。
    """
    try:
        db, _, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            success = loop.run_until_complete(db.clear_session_data(session_id, user_id))
            
            return jsonify({
                'success': success,
                'message': 'Session cleared' if success else 'Session not found'
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[取消会话] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/preview/<session_id>', methods=['GET'])
@log_method
@require_auth
def get_import_preview(session_id: str):
    """
    获取预览数据（分页）
    
    Query Parameters:
        - page: 页码（默认1）
        - page_size: 每页数量（默认50）
        
    Response:
        {
            'success': true,
            'data': {
                'preview': [...],
                'total': 100,
                'page': 1,
                'page_size': 50
            }
        }
    """
    try:
        page = request.args.get('page', 1, type=int)
        page_size = request.args.get('page_size', 50, type=int)
        
        db, _, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            # 获取预览数据
            previews = loop.run_until_complete(
                db.get_preview_by_session(session_id, user_id)
            )
            
            # 分页
            total = len(previews)
            start = (page - 1) * page_size
            end = start + page_size
            page_data = previews[start:end]
            
            # 转换为前端格式
            result = []
            for preview in page_data:
                result.append({
                    'id': preview['id'],
                    'time': preview['preview_date'],
                    'type': preview['preview_type'],
                    'amount': yuan_to_cents(preview['preview_amount']),
                    'destinationAmount': yuan_to_cents(preview.get('preview_destination_amount', 0)),
                    'categoryId': str(preview.get('category_id', '')),
                    'mainCategory': preview.get('preview_main_category', ''),
                    'subCategory': preview.get('preview_sub_category', ''),
                    'sourceAccountId': str(preview.get('preview_source_account_id', '')),
                    'destinationAccountId': str(preview.get('preview_destination_account_id', '')),
                    'counterparty': preview.get('preview_counterparty', ''),
                    'paymentMethod': preview.get('preview_payment_method', ''),
                    'description': preview.get('preview_description', ''),
                    'isSelected': preview.get('is_selected', 1) == 1
                })
            
            return jsonify({
                'success': True,
                'data': {
                    'preview': result,
                    'total': total,
                    'page': page,
                    'page_size': page_size
                }
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[获取预览] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import/v2/preview/<session_id>/update', methods=['PUT'])
@log_method
@require_auth
def update_preview_bill(session_id: str):
    """
    更新预览账单（用户编辑）
    
    Request:
        JSON:
            - id: 预览账单ID
            - 其他可更新字段...
    """
    try:
        data = request.get_json()
        if not data or 'id' not in data:
            return jsonify({
                'success': False,
                'error': 'Missing bill id'
            }), 400
        
        db, _, _, _ = get_app_context()
        user_id = getattr(request, 'user_id', 1)
        
        preview_id = data['id']
        
        # 使用session_id验证预览账单归属（可选的安全检查）
        logger.debug(f"[更新预览] session_id={session_id}, preview_id={preview_id}")
        
        updates = {
            'preview_type': data.get('type'),
            'preview_amount': data.get('amount'),
            'preview_destination_amount': data.get('destinationAmount'),
            'preview_main_category': data.get('mainCategory'),
            'preview_sub_category': data.get('subCategory'),
            'preview_source_account_id': data.get('sourceAccountId'),
            'preview_destination_account_id': data.get('destinationAccountId'),
            'preview_counterparty': data.get('counterparty'),
            'preview_payment_method': data.get('paymentMethod'),
            'preview_description': data.get('description'),
            'is_selected': 1 if data.get('isSelected', True) else 0
        }
        # 过滤None值
        updates = {k: v for k, v in updates.items() if v is not None}
        
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        
        try:
            success = loop.run_until_complete(
                db.update_preview_bill(preview_id, updates, user_id)
            )
            
            return jsonify({
                'success': success
            })
            
        finally:
            loop.close()
    
    except Exception as e:
        logger.error(f"[更新预览] 失败: {e}", exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500
