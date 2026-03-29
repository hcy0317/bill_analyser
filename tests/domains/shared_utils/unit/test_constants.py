from __future__ import annotations

from bill_analyser.utils import constants as constants_module
from bill_analyser.utils.constants import AccountCategory, ErrorCode, TransactionType



def test_type_name_helpers_return_localized_labels_and_unknown_fallbacks() -> None:
    """交易类型和账户分类显示名应支持多语言和未知值回退。"""
    assert constants_module.get_transaction_type_name(TransactionType.INCOME) == "收入"
    assert constants_module.get_transaction_type_name(TransactionType.EXPENSE, "en-US") == "Expense"
    assert constants_module.get_transaction_type_name(999) == "未知"

    assert constants_module.get_account_category_name(AccountCategory.CASH) == "现金"
    assert constants_module.get_account_category_name(AccountCategory.CREDIT_CARD, "en-US") == "Credit Card"
    assert constants_module.get_account_category_name(999) == "未知"



def test_asset_and_liability_helpers_classify_account_categories() -> None:
    """资产/负债分类辅助函数应与预定义类别集合保持一致。"""
    assert constants_module.is_asset_account(AccountCategory.CASH) is True
    assert constants_module.is_asset_account(AccountCategory.INVESTMENT) is True
    assert constants_module.is_asset_account(AccountCategory.LOAN) is False

    assert constants_module.is_liability_account(AccountCategory.CREDIT_CARD) is True
    assert constants_module.is_liability_account(AccountCategory.PAYABLE) is True
    assert constants_module.is_liability_account(AccountCategory.CHECKING) is False



def test_error_message_helper_and_type_mappings_stay_consistent() -> None:
    """错误消息和前后端类型映射应保持稳定契约。"""
    assert constants_module.get_error_message(ErrorCode.UNAUTHORIZED) == "未授权，请先登录"
    assert constants_module.get_error_message(ErrorCode.NOT_FOUND, "en-US") == "Resource not found"
    assert constants_module.get_error_message(999) == "未知错误"

    for frontend_type, backend_type in constants_module.FRONTEND_TO_BACKEND_TYPE.items():
        assert constants_module.BACKEND_TO_FRONTEND_TYPE[backend_type] == frontend_type
