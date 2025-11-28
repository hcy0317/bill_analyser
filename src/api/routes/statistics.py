"""
Statistics API Routes - 统计分析相关API端点
"""

import asyncio
import time
from datetime import datetime, timedelta
from flask import Blueprint, request, jsonify

from src.core.analyzer import Analyzer
from src.utils.logger import get_logger, log_method
from src.utils.currency import yuan_to_cents
from src.api.middleware.auth import require_auth

logger = get_logger('StatisticsAPI')

bp = Blueprint('statistics', __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app
    return current_app.config.get('DB_INSTANCE')


@bp.route('/overview', methods=['GET'])
@log_method
@require_auth
def get_overview():
    """获取概览统计"""
    try:
        period = request.args.get('period', 'month')
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')

        db = get_app_context()
        analyzer = Analyzer(db=db)

        # 构建过滤条件
        filters = {}
        if start_date:
            filters['start_date'] = start_date
        if end_date:
            filters['end_date'] = end_date

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        data = loop.run_until_complete(
            analyzer.generate_report(
                period=period,
                filters=filters if filters else None
            )
        )
        loop.close()

        # 提取summary字段到顶层以符合测试预期
        result = {
            'total_income': data.get('summary', {}).get('total_income', 0),
            'total_expense': data.get('summary', {}).get('total_expense', 0),
            'net_income': data.get('summary', {}).get('net_income', 0),
            'bill_count': data.get('total_records', 0),
            'by_category': data.get('by_category', {}),
            'by_type': data.get('by_type', {}),
            'top_income': data.get('top_income', []),
            'top_expenses': data.get('top_expenses', []),
            'period': data.get('period', ''),
            'start_date': data.get('start_date', ''),
            'end_date': data.get('end_date', ''),
        }

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("获取概览统计失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/trends', methods=['GET'])
@log_method
@require_auth
def get_trends():
    """获取趋势数据"""
    try:
        period = request.args.get('period', 'month')
        category = request.args.get('category')

        db = get_app_context()
        analyzer = Analyzer(db=db)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        trends = loop.run_until_complete(
            analyzer.get_trends(period=period, category=category)
        )
        loop.close()

        return jsonify({
            'success': True,
            'result': trends
        })

    except Exception as e:
        logger.error("获取趋势数据失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/comparison', methods=['GET'])
@log_method
@require_auth
def get_comparison():
    """获取对比数据"""
    try:
        period = request.args.get('period', 'month')
        compare_type = request.args.get('type', 'category')  # category, month, year

        db = get_app_context()
        analyzer = Analyzer(db=db)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        comparison = loop.run_until_complete(
            analyzer.get_comparison(period=period, compare_type=compare_type)
        )
        loop.close()

        return jsonify({
            'success': True,
            'result': comparison
        })

    except Exception as e:
        logger.error("获取对比数据失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/category', methods=['GET'])
@log_method
@require_auth
def get_category_analysis():
    """获取分类分析"""
    try:
        period = request.args.get('period', 'month')
        main_category = request.args.get('main_category')

        db = get_app_context()
        analyzer = Analyzer(db=db)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        analysis = loop.run_until_complete(
            analyzer.analyze_category(
                period=period,
                main_category=main_category
            )
        )
        loop.close()

        return jsonify({
            'success': True,
            'data': analysis
        })

    except Exception as e:
        logger.error("获取分类分析失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/trend', methods=['GET'])
@log_method
@require_auth
def get_trend():
    """获取趋势数据"""
    try:
        granularity = request.args.get('granularity', 'month')  # day/week/month
        category = request.args.get('category')

        db = get_app_context()
        analyzer = Analyzer(db=db)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            analyzer.get_trends(
                period=granularity,
                category=category
            )
        )
        loop.close()

        # 提取trends数组,并转换字段名以符合测试预期
        trends_data = result.get('trends', [])
        formatted_trends = []
        for trend in trends_data:
            formatted_trends.append({
                'date': trend.get('period', ''),
                'income': trend.get('income', 0),
                'expense': trend.get('expense', 0),
                'net': trend.get('net', 0)
            })

        return jsonify({
            'success': True,
            'data': formatted_trends
        })

    except Exception as e:
        logger.error("获取趋势数据失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/category-pie', methods=['GET'])
@log_method
@require_auth
def get_category_pie():
    """获取分类饼图数据"""
    try:
        bill_type = request.args.get('type', '支出')  # 支出/收入
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')

        db = get_app_context()

        # 构建查询条件
        filters = {'type': bill_type}
        if start_date:
            filters['start_date'] = start_date
        if end_date:
            filters['end_date'] = end_date

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bills, _ = loop.run_until_complete(
            db.query_bills(page=1, page_size=100000, filters=filters, user_id=request.user_id)
        )
        loop.close()

        # 按分类汇总
        category_totals = {}
        for bill in bills:
            category = bill.get('main_category', '未分类')
            amount = abs(float(bill.get('amount', 0)))
            category_totals[category] = category_totals.get(category, 0) + amount

        # 转换为饼图数据格式
        pie_data = [
            {'name': category, 'value': amount}
            for category, amount in category_totals.items()
        ]

        # 按金额降序排序
        pie_data.sort(key=lambda x: x['value'], reverse=True)

        return jsonify({
            'success': True,
            'data': pie_data
        })

    except Exception as e:
        logger.error("获取分类饼图数据失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/top-merchants', methods=['GET'])
@log_method
@require_auth
def get_top_merchants():
    """获取TOP商家"""
    try:
        limit = int(request.args.get('limit', 10))
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')

        db = get_app_context()

        # 构建查询条件
        filters = {}
        if start_date:
            filters['start_date'] = start_date
        if end_date:
            filters['end_date'] = end_date

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bills, _ = loop.run_until_complete(
            db.query_bills(page=1, page_size=100000, filters=filters, user_id=request.user_id)
        )
        loop.close()

        # 按商家汇总
        merchant_stats = {}
        for bill in bills:
            merchant = bill.get('counterparty', '未知商家')
            amount = abs(float(bill.get('amount', 0)))

            if merchant not in merchant_stats:
                merchant_stats[merchant] = {'amount': 0, 'count': 0}

            merchant_stats[merchant]['amount'] += amount
            merchant_stats[merchant]['count'] += 1

        # 转换为列表并排序
        top_merchants = [
            {
                'name': merchant,
                'amount': stats['amount'],
                'count': stats['count']
            }
            for merchant, stats in merchant_stats.items()
        ]

        # 按金额降序排序并限制数量
        top_merchants.sort(key=lambda x: x['amount'], reverse=True)
        top_merchants = top_merchants[:limit]

        return jsonify({
            'success': True,
            'data': top_merchants
        })

    except Exception as e:
        logger.error("获取TOP商家失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/amounts', methods=['GET'])
@log_method
@require_auth
def get_transaction_amounts():
    """
    获取多个时间段的交易金额统计

    Query参数:
        query: 时间段查询字符串，格式: "period1_start_end|period2_start_end"
        use_transaction_timezone: 是否使用交易时区 (可选)

    Returns:
        JSON响应，包含各时间段的收入/支出/净额统计
    """
    try:
        logger.info("Amounts API called with args: %s", request.args)
        query_str = request.args.get('query', '')
        if not query_str:
            # 尝试兼容旧版API参数 'periods'
            query_str = request.args.get('periods', '')

        # use_transaction_timezone = request.args.get('use_transaction_timezone', 'false').lower() == 'true'  # noqa: E501 # pylint: disable=line-too-long

        if not query_str:
            return jsonify({
                'success': False,
                'error': 'Missing query parameter'
            }), 400

        db = get_app_context()
        results = {}

        # 解析查询字符串: "today_1763481600_1763567999|thisWeek_1763308800_1763913599"
        for period_query in query_str.split('|'):
            parts = period_query.split('_')
            if len(parts) != 3:
                continue

            period_name, start_timestamp, end_timestamp = parts

            # 转换时间戳为日期字符串
            start_date = datetime.fromtimestamp(int(start_timestamp)).strftime('%Y-%m-%d')
            end_date = datetime.fromtimestamp(int(end_timestamp)).strftime('%Y-%m-%d')

            # 查询该时间段的账单
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            bills, _ = loop.run_until_complete(
                db.query_bills(
                    page=1,
                    page_size=100000,
                    filters={
                        'start_date': start_date,
                        'end_date': end_date
                    },
                    user_id=request.user_id
                )
            )
            loop.close()

            # 计算统计数据（单位：元）
            total_income = sum(
                abs(float(bill.get('amount', 0)))
                for bill in bills
                if bill.get('type') == '收入'
            )
            total_expense = sum(
                abs(float(bill.get('amount', 0)))
                for bill in bills
                if bill.get('type') == '支出'
            )

            # 转换为前端期望的单位（分）
            # 前端使用整数分作为金额单位，避免浮点数精度问题
            income_cents = yuan_to_cents(total_income)
            expense_cents = yuan_to_cents(total_expense)

            # 构造前端期望的数据格式
            # 前端期望: { startTime, endTime, amounts: [{ currency, incomeAmount, expenseAmount }] }
            results[period_name] = {
                'startTime': int(start_timestamp),
                'endTime': int(end_timestamp),
                'amounts': [{
                    'currency': 'CNY',  # 默认人民币，后续可以支持多货币
                    'incomeAmount': income_cents,    # 单位：分(cents)
                    'expenseAmount': expense_cents   # 单位：分(cents)
                }]
            }

        return jsonify({
            'success': True,
            'result': results
        })

    except Exception as e:
        logger.error("获取交易金额统计失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/exchange-rates', methods=['GET'])
@log_method
@require_auth
def get_exchange_rates():
    """
    获取最新汇率数据

    Query Parameters:
        base_currency: 基准货币代码，默认为CNY

    Returns:
        JSON响应，包含最新汇率信息

    Response Format:
        {
            "success": true,
            "result": {
                "dataSource": "内置汇率数据",
                "referenceUrl": "",
                "updateTime": 1700000000,
                "baseCurrency": "CNY",
                "exchangeRates": [
                    {"currency": "USD", "rate": "0.139"},
                    {"currency": "EUR", "rate": "0.128"}
                ]
            }
        }
    """
    try:
        logger.info("开始处理汇率数据请求")

        # 获取请求参数
        base_currency = request.args.get('base_currency', 'CNY')
        logger.debug("请求参数: base_currency=%s", base_currency)

        # 获取当前时间戳
        update_time = int(time.time())
        update_date = datetime.now().strftime('%Y-%m-%d')

        logger.debug("生成汇率数据: 时间=%s, 时间戳=%d", update_date, update_time)

        # 构建汇率数据（以CNY为基准）
        # 这些是示例汇率，实际应该从外部API获取
        cny_based_rates = {
            'USD': 0.139,   # 1 CNY = 0.139 USD (约7.2 CNY/USD)
            'EUR': 0.128,   # 1 CNY = 0.128 EUR
            'GBP': 0.110,   # 1 CNY = 0.110 GBP
            'JPY': 20.76,   # 1 CNY = 20.76 JPY
            'HKD': 1.087,   # 1 CNY = 1.087 HKD
            'KRW': 183.33,  # 1 CNY = 183.33 KRW
            'AUD': 0.211,   # 1 CNY = 0.211 AUD
            'CAD': 0.189,   # 1 CNY = 0.189 CAD
            'SGD': 0.186,   # 1 CNY = 0.186 SGD
            'TWD': 4.35,    # 1 CNY = 4.35 TWD
            'MYR': 0.646,   # 1 CNY = 0.646 MYR
            'THB': 4.86,    # 1 CNY = 4.86 THB
            'VND': 3425.0,  # 1 CNY = 3425 VND
        }

        # 如果基准货币不是CNY，需要转换汇率
        exchange_rates_list = []

        if base_currency == 'CNY':
            # 直接使用CNY基准的汇率
            for currency, rate in cny_based_rates.items():
                exchange_rates_list.append({
                    'currency': currency,
                    'rate': str(round(rate, 6))
                })
            logger.info("使用CNY基准汇率，生成%d条汇率数据", len(exchange_rates_list))

        elif base_currency in cny_based_rates:
            # 转换为其他货币为基准
            base_rate = cny_based_rates[base_currency]

            # 添加CNY汇率（倒数）
            exchange_rates_list.append({
                'currency': 'CNY',
                'rate': str(round(1.0 / base_rate, 6))
            })

            # 添加其他货币汇率（交叉汇率）
            for currency, rate in cny_based_rates.items():
                if currency != base_currency:
                    # 交叉汇率 = 目标货币对CNY汇率 / 基准货币对CNY汇率
                    cross_rate = rate / base_rate
                    exchange_rates_list.append({
                        'currency': currency,
                        'rate': str(round(cross_rate, 6))
                    })

            logger.info("转换为%s基准汇率，生成%d条汇率数据",
                       base_currency, len(exchange_rates_list))
        else:
            # 不支持的基准货币，返回空列表
            logger.warning("不支持的基准货币: %s，返回空汇率数据", base_currency)

        # 构建响应数据（前端期望result字段，不是data字段）
        result = {
            'dataSource': '内置汇率数据',
            'referenceUrl': '',
            'updateTime': update_time,
            'baseCurrency': base_currency,
            'exchangeRates': exchange_rates_list
        }

        logger.info("成功生成汇率数据: 基准=%s, 汇率数=%d, 时间戳=%d",
                   base_currency, len(exchange_rates_list), update_time)
        logger.debug("返回数据: %s", result)

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("获取汇率数据失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


# ==================== V1 统计分析API (ezBookkeeping兼容) ====================

bp_v1 = Blueprint('statistics_v1', __name__)


@bp_v1.route('/v1/transactions/statistics.json', methods=['GET'])
@log_method
@require_auth
def get_categorical_analysis():
    """
    分类分析API - 按分类汇总收支统计

    Query Parameters:
        startTime: 开始时间戳（秒）
        endTime: 结束时间戳（秒）
        tagIds: 标签ID列表（逗号分隔，可选）
        tagFilterType: 标签过滤类型（可选）
        keyword: 关键词搜索（可选）
        useTransactionTimezone: 是否使用交易时区（可选）

    Returns:
        JSON响应格式:
        {
            "success": true,
            "result": {
                "startTime": 1732204800,
                "endTime": 1732291199,
                "items": [
                    {
                        "categoryId": "1",
                        "accountId": "214",
                        "amount": 11100  // 单位：分(cents)
                    }
                ]
            }
        }
    """
    try:
        logger.info("[分类分析] API调用: %s", request.args)
        logger.info("[分类分析] 完整请求: method=%s, path=%s, args=%s",
                   request.method, request.path, dict(request.args))

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_time = request.args.get('startTime') or request.args.get('start_time')
        end_time = request.args.get('endTime') or request.args.get('end_time')
        keyword = request.args.get('keyword', '')
        # tag_ids = request.args.get('tagIds', '')
        # tag_filter_type = request.args.get('tagFilterType', type=int)
        # use_transaction_timezone = request.args.get('useTransactionTimezone', 'false').lower() == 'true'  # noqa: E501 # pylint: disable=line-too-long

        logger.info("[分类分析] 解析参数: start_time=%s, end_time=%s, keyword=%s",
                   start_time, end_time, keyword)

        # 如果缺少时间参数,使用本月作为默认范围
        if start_time is None or end_time is None:
            now = datetime.now()
            # 本月第一天00:00:00
            month_start = datetime(now.year, now.month, 1)
            # 下月第一天00:00:00
            if now.month == 12:
                month_end = datetime(now.year + 1, 1, 1)
            else:
                month_end = datetime(now.year, now.month + 1, 1)

            start_time = int(month_start.timestamp())
            end_time = int(month_end.timestamp()) - 1  # 本月最后一秒

            logger.warning("[分类分析] 缺少时间参数,使用本月作为默认范围: start_time=%d, end_time=%d (%s ~ %s)",
                        start_time, end_time, month_start.strftime('%Y-%m-%d'), month_end.strftime('%Y-%m-%d'))

        # 转换为整数
        try:
            start_time = int(start_time)
            end_time = int(end_time)
        except (ValueError, TypeError) as e:
            logger.error("[分类分析] 时间戳格式错误: %s", e)
            return jsonify({
                'success': False,
                'error': f'Invalid timestamp format: {e}'
            }), 400

        # 转换时间戳为日期字符串
        start_date = datetime.fromtimestamp(start_time).strftime('%Y-%m-%d')
        end_date = datetime.fromtimestamp(end_time).strftime('%Y-%m-%d')

        logger.info("[分类分析] 查询时间范围: %s 到 %s", start_date, end_date)

        db = get_app_context()

        # 构建查询过滤条件
        filters = {
            'start_date': start_date,
            'end_date': end_date
        }

        if keyword:
            filters['keyword'] = keyword
            logger.info("[分类分析] 关键词筛选: %s", keyword)

        # 查询账单数据
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            bills, _ = loop.run_until_complete(
                db.query_bills(page=1, page_size=100000, filters=filters, user_id=request.user_id)
            )
            logger.info("[分类分析] 查询到 %d 条账单", len(bills))

            # 记录查询到的账单详情（前3条示例）
            for i, bill in enumerate(bills[:3]):
                logger.debug("[分类分析] 账单示例%d: ID=%s, 日期=%s, 类型=%s, 金额=%s, 渠道=%s, 分类=%s-%s",
                           i+1, bill.get('id'), bill.get('date'), bill.get('type'),
                           bill.get('amount'), bill.get('channel'),
                           bill.get('main_category'), bill.get('sub_category'))

            # 获取所有分类和账户映射（用于ID转换）
            categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            logger.debug("[分类分析] 分类列表: %d个，前3个: %s",
                        len(categories),
                        [(c.get('id'), c.get('main_category'), c.get('sub_category')) for c in categories[:3]])
            logger.debug("[分类分析] 账户列表: %d个，前3个: %s",
                        len(accounts),
                        [(a.get('id'), a.get('name')) for a in accounts[:3]])
        finally:
            loop.close()

        # 构建分类和账户名称到ID的映射
        category_name_to_id = {}
        for cat in categories:
            main_cat = cat.get('main_category', '')
            sub_cat = cat.get('sub_category', '')
            key = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
            category_name_to_id[key] = str(cat['id'])

        account_name_to_id = {}
        valid_account_ids = set()
        for acc in accounts:
            account_name_to_id[acc['name']] = str(acc['id'])
            valid_account_ids.add(str(acc['id']))

        logger.debug("[分类分析] 分类映射: %d 个, 账户映射: %d 个",
                    len(category_name_to_id), len(account_name_to_id))

        # 按 (分类ID, 账户ID) 汇总金额
        statistics_map = {}

        logger.info("[分类分析] 开始处理 %d 条账单进行统计汇总...", len(bills))

        for idx, bill in enumerate(bills):
            # 获取分类ID
            main_category = bill.get('main_category', '未分类')
            sub_category = bill.get('sub_category', '')
            cat_key = f"{main_category}-{sub_category}" if sub_category else main_category
            category_id = category_name_to_id.get(cat_key, '0')

            # 获取账户ID
            account_name = bill.get('channel', '未知账户')
            account_id = str(bill.get('source_account_id', 0))

            if account_id not in valid_account_ids:
                account_id = account_name_to_id.get(account_name, '0')

            # 计算金额（分为单位）
            amount_yuan = float(bill.get('amount', 0))
            amount_cents = yuan_to_cents(amount_yuan)

            # 按交易类型处理符号
            bill_type = bill.get('type', '')
            if bill_type == '支出':
                amount_cents = -abs(amount_cents)  # 支出为负数
            elif bill_type == '收入':
                amount_cents = abs(amount_cents)   # 收入为正数
            elif bill_type == '转账':
                # 转账根据账户判断方向
                dest_account = bill.get('destination_account', '')
                if dest_account and dest_account != account_name:
                    amount_cents = -abs(amount_cents)  # 转出
                else:
                    amount_cents = abs(amount_cents)   # 转入

            # 汇总到统计字典
            key = (category_id, account_id)
            if key not in statistics_map:
                statistics_map[key] = 0
            statistics_map[key] += amount_cents

            # 记录前3条账单的处理详情
            if idx < 3:
                logger.debug("[分类分析] 处理账单%d: 类型=%s, 原始金额=%.2f元, 转换为%d分, 分类ID=%s, 账户ID=%s, 累计=%d分",
                           idx+1, bill_type, amount_yuan, amount_cents, category_id, account_id, statistics_map[key])

        # 转换为前端期望的数组格式
        result_items = [
            {
                'categoryId': cat_id,
                'accountId': acc_id,
                'amount': amount
            }
            for (cat_id, acc_id), amount in statistics_map.items()
        ]

        logger.info("[分类分析] 生成 %d 条统计记录", len(result_items))

        # 记录统计结果详情（前3条）
        for i, item in enumerate(result_items[:3]):
            logger.debug("[分类分析] 统计结果%d: 分类ID=%s, 账户ID=%s, 金额=%d分 (%.2f元)",
                       i+1, item['categoryId'], item['accountId'], item['amount'], item['amount']/100.0)

        logger.info("[分类分析] ✅ 返回结果: startTime=%d, endTime=%d, items=%d条",
                   start_time, end_time, len(result_items))

        return jsonify({
            'success': True,
            'result': {
                'startTime': start_time,
                'endTime': end_time,
                'items': result_items
            }
        })

    except Exception as e:
        logger.error("[分类分析] 失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/transactions/statistics/trends.json', methods=['GET'])
@log_method
@require_auth
def get_trend_analysis():
    """
    趋势分析API - 按年月分组的分类统计

    Query Parameters:
        startYearMonth: 开始年月（格式: 202411）
        endYearMonth: 结束年月（格式: 202412）
        tagIds: 标签ID列表（逗号分隔，可选）
        tagFilterType: 标签过滤类型（可选）
        keyword: 关键词搜索（可选）
        useTransactionTimezone: 是否使用交易时区（可选）

    Returns:
        JSON响应格式:
        {
            "success": true,
            "result": [
                {
                    "year": 2024,
                    "month": 11,
                    "items": [
                        {
                            "categoryId": "1",
                            "accountId": "214",
                            "amount": 11100
                        }
                    ]
                }
            ]
        }
    """
    try:
        logger.info("[趋势分析] API调用: %s", request.args)
        logger.info("[趋势分析] 完整请求: method=%s, path=%s, args=%s",
                   request.method, request.path, dict(request.args))

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_year_month = (request.args.get('startYearMonth') or
                           request.args.get('start_year_month') or '')
        end_year_month = (request.args.get('endYearMonth') or
                         request.args.get('end_year_month') or '')
        keyword = request.args.get('keyword', '')

        logger.info("[趋势分析] 解析参数: start_year_month=%s, end_year_month=%s, keyword=%s",
                   start_year_month, end_year_month, keyword)

        if not start_year_month or not end_year_month:
            now = datetime.now()
            start_year_month = f"{now.year}01"  # 本年1月
            end_year_month = f"{now.year}12"    # 本年12月

            logger.warning("[趋势分析] 缺少年月参数,使用本年作为默认范围: start_year_month=%s, end_year_month=%s",
                        start_year_month, end_year_month)

        # 解析年月字符串（支持两种格式: 202411 或 2024-11）
        try:
            # 去除连字符
            start_year_month_clean = start_year_month.replace('-', '')
            end_year_month_clean = end_year_month.replace('-', '')

            logger.info("[趋势分析] 清理后的年月: start=%s, end=%s",
                       start_year_month_clean, end_year_month_clean)

            start_year = int(start_year_month_clean[:4])
            start_month = int(start_year_month_clean[4:6])
            end_year = int(end_year_month_clean[:4])
            end_month = int(end_year_month_clean[4:6])

            logger.info("[趋势分析] 解析年月: start=%d-%02d, end=%d-%02d",
                       start_year, start_month, end_year, end_month)
        except (ValueError, IndexError) as e:
            logger.error("[趋势分析] 年月格式错误: %s", e)
            return jsonify({
                'success': False,
                'error': f'Invalid year-month format (expected: 202411 or 2024-11): {e}'
            }), 400

        start_date = f"{start_year}-{start_month:02d}-01"
        # 计算结束日期（月末最后一天）
        from calendar import monthrange
        _, last_day = monthrange(end_year, end_month)
        end_date = f"{end_year}-{end_month:02d}-{last_day}"

        logger.info("[趋势分析] 查询时间范围: %s 到 %s", start_date, end_date)

        db = get_app_context()

        # 构建查询过滤条件
        filters = {
            'start_date': start_date,
            'end_date': end_date
        }

        if keyword:
            filters['keyword'] = keyword

        # 查询账单数据
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            bills, _ = loop.run_until_complete(
                db.query_bills(page=1, page_size=100000, filters=filters, user_id=request.user_id)
            )
            logger.info("[趋势分析] 查询到 %d 条账单", len(bills))

            # 记录账单日期分布
            if bills:
                dates = [b.get('date', '')[:10] for b in bills if b.get('date')]
                logger.debug("[趋势分析] 账单日期范围: %s 到 %s",
                           min(dates) if dates else 'N/A',
                           max(dates) if dates else 'N/A')
                # 前3条账单示例
                for i, bill in enumerate(bills[:3]):
                    logger.debug("[趋势分析] 账单示例%d: ID=%s, 日期=%s, 类型=%s, 金额=%s",
                               i+1, bill.get('id'), bill.get('date'), bill.get('type'), bill.get('amount'))

            # 获取映射数据
            categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            logger.debug("[趋势分析] 映射数据: 分类=%d个, 账户=%d个", len(categories), len(accounts))
        finally:
            loop.close()

        # 构建映射
        category_name_to_id = {}
        for cat in categories:
            main_cat = cat.get('main_category', '')
            sub_cat = cat.get('sub_category', '')
            key = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
            category_name_to_id[key] = str(cat['id'])

        account_name_to_id = {}
        valid_account_ids = set()
        for acc in accounts:
            account_name_to_id[acc['name']] = str(acc['id'])
            valid_account_ids.add(str(acc['id']))

        # 按年月分组统计
        # 结构: {(year, month): {(category_id, account_id): amount}}
        monthly_stats = {}

        for bill in bills:
            # 解析账单日期
            bill_date_str = bill.get('date', '')
            if not bill_date_str:
                continue

            try:
                bill_date = datetime.fromisoformat(bill_date_str.replace('Z', '+00:00'))
                year = bill_date.year
                month = bill_date.month
            except (ValueError, AttributeError):
                continue

            # 获取分类和账户ID
            main_category = bill.get('main_category', '未分类')
            sub_category = bill.get('sub_category', '')
            cat_key = f"{main_category}-{sub_category}" if sub_category else main_category
            category_id = category_name_to_id.get(cat_key, '0')

            account_name = bill.get('channel', '未知账户')
            account_id = str(bill.get('source_account_id', 0))

            if account_id not in valid_account_ids:
                account_id = account_name_to_id.get(account_name, '0')

            # 计算金额
            amount_yuan = float(bill.get('amount', 0))
            amount_cents = yuan_to_cents(amount_yuan)

            bill_type = bill.get('type', '')
            if bill_type == '支出':
                amount_cents = -abs(amount_cents)
            elif bill_type == '收入':
                amount_cents = abs(amount_cents)
            elif bill_type == '转账':
                dest_account = bill.get('destination_account', '')
                if dest_account and dest_account != account_name:
                    amount_cents = -abs(amount_cents)
                else:
                    amount_cents = abs(amount_cents)

            # 汇总统计
            month_key = (year, month)
            if month_key not in monthly_stats:
                monthly_stats[month_key] = {}

            stat_key = (category_id, account_id)
            if stat_key not in monthly_stats[month_key]:
                monthly_stats[month_key][stat_key] = 0
            monthly_stats[month_key][stat_key] += amount_cents

        # 转换为前端期望的数组格式
        result = []

        logger.info("[趋势分析] 月度统计: 共处理 %d 个月的数据，有数据的月份: %d 个",
                   (end_year - start_year) * 12 + (end_month - start_month) + 1,
                   len(monthly_stats))
        logger.debug("[趋势分析] 有数据的月份: %s", list(monthly_stats.keys()))

        # 遍历所有年月（包括没有数据的月份）
        current_year, current_month = start_year, start_month
        while (current_year, current_month) <= (end_year, end_month):
            month_key = (current_year, current_month)
            month_data = monthly_stats.get(month_key, {})

            items = [
                {
                    'categoryId': cat_id,
                    'accountId': acc_id,
                    'amount': amount
                }
                for (cat_id, acc_id), amount in month_data.items()
            ]

            result.append({
                'year': current_year,
                'month': current_month,
                'items': items
            })

            # 递增月份
            current_month += 1
            if current_month > 12:
                current_month = 1
                current_year += 1

        logger.info("[趋势分析] 生成 %d 个月份的统计数据", len(result))

        # 记录结果详情（前3个月）
        for i, month_data in enumerate(result[:3]):
            logger.debug("[趋势分析] 月份%d: %d年%d月, 统计项=%d个",
                       i+1, month_data['year'], month_data['month'], len(month_data['items']))
            if month_data['items']:
                # 计算该月总收入和总支出
                total_income = sum(item['amount'] for item in month_data['items'] if item['amount'] > 0)
                total_expense = sum(abs(item['amount']) for item in month_data['items'] if item['amount'] < 0)
                logger.debug("[趋势分析]   该月: 收入=%.2f元, 支出=%.2f元",
                           total_income/100.0, total_expense/100.0)

        logger.info("[趋势分析] ✅ 返回结果: %d个月的数据", len(result))

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("[趋势分析] 失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/transactions/statistics/asset_trends.json', methods=['GET'])
@log_method
@require_auth
def get_asset_trends():
    """
    资产趋势API - 每日账户余额变化统计

    Query Parameters:
        startTime: 开始时间戳（秒）
        endTime: 结束时间戳（秒）

    Returns:
        JSON响应格式:
        {
            "success": true,
            "result": [
                {
                    "year": 2024,
                    "month": 11,
                    "day": 21,
                    "items": [
                        {
                            "accountId": "214",
                            "accountOpeningBalance": 0,
                            "accountClosingBalance": 11100
                        }
                    ]
                }
            ]
        }
    """
    try:
        logger.info("[资产趋势] API调用: %s", request.args)
        logger.info("[资产趋势] 完整请求: method=%s, path=%s, args=%s",
                   request.method, request.path, dict(request.args))

        # 解析请求参数（支持驼峰和下划线两种命名）
        start_time = request.args.get('startTime') or request.args.get('start_time')
        end_time = request.args.get('endTime') or request.args.get('end_time')

        logger.info("[资产趋势] 解析参数: start_time=%s, end_time=%s",
                   start_time, end_time)

        if start_time is None or end_time is None:
            logger.error("[资产趋势] 缺少必需参数: start_time=%s, end_time=%s",
                        start_time, end_time)
            return jsonify({
                'success': False,
                'error': 'Missing required parameters: startTime/start_time, endTime/end_time'
            }), 400

        # 转换为整数
        try:
            start_time = int(start_time)
            end_time = int(end_time)
        except (ValueError, TypeError) as e:
            logger.error("[资产趋势] 时间戳格式错误: %s", e)
            return jsonify({
                'success': False,
                'error': f'Invalid timestamp format: {e}'
            }), 400

        # 【关键】验证时间范围，防止无限循环（限制最多365天，支持本年查询）
        time_diff_seconds = end_time - start_time
        time_diff_days = time_diff_seconds // 86400
        logger.info("[资产趋势] 时间跨度: %d天", time_diff_days)

        if time_diff_days > 365:
            logger.warning("[资产趋势] 时间跨度超过365天限制，拒绝请求: %d天", time_diff_days)
            error_msg = '资产趋势查询最多支持365天范围，请缩小时间范围'
            return jsonify({
                'success': False,
                'errorMessage': error_msg,
                'error': error_msg,  # 兼容前端error字段
                'errorCode': 400
            }), 400

        # 转换时间戳为日期
        start_date = datetime.fromtimestamp(start_time)
        end_date = datetime.fromtimestamp(end_time)

        logger.info("[资产趋势] 查询时间范围: %s 到 %s",
                   start_date.strftime('%Y-%m-%d'), end_date.strftime('%Y-%m-%d'))

        db = get_app_context()

        # 优化：一次性查询所有数据，避免N+1查询
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 1. 获取所有账户
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))
            logger.info("[资产趋势] 查询到 %d 个账户", len(accounts))

            # 2. 获取起始日期前的所有账户余额（期初余额）
            start_date_str = start_date.strftime('%Y-%m-%d')
            initial_balances = loop.run_until_complete(db.get_balances_before_date(start_date_str, user_id=request.user_id))
            logger.info("[资产趋势] 已计算期初余额")

            # 3. 获取时间范围内的所有账单
            end_date_str = end_date.strftime('%Y-%m-%d')
            bills_in_range, _ = loop.run_until_complete(db.query_bills(
                page=1,
                page_size=1000000,  # 足够大的数量以获取所有账单
                filters={
                    'start_date': start_date_str,
                    'end_date': end_date_str
                },
                user_id=request.user_id
            ))
            logger.info("[资产趋势] 查询到范围内 %d 条账单", len(bills_in_range))

        finally:
            loop.close()

        # 初始化当前余额（账户初始余额 + 历史累计余额）
        current_balances = {}
        for acc in accounts:
            acc_id = acc['id']
            # 账户创建时的初始余额
            acc_initial = float(acc.get('initial_balance', 0.0))
            # 历史累计余额
            history_balance = initial_balances.get(acc_id, 0.0)
            current_balances[acc_id] = acc_initial + history_balance

        # 按日期分组账单
        bills_by_date = {}
        for bill in bills_in_range:
            # 截取日期部分 YYYY-MM-DD
            date_str = bill.get('date', '')[:10]
            if not date_str:
                continue
            if date_str not in bills_by_date:
                bills_by_date[date_str] = []
            bills_by_date[date_str].append(bill)

        # 按日期遍历，计算每日余额
        result = []
        current_date = start_date
        day_count = 0

        while current_date <= end_date:
            day_count += 1
            date_str = current_date.strftime('%Y-%m-%d')

            if day_count <= 3 or day_count % 10 == 0:
                logger.info("[资产趋势] 计算第%d天: %s", day_count, date_str)

            # 获取当天的账单
            day_bills = bills_by_date.get(date_str, [])

            # 保存当天的期初余额快照
            opening_balances = current_balances.copy()

            # 根据当天账单更新余额
            for bill in day_bills:
                amount = float(bill.get('amount', 0))
                bill_type = bill.get('type', '')
                source_id = bill.get('source_account_id', 0)
                dest_id = bill.get('destination_account_id', 0)
                dest_amount = float(bill.get('destination_amount', 0))
                if dest_amount == 0:
                    dest_amount = amount

                if bill_type == '收入':
                    if source_id in current_balances:
                        current_balances[source_id] += amount
                elif bill_type == '支出':
                    if source_id in current_balances:
                        current_balances[source_id] -= abs(amount)
                elif bill_type == '转账':
                    if source_id in current_balances:
                        current_balances[source_id] -= abs(amount)
                    if dest_id in current_balances:
                        current_balances[dest_id] += abs(dest_amount)

            # 生成当天的结果项
            day_items = []
            for acc in accounts:
                acc_id = acc['id']
                opening = opening_balances.get(acc_id, 0.0)
                closing = current_balances.get(acc_id, 0.0)

                day_items.append({
                    'accountId': str(acc_id),
                    'accountOpeningBalance': yuan_to_cents(opening),
                    'accountClosingBalance': yuan_to_cents(closing)
                })

            result.append({
                'year': current_date.year,
                'month': current_date.month,
                'day': current_date.day,
                'items': day_items
            })

            current_date += timedelta(days=1)

        logger.info("[资产趋势] 生成 %d 天的资产数据", len(result))

        # 统计结果概要
        if result:
            total_accounts_per_day = len(result[0]['items']) if result[0]['items'] else 0
            logger.info("[资产趋势] 结果统计: 天数=%d, 每天账户数=%d", len(result), total_accounts_per_day)

            # 记录第一天的详情
            first_day = result[0]
            logger.debug("[资产趋势] 第一天(%d-%02d-%02d)前3个账户:",
                       first_day['year'], first_day['month'], first_day['day'])
            for i, item in enumerate(first_day['items'][:3]):
                logger.debug("[资产趋势]   账户%d: ID=%s, 期初=%.2f元, 期末=%.2f元",
                           i+1, item['accountId'],
                           item['accountOpeningBalance']/100.0,
                           item['accountClosingBalance']/100.0)

        logger.info("[资产趋势] ✅ 返回结果: %d天的数据", len(result))

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("[资产趋势] 失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500
