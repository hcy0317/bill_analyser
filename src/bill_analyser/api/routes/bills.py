"""
Bills API Routes - 账单相关API端点

重构后使用统一事务适配器进行数据格式转换，
消除冗余代码，提升性能和可维护性。
"""

import asyncio
import base64
import csv
import json
import mimetypes
import os
import re
import uuid
from datetime import datetime
from pathlib import Path
from typing import Any

from flask import Blueprint, jsonify, request
from werkzeug.utils import secure_filename

try:
    import openpyxl
except ImportError:  # pragma: no cover - 依赖在运行环境通常存在
    openpyxl = None

from bill_analyser.api.adapters.transaction_adapter import TransactionAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.constants import BACKEND_TO_FRONTEND_TYPE
from bill_analyser.utils.currency import yuan_to_cents  # 金额单位转换工具
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("BillsAPI")

# 主蓝图 - 现代RESTful API
bp = Blueprint("bills", __name__)

# 上传文件配置 - 指向项目根目录的 uploads 文件夹
UPLOAD_FOLDER = Path(__file__).parent.parent.parent.parent / "uploads"
ALLOWED_EXTENSIONS = {"csv", "xlsx", "xls", "txt"}
ALLOWED_PICTURE_EXTENSIONS = {"png", "jpg", "jpeg", "gif", "webp", "bmp"}
MAX_FILE_SIZE = 10 * 1024 * 1024  # 10MB

# 确保上传目录存在
UPLOAD_FOLDER.mkdir(parents=True, exist_ok=True)


def _get_query_arg(args, *names, default=None):
    """按顺序获取第一个非空查询参数。"""
    for name in names:
        value = args.get(name)
        if value not in (None, ""):
            return value
    return default


def _parse_int_list(raw_value: str):
    """解析逗号分隔的整数列表。"""
    if not raw_value:
        return []
    return [int(item) for item in str(raw_value).split(",") if str(item).strip()]


async def _apply_common_transaction_filters(args, filters, db, user_id: int):
    """应用交易列表公共筛选条件。"""
    keyword = _get_query_arg(args, "keyword", default="")
    if keyword:
        from urllib.parse import unquote

        filters["keyword"] = unquote(keyword)

    account_ids_str = _get_query_arg(args, "accountIds", "account_ids", default="")
    if account_ids_str:
        try:
            account_ids = _parse_int_list(account_ids_str)
            if account_ids:
                filters["account_ids"] = account_ids
        except ValueError:
            logger.warning(f"无效的account_ids参数: {account_ids_str}")

    category_ids_str = _get_query_arg(args, "categoryIds", "category_ids", default="")
    if category_ids_str:
        try:
            category_ids = _parse_int_list(category_ids_str)
            if category_ids:
                all_categories = await db.get_all_categories(user_id=user_id)
                target_categories = []

                for cat in all_categories:
                    if cat["id"] in category_ids:
                        target_categories.append({"main": cat.get("main_category"), "sub": cat.get("sub_category")})

                if target_categories:
                    # 主蓝图 - 现代RESTful API
                    filters["categories"] = target_categories
        except ValueError:
            logger.warning(f"无效的category_ids参数: {category_ids_str}")

    tag_ids_str = _get_query_arg(args, "tagIds", "tag_ids", default="")
    if tag_ids_str:
        try:
            tag_ids = _parse_int_list(tag_ids_str)
            if tag_ids:
                filters["tag_ids"] = tag_ids
        except ValueError:
            logger.warning(f"无效的tag_ids参数: {tag_ids_str}")

    amount_filter = _get_query_arg(args, "amountFilter", "amount_filter", default="")
    if amount_filter:
        filters["amount_filter"] = amount_filter


def allowed_file(filename):
    """检查文件扩展名是否允许"""
    return "." in filename and filename.rsplit(".", 1)[1].lower() in ALLOWED_EXTENSIONS


IMPORT_COLUMN_TYPE_KEYWORDS = {
    1: ["交易时间", "入账时间", "记账时间", "发生时间", "交易日期", "时间", "日期", "datetime", "date", "time"],
    2: ["时区", "timezone", "tz"],
    3: ["交易类型", "收支类型", "类型", "类别", "type"],
    4: ["分类", "一级分类", "主分类", "category"],
    5: ["子分类", "二级分类", "次分类", "subcategory", "subcategoryname"],
    6: ["账户", "账户名", "账户名称", "账号", "付款账户", "支付账户", "account"],
    7: ["币种", "货币", "currency"],
    8: ["金额", "交易金额", "发生金额", "收支金额", "amount", "money"],
    9: ["对方账户", "相关账户", "转入账户", "目标账户", "收款账户", "destinationaccount", "relatedaccount"],
    10: ["对方币种", "目标币种", "转入币种", "destinationcurrency", "relatedcurrency"],
    11: ["对方金额", "目标金额", "转入金额", "收款金额", "destinationamount", "relatedamount"],
    12: ["地理位置", "位置", "经纬度", "坐标", "location", "geolocation"],
    13: ["标签", "标记", "tags", "tag"],
    14: ["备注", "摘要", "描述", "说明", "附言", "用途", "memo", "remark", "description", "note", "detail"],
}

LEGACY_IMPORT_FIELD_TO_COLUMN_TYPE = {
    "date": 1,
    "time": 1,
    "type": 3,
    "category": 4,
    "subcategory": 5,
    "sub_category": 5,
    "account": 6,
    "accountname": 6,
    "currency": 7,
    "amount": 8,
    "relatedaccount": 9,
    "relatedaccountname": 9,
    "relatedcurrency": 10,
    "relatedamount": 11,
    "geolocation": 12,
    "tags": 13,
    "description": 14,
    "comment": 14,
    "memo": 14,
}

AUTO_TRANSACTION_TYPE_MAPPING = {
    "支出": 3,
    "收入": 2,
    "转账": 4,
    "投资": 5,
    "退款": 2,
    "expense": 3,
    "income": 2,
    "transfer": 4,
    "investment": 5,
}


def _normalize_import_suggestion_text(value: Any) -> str:
    """标准化导入建议比较文本。"""
    text = str(value or "").strip().lower()
    return re.sub(r"[\s_\-\/\\()（）\[\]【】:：]+", "", text)


def _score_header_keyword_match(normalized_header: str, column_type: int) -> float:
    """根据关键词规则计算表头与导入列类型的匹配分。"""
    best_score = 0.0
    for keyword in IMPORT_COLUMN_TYPE_KEYWORDS.get(column_type, []):
        normalized_keyword = _normalize_import_suggestion_text(keyword)
        if not normalized_keyword:
            continue
        if normalized_header == normalized_keyword:
            best_score = max(best_score, 10.0)
        elif normalized_keyword in normalized_header or normalized_header in normalized_keyword:
            best_score = max(best_score, 6.0)

    if column_type == 9 and any(token in normalized_header for token in ["对方", "转入", "目标", "收款"]):
        best_score = max(best_score, 8.0)
    if (
        column_type == 11
        and any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
        and "金额" in str(normalized_header)
    ):
        best_score = max(best_score, 8.0)
    if (
        column_type == 6
        and "账户" in str(normalized_header)
        and not any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
    ):
        best_score = max(best_score, 7.0)
    if (
        column_type == 8
        and "金额" in str(normalized_header)
        and not any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
    ):
        best_score = max(best_score, 7.0)

    return best_score


def _extract_import_config_header_type_pairs(config: dict[str, Any]) -> list[tuple[str, int, float]]:
    """从历史模板中提取 表头 -> 导入列类型 的关联。"""
    field_mappings = config.get("field_mappings") or {}
    sample_headers = config.get("sample_headers") or []
    use_count = float(config.get("use_count", 0) or 0)
    base_weight = 3.0 + min(use_count, 20.0) * 0.1
    pairs: list[tuple[str, int, float]] = []

    column_mapping = field_mappings.get("columnMapping") if isinstance(field_mappings, dict) else None
    if isinstance(column_mapping, dict):
        for column_type, column_index in column_mapping.items():
            try:
                column_type_int = int(column_type)
                column_index_int = int(column_index)
            except TypeError, ValueError:
                continue
            if 0 <= column_index_int < len(sample_headers):
                normalized_header = _normalize_import_suggestion_text(sample_headers[column_index_int])
                if normalized_header:
                    pairs.append((normalized_header, column_type_int, base_weight))
        return pairs

    if isinstance(field_mappings, dict):
        for field_name, header_name in field_mappings.items():
            normalized_field = _normalize_import_suggestion_text(field_name)
            column_type_int = LEGACY_IMPORT_FIELD_TO_COLUMN_TYPE.get(normalized_field)
            normalized_header = _normalize_import_suggestion_text(header_name)
            if column_type_int and normalized_header:
                pairs.append((normalized_header, column_type_int, base_weight))

    return pairs


def _build_auto_transaction_type_mapping(sample_rows: list[list[Any]], type_column_index: int | None) -> dict[str, int]:
    """根据样本行自动推断交易类型映射。"""
    if type_column_index is None:
        return {}

    result: dict[str, int] = {}
    for row in sample_rows[:100]:
        if not isinstance(row, list) or type_column_index >= len(row):
            continue
        raw_value = str(row[type_column_index] or "").strip()
        if not raw_value or raw_value in result:
            continue
        mapped_type = AUTO_TRANSACTION_TYPE_MAPPING.get(raw_value.lower())
        if mapped_type:
            result[raw_value] = mapped_type
    return result


def _build_import_mapping_suggestion(
    headers: list[Any], configs: list[dict[str, Any]], sample_rows: list[list[Any]] | None = None
) -> dict[str, Any]:
    """基于表头关键词和历史模板构建列映射建议。"""
    normalized_headers = [_normalize_import_suggestion_text(header) for header in headers]
    historical_scores: dict[tuple[int, int], float] = {}

    for config in configs:
        for normalized_header, column_type, weight in _extract_import_config_header_type_pairs(config):
            for index, incoming_header in enumerate(normalized_headers):
                if incoming_header and incoming_header == normalized_header:
                    historical_scores[(column_type, index)] = historical_scores.get((column_type, index), 0.0) + weight

    candidates: list[tuple[float, int, int]] = []
    for index, normalized_header in enumerate(normalized_headers):
        if not normalized_header:
            continue
        for column_type in IMPORT_COLUMN_TYPE_KEYWORDS:
            score = _score_header_keyword_match(normalized_header, column_type)
            score += historical_scores.get((column_type, index), 0.0)
            if score > 0:
                candidates.append((score, column_type, index))

    candidates.sort(key=lambda item: (-item[0], item[1], item[2]))
    chosen_types = set()
    chosen_indices = set()
    column_mapping: dict[str, int] = {}
    suggestions: list[dict[str, Any]] = []

    for score, column_type, index in candidates:
        if score < 5.0:
            continue
        if column_type in chosen_types or index in chosen_indices:
            continue
        chosen_types.add(column_type)
        chosen_indices.add(index)
        column_mapping[str(column_type)] = index
        suggestions.append(
            {"columnType": column_type, "columnIndex": index, "header": headers[index], "score": round(score, 2)}
        )

    transaction_type_mapping = _build_auto_transaction_type_mapping(sample_rows or [], column_mapping.get("3"))

    return {
        "includeHeader": True,
        "columnMapping": column_mapping,
        "transactionTypeMapping": transaction_type_mapping,
        "suggestions": suggestions,
    }


def allowed_picture_file(filename):
    """检查图片扩展名是否允许。"""
    return "." in filename and filename.rsplit(".", 1)[1].lower() in ALLOWED_PICTURE_EXTENSIONS


def _parse_json_form_field(raw_value, default):
    """解析 multipart/form-data 中的 JSON 字段。"""
    if raw_value in (None, ""):
        return default

    try:
        return json.loads(raw_value)
    except TypeError, ValueError:
        return default


def _parse_bool_form_field(raw_value, default=False):
    """解析布尔表单字段。"""
    if raw_value in (None, ""):
        return default

    return str(raw_value).strip().lower() in ("1", "true", "yes", "on")


def _read_text_with_fallback(file_path: Path, requested_encoding: str = "") -> tuple[str, str]:
    """按多种编码回退读取文本文件。"""
    encodings = []
    if requested_encoding:
        encodings.append(str(requested_encoding).strip())
    encodings.extend(["utf-8", "utf-8-sig", "gbk", "gb18030"])

    seen = set()
    for encoding in encodings:
        if not encoding or encoding.lower() in seen:
            continue
        seen.add(encoding.lower())
        try:
            with open(file_path, encoding=encoding) as file_obj:
                return file_obj.read(), encoding
        except UnicodeDecodeError:
            continue

    raise UnicodeDecodeError("unknown", b"", 0, 1, "unable to decode text file")


def _detect_csv_delimiter(sample_text: str, fallback: str = ",") -> str:
    """检测 CSV/TXT 分隔符。"""
    if not sample_text:
        return fallback

    try:
        return csv.Sniffer().sniff(sample_text).delimiter
    except csv.Error:
        candidates = [",", "\t", ";", "|"]
        counts = {candidate: sample_text.count(candidate) for candidate in candidates}
        best = max(counts.items(), key=lambda item: item[1])
        return best[0] if best[1] > 0 else fallback


def _load_generic_import_rows(
    file_path: Path, requested_encoding: str = "", delimiter: str | None = None
) -> tuple[list[list[str]], str, str]:
    """读取通用表格文件为二维数组。"""
    suffix = file_path.suffix.lower()

    if suffix in (".csv", ".txt"):
        text, actual_encoding = _read_text_with_fallback(file_path, requested_encoding)
        actual_delimiter = delimiter or _detect_csv_delimiter(text[:2048], ",")
        reader = csv.reader(text.splitlines(), delimiter=actual_delimiter)
        rows = [[str(cell).strip() for cell in row] for row in reader]
        return rows, actual_encoding, actual_delimiter

    if suffix in (".xlsx", ".xls"):
        if openpyxl is None:
            raise ImportError("需要安装 openpyxl 以支持 Excel 导入")

        workbook = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
        try:
            sheet = workbook.worksheets[0]
            rows = []
            for row in sheet.iter_rows(values_only=True):
                rows.append(["" if cell is None else str(cell).strip() for cell in row])
            return rows, "utf-8", ""
        finally:
            workbook.close()

    raise ValueError(f"Unsupported file format for generic import: {suffix}")


def _parse_generic_import_time(raw_value: Any, time_format: str = "") -> int:
    """解析导入时间并返回 Unix 秒时间戳。"""
    if raw_value in (None, ""):
        return int(datetime.now().timestamp())

    if isinstance(raw_value, datetime):
        return int(raw_value.timestamp())

    value = str(raw_value).strip()
    if not value:
        return int(datetime.now().timestamp())

    candidate_formats = []
    if time_format:
        candidate_formats.append(time_format)
    candidate_formats.extend(
        [
            "%Y-%m-%d %H:%M:%S",
            "%Y/%m/%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y/%m/%d %H:%M",
            "%Y-%m-%d",
            "%Y/%m/%d",
            "%Y.%m.%d %H:%M:%S",
            "%Y.%m.%d",
            "%d/%m/%Y %H:%M:%S",
            "%d/%m/%Y",
            "%m/%d/%Y %H:%M:%S",
            "%m/%d/%Y",
        ]
    )

    for fmt in candidate_formats:
        try:
            return int(datetime.strptime(value, fmt).timestamp())
        except ValueError:
            continue

    try:
        return int(datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp())
    except ValueError:
        logger.debug("[通用导入] 无法解析时间，使用当前时间: %s", value)
        return int(datetime.now().timestamp())


def _parse_generic_import_amount(
    raw_value: Any, decimal_separator: str = ".", grouping_symbol: str | None = None
) -> float:
    """解析导入金额。"""
    if raw_value in (None, ""):
        return 0.0

    if isinstance(raw_value, (int, float)):
        return abs(float(raw_value))

    value = str(raw_value).strip()
    if not value:
        return 0.0

    if grouping_symbol:
        value = value.replace(grouping_symbol, "")
    if decimal_separator and decimal_separator != ".":
        value = value.replace(decimal_separator, ".")
    value = value.replace("¥", "").replace("￥", "").replace(",", "")

    if value.startswith("(") and value.endswith(")"):
        value = "-" + value[1:-1]

    try:
        return abs(float(value))
    except ValueError:
        logger.debug("[通用导入] 无法解析金额，使用0: %s", raw_value)
        return 0.0


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
        "支出": "支出",
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
    except TypeError, ValueError:
        return ""

    if column_index < 0 or column_index >= len(row):
        return ""

    return str(row[column_index]).strip()


def _build_original_category(main_category: str, sub_category: str) -> str:
    """构建原始分类展示文本。"""
    if main_category and sub_category:
        return f"{main_category}/{sub_category}"
    return main_category or sub_category or ""


def _convert_bill_to_import_item(bill: dict[str, Any]) -> dict[str, Any]:
    """统一把解析账单转换为前端导入检查页结构。"""
    type_name = str(bill.get("type", "") or "").strip()
    frontend_type = BACKEND_TO_FRONTEND_TYPE.get(type_name)
    if frontend_type is None:
        frontend_type = {"余额调整": 1, "收入": 2, "支出": 3, "转账": 4, "投资": 5, "退款": 2}.get(type_name, 3)

    main_category = str(bill.get("main_category", "") or "").strip()
    sub_category = str(bill.get("sub_category", "") or "").strip()
    source_amount = int(round(abs(float(bill.get("amount", 0) or 0)) * 100))
    destination_amount_raw = bill.get("destination_amount", bill.get("related_amount", 0) or 0)
    destination_amount = int(round(abs(float(destination_amount_raw or 0)) * 100))
    time_value = _parse_generic_import_time(bill.get("trade_time") or bill.get("date") or "", "")
    original_tag_names = bill.get("original_tag_names") or []
    if not isinstance(original_tag_names, list):
        original_tag_names = []

    item = {
        "type": frontend_type,
        "categoryId": "",
        "originalCategoryName": _build_original_category(main_category, sub_category),
        "time": time_value,
        "utcOffset": 0,
        "sourceAccountId": "",
        "originalSourceAccountName": str(bill.get("account") or bill.get("payment_method") or "").strip(),
        "originalSourceAccountCurrency": str(bill.get("account_currency") or "CNY").strip() or "CNY",
        "destinationAccountId": "",
        "originalDestinationAccountName": str(
            bill.get("related_account") or bill.get("destination_account_name") or ""
        ).strip(),
        "originalDestinationAccountCurrency": str(bill.get("related_account_currency") or "CNY").strip() or "CNY",
        "sourceAmount": source_amount,
        "destinationAmount": destination_amount if frontend_type in (4, 5) else 0,
        "tagIds": [],
        "originalTagNames": original_tag_names,
        "comment": str(bill.get("description", "") or "").strip(),
        "counterparty": str(bill.get("counterparty", "") or "").strip(),
        "paymentMethod": str(bill.get("payment_method", "") or "").strip(),
        # 兼容旧返回结构
        "timeText": bill.get("trade_time") or bill.get("date") or "",
        "categoryName": main_category,
        "subCategoryName": sub_category,
        "accountName": str(bill.get("account", "") or "").strip(),
        "amount": abs(float(bill.get("amount", 0) or 0)),
        "description": str(bill.get("description", "") or "").strip(),
    }

    return item


def _build_account_mapping_payload(accounts: list[dict[str, Any]]) -> dict[str, Any]:
    """构建导入检查页使用的账户映射。"""
    return {
        "id_to_account": {int(acc["id"]): acc for acc in accounts if acc.get("id") is not None},
        "name_to_id": {
            str(acc.get("name") or "").strip(): int(acc["id"])
            for acc in accounts
            if acc.get("id") is not None and str(acc.get("name") or "").strip()
        },
        "id_to_name": {
            int(acc["id"]): str(acc.get("name") or "").strip() for acc in accounts if acc.get("id") is not None
        },
    }


def _build_category_mapping_payload(categories: list[dict[str, Any]]) -> dict[str, Any]:
    """构建导入检查页使用的分类映射。"""
    return {
        "id_to_category": {int(cat["id"]): cat for cat in categories if cat.get("id") is not None},
        "name_to_id": {
            (str(cat.get("main_category") or "").strip(), str(cat.get("sub_category") or "").strip()): int(cat["id"])
            for cat in categories
            if cat.get("id") is not None and str(cat.get("main_category") or "").strip()
        },
    }


async def _apply_learning_to_column_mapping_bills(
    bills: list[dict[str, Any]], db, bill_service, user_id: int
) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any], int]:
    """为通用列映射导入结果应用长期学习规则。"""
    enriched_bills = [dict(bill) for bill in bills]
    applied_count = 0

    if enriched_bills and bill_service:
        original_snapshots = [
            {
                "type": str(bill.get("type", "") or "").strip(),
                "main_category": str(bill.get("main_category", "") or "").strip(),
                "sub_category": str(bill.get("sub_category", "") or "").strip(),
                "has_explicit_type": bool(bill.get("_import_has_explicit_type")),
                "has_explicit_category": bool(bill.get("_import_has_explicit_category")),
            }
            for bill in enriched_bills
        ]

        applied_count = await bill_service._apply_import_learning_rules(  # pylint: disable=protected-access
            enriched_bills, user_id=user_id, type_only=False, record_usage=False
        )

        for bill, snapshot in zip(enriched_bills, original_snapshots):
            if snapshot["has_explicit_type"] and snapshot["type"]:
                bill["type"] = snapshot["type"]
            if snapshot["has_explicit_category"]:
                bill["main_category"] = snapshot["main_category"]
                bill["sub_category"] = snapshot["sub_category"]

    accounts = await db.get_all_accounts(user_id=user_id) if db else []
    categories = await db.get_all_categories(user_id=user_id) if db else []

    return (
        enriched_bills,
        _build_account_mapping_payload(accounts),
        _build_category_mapping_payload(categories),
        applied_count,
    )


def _convert_bill_to_import_item_with_mappings(
    bill: dict[str, Any],
    account_mappings: dict[str, Any] | None = None,
    category_mappings: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """将账单转换为导入检查页结构，并补充已匹配的分类/账户ID。"""
    item = _convert_bill_to_import_item(bill)

    source_account_id = bill.get("source_account_id")
    destination_account_id = bill.get("destination_account_id")
    main_category = str(bill.get("main_category", "") or "").strip()
    sub_category = str(bill.get("sub_category", "") or "").strip()

    if source_account_id not in (None, "", 0, "0"):
        item["sourceAccountId"] = str(source_account_id)
    if destination_account_id not in (None, "", 0, "0"):
        item["destinationAccountId"] = str(destination_account_id)

    if category_mappings and main_category:
        category_id = category_mappings.get("name_to_id", {}).get((main_category, sub_category))
        if category_id is None:
            category_id = category_mappings.get("name_to_id", {}).get((main_category, ""))
        if category_id is not None:
            item["categoryId"] = str(category_id)

    if account_mappings:
        id_to_account = account_mappings.get("id_to_account", {})

        if source_account_id not in (None, "", 0, "0"):
            source_account = id_to_account.get(int(source_account_id))
            if source_account:
                item["accountName"] = str(source_account.get("name") or item.get("accountName") or "").strip()
                item["originalSourceAccountName"] = item.get("originalSourceAccountName") or item["accountName"]
                item["originalSourceAccountCurrency"] = (
                    str(source_account.get("currency") or item.get("originalSourceAccountCurrency") or "CNY").strip()
                    or "CNY"
                )

        if destination_account_id not in (None, "", 0, "0"):
            destination_account = id_to_account.get(int(destination_account_id))
            if destination_account:
                item["originalDestinationAccountName"] = (
                    item.get("originalDestinationAccountName") or str(destination_account.get("name") or "").strip()
                )
                item["originalDestinationAccountCurrency"] = (
                    str(
                        destination_account.get("currency") or item.get("originalDestinationAccountCurrency") or "CNY"
                    ).strip()
                    or "CNY"
                )

    return item


def _parse_import_file_with_column_mapping(
    file_path: Path,
    column_mapping: dict[str, Any],
    transaction_type_mapping: dict[str, Any],
    has_header_line: bool,
    time_format: str,
    amount_decimal_separator: str,
    amount_digit_grouping_symbol: str,
    tag_separator: str,
    file_encoding: str,
    delimiter: str,
) -> tuple[list[dict[str, Any]], str, str]:
    """按列映射解析通用表格文件。"""
    rows, actual_encoding, actual_delimiter = _load_generic_import_rows(
        file_path, requested_encoding=file_encoding, delimiter=delimiter
    )
    if not rows:
        return [], actual_encoding, actual_delimiter

    start_index = 1 if has_header_line else 0
    normalized_bills = []

    for row_index, row in enumerate(rows[start_index:], start=1):
        date_value = _get_mapped_cell(row, column_mapping, 1)
        type_value = _get_mapped_cell(row, column_mapping, 3)
        amount_value = _get_mapped_cell(row, column_mapping, 8)

        if not date_value and not type_value and not amount_value:
            continue

        try:
            type_name = _map_generic_import_type(type_value, transaction_type_mapping)
            amount = _parse_generic_import_amount(
                amount_value,
                decimal_separator=amount_decimal_separator or ".",
                grouping_symbol=amount_digit_grouping_symbol or None,
            )
            related_amount_raw = _get_mapped_cell(row, column_mapping, 11)
            related_amount = _parse_generic_import_amount(
                related_amount_raw,
                decimal_separator=amount_decimal_separator or ".",
                grouping_symbol=amount_digit_grouping_symbol or None,
            )

            main_category = _get_mapped_cell(row, column_mapping, 4)
            sub_category = _get_mapped_cell(row, column_mapping, 5)
            raw_tags = _get_mapped_cell(row, column_mapping, 13)
            original_tag_names = []
            if raw_tags:
                separator = tag_separator or ";"
                original_tag_names = [item.strip() for item in raw_tags.split(separator) if item.strip()]

            normalized_bill = {
                "trade_time": datetime.fromtimestamp(_parse_generic_import_time(date_value, time_format)).strftime(
                    "%Y-%m-%d %H:%M:%S"
                ),
                "type": type_name,
                "amount": amount,
                "destination_amount": related_amount or amount,
                "account": _get_mapped_cell(row, column_mapping, 6),
                "account_currency": _get_mapped_cell(row, column_mapping, 7) or "CNY",
                "related_account": _get_mapped_cell(row, column_mapping, 9),
                "related_account_currency": _get_mapped_cell(row, column_mapping, 10) or "CNY",
                "description": _get_mapped_cell(row, column_mapping, 14),
                "main_category": main_category,
                "sub_category": sub_category,
                "counterparty": _get_mapped_cell(row, column_mapping, 9),
                "payment_method": _get_mapped_cell(row, column_mapping, 6),
                "original_tag_names": original_tag_names,
                "_import_has_explicit_type": bool(str(type_value or "").strip()),
                "_import_has_explicit_category": bool(main_category or sub_category),
            }

            normalized_bills.append(normalized_bill)
        except Exception as row_error:  # pylint: disable=broad-except
            logger.warning("[通用导入] 解析第 %s 行失败: %s", row_index, row_error)

    return normalized_bills, actual_encoding, actual_delimiter


def _build_picture_data_url(file_path: Path) -> str:
    """将图片文件转换为 data URL，供前端直接预览。"""
    mime_type, _ = mimetypes.guess_type(str(file_path))
    if not mime_type:
        mime_type = "application/octet-stream"

    with open(file_path, "rb") as file_obj:
        encoded = base64.b64encode(file_obj.read()).decode("ascii")

    return f"data:{mime_type};base64,{encoded}"


def get_app_context(user_id: int = None):
    """获取应用上下文中的基础服务实例。"""
    from flask import current_app

    db = current_app.config.get("DB_INSTANCE")
    bill_service = current_app.config.get("BILL_SERVICE_INSTANCE")
    category_engine = current_app.config.get("CATEGORY_ENGINE_INSTANCE")

    # 自动获取user_id
    if user_id is None:
        user_id = getattr(request, "user_id", 1)

    _ = user_id

    return db, bill_service, category_engine


def get_app_context_with_adapter(user_id: int = None):
    """获取应用上下文中的基础服务实例及事务适配器。"""
    db, bill_service, category_engine = get_app_context(user_id=user_id)

    if user_id is None:
        user_id = getattr(request, "user_id", 1)

    # 创建adapter实例(带数据库引用和用户ID)
    adapter = TransactionAdapter(db=db, user_id=user_id)

    return db, bill_service, category_engine, adapter


async def sync_balances_for_bill(db, bill_data):
    """同步账单相关账户的余额

    Args:
        db: 数据库实例
        bill_data: 账单数据字典 (包含 source_account_id, destination_account_id)
    """
    try:
        # 同步源账户
        source_id = bill_data.get("source_account_id")
        if source_id:
            await db.sync_account_balance(int(source_id))
            logger.info(f"已同步源账户余额: {source_id}")

        # 同步目标账户 (转账/投资)
        dest_id = bill_data.get("destination_account_id")
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
        return "0"

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
    loop.close()

    for cat in categories:
        if cat.get("main_category") == main_category and cat.get("sub_category", "") == sub_category:
            return str(cat["id"])

    return "0"


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
        return datetime.strptime(date_str, "%Y-%m-%d %H:%M:%S")
    except ValueError:
        pass

    # 回退到只有日期的格式
    try:
        return datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError as e:
        logger.error(f"无法解析日期字符串: {date_str}, 错误: {e}")
        return datetime.now()  # 返回当前时间作为默认值


@bp.route("/", methods=["GET"])
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
        page = int(_get_query_arg(request.args, "page", default=1))
        page_size = int(_get_query_arg(request.args, "page_size", "count", default=20))
        max_time = int(_get_query_arg(request.args, "max_time", default=0) or 0)
        min_time = int(_get_query_arg(request.args, "min_time", default=0) or 0)

        # 构建过滤条件
        filters = {}
        # 交易类型映射：前端v1格式(数字) -> 后端格式(中文)
        # 0=全部(不过滤), 2=收入, 3=支出, 4=转账, 5=投资
        if request.args.get("type"):
            type_param = request.args.get("type")
            try:
                type_int = int(type_param)
                type_mapping = {2: "收入", 3: "支出", 4: "转账", 5: "投资"}
                # type=0表示全部类型，不设置过滤条件
                if type_int in type_mapping:
                    filters["type"] = type_mapping[type_int]
                elif type_int != 0:
                    logger.warning(f"未知的type参数: {type_int}, 已忽略")
            except ValueError:
                # 如果不是数字，认为是中文类型名，直接使用
                filters["type"] = type_param
        if request.args.get("main_category"):
            filters["main_category"] = request.args.get("main_category")
        if request.args.get("sub_category"):
            filters["sub_category"] = request.args.get("sub_category")
        if request.args.get("start_date"):
            filters["start_date"] = request.args.get("start_date")
        if request.args.get("end_date"):
            filters["end_date"] = request.args.get("end_date")
        if min_time > 0:
            filters["start_date"] = datetime.fromtimestamp(min_time / 1000).strftime("%Y-%m-%d")
        if max_time > 0:
            filters["end_date"] = datetime.fromtimestamp(max_time / 1000).strftime("%Y-%m-%d")

        db, _, _, adapter = get_app_context_with_adapter()

        # 异步调用
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        loop.run_until_complete(_apply_common_transaction_filters(request.args, filters, db, request.user_id))
        bills, total = loop.run_until_complete(
            db.query_bills(page=page, page_size=page_size, filters=filters, user_id=request.user_id)
        )

        # 使用adapter批量转换为v1格式
        response = loop.run_until_complete(adapter.backend_list_to_frontend(bills, total, page, page_size))
        loop.close()

        # 添加额外的分页信息
        response["result"]["total"] = total
        response["result"]["page"] = page
        response["result"]["page_size"] = page_size
        response["result"]["total_pages"] = (total + page_size - 1) // page_size

        return jsonify(response)

    except Exception as e:
        logger.error("获取账单列表失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/pictures", methods=["POST"])
@log_method
@require_auth
def upload_transaction_picture_rest():
    """上传交易图片（REST 主链）。"""
    try:
        if "picture" not in request.files:
            return jsonify({"success": False, "error": "Missing picture file"}), 400

        picture = request.files["picture"]
        if not picture or not picture.filename:
            return jsonify({"success": False, "error": "Invalid picture file"}), 400

        if not allowed_picture_file(picture.filename):
            return jsonify(
                {
                    "success": False,
                    "error": f"Picture type not allowed. Supported: {', '.join(sorted(ALLOWED_PICTURE_EXTENSIONS))}",
                }
            ), 400

        filename = secure_filename(picture.filename)
        suffix = Path(filename).suffix.lower()
        picture_id = f"{uuid.uuid4().hex}{suffix}"
        file_path = UPLOAD_FOLDER / picture_id
        picture.save(str(file_path))

        logger.info("[交易图片上传] user_id=%s, picture_id=%s", request.user_id, picture_id)

        return jsonify(
            {"success": True, "result": {"pictureId": picture_id, "originalUrl": _build_picture_data_url(file_path)}}
        )

    except Exception as e:
        logger.error("上传交易图片失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/pictures/unused", methods=["POST"])
@log_method
@require_auth
def remove_unused_transaction_picture_rest():
    """删除未使用的交易图片（REST 主链）。"""
    try:
        data = request.get_json() or {}
        picture_id = str(data.get("id", "") or "").strip()

        if not picture_id:
            return jsonify({"success": False, "error": "Missing picture id"}), 400

        file_path = UPLOAD_FOLDER / secure_filename(picture_id)
        if file_path.exists() and file_path.is_file():
            os.remove(file_path)
            logger.info("[交易图片删除] user_id=%s, picture_id=%s", request.user_id, picture_id)

        return jsonify({"success": True, "result": True})

    except Exception as e:
        logger.error("删除未使用交易图片失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/by-month", methods=["GET"])
@log_method
@require_auth
def get_bills_rest_by_month():
    """按月查询账单列表（REST 主链）。"""
    try:
        year = int(_get_query_arg(request.args, "year", default=datetime.now().year))
        month = int(_get_query_arg(request.args, "month", default=datetime.now().month))
        transaction_type = _get_query_arg(request.args, "type", default="0")

        filters = {}
        start_date = f"{year:04d}-{month:02d}-01"
        if month == 12:
            end_date = f"{year + 1:04d}-01-01"
        else:
            end_date = f"{year:04d}-{month + 1:02d}-01"

        filters["start_date"] = start_date
        filters["end_date"] = end_date

        if transaction_type and int(transaction_type) > 0:
            type_int = int(transaction_type)
            if type_int in BACKEND_TO_FRONTEND_TYPE.values():
                for chinese, v1_type in BACKEND_TO_FRONTEND_TYPE.items():
                    if v1_type == type_int:
                        filters["type"] = chinese
                        break

        db, _, _, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        loop.run_until_complete(_apply_common_transaction_filters(request.args, filters, db, request.user_id))
        bills, total = loop.run_until_complete(
            db.query_bills(page=1, page_size=10000, filters=filters, user_id=request.user_id)
        )
        response = loop.run_until_complete(adapter.backend_list_to_frontend(bills, total, 1, 10000))
        loop.close()

        return jsonify(response)

    except Exception as e:
        logger.error("按月获取账单列表失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["GET"])
@log_method
@require_auth
def get_bill(bill_id: int):
    """获取单个账单详情"""
    try:
        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({"success": True, "result": v1_bill})

    except Exception as e:
        logger.error("获取账单详情失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-candidates", methods=["GET"])
@log_method
@require_auth
def get_bill_recurring_candidates(bill_id: int):
    """获取账单可匹配的定时交易候选。"""
    try:
        tolerance_days = request.args.get("toleranceDays", default=3, type=int)
        tolerance_days = max(0, min(tolerance_days, 31))

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            db.get_recurring_candidates_for_bill(bill_id, user_id=request.user_id, tolerance_days=tolerance_days)
        )
        loop.close()

        if not result.get("bill"):
            return jsonify({"success": False, "error": "Bill not found"}), 404

        return jsonify(
            {
                "success": True,
                "result": {
                    "billId": bill_id,
                    "linkedRecurringId": result.get("linked_recurring_id"),
                    "linkedRecurringName": result.get("linked_recurring_name", ""),
                    "candidates": result.get("candidates", []),
                },
            }
        )
    except Exception as e:
        logger.error("获取定时交易候选失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-match", methods=["PUT"])
@log_method
@require_auth
def bind_bill_recurring_match(bill_id: int):
    """将账单绑定到定时交易。"""
    try:
        data = request.get_json(silent=True) or {}
        recurring_id = data.get("recurringId")
        if recurring_id in (None, ""):
            return jsonify({"success": False, "error": "Missing recurringId"}), 400

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.bind_bill_to_recurring(bill_id, int(recurring_id), user_id=request.user_id))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Bill or recurring template not found"}), 404

        return jsonify({"success": True, "result": result})
    except Exception as e:
        logger.error("绑定定时交易失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-match", methods=["DELETE"])
@log_method
@require_auth
def unbind_bill_recurring_match(bill_id: int):
    """取消账单与定时交易的绑定。"""
    try:
        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.unbind_bill_from_recurring(bill_id, user_id=request.user_id))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Bill not found"}), 404

        return jsonify({"success": True, "result": True})
    except Exception as e:
        logger.error("取消定时交易绑定失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/get", methods=["GET"])
@log_method
@require_auth
def get_bill_by_query():
    """通过查询参数获取单个账单详情 (v1兼容)"""
    try:
        bill_id = request.args.get("id", type=int)
        if not bill_id:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({"success": True, "result": v1_bill})

    except Exception as e:
        logger.error("获取账单详情失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/modify", methods=["POST"])
@log_method
@require_auth
def modify_bill():
    """修改账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        bill_id = int(data["id"])
        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：先获取原账单类型**
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 转换前端格式
        backend_data, metadata = adapter.frontend_to_backend(data)

        # **关键修复：只更新传入的字段**
        # 如果frontend_data只包含部分字段（如只有remark），只更新那些字段
        if "remark" in data and "type" not in data:
            # 简单更新模式：只有备注等非关键字段
            backend_data = {}
            if "remark" in data:
                backend_data["description"] = data["remark"]
            if "comment" in data:
                backend_data["description"] = data["comment"]
            # 保持原类型
            backend_data["type"] = old_bill["type"]
            logger.info(f"简单更新模式：只更新 description={backend_data.get('description')}")

        # **保持原类型：如果前端没有传type，使用原账单的类型**
        if "type" not in backend_data or not backend_data["type"]:
            backend_data["type"] = old_bill["type"]
            logger.info(f"保持原类型: {old_bill['type']}")

        # source_account_id已经在adapter中设置，无需额外查询
        # 保持原有的source_account_id
        if "source_account_id" not in backend_data and old_bill:
            backend_data["source_account_id"] = old_bill.get("source_account_id", 0)

        # 查询分类
        if metadata.get("category_id"):
            try:
                category = loop.run_until_complete(
                    db.get_category_by_id(int(metadata["category_id"]), user_id=request.user_id)
                )
                if category:
                    backend_data["main_category"] = category.get("main_category")
                    backend_data["sub_category"] = category.get("sub_category")
            except ValueError:
                pass

        # **保持其他关键字段**
        for field in ["destination_account_id", "destination_amount"]:
            if field not in backend_data and field in old_bill:
                backend_data[field] = old_bill[field]
                logger.info(f"保持原字段 {field}: {old_bill[field]}")

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # 更新标签
            if "tagIds" in data:
                loop.run_until_complete(db.update_bill_tags(bill_id, metadata["tag_ids"], user_id=request.user_id))

            # **使用全量同步更新余额**
            # 1. 同步旧账单相关的账户余额
            old_sync_data = {
                "source_account_id": old_bill.get("source_account_id"),
                "destination_account_id": old_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, old_sync_data))

            # 2. 同步新账单相关的账户余额 (如果账户发生了变化)
            new_sync_data = {
                "source_account_id": backend_data.get("source_account_id", old_bill.get("source_account_id")),
                "destination_account_id": backend_data.get(
                    "destination_account_id", old_bill.get("destination_account_id")
                ),
            }
            loop.run_until_complete(sync_balances_for_bill(db, new_sync_data))

        loop.close()

        if result:
            return jsonify({"success": True, "result": {"id": str(bill_id)}})
        else:
            return jsonify({"success": False, "error": "Failed to update bill"}), 500

    except Exception as e:
        logger.error("修改账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/delete", methods=["POST"])
@log_method
@require_auth
def delete_bill_by_query():
    """删除账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        bill_id = int(data["id"])
        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：添加余额回滚逻辑**
        # 先获取账单信息
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            sync_data = {
                "source_account_id": bill.get("source_account_id"),
                "destination_account_id": bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data))

        loop.close()

        if result:
            return jsonify({"success": True})
        else:
            return jsonify({"success": False, "error": "Failed to delete bill"}), 500

    except Exception as e:
        logger.error("删除账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


def _prepare_backend_bill_for_create(
    frontend_data: dict[str, Any], db, category_engine, adapter, loop, user_id: int
) -> tuple[dict[str, Any], dict[str, Any]]:
    """将前端交易转换为可写入数据库的账单数据。"""
    backend_data, metadata = adapter.frontend_to_backend(frontend_data)
    logger.info("转换后的后端数据: %s", backend_data)
    logger.info("元数据: %s", metadata)

    # v6.89: create_bill() 会直接按传入字段构造 INSERT，bills.counterparty 为 NOT NULL。
    # 批量手工录入请求通常不显式提供 counterparty，因此这里统一补齐回退值，
    # 同时也保证 description 在前端未填写时仍有可写入的默认文本。
    description_fallback = str(
        frontend_data.get("comment") or frontend_data.get("remark") or frontend_data.get("description") or ""
    ).strip()
    counterparty_fallback = str(
        frontend_data.get("counterparty")
        or frontend_data.get("payee")
        or frontend_data.get("merchant")
        or frontend_data.get("merchantName")
        or frontend_data.get("shopName")
        or frontend_data.get("targetAccountName")
        or description_fallback
        or backend_data.get("payment_method")
        or "手工录入"
    ).strip()

    backend_data["description"] = str(
        backend_data.get("description") or description_fallback or counterparty_fallback
    ).strip()
    backend_data["counterparty"] = str(backend_data.get("counterparty") or counterparty_fallback).strip()

    if metadata.get("auto_invest_account", False) and backend_data.get("type") == "投资":
        all_accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        investment_account = None

        for acc in all_accounts:
            if acc["name"] in ["活期资产", "投资账户", "中信建投证券"]:
                investment_account = acc
                break

        if investment_account:
            backend_data["destination_account_id"] = investment_account["id"]
            backend_data["destination_amount"] = backend_data["amount"]
            logger.info("投资自动设置目标账户: %s (ID=%s)", investment_account["name"], investment_account["id"])
        else:
            logger.warning("未找到合适的投资目标账户，destination_account_id保持为0")

    source_account_id = backend_data.get("source_account_id")
    logger.info("[创建账单] 收到的source_account_id: %s (类型: %s)", source_account_id, type(source_account_id))
    logger.info(
        "[创建账单] 收到的destination_account_id: %s (类型: %s)",
        backend_data.get("destination_account_id"),
        type(backend_data.get("destination_account_id")),
    )

    if not source_account_id or source_account_id == 0:
        all_accounts_fallback = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        if all_accounts_fallback:
            fallback_account = all_accounts_fallback[0]
            backend_data["source_account_id"] = fallback_account["id"]
            logger.warning(
                "⚠ source_account_id为0，使用默认账户: %s (ID=%s)", fallback_account["name"], fallback_account["id"]
            )
        else:
            logger.error("✗✗✗ 没有可用账户，创建将失败！")
            raise ValueError("No account available")

    if metadata.get("category_id"):
        try:
            category = loop.run_until_complete(db.get_category_by_id(int(metadata["category_id"]), user_id=user_id))
            if category:
                backend_data["main_category"] = category.get("main_category", "")
                backend_data["sub_category"] = category.get("sub_category", "")
                logger.info("查询到分类: %s - %s", backend_data["main_category"], backend_data["sub_category"])
        except ValueError:
            logger.error("无效的分类ID: %s", metadata["category_id"])

    if not backend_data.get("main_category"):
        main_cat, sub_cat = category_engine.match_category(backend_data)
        if main_cat:
            backend_data["main_category"] = main_cat
            backend_data["sub_category"] = sub_cat
            logger.info("自动分类（规则匹配）: %s - %s", main_cat, sub_cat)
        else:
            default_categories = {
                "收入": ("工资", ""),
                "支出": ("其他", "日常支出"),
                "转账": ("转账", ""),
                "投资": ("投资理财", "证券投资"),
            }
            bill_type = backend_data.get("type", "支出")
            if bill_type in default_categories:
                backend_data["main_category"], backend_data["sub_category"] = default_categories[bill_type]
                logger.info(
                    "自动分类（默认分类）: %s - %s", backend_data["main_category"], backend_data["sub_category"]
                )
            else:
                backend_data["main_category"] = "其他"
                backend_data["sub_category"] = ""
                logger.warning("未知账单类型: %s，使用默认分类'其他'", bill_type)

    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    backend_data["created_at"] = now
    backend_data["updated_at"] = now

    logger.info("=" * 60)
    logger.info("📝 准备插入数据库的完整数据:")
    logger.info("  - type: %s", backend_data.get("type"))
    logger.info("  - amount: %s", backend_data.get("amount"))
    logger.info("  - counterparty: %s", backend_data.get("counterparty"))
    logger.info("  - description: %s", backend_data.get("description"))
    logger.info("  - source_account_id: %s", backend_data.get("source_account_id"))
    logger.info("  - destination_account_id: %s", backend_data.get("destination_account_id"))
    logger.info("  - destination_amount: %s", backend_data.get("destination_amount"))
    logger.info("  - date: %s", backend_data.get("date"))
    logger.info("  - main_category: %s", backend_data.get("main_category"))
    logger.info("  - sub_category: %s", backend_data.get("sub_category"))
    logger.info("=" * 60)

    return backend_data, metadata


def _create_bill_and_build_response(
    backend_data: dict[str, Any], metadata: dict[str, Any], db, adapter, loop, user_id: int
) -> tuple[int, dict[str, Any]]:
    """写入账单并返回前端响应格式。"""
    bill_id = loop.run_until_complete(db.create_bill(backend_data, user_id=user_id))

    if not bill_id:
        raise RuntimeError("Failed to create bill")

    if metadata.get("tag_ids"):
        logger.info("[创建账单] 保存标签: %s", metadata["tag_ids"])
        loop.run_until_complete(db.add_tags_to_bill(bill_id, metadata["tag_ids"], user_id=user_id))

    sync_data = {
        "source_account_id": backend_data.get("source_account_id"),
        "destination_account_id": backend_data.get("destination_account_id"),
    }
    logger.info(
        "🔄 同步账户余额: source=%s, dest=%s", sync_data["source_account_id"], sync_data["destination_account_id"]
    )
    loop.run_until_complete(sync_balances_for_bill(db, sync_data))

    bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=user_id))
    tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=user_id))
    frontend_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))

    return bill_id, frontend_bill


@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_bill():
    """创建单条账单"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.info("收到前端数据: %s", frontend_data)

        db, _, category_engine, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            backend_data, metadata = _prepare_backend_bill_for_create(
                frontend_data, db, category_engine, adapter, loop, request.user_id
            )
            bill_id, v1_bill = _create_bill_and_build_response(
                backend_data, metadata, db, adapter, loop, request.user_id
            )

            logger.info("✅ 账单创建成功，ID: %s", bill_id)
            logger.info("返回创建的账单(v1格式): %s", v1_bill)

            return jsonify({"success": True, "result": v1_bill}), 201
        finally:
            loop.close()

    except ValueError as e:
        logger.error("创建账单参数错误: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 400

    except Exception as e:
        logger.error("创建账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def batch_create_bills():
    """批量创建账单。"""
    try:
        payload = request.get_json(silent=True)
        transactions = []

        if isinstance(payload, dict):
            transactions = payload.get("transactions") or payload.get("bills") or []
        elif isinstance(payload, list):
            transactions = payload

        if not isinstance(transactions, list) or not transactions:
            return jsonify({"success": False, "error": "transactions is required"}), 400

        db, _, category_engine, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            prepared_items: list[tuple[dict[str, Any], dict[str, Any]]] = []
            for index, transaction in enumerate(transactions):
                if not isinstance(transaction, dict):
                    return jsonify({"success": False, "error": f"transactions[{index}] must be an object"}), 400

                try:
                    prepared_items.append(
                        _prepare_backend_bill_for_create(
                            transaction, db, category_engine, adapter, loop, request.user_id
                        )
                    )
                except ValueError as prepare_error:
                    logger.error("批量创建预校验失败: index=%s, error=%s", index, prepare_error)
                    return jsonify(
                        {
                            "success": False,
                            "error": str(prepare_error),
                            "result": {"failedIndex": index, "createdCount": 0, "items": []},
                        }
                    ), 400

            created_items: list[dict[str, Any]] = []
            created_ids: list[str] = []

            for index, (backend_data, metadata) in enumerate(prepared_items):
                try:
                    bill_id, frontend_bill = _create_bill_and_build_response(
                        backend_data, metadata, db, adapter, loop, request.user_id
                    )
                    created_items.append(frontend_bill)
                    created_ids.append(str(bill_id))
                except Exception as create_error:
                    logger.error("批量创建账单失败: index=%s, error=%s", index, create_error, exc_info=True)
                    return jsonify(
                        {
                            "success": False,
                            "error": str(create_error),
                            "result": {
                                "failedIndex": index,
                                "createdCount": len(created_items),
                                "items": created_items,
                                "ids": created_ids,
                            },
                        }
                    ), 500

            return jsonify(
                {
                    "success": True,
                    "result": {"items": created_items, "ids": created_ids, "createdCount": len(created_items)},
                }
            ), 201
        finally:
            loop.close()

    except Exception as e:
        logger.error("批量创建账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["PUT"])
@log_method
@require_auth
def update_bill(bill_id: int):
    """更新账单(支持v1格式)"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.info(f"更新账单 {bill_id}, 收到前端数据: {frontend_data}")

        db, _, category_engine, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取原账单，用于余额回滚
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 判断是前端格式还是后端格式
        if "sourceAmount" in frontend_data or "sourceAccountId" in frontend_data:
            # 前端v1格式,需要转换
            backend_data, metadata = adapter.frontend_to_backend(frontend_data)

            # source_account_id已在adapter中处理，无需额外查询
            # 验证source_account_id是否有效即可
            if metadata.get("source_account_id"):
                logger.info(f"源账户ID: {metadata['source_account_id']}")

            # 查询分类名称
            if metadata.get("category_id"):
                try:
                    category = loop.run_until_complete(
                        db.get_category_by_id(int(metadata["category_id"]), user_id=request.user_id)
                    )
                    if category:
                        backend_data["main_category"] = category.get("main_category", "")
                        backend_data["sub_category"] = category.get("sub_category", "")
                        logger.info(f"查询到分类: {backend_data['main_category']} - {backend_data['sub_category']}")
                except ValueError:
                    logger.error(f"无效的分类ID: {metadata['category_id']}")
        else:
            # 后端格式,直接使用
            backend_data = frontend_data
            metadata = {}
            # 如果有描述或对方变更，重新分类
            if "description" in backend_data or "counterparty" in backend_data:
                # 合并旧数据
                old_bill.update(backend_data)
                # 重新分类
                main_cat, sub_cat = category_engine.match_category(old_bill)
                if main_cat:
                    backend_data["main_category"] = main_cat
                    backend_data["sub_category"] = sub_cat

        backend_data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        logger.info(f"准备更新的后端数据: {backend_data}")

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # **处理标签更新**
            if "tag_ids" in metadata:
                tag_ids = metadata["tag_ids"]
                logger.info(f"[更新账单] 更新标签: {tag_ids}")
                loop.run_until_complete(db.update_bill_tags(bill_id, tag_ids, user_id=request.user_id))
            else:
                logger.debug("[更新账单] 未提供标签数据，保持现有标签")

            # 获取更新后的账单
            updated_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

            # 获取标签
            tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))
            logger.info(f"[更新账单] 获取到标签: {tags}")

            # **同步账户余额**
            # 1. 同步旧账单关联的账户 (回滚旧金额)
            sync_data_old = {
                "source_account_id": old_bill.get("source_account_id"),
                "destination_account_id": old_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_old))

            # 2. 同步新账单关联的账户 (应用新金额)
            # 如果账户ID没变，其实会被同步两次，但这保证了数据的最终一致性
            sync_data_new = {
                "source_account_id": updated_bill.get("source_account_id"),
                "destination_account_id": updated_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_new))

            # 使用adapter转换为v1格式
            v1_bill = loop.run_until_complete(adapter.backend_to_frontend(updated_bill, tags=tags))
            loop.close()

            return jsonify({"success": True, "result": v1_bill})

        loop.close()
        return jsonify({"success": False, "error": "Bill not found or update failed"}), 404

    except Exception as e:
        logger.error(f"更新账单失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_bill(bill_id: int):
    """删除账单"""
    try:
        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 先获取账单信息，用于回滚余额
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 删除账单
        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            try:
                sync_data = {
                    "source_account_id": bill.get("source_account_id"),
                    "destination_account_id": bill.get("destination_account_id"),
                }
                loop.run_until_complete(sync_balances_for_bill(db, sync_data))
            except Exception as e:
                logger.error(f"删除账单后同步余额失败: {e}", exc_info=True)
                # 不阻断删除成功的响应

            loop.close()
            return jsonify({"success": True, "result": True, "message": "Bill deleted successfully"})

        loop.close()
        return jsonify({"success": False, "error": "Failed to delete bill"}), 500
    except Exception as e:
        logger.error("删除账单失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def import_bills_batch():
    """批量导入账单"""
    try:
        data = request.get_json()
        if not data or "file_path" not in data:
            return jsonify({"success": False, "error": "file_path is required"}), 400

        _, bill_service, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(bill_service.import_bills(data["file_path"]))
        loop.close()

        return jsonify({"success": result["success"], "result": result})

    except Exception as e:
        logger.error(f"批量导入账单失败: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/upload", methods=["POST"])
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
        if "file" not in request.files:
            return jsonify({"success": False, "error": "No file provided"}), 400

        file = request.files["file"]

        # 检查文件名是否为空
        if file.filename == "":
            return jsonify({"success": False, "error": "No file selected"}), 400

        # 检查文件扩展名
        if not allowed_file(file.filename):
            return jsonify(
                {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
            ), 400

        # 获取解析器类型
        parser_type = request.form.get("parser_type", "auto")
        preview_only = request.form.get("preview_only", "false").lower() == "true"

        # 保存文件
        if file.filename is None:
            return jsonify({"success": False, "error": "Invalid filename"}), 400

        filename = secure_filename(file.filename)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        unique_filename = f"{timestamp}_{filename}"
        file_path = UPLOAD_FOLDER / unique_filename

        file.save(str(file_path))
        logger.info(f"文件已保存: {file_path}")

        # 导入账单
        _, bill_service, _ = get_app_context()

        # 获取用户ID
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取解析结果
        result = loop.run_until_complete(
            bill_service.import_bills(
                str(file_path), parser_type=parser_type, preview_only=preview_only, user_id=user_id
            )
        )

        loop.close()

        # 如果不是预览模式且导入成功，删除临时文件
        if not preview_only and result.get("success"):
            try:
                os.remove(file_path)
                logger.info(f"临时文件已删除: {file_path}")
            except Exception as e:
                logger.warning(f"删除临时文件失败: {e}")

        # 日志记录返回数据
        preview_count = len(result.get("preview", []))
        logger.info(
            f"[导入API返回] success={result.get('success')}, "
            f"preview_count={preview_count}, "
            f"total={result.get('total')}, valid={result.get('valid')}"
        )

        # 前端期望格式: { success, data: { preview: [...] } }
        return jsonify(
            {
                "success": result.get("success", False),
                "data": {
                    "preview": result.get("preview", []),
                    "total": result.get("total", 0),
                    "valid": result.get("valid", 0),
                    "invalid": result.get("invalid", 0),
                    "inserted": result.get("inserted", 0),
                    "duplicates": result.get("duplicates", 0),
                    "dedup_stats": result.get("dedup_stats"),
                    "parser_type": result.get("parser_type", "unknown"),
                    "errors": result.get("errors", []),
                },
            }
        )

    except Exception as e:
        logger.error(f"上传并导入账单失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/parsers", methods=["GET"])
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
                "id": "auto",
                "name": "自动识别",
                "description": "自动检测文件类型并选择合适的解析器",
                "supported_formats": ["csv"],
            },
            {
                "id": "wechat",
                "name": "微信支付",
                "description": "解析微信支付账单CSV文件",
                "supported_formats": ["csv"],
            },
            {
                "id": "alipay",
                "name": "支付宝",
                "description": "解析支付宝交易明细CSV文件",
                "supported_formats": ["csv"],
            },
            {
                "id": "icbc",
                "name": "工商银行",
                "description": "解析工商银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "cmbc",
                "name": "招商银行",
                "description": "解析招商银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "abc",
                "name": "农业银行",
                "description": "解析农业银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "ccb",
                "name": "建设银行",
                "description": "解析建设银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
        ]

        return jsonify({"success": True, "result": parsers})

    except Exception as e:
        logger.error(f"获取解析器列表失败: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/reclassify", methods=["POST"])
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
        if not data or "transactions" not in data:
            return jsonify({"success": False, "error": "缺少transactions字段"}), 400

        transactions = data["transactions"]
        if not isinstance(transactions, list):
            return jsonify({"success": False, "error": "transactions必须是数组"}), 400

        logger.info(f"[重新分类] 收到 {len(transactions)} 条交易")

        # 获取分类引擎和数据库
        db, _, category_engine = get_app_context()
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
                key = (cat.get("main_category", ""), cat.get("sub_category", ""))
                category_map[key] = cat

            results = []
            for idx, trans in enumerate(transactions):
                result = {"index": idx}

                # 构建用于分类匹配的bill结构
                bill = {
                    "description": trans.get("description", ""),
                    "counterparty": trans.get("counterparty", ""),
                    "amount": float(trans.get("amount", 0)),
                    "type": trans.get("type", "支出"),
                }

                # 1. 分类匹配
                main_cat, sub_cat = category_engine.match_category(bill)
                if main_cat:
                    result["mainCategory"] = main_cat
                    result["subCategory"] = sub_cat or ""
                    # 查找分类ID
                    cat_info = category_map.get((main_cat, sub_cat or ""))
                    if cat_info:
                        result["categoryId"] = str(cat_info.get("id", ""))
                        result["categoryName"] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                    else:
                        result["categoryId"] = ""
                        result["categoryName"] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                else:
                    result["categoryId"] = ""
                    result["categoryName"] = ""

                # 2. 账户匹配（通过账户名称或别名）
                original_source = trans.get("originalSourceAccountName", "")
                original_dest = trans.get("originalDestinationAccountName", "")

                if original_source:
                    source_account = _match_account_by_name(all_accounts, original_source)
                    result["sourceAccountId"] = str(source_account["id"]) if source_account else ""
                    result["sourceAccountName"] = source_account["name"] if source_account else ""

                if original_dest:
                    dest_account = _match_account_by_name(all_accounts, original_dest)
                    result["destinationAccountId"] = str(dest_account["id"]) if dest_account else ""
                    result["destinationAccountName"] = dest_account["name"] if dest_account else ""

                results.append(result)

            logger.info(f"[重新分类] 完成 {len(results)} 条交易的重新分类")

            return jsonify({"success": True, "result": results})

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"重新分类失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


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
        if account.get("name", "").lower() == name_lower:
            return account

        # 匹配别名（存储在comment字段或aliases字段）
        aliases_str = account.get("aliases", "") or account.get("comment", "")
        if aliases_str:
            aliases = [a.strip().lower() for a in aliases_str.split(",")]
            if name_lower in aliases:
                return account

    # 模糊匹配：名称包含关系
    for account in accounts:
        account_name = account.get("name", "").lower()
        if name_lower in account_name or account_name in name_lower:
            return account

    return None


@bp.route("/import/v2/reclassify/<session_id>", methods=["POST"])
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

        data = request.get_json(silent=True) or {}
        preview_updates = data.get("preview_updates") or []

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 1. 执行重新分类
            reclassify_result = loop.run_until_complete(
                bill_service.reclassify_preview_bills(
                    session_id, preview_updates=preview_updates, user_id=request.user_id
                )
            )

            if not reclassify_result.get("success"):
                return jsonify(
                    {
                        "success": False,
                        "error": reclassify_result.get("errors", ["未知错误"])[0]
                        if reclassify_result.get("errors")
                        else "重新分类失败",
                    }
                ), 500

            # 2. 获取更新后的预览数据
            preview_data = loop.run_until_complete(bill_service.get_import_preview(session_id))

            logger.info(
                f"[v2重新分类] 完成 session={session_id}, "
                f"total={reclassify_result.get('total')}, "
                f"categorized={reclassify_result.get('categorized')}, "
                f"account_matched={reclassify_result.get('account_matched')}"
            )

            return jsonify(
                {
                    "success": True,
                    "data": {
                        "session_id": session_id,
                        "total": reclassify_result.get("total", 0),
                        "categorized": reclassify_result.get("categorized", 0),
                        "account_matched": reclassify_result.get("account_matched", 0),
                        "session_samples_saved": reclassify_result.get("session_samples_saved", 0),
                        "annotation_applied": reclassify_result.get("annotation_applied", 0),
                        "preview": preview_data,
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[v2重新分类] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/learning/<session_id>/promote", methods=["POST"])
@log_method
@require_auth
def promote_import_learning(session_id: str):
    """将当前导入会话中的人工标注提升为长期学习规则。"""
    try:
        logger.info("[长期学习提升] session_id=%s, user_id=%s", session_id, request.user_id)
        data = request.get_json(silent=True) or {}
        preview_updates = data.get("preview_updates") or []

        _, bill_service, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            promote_result = loop.run_until_complete(
                bill_service.promote_session_annotations_to_learning(
                    session_id, preview_updates=preview_updates, user_id=request.user_id
                )
            )

            return jsonify({"success": True, "data": promote_result})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[长期学习提升] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules", methods=["GET"])
@log_method
@require_auth
def list_import_learning_rules():
    """获取当前用户的长期导入学习规则列表。"""
    try:
        db, _, _ = get_app_context()
        page = max(int(request.args.get("page", 1) or 1), 1)
        page_size = int(request.args.get("pageSize", request.args.get("limit", 100)) or 100)
        enabled_only = str(request.args.get("enabledOnly", "")).lower() in ("1", "true", "yes")

        if page_size == 0:
            page_size = 100

        page_size = max(min(page_size, 500), -1)
        limit = page_size if page_size > 0 else None
        offset = (page - 1) * page_size if page_size > 0 else 0

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            total_count = loop.run_until_complete(
                db.count_import_learning_rules(user_id=request.user_id, enabled_only=enabled_only)
            )

            rules = loop.run_until_complete(
                db.get_import_learning_rules(
                    user_id=request.user_id, enabled_only=enabled_only, limit=limit, offset=offset
                )
            )

            categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            categories_by_id = {
                int(category["id"]): category for category in categories if category.get("id") is not None
            }
            accounts_by_id = {int(account["id"]): account for account in accounts if account.get("id") is not None}

            result = []
            for rule in rules:
                learned_category_id = rule.get("learned_category_id")
                learned_source_account_id = rule.get("learned_source_account_id")
                learned_destination_account_id = rule.get("learned_destination_account_id")

                learned_category = categories_by_id.get(int(learned_category_id)) if learned_category_id else None
                source_account = (
                    accounts_by_id.get(int(learned_source_account_id)) if learned_source_account_id else None
                )
                destination_account = (
                    accounts_by_id.get(int(learned_destination_account_id)) if learned_destination_account_id else None
                )

                result.append(
                    {
                        "id": rule.get("id"),
                        "matchType": rule.get("match_type", ""),
                        "matchValue": rule.get("match_value", ""),
                        "learnedType": rule.get("learned_type", ""),
                        "learnedCategoryId": rule.get("learned_category_id") or "",
                        "learnedCategoryName": (
                            f"{learned_category.get('main_category', '')}/{learned_category.get('sub_category', '')}"
                            if learned_category and learned_category.get("sub_category")
                            else (learned_category.get("main_category", "") if learned_category else "")
                        ),
                        "learnedSourceAccountId": rule.get("learned_source_account_id") or "",
                        "learnedSourceAccountName": source_account.get("name", "") if source_account else "",
                        "learnedDestinationAccountId": rule.get("learned_destination_account_id") or "",
                        "learnedDestinationAccountName": destination_account.get("name", "")
                        if destination_account
                        else "",
                        "enabled": bool(rule.get("enabled", 1)),
                        "appliedCount": int(rule.get("applied_count", 0) or 0),
                        "createdAt": rule.get("created_at", ""),
                        "updatedAt": rule.get("updated_at", ""),
                        "lastAppliedAt": rule.get("last_applied_at", ""),
                    }
                )

            total_pages = 1
            if page_size > 0:
                total_pages = max((total_count + page_size - 1) // page_size, 1)

            effective_page = min(page, total_pages) if total_count > 0 else 1

            response = jsonify(
                {
                    "success": True,
                    "result": result,
                    "totalCount": total_count,
                    "page": effective_page,
                    "pageSize": page_size,
                    "totalPages": total_pages,
                }
            )
            response.headers["Cache-Control"] = "no-store, no-cache, must-revalidate, max-age=0"
            response.headers["Pragma"] = "no-cache"
            response.headers["Expires"] = "0"
            return response
        finally:
            loop.close()

    except Exception as e:
        logger.error("[长期学习规则列表] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules/<int:rule_id>", methods=["PUT"])
@log_method
@require_auth
def update_import_learning_rule(rule_id: int):
    """启用或禁用单条长期导入学习规则。"""
    try:
        data = request.get_json(silent=True) or {}
        if "enabled" not in data:
            return jsonify({"success": False, "error": "enabled is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(
                db.set_import_learning_rule_enabled(rule_id, bool(data.get("enabled")), user_id=request.user_id)
            )
            if not success:
                return jsonify({"success": False, "error": "Rule not found"}), 404

            return jsonify({"success": True, "result": True})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[更新长期学习规则] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/learning-rules/<int:rule_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_import_learning_rule(rule_id: int):
    """删除单条长期导入学习规则。"""
    try:
        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.delete_import_learning_rule(rule_id, user_id=request.user_id))
            if not success:
                return jsonify({"success": False, "error": "Rule not found"}), 404

            return jsonify({"success": True, "result": True})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[删除长期学习规则] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs", methods=["GET"])
@log_method
@require_auth
def list_import_configs():
    """获取当前用户的导入列映射模板。"""
    try:
        db, _, _ = get_app_context()
        file_format = str(request.args.get("file_format", "") or "").strip().lower() or None
        limit = int(request.args.get("limit", 100) or 100)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            configs = loop.run_until_complete(
                db.get_import_configs(user_id=request.user_id, file_format=file_format, limit=limit)
            )

            result = []
            for config in configs:
                result.append(
                    {
                        "id": config.get("id"),
                        "name": config.get("name", ""),
                        "fileFormat": config.get("file_format", ""),
                        "description": config.get("description", ""),
                        "descriptionSummary": config.get("description_summary", ""),
                        "fieldMappings": config.get("field_mappings", {}),
                        "dateFormat": config.get("date_format", ""),
                        "encoding": config.get("encoding", "utf-8"),
                        "delimiter": config.get("delimiter"),
                        "skipRows": int(config.get("skip_rows", 0) or 0),
                        "hasHeader": bool(config.get("has_header", True)),
                        "customRules": config.get("custom_rules", {}),
                        "sampleHeaders": config.get("sample_headers", []),
                        "headerSignature": config.get("header_signature", ""),
                        "isDefault": bool(config.get("is_default", False)),
                        "defaultRecommendation": bool(config.get("default_recommendation", False)),
                        "useCount": int(config.get("use_count", 0) or 0),
                        "lastUsedAt": config.get("last_used_at", ""),
                        "createdAt": config.get("created_at", ""),
                        "updatedAt": config.get("updated_at", ""),
                    }
                )

            return jsonify({"success": True, "result": result})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[导入模板列表] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs", methods=["POST"])
@log_method
@require_auth
def save_import_config():
    """保存导入列映射模板。"""
    try:
        data = request.get_json(silent=True) or {}
        if not data.get("name"):
            return jsonify({"success": False, "error": "name is required"}), 400
        if not data.get("fileFormat"):
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not data.get("fieldMappings"):
            return jsonify({"success": False, "error": "fieldMappings is required"}), 400

        db, _, _ = get_app_context()
        payload = {
            "id": data.get("id"),
            "name": data.get("name"),
            "file_format": data.get("fileFormat"),
            "description": data.get("description", ""),
            "field_mappings": data.get("fieldMappings", {}),
            "date_format": data.get("dateFormat", ""),
            "encoding": data.get("encoding", "utf-8"),
            "delimiter": data.get("delimiter"),
            "skip_rows": data.get("skipRows", 0),
            "has_header": data.get("hasHeader", True),
            "custom_rules": data.get("customRules", {}),
            "sample_headers": data.get("sampleHeaders") or data.get("headers") or [],
            "is_default": data.get("isDefault", False),
        }

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            config_id = loop.run_until_complete(db.save_import_config(payload, user_id=request.user_id))
            return jsonify({"success": True, "result": {"id": config_id}}), 201
        finally:
            loop.close()

    except ValueError as e:
        return jsonify({"success": False, "error": str(e)}), 400
    except Exception as e:
        logger.error("[保存导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/preview", methods=["POST"])
@log_method
@require_auth
def preview_import_file():
    """预览通用表格导入文件内容。"""
    temp_file_path = None
    try:
        if "file" not in request.files:
            return jsonify({"success": False, "error": "No file provided"}), 400

        file = request.files["file"]
        if not file or not file.filename:
            return jsonify({"success": False, "error": "No file selected"}), 400
        if not allowed_file(file.filename):
            return jsonify(
                {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
            ), 400

        filename = secure_filename(file.filename)
        unique_filename = f"preview_{datetime.now().strftime('%Y%m%d_%H%M%S')}_{filename}"
        temp_file_path = UPLOAD_FOLDER / unique_filename
        file.save(str(temp_file_path))

        requested_encoding = str(request.form.get("fileEncoding", "") or "").strip()
        requested_delimiter = str(request.form.get("delimiter", "") or "").strip() or None
        rows, actual_encoding, actual_delimiter = _load_generic_import_rows(
            temp_file_path, requested_encoding=requested_encoding, delimiter=requested_delimiter
        )

        headers = rows[0] if rows else []
        sample_rows = rows[1:11] if len(rows) > 1 else []

        return jsonify(
            {
                "success": True,
                "result": {
                    "headers": headers,
                    "sampleData": rows[:50],
                    "previewRows": sample_rows,
                    "totalRows": max(len(rows) - 1, 0),
                    "encoding": actual_encoding,
                    "delimiter": actual_delimiter,
                },
            }
        )
    except Exception as e:  # pylint: disable=broad-except
        logger.error("[导入文件预览] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500
    finally:
        if temp_file_path and temp_file_path.exists():
            try:
                os.remove(temp_file_path)
            except OSError:
                logger.warning("[导入文件预览] 删除临时文件失败: %s", temp_file_path)


@bp.route("/import/configs/match", methods=["POST"])
@log_method
@require_auth
def match_import_config():
    """根据文件格式与表头自动匹配导入模板。"""
    try:
        data = request.get_json(silent=True) or {}
        file_format = str(data.get("fileFormat", "") or "").strip().lower()
        headers = data.get("headers") or []
        if not file_format:
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not isinstance(headers, list) or not headers:
            return jsonify({"success": False, "error": "headers is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            matched = loop.run_until_complete(
                db.find_matching_import_config(file_format=file_format, headers=headers, user_id=request.user_id)
            )

            if not matched:
                return jsonify({"success": True, "result": None})

            return jsonify(
                {
                    "success": True,
                    "result": {
                        "id": matched.get("id"),
                        "name": matched.get("name", ""),
                        "fileFormat": matched.get("file_format", ""),
                        "description": matched.get("description", ""),
                        "descriptionSummary": matched.get("description_summary", ""),
                        "fieldMappings": matched.get("field_mappings", {}),
                        "dateFormat": matched.get("date_format", ""),
                        "encoding": matched.get("encoding", "utf-8"),
                        "delimiter": matched.get("delimiter"),
                        "skipRows": int(matched.get("skip_rows", 0) or 0),
                        "hasHeader": bool(matched.get("has_header", True)),
                        "customRules": matched.get("custom_rules", {}),
                        "sampleHeaders": matched.get("sample_headers", []),
                        "defaultRecommendation": bool(matched.get("default_recommendation", False)),
                        "matchScore": matched.get("match_score", 0),
                        "matchReason": matched.get("match_reason", ""),
                        "matchedHeaderCount": matched.get("matched_header_count", 0),
                    },
                }
            )
        finally:
            loop.close()

    except Exception as e:
        logger.error("[匹配导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs/suggest", methods=["POST"])
@log_method
@require_auth
def suggest_import_config():
    """基于表头和样本行自动建议列映射。"""
    try:
        data = request.get_json(silent=True) or {}
        file_format = str(data.get("fileFormat", "") or "").strip().lower()
        headers = data.get("headers") or []
        sample_rows = data.get("sampleRows") or []
        if not file_format:
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not isinstance(headers, list) or not headers:
            return jsonify({"success": False, "error": "headers is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            configs = loop.run_until_complete(
                db.get_import_configs(user_id=request.user_id, file_format=file_format, limit=200)
            )
        finally:
            loop.close()

        suggestion = _build_import_mapping_suggestion(headers, configs, sample_rows)
        return jsonify({"success": True, "result": suggestion})

    except Exception as e:
        logger.error("[导入模板建议] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs/<int:config_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_import_config(config_id: int):
    """删除导入列映射模板。"""
    try:
        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.delete_import_config(config_id, user_id=request.user_id))
            if not success:
                return jsonify({"success": False, "error": "Config not found"}), 404

            return jsonify({"success": True, "result": True})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[删除导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/confirm", methods=["POST"])
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
        if not data or "bills" not in data:
            return jsonify({"success": False, "error": "bills is required"}), 400

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(bill_service.import_preview_confirmed(data["bills"], user_id=user_id))
        loop.close()

        return jsonify({"success": result.get("success", False), "result": result})

    except Exception as e:
        logger.error(f"确认导入失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/category/quick-add-keyword", methods=["POST"])
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
            return jsonify({"success": False, "error": "Request body is required"}), 400

        main_category = data.get("main_category")
        sub_category = data.get("sub_category")
        keyword = data.get("keyword")

        if not main_category or not keyword:
            return jsonify({"success": False, "error": "main_category and keyword are required"}), 400

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        success = loop.run_until_complete(
            bill_service.add_category_keyword(main_category, sub_category, keyword, user_id=user_id)
        )
        loop.close()

        if success:
            return jsonify({"success": True, "message": "Keyword added successfully"})
        else:
            return jsonify({"success": False, "error": "Failed to add keyword"}), 400

    except Exception as e:
        logger.error(f"添加关键词失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/category/refresh", methods=["POST"])
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
        bill_ids = data.get("bill_ids")

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(bill_service.refresh_category_for_bills(bill_ids, user_id=user_id))
        loop.close()

        return jsonify({"success": result.get("success", False), "result": result})

    except Exception as e:
        logger.error(f"刷新分类失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch/update", methods=["PUT"])
@log_method
@require_auth
def batch_update_bills():
    """批量更新账单"""
    try:
        data = request.get_json()
        if not data or "ids" not in data or "updates" not in data:
            return jsonify({"success": False, "error": "ids and updates are required"}), 400

        db, _, _ = get_app_context()

        ids = data["ids"]
        updates = data["updates"]
        updates["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.batch_update_bills(ids, updates, user_id=request.user_id))
        loop.close()

        return jsonify({"success": True, "result": {"updated_count": result}})

    except Exception as e:
        logger.error(f"批量更新账单失败: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch/delete", methods=["DELETE"])
@log_method
@require_auth
def batch_delete_bills():
    """批量删除账单"""
    try:
        data = request.get_json()
        if not data or "ids" not in data:
            return jsonify({"success": False, "error": "ids are required"}), 400

        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **收集受影响的账户ID**
        affected_accounts = set()
        # 注意：如果批量删除数量很大，这里可能会慢。但通常批量删除是分页的。
        # 为了准确同步余额，我们需要知道哪些账户被影响了。
        for bill_id in data["ids"]:
            try:
                bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
                if bill:
                    if bill.get("source_account_id"):
                        affected_accounts.add(int(bill["source_account_id"]))
                    if bill.get("destination_account_id"):
                        affected_accounts.add(int(bill["destination_account_id"]))
            except Exception as e:
                logger.warning(f"获取账单信息失败 (ID: {bill_id}): {e}")

        result = loop.run_until_complete(db.batch_delete_bills(data["ids"], user_id=request.user_id))

        # **同步余额**
        if result > 0:
            logger.info(f"批量删除成功，开始同步 {len(affected_accounts)} 个账户的余额")
            for account_id in affected_accounts:
                try:
                    loop.run_until_complete(db.sync_account_balance(account_id))
                except Exception as e:
                    logger.error(f"同步账户余额失败 (ID: {account_id}): {e}")

        loop.close()

        return jsonify({"success": True, "result": {"deleted_count": result}})

    except Exception as e:
        logger.error(f"批量删除账单失败: {e}")
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/parse_import", methods=["POST"])
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
        if "file" not in request.files:
            logger.error("未提供文件")
            return jsonify({"success": False, "error": "No file provided"}), 400

        file = request.files["file"]

        if file.filename == "":
            logger.error("文件名为空")
            return jsonify({"success": False, "error": "No file selected"}), 400

        # 获取解析器类型
        requested_file_type = request.form.get("fileType", "auto")
        parser_type = requested_file_type
        if parser_type == "auto":
            parser_type = None  # None表示自动检测

        # v6.89: 支持前端列映射通用表格解析
        column_mapping = _parse_json_form_field(request.form.get("columnMapping"), {})
        transaction_type_mapping = _parse_json_form_field(request.form.get("transactionTypeMapping"), {})
        has_header_line = _parse_bool_form_field(request.form.get("hasHeaderLine"), default=True)
        time_format = str(request.form.get("timeFormat", "") or "").strip()
        amount_decimal_separator = str(request.form.get("amountDecimalSeparator", ".") or ".").strip() or "."
        amount_digit_grouping_symbol = str(request.form.get("amountDigitGroupingSymbol", "") or "").strip()
        tag_separator = str(request.form.get("tagSeparator", ";") or ";").strip() or ";"
        file_encoding = str(request.form.get("fileEncoding", "") or "").strip()
        delimiter = str(request.form.get("delimiter", "") or "").strip() or None
        use_column_mapping = isinstance(column_mapping, dict) and bool(column_mapping)

        logger.info(f"文件: {file.filename}, 解析器类型: {parser_type or '自动检测'}")

        # 检查文件扩展名
        if not allowed_file(file.filename):
            logger.error(f"不支持的文件类型: {file.filename}")
            return jsonify(
                {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
            ), 400

        # 保存临时文件
        if file.filename is None:
            return jsonify({"success": False, "error": "Invalid filename"}), 400

        filename = secure_filename(file.filename)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        unique_filename = f"{timestamp}_{filename}"
        file_path = UPLOAD_FOLDER / unique_filename

        file.save(str(file_path))
        logger.info(f"临时文件已保存: {file_path}")

        items = []

        if use_column_mapping:
            logger.info("[通用导入] 使用列映射解析: file_type=%s, has_header=%s", requested_file_type, has_header_line)
            normalized_bills, actual_encoding, actual_delimiter = _parse_import_file_with_column_mapping(
                file_path,
                column_mapping=column_mapping,
                transaction_type_mapping=transaction_type_mapping,
                has_header_line=has_header_line,
                time_format=time_format,
                amount_decimal_separator=amount_decimal_separator,
                amount_digit_grouping_symbol=amount_digit_grouping_symbol,
                tag_separator=tag_separator,
                file_encoding=file_encoding,
                delimiter=delimiter,
            )
            logger.info(
                "[通用导入] 解析完成: %s 条记录, encoding=%s, delimiter=%s",
                len(normalized_bills),
                actual_encoding,
                actual_delimiter,
            )

            db, bill_service, _ = get_app_context(user_id=request.user_id)
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                normalized_bills, account_mappings, category_mappings, learning_applied = loop.run_until_complete(
                    _apply_learning_to_column_mapping_bills(normalized_bills, db, bill_service, request.user_id)
                )
            finally:
                loop.close()

            items = [
                _convert_bill_to_import_item_with_mappings(
                    bill, account_mappings=account_mappings, category_mappings=category_mappings
                )
                for bill in normalized_bills
            ]
            logger.info("[通用导入] 长期学习命中: %s 条", learning_applied)
        else:
            # 使用ParserFactory解析银行/平台原生账单
            from bill_analyser.parsers.factory import ParserFactory

            parser_factory = ParserFactory()

            logger.debug("开始解析文件...")
            bills = parser_factory.parse(str(file_path), parser_type=parser_type)
            logger.info(f"解析完成: {len(bills)} 条记录")
            items = [_convert_bill_to_import_item(bill) for bill in bills]

        # 删除临时文件
        try:
            os.remove(file_path)
            logger.debug(f"临时文件已删除: {file_path}")
        except Exception as e:
            logger.warning(f"删除临时文件失败: {e}")

        logger.info("=" * 50)

        return jsonify({"success": True, "result": {"items": items, "totalCount": len(items)}})

    except Exception as e:
        logger.error("解析导入文件失败: %s", e, exc_info=True)
        logger.info("=" * 50)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/reconciliation_statements", methods=["GET"])
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
    logger.info(
        "[get_reconciliation_statements] 入口参数: account_id=%s, start_time=%s, end_time=%s, "
        "category_ids=%s, type=%s, keyword=%s",
        request.args.get("account_id"),
        request.args.get("start_time"),
        request.args.get("end_time"),
        request.args.get("category_ids"),
        request.args.get("type"),
        request.args.get("keyword"),
    )

    try:
        # 获取必需参数
        account_id = request.args.get("account_id")
        start_time = request.args.get("start_time", type=int)
        end_time = request.args.get("end_time", type=int)

        # 注意：start_time=0, end_time=0 是有效值，表示查询全部
        if account_id is None or start_time is None or end_time is None:
            logger.error("[get_reconciliation_statements] 缺少必需参数")
            return jsonify(
                {"success": False, "error": "Missing required parameters: account_id, start_time, end_time"}
            ), 400

        # 获取可选参数
        category_ids = request.args.get("category_ids")  # 逗号分隔的分类ID
        trans_type = request.args.get("type", type=int)  # 1=收入, 2=支出, 3=转账
        keyword = request.args.get("keyword")

        # 转换Unix时间戳为日期字符串
        # 注意：start_time=0 和 end_time=0 表示查询全部，不设置日期限制
        if start_time == 0 and end_time == 0:
            start_date = None
            end_date = None
            logger.info("[get_reconciliation_statements] 查询全部时间范围")
        else:
            start_date = datetime.fromtimestamp(start_time).strftime("%Y-%m-%d")
            end_date = datetime.fromtimestamp(end_time).strftime("%Y-%m-%d")
            logger.info("[get_reconciliation_statements] 时间范围: %s 至 %s", start_date, end_date)

        # 验证account_id是有效数字
        try:
            account_id_int = int(account_id)
        except ValueError, TypeError:
            logger.error("[get_reconciliation_statements] 无效的账户ID: %s", account_id)
            return jsonify({"success": False, "error": f"Invalid account_id: {account_id}"}), 400

        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 获取账户信息
            account = loop.run_until_complete(db.get_account_by_id(account_id_int, user_id=request.user_id))
            if not account:
                logger.warning("[get_reconciliation_statements] 账户不存在: %s", account_id_int)
                return jsonify({"success": False, "error": "Account not found"}), 404

            account_name = account["name"]
            logger.info("[get_reconciliation_statements] 查询账户: %s (ID=%s)", account_name, account_id_int)

            # 构建查询筛选条件 - 使用账户ID而非名称，更精确
            filters = {
                "account_ids": [account_id_int]  # 使用账户ID列表精确匹配
            }

            # 只有在指定了时间范围时才添加日期过滤
            if start_date is not None and end_date is not None:
                filters["start_date"] = start_date
                filters["end_date"] = end_date

            # 添加可选筛选条件
            if category_ids:
                # 将分类ID字符串转换为列表
                category_id_list = [cid.strip() for cid in category_ids.split(",") if cid.strip()]
                if category_id_list:
                    filters["category_ids"] = category_id_list
                    logger.info("[get_reconciliation_statements] 分类筛选: %s", category_id_list)

            if trans_type:
                # 转换类型编号为中文名称
                type_map = {1: "收入", 2: "支出", 3: "转账", 4: "投资"}
                if trans_type in type_map:
                    filters["type"] = type_map[trans_type]
                    logger.info("[get_reconciliation_statements] 类型筛选: %s (%d)", filters["type"], trans_type)

            if keyword:
                filters["keyword"] = keyword
                logger.info("[get_reconciliation_statements] 关键词筛选: %s", keyword)

            # 查询账单
            logger.info("[get_reconciliation_statements] 查询筛选条件: %s", filters)
            bills, total = loop.run_until_complete(
                db.query_bills(
                    page=1,
                    page_size=10000,  # 获取所有匹配记录
                    filters=filters,
                    user_id=request.user_id,
                )
            )

            logger.info("[get_reconciliation_statements] 查询到账单数量: %d (total=%d)", len(bills), total)
            logger.info("[get_reconciliation_statements] 原始账单列表（前5条）:")
            for i, bill in enumerate(bills[:5], 1):
                logger.info(
                    f"  #{i}: ID={bill['id']}, Date={bill.get('date')}, "
                    f"Type={bill.get('type')}, Amount={bill.get('amount')}, "
                    f"Source={bill.get('source_account_id')}, Dest={bill.get('destination_account_id')}"
                )

            # 重要：数据库返回的是按时间倒序（最新在前），需要反转为正序进行余额计算
            bills_sorted = sorted(bills, key=lambda b: b.get("date", ""))

            # 记录排序后的顺序
            logger.info("[get_reconciliation_statements] 排序后的账单顺序（按时间正序）:")
            for i, bill in enumerate(bills_sorted, 1):
                logger.info(f"  #{i}: ID={bill['id']}, Date={bill.get('date')}, Amount={bill.get('amount')}")

            # 计算期初余额（查询开始时间之前的所有账单余额）
            opening_balance = 0.0
            try:
                # 如果指定了开始时间，查询开始时间之前最后一笔账单
                if start_date is not None:
                    pre_filters = {"end_date": start_date, "account_ids": [account_id_int]}
                    pre_bills, _ = loop.run_until_complete(
                        db.query_bills(page=1, page_size=1, filters=pre_filters, user_id=request.user_id)
                    )
                    # query_bills返回的是按date DESC排序，第一条就是最晚的
                    if pre_bills and len(pre_bills) > 0:
                        # 如果有历史账单，取最后一笔的账户余额
                        opening_balance = float(pre_bills[0].get("account_balance", 0))
                        logger.info("[get_reconciliation_statements] 期初余额: %.2f (基于历史账单)", opening_balance)
                    else:
                        opening_balance = 0.0
                        logger.info("[get_reconciliation_statements] 期初余额: 0.00 (无历史账单)")
                else:
                    # 如果是查询全部（start_time=0），期初余额使用账户初始余额
                    opening_balance = float(account.get("initial_balance", 0))
                    logger.info("[get_reconciliation_statements] 期初余额: %.2f (账户初始余额)", opening_balance)
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
            if db_category_map and "id_to_category" in db_category_map:
                for cat in db_category_map["id_to_category"].values():
                    category_map[(cat["main_category"], cat["sub_category"])] = cat

            # 计算统计数据
            total_inflows = 0.0  # 总收入
            total_outflows = 0.0  # 总支出
            closing_balance = opening_balance  # 期末余额 = 期初余额 + 净流入
            transactions = []

            # 第一步：按时间正序遍历，计算每笔交易的余额（包含期初和期末）
            balance_history = {}  # {bill_id: {'opening': xxx, 'closing': xxx}}
            current_balance = opening_balance  # 当前余额初始值为期初余额

            for bill in bills_sorted:
                bill_type = bill.get("type", "")
                amount = float(bill.get("amount", 0))
                source_account_id_raw = bill.get("source_account_id")
                dest_account_id_raw = bill.get("destination_account_id")

                # 安全转换账户ID
                source_acc_id = int(source_account_id_raw) if source_account_id_raw else None
                dest_acc_id = int(dest_account_id_raw) if dest_account_id_raw else None

                # 记录该交易的期初余额（交易前的余额）
                transaction_opening_balance = current_balance

                # 分类收入/支出，并更新余额
                if bill_type == "收入":
                    total_inflows += amount
                    current_balance += amount
                elif bill_type == "支出":
                    total_outflows += amount
                    current_balance -= amount
                elif bill_type == "转账":
                    # 转账需要判断是转入还是转出 - 使用账户ID比较
                    if dest_acc_id and dest_acc_id == account_id_int:
                        total_inflows += amount
                        current_balance += amount
                    elif source_acc_id and source_acc_id == account_id_int:
                        total_outflows += amount
                        current_balance -= amount
                    else:
                        logger.warning(
                            "[get_reconciliation_statements] 转账账单但账户ID不匹配: bill_id=%s", bill.get("id")
                        )
                        continue
                elif bill_type == "投资":
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
                balance_history[bill["id"]] = {
                    "opening": transaction_opening_balance,  # 交易前余额
                    "closing": current_balance,  # 交易后余额
                }
                logger.info(
                    f"[get_reconciliation_statements] ID={bill['id']}, Date={bill.get('date')}, "
                    f"Type={bill_type}, Amount={amount} → Opening={transaction_opening_balance}, Closing={current_balance}"
                )

            # 更新最终的期末余额
            closing_balance = current_balance

            # 第二步：转换所有交易为v1格式，并关联余额
            for bill in bills_sorted:  # 使用排序后的列表，保证balance_history的bill_id对应正确
                bill_id = bill["id"]

                # 跳过无效交易
                if bill_id not in balance_history:
                    continue

                # 使用adapter转换为v1格式，确保所有字段完整
                v1_transaction = loop.run_until_complete(adapter.backend_to_frontend(bill, account_map, category_map))

                # 添加余额字段（转换为分，前端使用accountOpeningBalance和accountClosingBalance字段）
                v1_transaction["accountOpeningBalance"] = yuan_to_cents(
                    balance_history[bill_id]["opening"]
                )  # 期初余额（分）
                v1_transaction["accountClosingBalance"] = yuan_to_cents(
                    balance_history[bill_id]["closing"]
                )  # 期末余额（分）
                transactions.append(v1_transaction)

            # 按时间倒序排序（最新交易在前）
            transactions.sort(key=lambda x: x["time"], reverse=True)

            # 计算净流入
            net_flow = total_inflows - total_outflows

            logger.info(
                "[get_reconciliation_statements] 统计结果: 期初=%.2f, 期末=%.2f, "
                "收入=%.2f, 支出=%.2f, 净流入=%.2f, 交易数=%d",
                opening_balance,
                closing_balance,
                total_inflows,
                total_outflows,
                net_flow,
                len(transactions),
            )

            # 构建返回结果 - 将所有金额从元转换为分（前端期望的单位）
            result = {
                "accountId": str(account_id),
                "accountName": account_name,
                "startTime": start_time,
                "endTime": end_time,
                "openingBalance": yuan_to_cents(opening_balance),  # 期初余额（分）
                "closingBalance": yuan_to_cents(closing_balance),  # 期末余额（分）
                "totalInflows": yuan_to_cents(total_inflows),  # 总收入（分）
                "totalOutflows": yuan_to_cents(total_outflows),  # 总支出（分）
                "netFlow": yuan_to_cents(net_flow),  # 净流入（分）
                "transactions": transactions,
                "itemCount": len(transactions),
            }

            logger.info("[get_reconciliation_statements] 返回成功, 交易数量: %d", len(transactions))

            # 记录前3笔交易的详细信息，验证accountOpeningBalance字段
            if transactions and len(transactions) > 0:
                logger.info("[get_reconciliation_statements] 前3笔交易详情（验证accountOpeningBalance）:")
                for i, txn in enumerate(transactions[:3], 1):
                    logger.info(
                        f"  #{i}: ID={txn.get('id')}, "
                        f"Date={txn.get('gregorianCalendarYearDashMonthDashDay')}, "
                        f"accountOpeningBalance={txn.get('accountOpeningBalance')}, "
                        f"accountClosingBalance={txn.get('accountClosingBalance')}, "
                        f"amount={txn.get('amount')}"
                    )

            return jsonify({"success": True, "result": result})

        finally:
            loop.close()

    except Exception as e:
        logger.error("[get_reconciliation_statements] 获取对账单失败: %s", e, exc_info=True)
        return jsonify(
            {"success": False, "error": str(e), "message": "Failed to retrieve reconciliation statements"}
        ), 500


# ==================== v6.47 三阶段导入API ====================


@bp.route("/import/v2/parse", methods=["POST"])
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
        if "files" not in request.files and "file" not in request.files:
            logger.warning("[阶段1-解析] 未找到上传文件")
            return jsonify({"success": False, "error": "No files provided"}), 400

        # 兼容单文件和多文件上传
        files = request.files.getlist("files") or [request.files["file"]]

        if not files or (len(files) == 1 and files[0].filename == ""):
            logger.warning("[阶段1-解析] 文件列表为空")
            return jsonify({"success": False, "error": "No files selected"}), 400

        # 获取服务实例
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        # 保存文件到临时目录
        saved_files = []
        for file in files:
            if file.filename and allowed_file(file.filename):
                filename = secure_filename(file.filename)
                timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")
                unique_filename = f"{timestamp}_{filename}"
                file_path = UPLOAD_FOLDER / unique_filename
                file.save(str(file_path))
                saved_files.append({"path": str(file_path), "original_name": file.filename})
                logger.info(f"[阶段1-解析] 文件已保存: {file_path}")
            else:
                logger.warning(f"[阶段1-解析] 跳过不支持的文件: {file.filename}")

        if not saved_files:
            logger.error("[阶段1-解析] 没有有效的文件")
            return jsonify({"success": False, "error": "No valid files to process"}), 400

        # 生成session_id
        session_id = str(uuid.uuid4())
        logger.info(f"[阶段1-解析] 生成会话ID: {session_id}")

        # 调用阶段1解析
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            file_paths = [f["path"] for f in saved_files]
            result = loop.run_until_complete(bill_service.import_stage1_parse(file_paths, session_id, user_id))

            logger.info(f"[阶段1-解析] 完成: session={result.get('session_id')}, 总数={result.get('total_parsed', 0)}")

            return jsonify(
                {
                    "success": result.get("success", False),
                    "data": {
                        "session_id": result.get("session_id"),
                        "parsed_count": result.get("total_parsed", 0),
                        "files": result.get("file_results", []),
                        "errors": result.get("errors", []),
                    },
                }
            )

        finally:
            loop.close()
            # 清理临时文件
            for f in saved_files:
                try:
                    os.remove(f["path"])
                    logger.debug(f"[阶段1-解析] 临时文件已删除: {f['path']}")
                except Exception as e:
                    logger.warning(f"[阶段1-解析] 删除临时文件失败: {e}")

    except Exception as e:
        logger.error(f"[阶段1-解析] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/dedup", methods=["POST"])
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
        if not data or "session_id" not in data:
            logger.warning("[阶段2-去重] 缺少session_id")
            return jsonify({"success": False, "error": "Missing session_id"}), 400

        session_id = data["session_id"]

        # 获取服务实例
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 执行阶段2去重处理
            result = loop.run_until_complete(bill_service.import_stage2_dedup(session_id, user_id))

            # 获取预览数据返回给前端
            preview_data = []
            if result.get("success"):
                preview_data = loop.run_until_complete(bill_service.get_import_preview(session_id, selected_only=False))

            logger.info(
                f"[阶段2-去重] 完成: session={session_id}, "
                f"原始={result.get('template_count', 0)}, "
                f"去重后={result.get('preview_count', 0)}, "
                f"预览数据={len(preview_data)}条"
            )

            return jsonify(
                {
                    "success": result.get("success", False),
                    "data": {
                        "session_id": session_id,
                        "preview": preview_data,
                        "total": result.get("template_count", 0),
                        "after_dedup": result.get("preview_count", 0),
                        "dedup_stats": result.get("dedup_stats", {}),
                        "match_stats": result.get("match_stats", {}),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[阶段2-去重] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/confirm", methods=["POST"])
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
        if not data or "session_id" not in data:
            logger.warning("[阶段3-确认] 缺少session_id")
            return jsonify({"success": False, "error": "Missing session_id"}), 400

        session_id = data["session_id"]
        selected_ids = data.get("selected_ids")  # 可选：用户选择的账单ID
        preview_updates = data.get("preview_updates")  # 可选：用户编辑后的数据

        # 获取服务实例
        db, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # v6.61: 如果有preview_updates，先重置所有选中状态，再更新选中的账单
            # 这确保只有前端传入的选中账单才会被导入，未选中的不会被导入
            if preview_updates:
                # 第一步：重置该会话所有账单的选中状态为未选中
                reset_count = loop.run_until_complete(db.reset_session_preview_selection(session_id))
                logger.info(f"[阶段3-确认] 已重置 {reset_count} 条账单的选中状态")

                # 第二步：更新前端传入的选中账单
                logger.info(f"[阶段3-确认] 更新 {len(preview_updates)} 条预览数据")
                loop.run_until_complete(db.update_preview_bills_batch(session_id, preview_updates, user_id))
                # 从preview_updates中提取选中的ID
                selected_ids = [u["id"] for u in preview_updates if u.get("selected", True) and u.get("id")]
                logger.info(f"[阶段3-确认] 选中的账单ID数: {len(selected_ids)}")

            result = loop.run_until_complete(bill_service.import_stage3_confirm(session_id, user_id, selected_ids))

            logger.info(f"[阶段3-确认] 完成: session={session_id}, 导入={result.get('imported_count', 0)}")

            return jsonify(
                {
                    "success": result.get("success", False),
                    "data": {
                        "imported_count": result.get("imported_count", 0),
                        "skipped_count": result.get("skipped_count", 0),
                        "errors": result.get("errors", []),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[阶段3-确认] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/session/<session_id>", methods=["GET"])
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
        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            session = loop.run_until_complete(db.get_import_session(session_id, user_id))

            if not session:
                return jsonify({"success": False, "error": "Session not found or expired"}), 404

            return jsonify(
                {
                    "success": True,
                    "data": {
                        "session_id": session["session_id"],
                        "status": session["status"],
                        "created_at": session["created_at"],
                        "parsed_count": session.get("parsed_count", 0),
                        "preview_count": session.get("preview_count", 0),
                        "file_paths": session.get("file_paths", ""),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[获取会话] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/session/<session_id>", methods=["DELETE"])
@log_method
@require_auth
def cancel_import_session(session_id: str):
    """
    取消/清理导入会话

    清理 bills_parser_template 和 bills_preview 中的临时数据。
    """
    try:
        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.clear_session_data(session_id, user_id))

            return jsonify({"success": success, "message": "Session cleared" if success else "Session not found"})

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[取消会话] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview/<session_id>", methods=["GET"])
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
        page = request.args.get("page", 1, type=int)
        page_size = request.args.get("page_size", 50, type=int)

        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 获取预览数据
            previews = loop.run_until_complete(db.get_preview_by_session(session_id, user_id))

            # 分页
            total = len(previews)
            start = (page - 1) * page_size
            end = start + page_size
            page_data = previews[start:end]

            # 转换为前端格式
            result = []
            for preview in page_data:
                result.append(
                    {
                        "id": preview["id"],
                        "time": preview["preview_date"],
                        "type": preview["preview_type"],
                        "amount": yuan_to_cents(preview["preview_amount"]),
                        "destinationAmount": yuan_to_cents(preview.get("preview_destination_amount", 0)),
                        "categoryId": str(preview.get("category_id", "")),
                        "mainCategory": preview.get("preview_main_category", ""),
                        "subCategory": preview.get("preview_sub_category", ""),
                        "sourceAccountId": str(preview.get("preview_source_account_id", "")),
                        "destinationAccountId": str(preview.get("preview_destination_account_id", "")),
                        "counterparty": preview.get("preview_counterparty", ""),
                        "paymentMethod": preview.get("preview_payment_method", ""),
                        "description": preview.get("preview_description", ""),
                        "isSelected": preview.get("is_selected", 1) == 1,
                    }
                )

            return jsonify(
                {"success": True, "data": {"preview": result, "total": total, "page": page, "page_size": page_size}}
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[获取预览] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview-item/<int:preview_id>/recurring-candidates", methods=["GET"])
@log_method
@require_auth
def get_preview_recurring_candidates(preview_id: int):
    """获取导入预览账单可匹配的定时交易候选。"""
    try:
        tolerance_days = request.args.get("toleranceDays", default=3, type=int)
        tolerance_days = max(0, min(tolerance_days, 31))

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            db.get_recurring_candidates_for_preview(preview_id, user_id=request.user_id, tolerance_days=tolerance_days)
        )
        loop.close()

        if not result.get("preview"):
            return jsonify({"success": False, "error": "Preview bill not found"}), 404

        return jsonify(
            {
                "success": True,
                "result": {
                    "previewId": preview_id,
                    "linkedRecurringId": result.get("linked_recurring_id"),
                    "candidates": result.get("candidates", []),
                },
            }
        )
    except Exception as e:
        logger.error("获取预览定时交易候选失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview/<session_id>/update", methods=["PUT"])
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
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing bill id"}), 400

        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        preview_id = data["id"]

        # 使用session_id验证预览账单归属（可选的安全检查）
        logger.debug(f"[更新预览] session_id={session_id}, preview_id={preview_id}")

        updates = {
            "preview_type": data.get("type"),
            "preview_amount": data.get("amount"),
            "preview_destination_amount": data.get("destinationAmount"),
            "preview_main_category": data.get("mainCategory"),
            "preview_sub_category": data.get("subCategory"),
            "preview_source_account_id": data.get("sourceAccountId"),
            "preview_destination_account_id": data.get("destinationAccountId"),
            "preview_counterparty": data.get("counterparty"),
            "preview_payment_method": data.get("paymentMethod"),
            "preview_description": data.get("description"),
            "is_selected": 1 if data.get("isSelected", True) else 0,
        }
        # 过滤None值
        updates = {k: v for k, v in updates.items() if v is not None}

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.update_preview_bill(preview_id, updates, user_id))

            return jsonify({"success": success})

        finally:
            loop.close()

    except Exception as e:
        logger.error(f"[更新预览] 失败: {e}", exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500
