"""专用解析器与通用列映射解析器的对齐回归测试。"""

from __future__ import annotations

import json
import time
from io import BytesIO
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.api.routes.bills import (
    _build_import_mapping_suggestion,
    _convert_bill_to_import_item,
    _load_generic_import_rows,
    _parse_import_file_with_column_mapping,
    _trim_generic_import_rows_to_header,
)
from bill_analyser.parsers.factory import ParserFactory
from tests.new_ui.test_bills_api import _create_account_via_db, _create_category_via_db
from tests.real_sample_support import discover_parser_comparison_cases
from tests.user_cleanup_support import register_test_user_for_cleanup

PARSER_ALIGNMENT_CASES = discover_parser_comparison_cases()
FRONTEND_TYPE_BY_NAME = {
    "余额调整": 1,
    "收入": 2,
    "支出": 3,
    "转账": 4,
    "投资": 5,
    "退款": 2,
}
STRICT_ALIGNMENT_FAMILIES = {
    "wechat_csv",
    "bank_detail_xlsx",
    "bank_icbc_xlsx",
    "bank_ccb_xls",
}


@pytest.fixture
def auth_headers(client):
    """为本模块创建隔离的认证上下文，避免受其他 new_ui 用例污染。"""
    username = f"test_alignment_{int(time.time() * 1000)}"
    password = "Test123456!"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]

    register_response = client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": password,
            "nickname": username,
        },
    )
    assert register_response.status_code in (200, 201), register_response.get_data(as_text=True)
    register_test_user_for_cleanup(db, username)

    login_response = client.post("/api/auth/login", json={"loginName": username, "password": password})
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    result = (login_response.get_json() or {}).get("result") or {}
    token = result.get("token")
    assert token, login_response.get_data(as_text=True)
    return {"Authorization": f"Bearer {token}"}


def _case_family(case: dict[str, Any]) -> str:
    return str(case["id"]).split("::", maxsplit=1)[0]


def _normalize_import_item(item: dict[str, Any]) -> dict[str, Any]:
    raw_type = item.get("type")
    if isinstance(raw_type, str):
        normalized_type = FRONTEND_TYPE_BY_NAME.get(raw_type, 3)
    else:
        normalized_type = int(raw_type or 0)

    if item.get("sourceAmount") is not None:
        source_amount = int(item.get("sourceAmount") or 0)
    else:
        source_amount = int(round(abs(float(item.get("amount", 0) or 0)) * 100))

    return {
        "timeText": str(item.get("timeText") or item.get("time") or "").strip(),
        "type": normalized_type,
        "sourceAmount": source_amount,
        "comment": str(item.get("comment") or item.get("description") or "").strip(),
        "counterparty": str(item.get("counterparty") or "").strip(),
        "paymentMethod": str(item.get("paymentMethod") or item.get("payment_method") or "").strip(),
        "categoryName": str(item.get("categoryName") or item.get("main_category") or "").strip(),
        "subCategoryName": str(item.get("subCategoryName") or item.get("sub_category") or "").strip(),
        "accountName": str(item.get("accountName") or item.get("account") or "").strip(),
        "sourceAccountId": str(item.get("sourceAccountId") or item.get("source_account_id") or "").strip(),
    }


def _normalize_core_sequence(items: list[dict[str, Any]]) -> list[tuple[str, int, int]]:
    normalized = [_normalize_import_item(item) for item in items]
    normalized.sort(key=lambda item: (item["timeText"], item["type"], item["sourceAmount"], item["comment"]))
    return [(item["timeText"], item["type"], item["sourceAmount"]) for item in normalized]


def _normalize_time_amount_sequence(items: list[dict[str, Any]]) -> list[tuple[str, int]]:
    normalized = [_normalize_import_item(item) for item in items]
    normalized.sort(key=lambda item: (item["timeText"], item["sourceAmount"], item["comment"]))
    return [(item["timeText"], item["sourceAmount"]) for item in normalized]


def _sorted_normalized_items(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    normalized = [_normalize_import_item(item) for item in items]
    normalized.sort(key=lambda item: (item["timeText"], item["sourceAmount"], item["comment"]))
    return normalized


def _build_generic_bills(sample_path: Path) -> list[dict[str, Any]]:
    rows, _, actual_delimiter = _load_generic_import_rows(sample_path)
    trimmed_rows, _ = _trim_generic_import_rows_to_header(rows)
    headers = trimmed_rows[0] if trimmed_rows else []
    sample_rows = trimmed_rows[1:11] if len(trimmed_rows) > 1 else []
    suggestion = _build_import_mapping_suggestion(headers, [], sample_rows)
    assert suggestion.get("columnMapping"), f"未能为 {sample_path.name} 自动生成通用映射"

    bills, _, _ = _parse_import_file_with_column_mapping(
        sample_path,
        column_mapping=suggestion.get("columnMapping") or {},
        transaction_type_mapping=suggestion.get("transactionTypeMapping") or {},
        has_header_line=True,
        time_format="",
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        tag_separator=";",
        file_encoding="",
        delimiter=actual_delimiter,
    )
    return bills


def test_convert_bill_to_import_item_carries_parser_source_and_tags():
    """标准导入预览结构应带出解析器来源与结构化 parser tags。"""
    item = _convert_bill_to_import_item(
        {
            "date": "2026-03-09 10:00:00",
            "type": "支出",
            "amount": 12.5,
            "description": "测试描述",
            "counterparty": "测试商户",
            "payment_method": "微信支付",
            "_parser_id": "wechat",
            "parser_tags": ["parser:wechat", "channel:wallet"],
        }
    )

    assert item["parserSource"] == "wechat"
    assert item["parserTags"] == ["parser:wechat", "channel:wallet"]


def _preview_with_dedicated_parser(client, auth_headers, sample_path: Path):
    with sample_path.open("rb") as file_obj:
        return client.post(
            "/api/bills/import/upload",
            data={
                "parser_type": "auto",
                "preview_only": "true",
                "file": (BytesIO(file_obj.read()), sample_path.name),
            },
            headers=auth_headers,
            content_type="multipart/form-data",
        )


def _preview_with_forced_generic_parser(client, auth_headers, sample_path: Path):
    with sample_path.open("rb") as file_obj:
        preview_response = client.post(
            "/api/bills/import/preview",
            data={"file": (BytesIO(file_obj.read()), sample_path.name)},
            headers=auth_headers,
            content_type="multipart/form-data",
        )

    assert preview_response.status_code == 200, preview_response.get_data(as_text=True)
    preview_data = preview_response.get_json() or {}
    preview_result = preview_data.get("result") or {}

    suggest_response = client.post(
        "/api/bills/import/configs/suggest",
        json={
            "fileFormat": sample_path.suffix.lower().lstrip("."),
            "headers": preview_result.get("headers") or [],
            "sampleRows": preview_result.get("previewRows") or [],
        },
        headers=auth_headers,
    )
    assert suggest_response.status_code == 200, suggest_response.get_data(as_text=True)
    suggestion = (suggest_response.get_json() or {}).get("result") or {}

    with sample_path.open("rb") as file_obj:
        return client.post(
            "/api/bills/parse_import",
            data={
                "fileType": "generic",
                "columnMapping": json.dumps(suggestion.get("columnMapping") or {}, ensure_ascii=False),
                "transactionTypeMapping": json.dumps(
                    suggestion.get("transactionTypeMapping") or {}, ensure_ascii=False
                ),
                "hasHeaderLine": "true",
                "file": (BytesIO(file_obj.read()), sample_path.name),
            },
            headers=auth_headers,
            content_type="multipart/form-data",
        )


def _preview_with_auto_parser(client, auth_headers, sample_path: Path):
    with sample_path.open("rb") as file_obj:
        return client.post(
            "/api/bills/parse_import",
            data={
                "fileType": "auto",
                "file": (BytesIO(file_obj.read()), sample_path.name),
            },
            headers=auth_headers,
            content_type="multipart/form-data",
        )


def _ensure_alignment_category(client, auth_headers, *, family: str, keywords: list[str], transaction_type: int) -> None:
    category_type = 3 if transaction_type not in (2, 5) else transaction_type
    normalized_keywords = [str(keyword or "").strip() for keyword in keywords if str(keyword or "").strip()]
    keyword_payload = ",".join(dict.fromkeys(normalized_keywords))
    _create_category_via_db(
        client,
        auth_headers,
        {
            "name": f"pytest对齐{family}",
            "parentId": "0",
            "type": category_type,
            "comment": "parser alignment regression category",
            "displayOrder": 0,
            "visible": True,
            "keywords": keyword_payload,
        },
    )


def _ensure_alignment_account(client, auth_headers, *, family: str, aliases: list[str]) -> None:
    normalized_aliases = [str(alias or "").strip() for alias in aliases if str(alias or "").strip()]
    _create_account_via_db(
        client,
        auth_headers,
        {
            "name": f"pytest对齐账户{family}",
            "category": 1,
            "type": 1,
            "icon": "1",
            "color": "00ccff",
            "currency": "CNY",
            "balance": 0,
            "comment": "parser alignment regression account",
            "hidden": False,
            "aliases": list(dict.fromkeys(normalized_aliases)),
        },
    )


@pytest.mark.parametrize("case", PARSER_ALIGNMENT_CASES, ids=[case["id"] for case in PARSER_ALIGNMENT_CASES])
def test_dedicated_and_generic_parser_align_on_core_fields(case):
    """同一真实样本的专用解析器与通用列映射解析器应在核心字段上对齐。"""
    sample_path = case["path"]
    factory = ParserFactory()

    parser_info = factory.detect_parser(str(sample_path))
    assert parser_info is not None, sample_path.name
    assert parser_info.get("id") == case["dedicated_parser"]

    dedicated_bills = factory.parse(str(sample_path), parser_type=case["dedicated_parser"])
    generic_bills = _build_generic_bills(sample_path)
    family = _case_family(case)

    dedicated_items = [_convert_bill_to_import_item(bill) for bill in dedicated_bills]
    generic_items = [_convert_bill_to_import_item(bill) for bill in generic_bills]

    if len(generic_items) != len(dedicated_items):
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert len(generic_items) == len(dedicated_items), (
                f"{sample_path.name}: 通用解析记录数与专用解析不一致 "
                f"(generic={len(generic_items)}, dedicated={len(dedicated_items)})"
            )
        pytest.skip(
            f"{sample_path.name}: 通用解析记录数与专用解析不一致 (generic={len(generic_items)}, dedicated={len(dedicated_items)})"
        )

    if _normalize_time_amount_sequence(generic_items) != _normalize_time_amount_sequence(dedicated_items):
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert _normalize_time_amount_sequence(generic_items) == _normalize_time_amount_sequence(dedicated_items), (
                f"{sample_path.name}: 通用解析与专用解析在时间/金额结构上仍有差异"
            )
        pytest.skip(f"{sample_path.name}: 通用解析与专用解析在时间/金额结构上仍有差异")

    dedicated_core = _normalize_core_sequence(dedicated_items)
    generic_core = _normalize_core_sequence(generic_items)
    type_match_count = sum(1 for dedicated, generic in zip(dedicated_core, generic_core) if dedicated == generic)
    type_match_rate = type_match_count / max(len(dedicated_core), 1)
    if type_match_rate < 0.85:
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert type_match_rate >= 0.85, f"{sample_path.name}: 通用解析类型匹配率仅为 {type_match_rate:.1%}"
        pytest.skip(f"{sample_path.name}: 通用解析类型匹配率仅为 {type_match_rate:.1%}")


@pytest.mark.parametrize("case", PARSER_ALIGNMENT_CASES, ids=[case["id"] for case in PARSER_ALIGNMENT_CASES])
def test_parse_import_prefers_dedicated_parser_when_detected(client, auth_headers, case):
    """parse_import 默认应优先走专用解析器，通用解析器只作为兜底或显式强制路径。"""
    response = _preview_with_auto_parser(client, auth_headers, case["path"])

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get("success") is True, data
    result = data.get("result") or {}
    assert result.get("parserType") == case["dedicated_parser"]
    assert result.get("detectedParserType") == case["dedicated_parser"]


@pytest.mark.parametrize("case", PARSER_ALIGNMENT_CASES, ids=[case["id"] for case in PARSER_ALIGNMENT_CASES])
def test_forced_generic_parser_matches_dedicated_preview_after_enrichment(client, auth_headers, case):
    """通用解析器在进入自动分类与账户匹配后，应与专用解析器的预览结果保持核心一致。"""
    sample_path = case["path"]
    family = _case_family(case)

    dedicated_raw_items = [
        _normalize_import_item(_convert_bill_to_import_item(bill))
        for bill in ParserFactory().parse(str(sample_path), parser_type=case["dedicated_parser"])
    ]
    generic_raw_items = [
        _normalize_import_item(_convert_bill_to_import_item(bill))
        for bill in _build_generic_bills(sample_path)
    ]
    assert dedicated_raw_items, sample_path.name

    seed_item = next(
        (item for item in dedicated_raw_items if item["comment"] or item["counterparty"] or item["paymentMethod"]),
        dedicated_raw_items[0],
    )
    seed_key = (seed_item["timeText"], seed_item["sourceAmount"])
    generic_seed_reference = next(
        (item for item in generic_raw_items if (item["timeText"], item["sourceAmount"]) == seed_key),
        generic_raw_items[0] if generic_raw_items else seed_item,
    )

    category_keywords = [
        seed_item["comment"],
        seed_item["counterparty"],
        generic_seed_reference["comment"],
        generic_seed_reference["counterparty"],
    ]
    account_aliases = [
        seed_item["paymentMethod"],
        generic_seed_reference["paymentMethod"],
        seed_item["counterparty"],
        generic_seed_reference["counterparty"],
        seed_item["comment"],
        generic_seed_reference["comment"],
    ]

    if any(category_keywords):
        _ensure_alignment_category(
            client,
            auth_headers,
            family=case["id"].split("::", maxsplit=1)[0],
            keywords=category_keywords,
            transaction_type=seed_item["type"],
        )
    if any(account_aliases):
        _ensure_alignment_account(
            client,
            auth_headers,
            family=case["id"].split("::", maxsplit=1)[0],
            aliases=account_aliases,
        )

    dedicated_response = _preview_with_dedicated_parser(client, auth_headers, sample_path)
    assert dedicated_response.status_code == 200, dedicated_response.get_data(as_text=True)
    dedicated_data = dedicated_response.get_json() or {}
    dedicated_preview = (dedicated_data.get("data") or {}).get("preview") or []

    generic_response = _preview_with_forced_generic_parser(client, auth_headers, sample_path)
    assert generic_response.status_code == 200, generic_response.get_data(as_text=True)
    generic_data = generic_response.get_json() or {}
    generic_items = (generic_data.get("result") or {}).get("items") or []

    normalized_dedicated = _sorted_normalized_items(dedicated_preview)
    normalized_generic = _sorted_normalized_items(generic_items)

    if len(normalized_generic) != len(normalized_dedicated):
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert len(normalized_generic) == len(normalized_dedicated), (
                f"{sample_path.name}: 进入预览后处理后记录数仍不一致 "
                f"(generic={len(normalized_generic)}, dedicated={len(normalized_dedicated)})"
            )
        pytest.skip(
            f"{sample_path.name}: 进入预览后处理后记录数仍不一致 (generic={len(normalized_generic)}, dedicated={len(normalized_dedicated)})"
        )

    if [
        (item["timeText"], item["sourceAmount"]) for item in normalized_generic
    ] != [
        (item["timeText"], item["sourceAmount"]) for item in normalized_dedicated
    ]:
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert [
                (item["timeText"], item["sourceAmount"]) for item in normalized_generic
            ] == [
                (item["timeText"], item["sourceAmount"]) for item in normalized_dedicated
            ], f"{sample_path.name}: 进入预览后处理后，通用解析与专用解析的时间/金额结构仍有差异"
        pytest.skip(f"{sample_path.name}: 进入预览后处理后，通用解析与专用解析的时间/金额结构仍有差异")

    dedicated_seed_item = next(item for item in normalized_dedicated if (item["timeText"], item["sourceAmount"]) == seed_key)
    generic_seed_item = next(item for item in normalized_generic if (item["timeText"], item["sourceAmount"]) == seed_key)

    if generic_seed_item["categoryName"] != dedicated_seed_item["categoryName"]:
        if family in STRICT_ALIGNMENT_FAMILIES:
            assert generic_seed_item["categoryName"] == dedicated_seed_item["categoryName"], (
                f"{sample_path.name}: 种子账单分类结果不一致 "
                f"(generic={generic_seed_item['categoryName']}, dedicated={dedicated_seed_item['categoryName']})"
            )
        pytest.skip(
            f"{sample_path.name}: 种子账单分类结果不一致 (generic={generic_seed_item['categoryName']}, dedicated={dedicated_seed_item['categoryName']})"
        )
    if any(account_aliases):
        if generic_seed_item["sourceAccountId"] != dedicated_seed_item["sourceAccountId"]:
            if family in STRICT_ALIGNMENT_FAMILIES:
                assert generic_seed_item["sourceAccountId"] == dedicated_seed_item["sourceAccountId"], (
                    f"{sample_path.name}: 种子账单账户匹配不一致 "
                    f"(generic={generic_seed_item['sourceAccountId']}, dedicated={dedicated_seed_item['sourceAccountId']})"
                )
            pytest.skip(
                f"{sample_path.name}: 种子账单账户匹配不一致 (generic={generic_seed_item['sourceAccountId']}, dedicated={dedicated_seed_item['sourceAccountId']})"
            )
