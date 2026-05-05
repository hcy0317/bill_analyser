# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403

def _frontend_type_number_to_backend_label(raw_type: Any) -> str:
    """前端数字类型转后端中文类型。"""
    type_value = str(raw_type).strip()
    mapping = {"1": "余额调整", "2": "收入", "3": "支出", "4": "转账", "5": "投资"}

    if type_value in mapping:
        return mapping[type_value]

    normalized = type_value.lower()
    keyword_mapping = {
        "income": "收入",
        "expense": "支出",
        "transfer": "转账",
        "investment": "投资",
        "modifybalance": "余额调整",
        "余额调整": "余额调整",
        "收入": "收入",
        "收": "收入",
        "支出": "支出",
        "支": "支出",
        "转账": "转账",
        "投资": "投资",
        "退款": "收入",
    }
    return keyword_mapping.get(normalized, "支出")


def _map_generic_import_type(raw_type: Any, transaction_type_mapping: dict[str, Any]) -> str:
    """根据前端交易类型映射解析后端账单类型。"""
    raw_type_text = str(raw_type or "").strip()
    if raw_type_text and transaction_type_mapping:
        if raw_type_text in transaction_type_mapping:
            return _frontend_type_number_to_backend_label(transaction_type_mapping[raw_type_text])

        lowered = raw_type_text.lower()
        for candidate, mapped_value in transaction_type_mapping.items():
            if str(candidate).strip().lower() == lowered:
                return _frontend_type_number_to_backend_label(mapped_value)

    return _frontend_type_number_to_backend_label(raw_type_text)


def _get_mapped_cell(row: list[str], column_mapping: dict[str, Any], column_type: int) -> str:
    """根据列映射读取单元格值。"""
    index = column_mapping.get(str(column_type))
    if index is None:
        return ""

    try:
        column_index = int(index)
    except (TypeError, ValueError):
        return ""

    if column_index < 0 or column_index >= len(row):
        return ""

    return str(row[column_index]).strip()


def _infer_generic_import_type_from_context(row: list[str], headers: list[str]) -> str | None:
    """根据上下文字段推断收入/支出类型。"""
    if not row:
        return None

    context_values = [str(cell or "").strip() for cell in row if str(cell or "").strip()]

    for header_candidates in (
        ["交易摘要", "摘要", "备注", "附言", "说明", "detail", "description", "note"],
        ["交易用途", "用途", "业务摘要", "业务说明", "purpose"],
    ):
        column_index = _find_generic_import_header_index(headers, header_candidates)
        if column_index is not None and column_index < len(row):
            value = str(row[column_index] or "").strip()
            if value:
                context_values.append(value)

    normalized_context = " ".join(context_values)
    if not normalized_context:
        return None

    income_keywords = ["入账", "转入", "来账", "收款", "工资", "补发", "退款", "退汇", "利息", "存入", "代发"]
    expense_keywords = ["转出", "支出", "付款", "消费", "提现", "扣款", "扣费", "缴费", "支付", "还款", "购买"]

    has_income_keyword = any(keyword in normalized_context for keyword in income_keywords)
    has_expense_keyword = any(keyword in normalized_context for keyword in expense_keywords)

    if has_income_keyword and not has_expense_keyword:
        return "收入"
    if has_expense_keyword and not has_income_keyword:
        return "支出"
    return None


def _find_generic_import_header_index(headers: list[str], candidates: list[str], exclude: set[int] | None = None) -> int | None:
    """根据候选关键字查找表头索引。"""
    excluded_indices = exclude or set()

    best_index = None
    best_score = 0.0
    for index, header in enumerate(headers):
        if index in excluded_indices:
            continue

        normalized_header = _normalize_import_suggestion_text(header)
        if not normalized_header:
            continue

        for candidate in candidates:
            normalized_candidate = _normalize_import_suggestion_text(candidate)
            if not normalized_candidate:
                continue

            score = 0.0
            if normalized_header == normalized_candidate:
                score = 10.0
            elif normalized_candidate in normalized_header or normalized_header in normalized_candidate:
                score = 6.0

            if score > best_score:
                best_score = score
                best_index = index

    return best_index if best_score > 0 else None


def _build_generic_import_trade_time(
    row: list[str], headers: list[str], column_mapping: dict[str, Any], raw_time_value: str
) -> str:
    """构建通用导入的交易时间文本，必要时合并日期列与时间列。"""
    normalized_time = re.sub(r"\s+", " ", str(raw_time_value or "").replace("\t", " ")).strip()

    mapped_time_index = column_mapping.get("1")
    try:
        mapped_time_index_int = int(mapped_time_index) if mapped_time_index is not None else None
    except (TypeError, ValueError):
        mapped_time_index_int = None

    if normalized_time and ":" in normalized_time:
        return normalized_time

    if not headers:
        return normalized_time

    time_index = _find_generic_import_header_index(headers, GENERIC_IMPORT_TIME_HEADERS, exclude={mapped_time_index_int} if mapped_time_index_int is not None else None)
    date_index = _find_generic_import_header_index(headers, GENERIC_IMPORT_DATE_HEADERS)

    date_text = normalized_time
    if not date_text and date_index is not None and date_index < len(row):
        date_text = re.sub(r"\s+", " ", str(row[date_index] or "").replace("\t", " ")).strip()

    time_text = ""
    if time_index is not None and time_index < len(row):
        time_text = re.sub(r"\s+", " ", str(row[time_index] or "").replace("\t", " ")).strip()

    if date_text and time_text and ":" in time_text and ":" not in date_text:
        return f"{date_text} {time_text}"

    return date_text or time_text


def _is_generic_import_repeated_header_row(row: list[str], headers: list[str]) -> bool:
    """判断数据行是否为重复出现的表头行。"""
    comparable_length = min(len(row), len(headers))
    if comparable_length == 0:
        return False

    matched_count = 0
    non_empty_count = 0
    for index in range(comparable_length):
        normalized_cell = _normalize_import_suggestion_text(row[index])
        normalized_header = _normalize_import_suggestion_text(headers[index])
        if not normalized_cell or not normalized_header:
            continue
        non_empty_count += 1
        if normalized_cell == normalized_header:
            matched_count += 1

    return matched_count >= 3 and matched_count == non_empty_count


def _generic_import_amount_column_has_signed_values(rows: list[list[str]], column_mapping: dict[str, Any]) -> bool:
    """判断金额列是否使用带符号单列来区分收入和支出。"""
    amount_index = column_mapping.get("8")
    try:
        amount_index_int = int(amount_index) if amount_index is not None else None
    except (TypeError, ValueError):
        amount_index_int = None

    if amount_index_int is None:
        return False

    has_positive = False
    has_negative = False
    for row in rows[:200]:
        if amount_index_int >= len(row):
            continue
        signed_amount = _parse_generic_import_signed_amount(row[amount_index_int], decimal_separator=".")
        if signed_amount is None:
            continue
        if signed_amount > 0:
            has_positive = True
        elif signed_amount < 0:
            has_negative = True
        if has_positive and has_negative:
            return True

    return False


def _resolve_generic_import_type_and_amount(
    row: list[str],
    headers: list[str],
    column_mapping: dict[str, Any],
    transaction_type_mapping: dict[str, Any],
    amount_decimal_separator: str,
    amount_digit_grouping_symbol: str,
    amount_column_has_signed_values: bool = False,
) -> tuple[str, float, str]:
    """解析通用导入的交易类型与金额，并兼容借贷双金额列。"""
    raw_type_value = _get_mapped_cell(row, column_mapping, 3)
    raw_amount_value = _get_mapped_cell(row, column_mapping, 8)
    mapped_type_index = column_mapping.get("3")
    try:
        int(mapped_type_index) if mapped_type_index is not None else None
    except (TypeError, ValueError):
        pass

    def _parse_optional_amount(raw_value: Any) -> tuple[str, float]:
        amount_text = str(raw_value or "").strip()
        if not amount_text:
            return "", 0.0

        return amount_text, _parse_generic_import_amount(
            amount_text,
            decimal_separator=amount_decimal_separator or ".",
            grouping_symbol=amount_digit_grouping_symbol or None,
        )

    if headers:
        income_amount_index = _find_generic_import_header_index(headers, GENERIC_IMPORT_INCOME_AMOUNT_HEADERS)
        expense_amount_index = _find_generic_import_header_index(headers, GENERIC_IMPORT_EXPENSE_AMOUNT_HEADERS)

        income_amount_value = (
            str(row[income_amount_index]).strip()
            if income_amount_index is not None and income_amount_index < len(row)
            else ""
        )
        expense_amount_value = (
            str(row[expense_amount_index]).strip()
            if expense_amount_index is not None and expense_amount_index < len(row)
            else ""
        )
        raw_amount_text, raw_amount = _parse_optional_amount(raw_amount_value)
        income_amount_text, income_amount = _parse_optional_amount(income_amount_value)
        expense_amount_text, expense_amount = _parse_optional_amount(expense_amount_value)

        has_income_amount = bool(income_amount_text) and abs(income_amount) > 0
        has_expense_amount = bool(expense_amount_text) and abs(expense_amount) > 0

        if not raw_type_value:
            direction_index = _find_generic_import_header_index(
                headers,
                ["收/支", "收支", "收支类型", "借贷标志", "借贷"],
            )
            if direction_index is not None and direction_index < len(row):
                raw_type_value = str(row[direction_index] or "").strip()

        if not raw_amount_text or (abs(raw_amount) == 0 and (has_income_amount or has_expense_amount)):
            if has_expense_amount:
                raw_amount_value = expense_amount_value
            elif has_income_amount:
                raw_amount_value = income_amount_value

        if not raw_type_value:
            if has_income_amount and not has_expense_amount:
                raw_type_value = "收入"
            elif has_expense_amount and not has_income_amount:
                raw_type_value = "支出"

    type_name = _map_generic_import_type(raw_type_value, transaction_type_mapping)
    raw_amount_text = str(raw_amount_value or "").strip()
    normalized_amount_text = raw_amount_text.replace("\t", " ").strip()
    signed_amount = _parse_generic_import_signed_amount(
        normalized_amount_text,
        decimal_separator=amount_decimal_separator or ".",
        grouping_symbol=amount_digit_grouping_symbol or None,
    )

    if not str(raw_type_value or "").strip():
        if normalized_amount_text.startswith("+"):
            type_name = "收入"
        elif normalized_amount_text.startswith("-"):
            type_name = "支出"
        elif amount_column_has_signed_values and signed_amount is not None and signed_amount != 0:
            type_name = "收入" if signed_amount > 0 else "支出"

    if type_name == "转账":
        inferred_type_name = _infer_generic_import_type_from_context(row, headers)
        if inferred_type_name in {"收入", "支出"}:
            type_name = inferred_type_name

    amount = _parse_generic_import_amount(
        normalized_amount_text,
        decimal_separator=amount_decimal_separator or ".",
        grouping_symbol=amount_digit_grouping_symbol or None,
    )

    return type_name, amount, raw_type_value


def _build_original_category(main_category: str, sub_category: str) -> str:
    """构建原始分类展示文本。"""
    if main_category and sub_category:
        return f"{main_category}/{sub_category}"
    return main_category or sub_category or ""

__all__ = [name for name in globals() if not name.startswith("__")]
