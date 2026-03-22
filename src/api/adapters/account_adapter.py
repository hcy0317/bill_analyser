"""Neutral account adapter.

在 REST 路由与前端模型之间做轻量字段转换。
"""

from __future__ import annotations

import json
from typing import Any, Dict, List

from src.utils.constants import DEFAULT_PARENT_ID
from src.utils.currency import cents_to_yuan, yuan_to_cents


def _parse_aliases(raw_value: Any) -> List[str]:
    """解析账户别名。"""
    if raw_value in (None, '', []):
        return []

    if isinstance(raw_value, list):
        return [str(item).strip() for item in raw_value if str(item).strip()]

    if isinstance(raw_value, str):
        text = raw_value.strip()
        if not text:
            return []

        if text.startswith('['):
            try:
                parsed = json.loads(text)
                if isinstance(parsed, list):
                    return [str(item).strip() for item in parsed if str(item).strip()]
            except (TypeError, ValueError, json.JSONDecodeError):
                pass

        return [item.strip() for item in text.split(',') if item.strip()]

    return []


class AccountAdapter:
    """账户数据适配器。"""

    def frontend_to_backend(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """前端账户数据 -> 后端账户数据。"""
        aliases = _parse_aliases(data.get('aliases'))
        balance_cents = data.get('balance', data.get('initial_balance', 0))
        balance_yuan = cents_to_yuan(balance_cents)

        return {
            'name': data.get('name', ''),
            'parent_id': int(data.get('parentId', data.get('parent_id', 0)) or 0),
            'category': data.get('category'),
            'type': data.get('type', 0),
            'icon': data.get('icon', ''),
            'color': data.get('color', ''),
            'currency': data.get('currency', 'CNY'),
            'balance': balance_yuan,
            'initial_balance': balance_yuan,
            'comment': data.get('comment', ''),
            'aliases': json.dumps(aliases, ensure_ascii=False),
            'display_order': data.get('displayOrder', data.get('display_order', 0)),
            'hidden': data.get('hidden', not data.get('visible', True)),
            'subAccounts': data.get('subAccounts', []),
            'credit_card_statement_date': data.get(
                'creditCardStatementDate',
                data.get('credit_card_statement_date')
            )
        }

    def backend_to_frontend(self, account: Dict[str, Any]) -> Dict[str, Any]:
        """后端账户数据 -> 前端账户数据。"""
        balance_yuan = account.get('balance', account.get('initial_balance', 0))

        result = {
            'id': str(account.get('id', '')),
            'name': account.get('name', ''),
            'parentId': str(account.get('parent_id', 0) or 0),
            'category': account.get('category', 0),
            'type': account.get('type', 0),
            'icon': account.get('icon', ''),
            'color': account.get('color', ''),
            'currency': account.get('currency', 'CNY'),
            'balance': yuan_to_cents(balance_yuan),
            'comment': account.get('comment', ''),
            'aliases': _parse_aliases(account.get('aliases')),
            'creditCardStatementDate': account.get('credit_card_statement_date'),
            'displayOrder': account.get('display_order', 0),
            'hidden': bool(account.get('hidden', False)),
            'visible': not bool(account.get('hidden', False)),
        }

        sub_accounts = account.get('subAccounts') or []
        if sub_accounts:
            result['subAccounts'] = [self.backend_to_frontend(sub) for sub in sub_accounts]

        return result

    def format_list_response(
        self,
        accounts: List[Dict[str, Any]],
        build_hierarchy_flag: bool = True
    ) -> Dict[str, Any]:
        """格式化账户列表响应。"""
        formatted = [self.backend_to_frontend(account) for account in accounts]

        if build_hierarchy_flag:
            by_parent: Dict[str, List[Dict[str, Any]]] = {}
            top_level: List[Dict[str, Any]] = []

            for account in formatted:
                parent_id = account.get('parentId', DEFAULT_PARENT_ID)
                if parent_id in ('', DEFAULT_PARENT_ID, '0'):
                    top_level.append(account)
                else:
                    by_parent.setdefault(parent_id, []).append(account)

            for account in top_level:
                children = by_parent.get(account['id'], [])
                if children:
                    account['subAccounts'] = children

            formatted = top_level

        return {
            'success': True,
            'result': formatted
        }