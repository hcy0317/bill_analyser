"""
Validator Module - 数据验证模块

提供账单字段验证功能。
"""

from datetime import datetime
from decimal import Decimal, InvalidOperation
from typing import Any, Dict, List, Optional, Tuple

from .logger import get_logger, log_method


class BillValidator:
    """账单数据验证器"""

    # 必需字段
    REQUIRED_FIELDS = ["date", "type", "amount", "counterparty", "description"]

    # 交易类型（包含投资类型）
    VALID_TYPES = ["收入", "支出", "转账", "退款", "投资"]

    def __init__(self):
        """初始化验证器"""
        self.logger = get_logger("BillValidator")

    @log_method
    def validate_bill(self, bill: Dict[str, Any]) -> Tuple[bool, List[str]]:
        """
        验证单条账单数据

        Args:
            bill: 账单数据字典

        Returns:
            Tuple[bool, List[str]]: (是否有效, 错误信息列表)
        """
        errors = []

        # 检查必需字段
        for field in self.REQUIRED_FIELDS:
            if field not in bill or bill[field] is None:
                errors.append(f"缺少必需字段: {field}")

        if errors:
            return False, errors

        # 验证日期
        date_valid, date_error = self._validate_date(bill["date"])
        if not date_valid:
            errors.append(date_error)

        # 验证类型
        type_valid, type_error = self._validate_type(bill["type"])
        if not type_valid:
            errors.append(type_error)

        # 验证金额
        amount_valid, amount_error = self._validate_amount(bill["amount"])
        if not amount_valid:
            errors.append(amount_error)

        # 验证对方信息
        counterparty_valid, counterparty_error = self._validate_counterparty(bill["counterparty"])
        if not counterparty_valid:
            errors.append(counterparty_error)

        # 验证描述
        desc_valid, desc_error = self._validate_description(bill["description"])
        if not desc_valid:
            errors.append(desc_error)

        is_valid = len(errors) == 0
        if is_valid:
            self.logger.debug(f"账单验证通过: {bill.get('date', 'unknown')}")
        else:
            self.logger.warning(f"账单验证失败: {', '.join(errors)}")

        return is_valid, errors

    def _validate_date(self, date_value: Any) -> Tuple[bool, Optional[str]]:
        """验证日期字段"""
        if isinstance(date_value, datetime):
            return True, None

        if isinstance(date_value, str):
            # 尝试解析常见日期格式
            formats = [
                "%Y-%m-%d %H:%M:%S",
                "%Y-%m-%d",
                "%Y/%m/%d %H:%M:%S",
                "%Y/%m/%d",
                "%Y年%m月%d日 %H:%M:%S",
                "%Y年%m月%d日",
            ]

            for fmt in formats:
                try:
                    datetime.strptime(date_value, fmt)
                    return True, None
                except ValueError:
                    continue

            return False, f"日期格式无效: {date_value}"

        return False, f"日期类型无效: {type(date_value).__name__}"

    def _validate_type(self, type_value: Any) -> Tuple[bool, Optional[str]]:
        """验证交易类型字段"""
        if not isinstance(type_value, str):
            return False, f"交易类型必须是字符串: {type(type_value).__name__}"

        if type_value not in self.VALID_TYPES:
            return False, f"交易类型无效: {type_value}，有效值: {', '.join(self.VALID_TYPES)}"

        return True, None

    def _validate_amount(self, amount_value: Any) -> Tuple[bool, Optional[str]]:
        """验证金额字段"""
        # 尝试转换为 Decimal
        try:
            if isinstance(amount_value, str):
                # 移除常见的货币符号和逗号
                amount_value = amount_value.replace("¥", "").replace("$", "").replace(",", "").strip()

            amount = Decimal(str(amount_value))

            # 检查金额范围
            if amount < Decimal("-999999999"):
                return False, f"金额过小: {amount}"
            if amount > Decimal("999999999"):
                return False, f"金额过大: {amount}"

            return True, None

        except (InvalidOperation, ValueError, TypeError) as e:
            return False, f"金额格式无效: {amount_value} ({type(e).__name__})"

    def _validate_counterparty(self, counterparty_value: Any) -> Tuple[bool, Optional[str]]:
        """验证对方信息字段"""
        if not isinstance(counterparty_value, str):
            return False, f"对方信息必须是字符串: {type(counterparty_value).__name__}"

        # 允许空的对方信息（某些交易如投资理财可能没有明确的对方）
        # 长度检查
        if len(counterparty_value) > 200:
            return False, f"对方信息过长: {len(counterparty_value)} 字符（最多200）"

        return True, None

    def _validate_description(self, description_value: Any) -> Tuple[bool, Optional[str]]:
        """验证描述字段"""
        if not isinstance(description_value, str):
            return False, f"描述必须是字符串: {type(description_value).__name__}"

        if len(description_value) > 500:
            return False, f"描述过长: {len(description_value)} 字符（最多500）"

        return True, None

    @log_method
    def validate_bills(self, bills: List[Dict[str, Any]]) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
        """
        批量验证账单数据

        Args:
            bills: 账单数据列表

        Returns:
            Tuple[List[Dict], List[Dict]]: (有效账单列表, 无效账单列表(含错误信息))
        """
        valid_bills = []
        invalid_bills = []

        self.logger.info(f"开始批量验证 {len(bills)} 条账单")

        for i, bill in enumerate(bills):
            is_valid, errors = self.validate_bill(bill)

            if is_valid:
                valid_bills.append(bill)
            else:
                invalid_bill = bill.copy()
                invalid_bill["_validation_errors"] = errors
                invalid_bill["_index"] = i
                invalid_bills.append(invalid_bill)

        self.logger.info(f"验证完成: 有效 {len(valid_bills)} 条, 无效 {len(invalid_bills)} 条")

        return valid_bills, invalid_bills

    @log_method
    def normalize_bill(self, bill: Dict[str, Any]) -> Dict[str, Any]:
        """
        规范化账单数据

        Args:
            bill: 原始账单数据

        Returns:
            Dict[str, Any]: 规范化后的账单数据
        """
        normalized = bill.copy()

        # 规范化日期
        if isinstance(normalized.get("date"), str):
            date_str = normalized["date"]
            for fmt in [
                "%Y-%m-%d %H:%M:%S",
                "%Y-%m-%d",
                "%Y/%m/%d %H:%M:%S",
                "%Y/%m/%d",
                "%Y年%m月%d日 %H:%M:%S",
                "%Y年%m月%d日",
            ]:
                try:
                    normalized["date"] = datetime.strptime(date_str, fmt).strftime("%Y-%m-%d %H:%M:%S")
                    break
                except ValueError:
                    continue

        # 规范化金额
        if isinstance(normalized.get("amount"), str):
            amount_str = normalized["amount"].replace("¥", "").replace("$", "").replace(",", "").strip()
            try:
                normalized["amount"] = float(Decimal(amount_str))
            except InvalidOperation, ValueError:
                pass

        # 去除字符串字段的首尾空白
        for field in ["type", "counterparty", "description"]:
            if isinstance(normalized.get(field), str):
                normalized[field] = normalized[field].strip()

        return normalized


# 全局验证器实例
_validator = BillValidator()


def validate_bill(bill: Dict[str, Any]) -> Tuple[bool, List[str]]:
    """
    验证单条账单

    Args:
        bill: 账单数据

    Returns:
        Tuple[bool, List[str]]: (是否有效, 错误列表)
    """
    return _validator.validate_bill(bill)


def validate_bills(bills: List[Dict[str, Any]]) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    """
    批量验证账单

    Args:
        bills: 账单列表

    Returns:
        Tuple[List[Dict], List[Dict]]: (有效账单, 无效账单)
    """
    return _validator.validate_bills(bills)


def normalize_bill(bill: Dict[str, Any]) -> Dict[str, Any]:
    """
    规范化账单数据

    Args:
        bill: 原始账单数据

    Returns:
        Dict[str, Any]: 规范化后的账单
    """
    return _validator.normalize_bill(bill)
