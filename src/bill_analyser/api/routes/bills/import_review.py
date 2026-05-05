# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403
from .import_mapping import *  # noqa: F403

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
    parser_source = str(bill.get("parserSource") or bill.get("_parser_id") or bill.get("parser_id") or "").strip()
    parser_tags = resolve_parser_tags(
        bill.get("parser_tags")
        or bill.get("parserTags")
        or bill.get("preview_parser_tags")
        or bill.get("preview_parser_tags_json"),
        parser_id=parser_source,
        payment_method=str(bill.get("payment_method", "") or bill.get("preview_payment_method", "")).strip(),
        channel=str(bill.get("channel", "")).strip(),
    )

    item = {
        "type": frontend_type,
        "categoryId": "",
        "originalCategoryName": _build_original_category(main_category, sub_category)
        or str(bill.get("original_category") or "").strip(),
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
        "parserSource": parser_source,
        "parserTags": parser_tags,
        "isManuallyAnnotated": bool(
            bill.get("isManuallyAnnotated")
            or bill.get("is_manually_annotated")
            or bill.get("preview_is_manually_annotated")
        ),
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


def _safe_int_identifier(raw_value: Any) -> int | None:
    """安全地将字符串标识转换为整数。"""
    try:
        text = str(raw_value or "").strip()
        if not text or not text.isdigit():
            return None
        return int(text)
    except (TypeError, ValueError):
        return None


async def _prepare_import_review_bills(
    bills: list[dict[str, Any]], db, bill_service, user_id: int
) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any], dict[str, int]]:
    """为导入检查页准备统一的学习、分类和账户匹配结果。"""
    enriched_bills = [dict(bill) for bill in bills]
    stats = {"learning_seeded": 0, "learning_replayed": 0}
    accounts = await db.get_all_accounts(user_id=user_id) if db else []
    categories = await db.get_all_categories(user_id=user_id) if db else []
    known_category_pairs = {
        (str(cat.get("main_category") or "").strip(), str(cat.get("sub_category") or "").strip())
        for cat in categories
        if str(cat.get("main_category") or "").strip()
    }

    if enriched_bills and bill_service:
        for bill in enriched_bills:
            if not bill.get("date") and bill.get("trade_time"):
                bill["date"] = bill["trade_time"]

        original_snapshots = [
            {
                "type": str(bill.get("type", "") or "").strip(),
                "main_category": str(bill.get("main_category", "") or "").strip(),
                "sub_category": str(bill.get("sub_category", "") or "").strip(),
                "original_category": str(bill.get("original_category", "") or "").strip(),
                "has_explicit_type": bool(bill.get("_import_has_explicit_type")),
                "has_explicit_category": bool(bill.get("_import_has_explicit_category")),
            }
            for bill in enriched_bills
        ]

        await bill_service.category_engine.load_rules_from_db(db, user_id=user_id)

        stats["learning_seeded"] = await bill_service._apply_import_learning_rules(  # pylint: disable=protected-access
            enriched_bills, user_id=user_id, type_only=True, record_usage=False
        )
        enriched_bills = await bill_service.category_engine.batch_match_categories(enriched_bills, types=None)
        enriched_bills = await bill_service._detect_investment_candidates(  # pylint: disable=protected-access
            enriched_bills, user_id=user_id
        )
        enriched_bills = await bill_service._match_accounts(enriched_bills, user_id)  # pylint: disable=protected-access
        enriched_bills = await bill_service._detect_cash_transfers(  # pylint: disable=protected-access
            enriched_bills, user_id=user_id
        )

        stats["learning_replayed"] = await bill_service._apply_import_learning_rules(  # pylint: disable=protected-access
            enriched_bills, user_id=user_id, type_only=False, record_usage=False
        )

        for bill, snapshot in zip(enriched_bills, original_snapshots):
            if snapshot["has_explicit_type"] and snapshot["type"]:
                bill["type"] = snapshot["type"]
            if snapshot["has_explicit_category"]:
                explicit_pair = (snapshot["main_category"], snapshot["sub_category"])
                if explicit_pair in known_category_pairs:
                    bill["main_category"] = snapshot["main_category"]
                    bill["sub_category"] = snapshot["sub_category"]
                else:
                    bill["main_category"] = ""
                    bill["sub_category"] = ""
                    bill["original_category"] = snapshot["original_category"] or _build_original_category(
                        snapshot["main_category"], snapshot["sub_category"]
                    )

    return (
        enriched_bills,
        _build_account_mapping_payload(accounts),
        _build_category_mapping_payload(categories),
        stats,
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
            source_account = id_to_account.get(_safe_int_identifier(source_account_id))
            if source_account:
                item["accountName"] = str(source_account.get("name") or item.get("accountName") or "").strip()
                item["originalSourceAccountName"] = item.get("originalSourceAccountName") or item["accountName"]
                item["originalSourceAccountCurrency"] = (
                    str(source_account.get("currency") or item.get("originalSourceAccountCurrency") or "CNY").strip()
                    or "CNY"
                )

        if destination_account_id not in (None, "", 0, "0"):
            destination_account = id_to_account.get(_safe_int_identifier(destination_account_id))
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

    header_row_index = _detect_generic_import_header_row_index(rows) if has_header_line else -1
    start_index = header_row_index + 1 if has_header_line else 0
    headers = rows[header_row_index] if has_header_line and 0 <= header_row_index < len(rows) else []
    normalized_bills = []
    amount_column_has_signed_values = _generic_import_amount_column_has_signed_values(rows[start_index:], column_mapping)

    for row_index, row in enumerate(rows[start_index:], start=start_index + 1):
        if headers and _is_generic_import_repeated_header_row(row, headers):
            logger.debug("[通用导入] 跳过重复表头行: row=%s", row_index)
            continue

        date_value = _get_mapped_cell(row, column_mapping, 1)

        type_name, amount, raw_type_value = _resolve_generic_import_type_and_amount(
            row,
            headers=headers,
            column_mapping=column_mapping,
            transaction_type_mapping=transaction_type_mapping,
            amount_decimal_separator=amount_decimal_separator,
            amount_digit_grouping_symbol=amount_digit_grouping_symbol,
            amount_column_has_signed_values=amount_column_has_signed_values,
        )

        if not date_value and not raw_type_value and not amount:
            continue

        try:
            related_amount_raw = _get_mapped_cell(row, column_mapping, 11)
            related_amount = _parse_generic_import_amount(
                related_amount_raw,
                decimal_separator=amount_decimal_separator or ".",
                grouping_symbol=amount_digit_grouping_symbol or None,
            )

            if amount == 0 and related_amount == 0:
                continue

            main_category = _get_mapped_cell(row, column_mapping, 4)
            sub_category = _get_mapped_cell(row, column_mapping, 5)
            original_category = _build_original_category(main_category, sub_category)
            raw_tags = _get_mapped_cell(row, column_mapping, 13)
            original_tag_names = []
            if raw_tags:
                separator = tag_separator or ";"
                original_tag_names = [
                    item.strip() for item in raw_tags.split(separator) if item.strip()
                ]

            trade_time_text = _build_generic_import_trade_time(
                row, headers, column_mapping, date_value
            )
            parsed_trade_datetime = _parse_generic_import_datetime(trade_time_text, time_format)
            if parsed_trade_datetime is None:
                logger.warning(
                    "[通用导入] 跳过无法解析交易时间的第 %s 行: %s",
                    row_index,
                    trade_time_text,
                )
                continue
            normalized_trade_time = parsed_trade_datetime.strftime("%Y-%m-%d %H:%M:%S")

            normalized_bill = {
                "date": normalized_trade_time,
                "trade_time": normalized_trade_time,
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
                "original_category": original_category,
                "counterparty": _get_mapped_cell(row, column_mapping, 9),
                "payment_method": _get_mapped_cell(row, column_mapping, 6),
                "original_tag_names": original_tag_names,
                "_import_has_explicit_type": bool(str(raw_type_value or "").strip()),
                "_import_has_explicit_category": bool(main_category or sub_category),
            }

            normalized_bills.append(normalized_bill)
        except Exception as row_error:  # pylint: disable=broad-except
            logger.warning("[通用导入] 解析第 %s 行失败: %s", row_index, row_error)

    return normalized_bills, actual_encoding, actual_delimiter


def _parse_import_file_with_auto_mapping(
    file_path: Path,
    configs: list[dict[str, Any]],
    requested_encoding: str = "",
    delimiter: str | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any], str, str]:
    """为未显式配置列映射的文件自动推断通用解析配置。"""
    rows, actual_encoding, actual_delimiter = _load_generic_import_rows(
        file_path, requested_encoding=requested_encoding, delimiter=delimiter
    )
    trimmed_rows, _ = _trim_generic_import_rows_to_header(rows)
    if not trimmed_rows:
        return [], {"columnMapping": {}, "transactionTypeMapping": {}}, actual_encoding, actual_delimiter

    headers = trimmed_rows[0]
    sample_rows = trimmed_rows[1:11] if len(trimmed_rows) > 1 else []
    suggestion = _build_import_mapping_suggestion(headers, configs, sample_rows)
    column_mapping = suggestion.get("columnMapping") or {}
    if not column_mapping:
        return [], suggestion, actual_encoding, actual_delimiter

    bills, actual_encoding, actual_delimiter = _parse_import_file_with_column_mapping(
        file_path,
        column_mapping=column_mapping,
        transaction_type_mapping=suggestion.get("transactionTypeMapping") or {},
        has_header_line=True,
        time_format="",
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        tag_separator=";",
        file_encoding=requested_encoding,
        delimiter=actual_delimiter or delimiter,
    )
    return bills, suggestion, actual_encoding, actual_delimiter


def _build_picture_data_url(file_path: Path) -> str:
    """将图片文件转换为 data URL，供前端直接预览。"""
    mime_type, _ = mimetypes.guess_type(str(file_path))
    if not mime_type:
        mime_type = "application/octet-stream"

    with open(file_path, "rb") as file_obj:
        encoded = base64.b64encode(file_obj.read()).decode("ascii")

    return f"data:{mime_type};base64,{encoded}"

__all__ = [name for name in globals() if not name.startswith("__")]
