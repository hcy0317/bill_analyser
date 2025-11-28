"""V1 API Adapter - ezBookkeeping v1 API格式适配器

集中管理前后端数据格式转换，遵循ezBookkeeping v1标准。
消除路由层中的重复代码，提供统一的转换接口。

Author: Bill Analyser Team
Created: 2025-11-21
"""

from datetime import datetime
from typing import Dict, Any, List, Tuple, Optional

from src.utils.currency import cents_to_yuan, yuan_to_cents
from src.utils.constants import (
    FRONTEND_TO_BACKEND_TYPE,
    BACKEND_TO_FRONTEND_TYPE,
    DEFAULT_UTC_OFFSET,
    TIMESTAMP_FORMAT_DB
)
from src.utils.logger import get_logger, log_method

logger = get_logger('V1Adapter')


class V1TransactionAdapter:
    """v1交易数据适配器"""

    def __init__(self, db=None, user_id: int = 1):
        """初始化适配器

        Args:
            db: Database实例，用于查询账户和分类映射
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        self.db = db
        self.user_id = user_id
        self.logger = get_logger('V1TransactionAdapter')

    @log_method
    def frontend_to_backend(self, frontend_data: Dict[str, Any]) -> Tuple[Dict[str, Any], Dict[str, Any]]:
        """将前端v1格式转换为后端数据库格式

        Args:
            frontend_data: ezBookkeeping v1请求格式
                {
                    "type": 3,
                    "time": "1732099200000" or 1732099200000,
                    "sourceAmount": 10050,
                    "sourceAccountId": "1",
                    "categoryId": "123",
                    ...
                }

        Returns:
            Tuple[Dict, Dict]: (backend_data, metadata)
                backend_data: 用于数据库INSERT/UPDATE的字典
                metadata: 元数据(如clientSessionId等)
        """
        self.logger.info(f"开始转换前端数据: type={frontend_data.get('type')}, "
                        f"amount={frontend_data.get('sourceAmount')}")

        backend_data = {}
        metadata = {}

        # 1. 转换交易类型: 整数 → 中文
        transaction_type = frontend_data.get('type', 3)
        backend_data['type'] = FRONTEND_TO_BACKEND_TYPE.get(transaction_type, '支出')

        # 2. 转换时间: Unix毫秒/字符串 → YYYY-MM-DD HH:MM:SS
        time_value = frontend_data.get('time')
        backend_data['date'] = self._parse_frontend_time(time_value)

        # 3. 转换金额: 分(整数) → 元(浮点数)
        source_amount = frontend_data.get('sourceAmount', 0)
        backend_data['amount'] = cents_to_yuan(source_amount)

        dest_amount = frontend_data.get('destinationAmount', 0)
        backend_data['destination_amount'] = cents_to_yuan(dest_amount)

        # 4. 转换账户ID: 字符串 → 整数
        # ⚠️ 关键修复: 保留原始ID值(包括0)，不转换为None，让后端逻辑判断
        source_account_id = frontend_data.get('sourceAccountId', '0')
        try:
            backend_data['source_account_id'] = int(source_account_id)
            self.logger.info(f"转换source_account_id: '{source_account_id}' -> {backend_data['source_account_id']}")
        except (ValueError, TypeError):
            backend_data['source_account_id'] = 0
            self.logger.warning(f"无效的source_account_id: {source_account_id}，使用默认值0")

        dest_account_id = frontend_data.get('destinationAccountId', '0')
        try:
            backend_data['destination_account_id'] = int(dest_account_id)
            self.logger.info((
                f"转换destination_account_id: '{dest_account_id}' -> "
                f"{backend_data['destination_account_id']}"
            ))
        except (ValueError, TypeError):
            backend_data['destination_account_id'] = 0
            self.logger.warning(
                f"无效的destination_account_id: {dest_account_id}，使用默认值0"
            )

        # 5. 其他字段直接映射
        backend_data['counterparty'] = frontend_data.get('counterparty', '')
        backend_data['description'] = frontend_data.get('comment', '')

        # 6. 元数据
        metadata['category_id'] = frontend_data.get('categoryId')

        # 转换标签ID: 字符串数组 → 整数数组
        tag_ids_raw = frontend_data.get('tagIds', [])
        if tag_ids_raw:
            try:
                metadata['tag_ids'] = [int(tid) for tid in tag_ids_raw if tid and tid != '0']
                self.logger.info(f"转换标签ID: {tag_ids_raw} -> {metadata['tag_ids']}")
            except (ValueError, TypeError) as e:
                self.logger.warning(f"无效的标签ID: {tag_ids_raw}, 错误: {e}")
                metadata['tag_ids'] = []
        else:
            metadata['tag_ids'] = []

        metadata['client_session_id'] = frontend_data.get('clientSessionId')
        metadata['hide_amount'] = frontend_data.get('hideAmount', False)
        metadata['utc_offset'] = frontend_data.get('utcOffset', DEFAULT_UTC_OFFSET)

        self.logger.info(f"转换完成: date={backend_data['date']}, amount={backend_data['amount']}元, tags={len(metadata['tag_ids'])}")

        return backend_data, metadata

    @log_method
    async def backend_to_frontend(self,
                                  bill: Dict[str, Any],
                                  account_map: Optional[Dict[int, Dict]] = None,
                                  category_map: Optional[Dict[Tuple[str, str], Dict]] = None,
                                  tags: List[Dict[str, Any]] = None) -> Dict[str, Any]:
        """将后端数据库格式转换为前端v1格式

        Args:
            bill: 数据库查询结果
            account_map: 账户ID→账户信息映射(可选，自动查询)
            category_map: (主分类,子分类)→分类信息映射(可选，自动查询)

        Returns:
            Dict: ezBookkeeping v1响应格式
        """
        self.logger.debug(f"转换账单ID={bill.get('id')} 到v1格式")

        # 如果没有提供映射，自动查询
        if account_map is None and self.db:
            account_map = await self._get_account_mappings()

        if category_map is None and self.db:
            category_map = await self._get_category_mappings()

        v1_bill = {}

        # 1. 基本字段
        v1_bill['id'] = str(bill['id'])

        # 2. 转换交易类型: 中文 → 整数
        bill_type = bill.get('type', '支出')
        v1_bill['type'] = BACKEND_TO_FRONTEND_TYPE.get(bill_type, 3)

        # 3. 转换时间: YYYY-MM-DD HH:MM:SS → Unix秒
        date_str = bill.get('date', '')
        v1_bill['time'] = self._parse_backend_date(date_str)
        v1_bill['timeSequenceId'] = str(v1_bill['time'])
        v1_bill['utcOffset'] = DEFAULT_UTC_OFFSET

        # 3.1 计算日期显示字段 (gregorianCalendarYearDashMonthDashDay和displayDayOfWeek)
        # 将Unix时间戳转换为日期对象
        bill_date = datetime.fromisoformat(date_str) if date_str else datetime.now()
        v1_bill['gregorianCalendarYearDashMonthDashDay'] = bill_date.strftime('%Y-%m-%d')
        v1_bill['gregorianCalendarDayOfMonth'] = bill_date.day
        # weekday(): 0=周一, 6=周日 → displayDayOfWeek: 1=周日, 2=周一, ..., 7=周六
        weekday = bill_date.weekday()
        # 确保返回int类型而非numpy.int64
        v1_bill['displayDayOfWeek'] = int(1 if weekday == 6 else weekday + 2)

        # 4. 转换金额: 元(浮点数) → 分(整数)
        v1_bill['sourceAmount'] = yuan_to_cents(bill.get('amount', 0))
        v1_bill['destinationAmount'] = yuan_to_cents(bill.get('destination_amount', 0))
        v1_bill['amount'] = v1_bill['sourceAmount']  # 别名

        # 5. 转换账户信息
        source_account_id = bill.get('source_account_id')
        v1_bill['sourceAccountId'] = str(source_account_id) if source_account_id else "0"

        if account_map and source_account_id and source_account_id in account_map:
            v1_bill['accountName'] = account_map[source_account_id]['name']
            v1_bill['accountId'] = v1_bill['sourceAccountId']  # 别名
        else:
            # 降级：source_account_id无效时使用默认值
            v1_bill['accountName'] = ''
            v1_bill['accountId'] = v1_bill['sourceAccountId']
            self.logger.warning(f"未找到账户信息: source_account_id={source_account_id}")

        dest_account_id = bill.get('destination_account_id')
        v1_bill['destinationAccountId'] = str(dest_account_id) if dest_account_id else "0"

        if account_map and dest_account_id and dest_account_id in account_map:
            v1_bill['destinationAccountName'] = account_map[dest_account_id]['name']
        else:
            v1_bill['destinationAccountName'] = ""

        # 6. 转换分类信息
        main_cat = bill.get('main_category', '')
        sub_cat = bill.get('sub_category', '')
        
        self.logger.info(f"[分类转换] bill_id={bill.get('id')}, main_cat={main_cat}, sub_cat={sub_cat}, category_map has {len(category_map)} items" if category_map else f"[分类转换] bill_id={bill.get('id')}, main_cat={main_cat}, sub_cat={sub_cat}, category_map=None")

        if category_map and (main_cat, sub_cat) in category_map:
            cat_info = category_map[(main_cat, sub_cat)]
            v1_bill['categoryId'] = str(cat_info['id'])
            v1_bill['categoryName'] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
            v1_bill['subCategoryName'] = sub_cat
            
            self.logger.info(f"[分类转换] 找到分类: id={cat_info['id']}, name={v1_bill['categoryName']}, type={cat_info.get('type')}")

            # 添加完整的category对象（供前端Transaction.of()使用）
            v1_bill['category'] = {
                'id': str(cat_info['id']),
                'name': v1_bill['categoryName'],
                'type': cat_info.get('type', 2),  # 默认为支出类型
                'parentId': str(cat_info.get('parent_id', 0)),
                'icon': cat_info.get('icon', ''),
                'color': cat_info.get('color', ''),
                'comment': cat_info.get('comment', ''),
                'displayOrder': cat_info.get('display_order', 0),
                'hidden': cat_info.get('hidden', False),
                'editable': True
            }
        else:
            v1_bill['categoryId'] = "0"
            v1_bill['categoryName'] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
            v1_bill['subCategoryName'] = sub_cat
            # 如果没有找到分类，不添加category对象
            self.logger.warning(f"[分类转换] 未找到分类映射: main_cat={main_cat}, sub_cat={sub_cat}, available keys sample: {list(category_map.keys())[:5] if category_map else 'None'}")

        # 7. 其他字段
        v1_bill['counterparty'] = bill.get('counterparty', '')
        v1_bill['comment'] = bill.get('description', '')
        v1_bill['description'] = v1_bill['comment']  # 别名

        # 8. UI相关字段
        v1_bill['editable'] = True
        v1_bill['confirmed'] = False
        v1_bill['hideAmount'] = False
        
        # 设置tagIds和tags数组
        if tags:
            v1_bill['tagIds'] = [str(t['id']) for t in tags]
            # 添加tags数组，包含id和name，供前端v-autocomplete使用
            v1_bill['tags'] = [{'id': str(t['id']), 'name': t['name']} for t in tags]
            self.logger.debug(f"转换标签: tagIds={v1_bill['tagIds']}, tags={v1_bill['tags']}")
        else:
            v1_bill['tagIds'] = []
            v1_bill['tags'] = []

        # 9. 关联账户(用于UI显示)
        v1_bill['relatedAccountId'] = v1_bill['accountId']
        v1_bill['relatedAccountName'] = v1_bill['accountName']

        return v1_bill

    @log_method
    async def backend_list_to_frontend(self,
                                      bills: List[Dict[str, Any]],
                                      total_count: int,
                                      page: int = 1,  # pylint: disable=unused-argument
                                      page_size: int = 50) -> Dict[str, Any]:  # pylint: disable=unused-argument
        """将后端账单列表转换为v1分页响应格式

        Args:
            bills: 数据库查询结果列表
            total_count: 总记录数
            page: 当前页码 (保留以保持API兼容性)
            page_size: 每页大小 (保留以保持API兼容性)

        Returns:
            Dict: ezBookkeeping v1分页响应
                {
                    "success": true,
                    "result": {
                        "items": [...],
                        "totalCount": 100,
                        "nextTimeSequenceId": 1732099199
                    }
                }
        """
        self.logger.info(f"转换账单列表: count={len(bills)}, total={total_count}")

        # 批量查询映射(一次性，避免重复查询)
        account_map = await self._get_account_mappings() if self.db else None
        category_map = await self._get_category_mappings() if self.db else None

        # 批量查询标签
        tags_map = {}
        if self.db and bills:
            bill_ids = [b['id'] for b in bills]
            tags_map = await self.db.get_tags_for_bills(bill_ids)

        # 并行转换所有账单
        v1_bills = []
        for bill in bills:
            v1_bill = await self.backend_to_frontend(bill, account_map, category_map, tags_map.get(bill['id'], []))
            v1_bills.append(v1_bill)

        # 计算nextTimeSequenceId(用于分页)
        next_time_sequence_id = None
        if v1_bills:
            last_bill = v1_bills[-1]
            next_time_sequence_id = int(last_bill['time']) - 1

        return {
            'success': True,
            'result': {
                'items': v1_bills,
                'totalCount': total_count,
                'nextTimeSequenceId': next_time_sequence_id
            }
        }

    def _parse_frontend_time(self, time_value: Any) -> str:
        """解析前端时间格式

        Args:
            time_value: Unix毫秒(int/string) 或日期字符串

        Returns:
            str: YYYY-MM-DD HH:MM:SS格式
        """
        if not time_value:
            return datetime.now().strftime(TIMESTAMP_FORMAT_DB)

        try:
            # 尝试解析为Unix毫秒时间戳
            if isinstance(time_value, str):
                timestamp_ms = int(time_value)
            else:
                timestamp_ms = int(time_value)

            # 毫秒转秒
            timestamp_s = timestamp_ms / 1000 if timestamp_ms > 10000000000 else timestamp_ms

            dt = datetime.fromtimestamp(timestamp_s)
            return dt.strftime(TIMESTAMP_FORMAT_DB)

        except (ValueError, TypeError):
            # 尝试直接解析字符串
            try:
                if isinstance(time_value, str):
                    # 尝试多种格式
                    for fmt in [TIMESTAMP_FORMAT_DB, '%Y-%m-%d', '%Y/%m/%d']:
                        try:
                            dt = datetime.strptime(time_value, fmt)
                            return dt.strftime(TIMESTAMP_FORMAT_DB)
                        except ValueError:
                            continue
            except Exception:
                pass

            # 降级：使用当前时间
            self.logger.warning(f"无法解析时间格式: {time_value}, 使用当前时间")
            return datetime.now().strftime(TIMESTAMP_FORMAT_DB)

    def _parse_backend_date(self, date_str: str) -> int:
        """解析后端日期格式

        Args:
            date_str: YYYY-MM-DD HH:MM:SS格式

        Returns:
            int: Unix秒时间戳
        """
        if not date_str:
            return int(datetime.now().timestamp())

        try:
            # 尝试多种格式
            for fmt in [TIMESTAMP_FORMAT_DB, '%Y-%m-%d']:
                try:
                    dt = datetime.strptime(date_str, fmt)
                    return int(dt.timestamp())
                except ValueError:
                    continue

            # 如果都失败，降级
            self.logger.warning(f"无法解析日期: {date_str}")
            return int(datetime.now().timestamp())

        except Exception as e:
            self.logger.error(f"日期解析异常: {date_str}, 错误: {e}")
            return int(datetime.now().timestamp())

    async def _get_account_mappings(self) -> Dict[int, Dict]:
        """获取账户映射(带缓存)

        Returns:
            Dict[int, Dict]: {account_id: {id, name, category, ...}}
        """
        if not self.db:
            return {}

        try:
            accounts = await self.db.get_all_accounts(user_id=self.user_id)
            return {acc['id']: acc for acc in accounts}
        except Exception as e:
            self.logger.error(f"查询账户映射失败: {e}")
            return {}

    async def _get_category_mappings(self) -> Dict[Tuple[str, str], Dict]:
        """获取分类映射(带缓存)

        Returns:
            Dict[Tuple[str,str], Dict]: {(main_category, sub_category): {id, ...}}
        """
        if not self.db:
            return {}

        try:
            categories = await self.db.get_all_categories(user_id=self.user_id)
            return {
                (cat['main_category'], cat['sub_category']): cat
                for cat in categories
            }
        except Exception as e:
            self.logger.error(f"查询分类映射失败: {e}")
            return {}


class V1ResponseBuilder:
    """v1响应构建器"""

    @staticmethod
    def success(data: Any, **kwargs) -> Dict[str, Any]:
        """构建成功响应

        Args:
            data: 响应数据
            **kwargs: 额外字段

        Returns:
            Dict: {"success": true, "result": data, ...}
        """
        response = {
            'success': True,
            'result': data
        }
        response.update(kwargs)
        return response

    @staticmethod
    def error(message: str, code: int = 500, **kwargs) -> Tuple[Dict[str, Any], int]:
        """构建错误响应

        Args:
            message: 错误消息
            code: HTTP状态码
            **kwargs: 额外字段

        Returns:
            Tuple[Dict, int]: ({"success": false, "error": message, ...}, code)
        """
        response = {
            'success': False,
            'error': message
        }
        response.update(kwargs)
        return response, code

    @staticmethod
    def paginated(items: List[Any],
                 total_count: int,
                 page: int = 1,  # pylint: disable=unused-argument
                 page_size: int = 50,  # pylint: disable=unused-argument
                 **kwargs) -> Dict[str, Any]:
        """构建分页响应

        Args:
            items: 数据列表
            total_count: 总记录数
            page: 当前页码 (保留以保持API兼容性)
            page_size: 每页大小 (保留以保持API兼容性)
            **kwargs: 额外字段

        Returns:
            Dict: ezBookkeeping v1分页格式
        """
        result = {
            'items': items,
            'totalCount': total_count
        }
        result.update(kwargs)

        return V1ResponseBuilder.success(result)

