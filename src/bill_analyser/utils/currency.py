"""Currency Conversion Utilities - 货币单位转换工具

统一管理前后端之间的货币单位转换：
- 前端: 分(cents) 整数
- 后端: 元(yuan) 浮点数

Author: Bill Analyser Team
Created: 2025-11-21
"""

import math
from decimal import ROUND_HALF_UP, Decimal

from .logger import get_logger

logger = get_logger("CurrencyUtils")


def cents_to_yuan(cents: int | str | None) -> float:
    """将分转换为元

    Args:
        cents: 分(整数或字符串)，前端金额单位

    Returns:
        float: 元(浮点数)，后端存储单位

    Examples:
        >>> cents_to_yuan(10050)
        100.50
        >>> cents_to_yuan("5000")
        50.0
        >>> cents_to_yuan(None)
        0.0
    """
    if cents is None or cents == "":
        return 0.0

    try:
        if isinstance(cents, str):
            cents = int(cents) if cents else 0

        # 使用Decimal确保精度
        decimal_cents = Decimal(str(cents))
        decimal_yuan = decimal_cents / Decimal("100")

        # 转换为float，保留2位小数
        return float(decimal_yuan.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))

    except (ValueError, TypeError) as e:
        logger.warning(f"货币转换失败 (分→元): {cents}, 错误: {e}")
        return 0.0


def yuan_to_cents(yuan: float | str | None) -> int:
    """将元转换为分

    Args:
        yuan: 元(浮点数/整数/字符串)，后端存储单位

    Returns:
        int: 分(整数)，前端金额单位

    Examples:
        >>> yuan_to_cents(100.50)
        10050
        >>> yuan_to_cents("50.0")
        5000
        >>> yuan_to_cents(None)
        0
    """
    if yuan is None or yuan == "":
        return 0

    try:
        if isinstance(yuan, str):
            yuan = float(yuan) if yuan else 0.0

        # 使用Decimal确保精度
        decimal_yuan = Decimal(str(yuan))
        decimal_cents = decimal_yuan * Decimal("100")

        # 四舍五入到整数
        return int(decimal_cents.quantize(Decimal("1"), rounding=ROUND_HALF_UP))

    except (ValueError, TypeError) as e:
        logger.warning(f"货币转换失败 (元→分): {yuan}, 错误: {e}")
        return 0


def format_currency_display(yuan: float | None, currency: str = "CNY") -> str:
    """格式化货币显示

    Args:
        yuan: 金额(元)
        currency: 货币代码，默认CNY

    Returns:
        str: 格式化的货币字符串

    Examples:
        >>> format_currency_display(1234.56)
        '¥1,234.56'
        >>> format_currency_display(1234.56, 'USD')
        '$1,234.56'
    """
    if yuan is None:
        yuan = 0.0

    # 货币符号映射
    currency_symbols = {"CNY": "¥", "USD": "$", "EUR": "€", "GBP": "£", "JPY": "¥", "HKD": "HK$"}

    symbol = currency_symbols.get(currency, currency)

    # 格式化为千位分隔符
    formatted = f"{yuan:,.2f}"

    return f"{symbol}{formatted}"


def validate_amount(
    amount: float | str | None, min_value: float = 0.01, max_value: float = 999999999.99
) -> bool:
    """验证金额是否合法

    Args:
        amount: 待验证金额(元)
        min_value: 最小值(默认0.01元)
        max_value: 最大值(默认10亿元)

    Returns:
        bool: 是否合法
    """
    if amount is None or amount == "":
        return False

    try:
        if isinstance(amount, str):
            amount = float(amount)

        amount = float(amount)

        # 检查范围
        if amount < min_value or amount > max_value:
            return False

        # 检查是否为有限数（排除NaN和Inf）
        if not math.isfinite(amount):
            return False

        return True

    except (ValueError, TypeError):
        return False


# 导出常量
CENTS_IN_YUAN = 100
MIN_TRANSACTION_AMOUNT = 0.01  # 1分
MAX_TRANSACTION_AMOUNT = 999999999.99  # 约10亿元
