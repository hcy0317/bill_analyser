"""Neutral transaction adapter."""

from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple

from src.api.adapters.account_adapter import AccountAdapter
from src.api.adapters.category_adapter import CategoryAdapter
from src.utils.constants import BACKEND_TO_FRONTEND_TYPE, FRONTEND_TO_BACKEND_TYPE
from src.utils.currency import cents_to_yuan, yuan_to_cents


class ResponseBuilder:
    """构建常见事务响应。"""

    @staticmethod
    def build_page(items: List[Dict[str, Any]], total: int, page: int, page_size: int) -> Dict[str, Any]:
        return {
            'success': True,
            'result': {
                'items': items,
                'totalCount': total,
                'page': page,
                'pageSize': page_size,
                'total': total
            }
        }


class TransactionAdapter:
    """交易数据适配器。"""

    def __init__(self, db=None, user_id: int = 1):
        self.db = db
        self.user_id = user_id
        self.account_adapter = AccountAdapter()
        self.category_adapter = CategoryAdapter()

    def frontend_to_backend(self, frontend_data: Dict[str, Any]) -> Tuple[Dict[str, Any], Dict[str, Any]]:
        """前端交易 -> 后端账单。"""
        tx_type = frontend_data.get('type')
        backend_type = FRONTEND_TO_BACKEND_TYPE.get(int(tx_type), '支出') if tx_type not in (None, '') else ''

        time_ms = frontend_data.get('time')
        date_str = ''
        if time_ms not in (None, ''):
            try:
                date_str = datetime.fromtimestamp(int(time_ms) / 1000).strftime('%Y-%m-%d %H:%M:%S')
            except (TypeError, ValueError, OSError):
                date_str = ''

        source_amount = cents_to_yuan(frontend_data.get('sourceAmount'))
        destination_amount = cents_to_yuan(frontend_data.get('destinationAmount'))

        amount = source_amount
        if backend_type in ('支出', '转账', '投资') and amount > 0:
            amount = -amount

        backend_data: Dict[str, Any] = {
            'type': backend_type,
            'date': date_str,
            'amount': amount,
            'destination_amount': destination_amount,
            'source_account_id': int(frontend_data.get('sourceAccountId', 0) or 0),
            'destination_account_id': int(frontend_data.get('destinationAccountId', 0) or 0),
            'description': frontend_data.get('comment', frontend_data.get('remark', '')),
        }

        metadata = {
            'category_id': str(frontend_data.get('categoryId', '') or ''),
            'source_account_id': backend_data['source_account_id'],
            'destination_account_id': backend_data['destination_account_id'],
            'tag_ids': [int(tag_id) for tag_id in frontend_data.get('tagIds', []) if str(tag_id).strip() and str(tag_id) != '0'],
            'auto_invest_account': False,
        }
        return backend_data, metadata

    async def _build_account_map(self) -> Dict[int, Dict[str, Any]]:
        if not self.db:
            return {}
        accounts = await self.db.get_all_accounts(user_id=self.user_id)
        return {int(account['id']): account for account in accounts}

    async def _build_category_maps(self) -> Tuple[Dict[int, Dict[str, Any]], Dict[Tuple[str, str], int]]:
        if not self.db:
            return {}, {}
        categories = await self.db.get_all_categories(user_id=self.user_id)
        by_id = {int(category['id']): category for category in categories}
        by_name = {
            (category.get('main_category', ''), category.get('sub_category', '')): int(category['id'])
            for category in categories
        }
        return by_id, by_name

    async def backend_to_frontend(
        self,
        bill: Dict[str, Any],
        account_map: Optional[Dict[str, Any]] = None,
        category_map: Optional[Dict[str, Any]] = None,
        tags: Optional[List[Dict[str, Any]]] = None
    ) -> Dict[str, Any]:
        """后端账单 -> 前端交易。"""
        if account_map and 'id_to_account' in account_map:
            id_to_account = account_map['id_to_account']
        else:
            id_to_account = await self._build_account_map()

        if category_map and 'id_to_category' in category_map:
            id_to_category = category_map['id_to_category']
            name_to_id = category_map.get('name_to_id', {})
        else:
            id_to_category, name_to_id = await self._build_category_maps()

        bill_type = str(bill.get('type', '') or '')
        frontend_type = BACKEND_TO_FRONTEND_TYPE.get(bill_type, 3)
        category_id = name_to_id.get((bill.get('main_category', ''), bill.get('sub_category', '')), 0)

        time_value = 0
        date_text = str(bill.get('date', '') or '')
        try:
            fmt = '%Y-%m-%d %H:%M:%S' if len(date_text) > 10 else '%Y-%m-%d'
            time_value = int(datetime.strptime(date_text[:19] if len(date_text) > 19 else date_text, fmt).timestamp() * 1000)
        except (ValueError, TypeError):
            time_value = 0

        amount = float(bill.get('amount', 0) or 0)
        destination_amount = float(bill.get('destination_amount', 0) or 0)
        source_account_id = int(bill.get('source_account_id', 0) or 0)
        destination_account_id = int(bill.get('destination_account_id', 0) or 0)

        result: Dict[str, Any] = {
            'id': str(bill.get('id', '')),
            'timeSequenceId': str(bill.get('time_sequence_id', bill.get('id', ''))),
            'type': frontend_type,
            'categoryId': str(category_id or '0'),
            'time': time_value,
            'utcOffset': int(bill.get('utc_offset', 480) or 480),
            'sourceAccountId': str(source_account_id or '0'),
            'destinationAccountId': str(destination_account_id or '0'),
            'sourceAmount': yuan_to_cents(abs(amount)),
            'destinationAmount': yuan_to_cents(abs(destination_amount or amount)),
            'hideAmount': bool(bill.get('hide_amount', False)),
            'tagIds': [str(tag.get('id')) for tag in (tags or []) if tag.get('id') is not None],
            'tags': [
                {'id': str(tag.get('id')), 'name': tag.get('name', '')}
                for tag in (tags or []) if tag.get('id') is not None
            ],
            'comment': bill.get('description', ''),
            'editable': True,
        }

        category = id_to_category.get(int(category_id)) if category_id else None
        if category:
            result['category'] = self.category_adapter.backend_to_frontend(
                category,
                parent_id='0' if not category.get('sub_category') else f"virtual_{category.get('main_category', '')}"
            )

        source_account = id_to_account.get(source_account_id)
        if source_account:
            result['sourceAccount'] = self.account_adapter.backend_to_frontend(source_account)

        dest_account = id_to_account.get(destination_account_id)
        if dest_account:
            result['destinationAccount'] = self.account_adapter.backend_to_frontend(dest_account)

        if time_value:
            dt = datetime.fromtimestamp(time_value / 1000)
            result['gregorianCalendarYearDashMonthDashDay'] = dt.strftime('%Y-%m-%d')
            result['gregorianCalendarDayOfMonth'] = dt.day
            result['displayDayOfWeek'] = ((dt.weekday() + 1) % 7) + 1

        return result

    async def backend_list_to_frontend(
        self,
        bills: List[Dict[str, Any]],
        total: int,
        page: int,
        page_size: int
    ) -> Dict[str, Any]:
        """批量转换账单列表。"""
        account_map = {'id_to_account': await self._build_account_map()}
        category_by_id, category_name_to_id = await self._build_category_maps()
        category_map = {'id_to_category': category_by_id, 'name_to_id': category_name_to_id}
        items = [
            await self.backend_to_frontend(bill, account_map, category_map)
            for bill in bills
        ]
        return ResponseBuilder.build_page(items, total, page, page_size)