r"""End-to-end acceptance checklist for Bill Analyser.

Usage:
    .\.venv\Scripts\python tests\e2e_acceptance_check.py
    .\.venv\Scripts\python tests\e2e_acceptance_check.py --username comprehensive_test --password Test@123456

Checks:
1. Login and token retrieval
2. Exchange rates include CNY=1.0 (base_currency=CNY)
3. Statistics v1 time filter pass-through for ThisYear/LastYear/All(start=0)
4. Categories endpoint returns data and keywords are propagated

Exit codes:
    0 = all passed
    2 = one or more checks failed
    1 = runtime/connection error
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

import bcrypt


@dataclass
class CheckResult:
    """Single checklist result."""

    name: str
    passed: bool
    detail: str


def _request_json(
    method: str,
    url: str,
    headers: dict[str, str] | None = None,
    payload: dict[str, Any] | None = None,
    timeout: int = 15,
) -> tuple[int, dict[str, Any]]:
    """HTTP JSON request helper."""
    request_headers = {"Accept": "application/json"}
    if headers:
        request_headers.update(headers)

    body_bytes = None
    if payload is not None:
        request_headers["Content-Type"] = "application/json"
        body_bytes = json.dumps(payload).encode("utf-8")

    req = Request(url=url, data=body_bytes, headers=request_headers, method=method)

    try:
        with urlopen(req, timeout=timeout) as resp:
            status = getattr(resp, "status", 200)
            raw = resp.read().decode("utf-8")
            data = json.loads(raw) if raw else {}
            return status, data
    except HTTPError as error:
        raw = error.read().decode("utf-8") if error.fp else ""
        data = json.loads(raw) if raw else {}
        return error.code, data


def _repair_user_password_hash(db_path: Path, username: str, password: str) -> bool:
    """Repair user password hash to bcrypt in one explicit target DB."""
    if not db_path.exists():
        return False

    password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")

    conn = sqlite3.connect(str(db_path))
    try:
        cursor = conn.cursor()
        cursor.execute(
            "UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE username = ?",
            (password_hash, username),
        )
        conn.commit()
        return cursor.rowcount > 0
    finally:
        conn.close()


def _flatten_categories(result_data: Any) -> list[dict[str, Any]]:
    """Flatten category result structure into a single list (top + subs)."""
    flat: list[dict[str, Any]] = []

    if isinstance(result_data, dict):
        for value in result_data.values():
            if not isinstance(value, list):
                continue
            for top in value:
                if not isinstance(top, dict):
                    continue
                flat.append(top)
                subs = top.get("subCategories") or top.get("sub_categories") or []
                if isinstance(subs, list):
                    for sub in subs:
                        if isinstance(sub, dict):
                            flat.append(sub)
    elif isinstance(result_data, list):
        for item in result_data:
            if isinstance(item, dict):
                flat.append(item)

    return flat


def run_checklist(
    base_url: str,
    username: str,
    password: str,
    repair_invalid_salt: bool = False,
    repair_db_path: Path | None = None,
) -> list[CheckResult]:
    """Run full acceptance checklist and return result list."""
    results: list[CheckResult] = []

    # 0) 健康检查
    try:
        health_status, _ = _request_json("GET", f"{base_url}/api/health")
    except URLError as error:
        raise RuntimeError(f"Backend unavailable: {error}") from error

    if health_status != 200:
        raise RuntimeError(f"Backend health check failed: status={health_status}")

    # 1) 登录
    login_status, login_body = _request_json(
        "POST",
        f"{base_url}/api/authorize.json",
        payload={"loginName": username, "password": password},
    )

    token = (login_body.get("result") or {}).get("token") if isinstance(login_body, dict) else None

    if login_status != 200 and repair_invalid_salt:
        message = str(login_body.get("message", "")) if isinstance(login_body, dict) else ""
        if (
            "Invalid salt" in message
            and repair_db_path is not None
            and _repair_user_password_hash(repair_db_path, username, password)
        ):
            login_status, login_body = _request_json(
                "POST",
                f"{base_url}/api/authorize.json",
                payload={"loginName": username, "password": password},
            )
            token = (login_body.get("result") or {}).get("token") if isinstance(login_body, dict) else None

    login_ok = login_status == 200 and bool(token)
    results.append(
        CheckResult(
            "Login token",
            login_ok,
            f"status={login_status}, hasToken={bool(token)}",
        )
    )

    if not token:
        return results

    auth_headers = {"Authorization": f"Bearer {token}"}

    # 2) 汇率结果中应包含 CNY=1.0
    rate_status, rate_body = _request_json(
        "GET",
        f"{base_url}/api/statistics/exchange-rates?base_currency=CNY",
        headers=auth_headers,
    )
    exchange_rates = ((rate_body.get("result") or {}).get("exchangeRates") or []) if isinstance(rate_body, dict) else []
    cny_item = next((item for item in exchange_rates if str(item.get("currency", "")).upper() == "CNY"), None)
    cny_rate_ok = bool(cny_item) and str(cny_item.get("rate")) in {"1", "1.0", "1.00"}
    results.append(
        CheckResult(
            "CNY rate exists",
            rate_status == 200 and cny_rate_ok,
            f"status={rate_status}, rates={len(exchange_rates)}, cnyRate={(cny_item or {}).get('rate') if cny_item else None}",
        )
    )

    # 3) 统计日期筛选透传
    now = datetime.now()
    this_year_start = int(datetime(now.year, 1, 1).timestamp())
    this_year_end = int(datetime(now.year, 12, 31, 23, 59, 59).timestamp())
    last_year_start = int(datetime(now.year - 1, 1, 1).timestamp())
    last_year_end = int(datetime(now.year - 1, 12, 31, 23, 59, 59).timestamp())
    all_end = int(time.time())

    cases = [
        ("ThisYear", this_year_start, this_year_end),
        ("LastYear", last_year_start, last_year_end),
        ("All", 0, all_end),
    ]

    for label, start_time, end_time in cases:
        query = urlencode({"startTime": start_time, "endTime": end_time})
        stats_status, stats_body = _request_json(
            "GET",
            f"{base_url}/api/v1/transactions/statistics.json?{query}",
            headers=auth_headers,
        )

        result_data = stats_body.get("result") if isinstance(stats_body, dict) else None
        ret_start = int((result_data or {}).get("startTime", -1))
        ret_end = int((result_data or {}).get("endTime", -1))
        items_len = len((result_data or {}).get("items") or [])

        pass_through_ok = stats_status == 200 and ret_start == int(start_time) and ret_end == int(end_time)
        results.append(
            CheckResult(
                f"Stats filter {label}",
                pass_through_ok,
                f"status={stats_status}, req=({start_time},{end_time}), ret=({ret_start},{ret_end}), items={items_len}",
            )
        )

    # 4) 分类接口与关键词透传
    category_status, category_body = _request_json(
        "GET",
        f"{base_url}/api/categories/",
        headers=auth_headers,
    )

    category_result = category_body.get("result") if isinstance(category_body, dict) else None
    flat_categories = _flatten_categories(category_result)
    non_empty_keywords = [
        category
        for category in flat_categories
        if str(category.get("keywords", "")).strip()
    ]

    results.append(
        CheckResult(
            "Categories endpoint",
            category_status == 200 and len(flat_categories) > 0,
            f"status={category_status}, flattenCategories={len(flat_categories)}",
        )
    )
    results.append(
        CheckResult(
            "Keywords propagated",
            len(non_empty_keywords) > 0,
            f"nonEmptyKeywords={len(non_empty_keywords)}",
        )
    )

    return results


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run Bill Analyser E2E acceptance checklist")
    parser.add_argument("--base-url", default="http://127.0.0.1:5000", help="Backend base URL")
    parser.add_argument("--username", default="comprehensive_test", help="Login username")
    parser.add_argument("--password", default="Test@123456", help="Login password")
    parser.add_argument(
        "--repair-invalid-salt",
        action="store_true",
        help="Auto-repair test user password hash when login fails with 'Invalid salt'",
    )
    parser.add_argument(
        "--repair-db-path",
        type=Path,
        help="在启用 --repair-invalid-salt 时用于修复密码哈希的显式数据库路径",
    )
    return parser


def main() -> int:
    args = _build_parser().parse_args()

    print("=== E2E ACCEPTANCE CHECKLIST ===")
    print(f"Target: {args.base_url}")
    print(f"User: {args.username}")

    try:
        results = run_checklist(
            base_url=args.base_url.rstrip("/"),
            username=args.username,
            password=args.password,
            repair_invalid_salt=args.repair_invalid_salt,
            repair_db_path=args.repair_db_path.resolve() if args.repair_db_path else None,
        )
    except RuntimeError as error:
        print(f"[ERROR] {error}")
        return 1

    failed = [result for result in results if not result.passed]

    for result in results:
        print(f"[{'PASS' if result.passed else 'FAIL'}] {result.name}: {result.detail}")

    print("\n=== SUMMARY ===")
    print(f"Passed={len(results) - len(failed)} Failed={len(failed)} Total={len(results)}")

    if failed:
        print("--- Failed Items ---")
        for result in failed:
            print(f" - {result.name}: {result.detail}")
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
