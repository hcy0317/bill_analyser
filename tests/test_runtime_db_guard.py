"""Regression guards for runtime database cleanliness and pytest isolation."""

# pylint: disable=line-too-long

from __future__ import annotations

import asyncio
import re
import sqlite3
from pathlib import Path

import aiosqlite
import pytest

from bill_analyser.utils.config import load_default_user_settings

REPO_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_DB_PATH = REPO_ROOT / "data" / "bills.db"
SUSPICIOUS_NONPREFIXED_USER_PATTERNS = (
    re.compile(r"^cat_reg_\d+$"),
    re.compile(r"^comprehensive_test$"),
)


async def _attempt_aiosqlite_runtime_connection(runtime_db_path: Path) -> None:
    connection = await aiosqlite.connect(str(runtime_db_path))
    await connection.close()


async def _attempt_aiosqlite_runtime_connection_via_uri(runtime_db_path: Path) -> None:
    connection = await aiosqlite.connect(f"file:{runtime_db_path.as_posix()}?mode=rw", uri=True)
    await connection.close()


def _open_runtime_db_readonly() -> sqlite3.Connection:
    return sqlite3.connect(f"file:{RUNTIME_DB_PATH.as_posix()}?mode=ro", uri=True)


def test_pytest_blocks_runtime_db_connections() -> None:
    """Pytest should refuse direct runtime DB connections unless they are explicit read-only audits."""
    with pytest.raises(RuntimeError, match="must not open"):
        sqlite3.connect(str(RUNTIME_DB_PATH))

    with pytest.raises(RuntimeError, match="must not open"):
        sqlite3.connect(f"file:{RUNTIME_DB_PATH.as_posix()}?mode=rw", uri=True)

    with pytest.raises(RuntimeError, match="must not open"):
        asyncio.run(_attempt_aiosqlite_runtime_connection(RUNTIME_DB_PATH))

    with pytest.raises(RuntimeError, match="must not open"):
        asyncio.run(_attempt_aiosqlite_runtime_connection_via_uri(RUNTIME_DB_PATH))


def test_pytest_test_files_do_not_reference_runtime_bills_db_directly() -> None:
    """Collected pytest files should not hardcode runtime bills.db paths."""
    forbidden_patterns = (
        re.compile(r"src/data/bills\.db", re.IGNORECASE),
        re.compile(r"os\.path\.join\([\s\S]{0,160}?['\"]data['\"][\s\S]{0,160}?['\"]bills\.db['\"]", re.IGNORECASE),
        re.compile(r"Path\([\s\S]{0,160}?\)\s*/\s*['\"]data['\"]\s*/\s*['\"]bills\.db['\"]", re.IGNORECASE),
    )
    offenders: list[str] = []
    for file_path in REPO_ROOT.glob("tests/**/test_*.py"):
        text = file_path.read_text(encoding="utf-8", errors="ignore")
        if any(pattern.search(text) for pattern in forbidden_patterns):
            offenders.append(str(file_path.relative_to(REPO_ROOT)))

    assert not offenders, f"pytest 测试文件不应直连运行时 bills.db: {offenders}"


def test_runtime_bills_db_has_no_test_residue_when_present() -> None:
    """The real runtime bills.db should stay free of test-created residue."""
    if not RUNTIME_DB_PATH.exists():
        pytest.skip("运行时 bills.db 不存在，跳过洁净守卫")

    default_user = load_default_user_settings()
    protected_username = str((default_user or {}).get("username") or "admin").strip().lower()
    protected_email = str((default_user or {}).get("email") or "").strip()
    protected_nickname = str((default_user or {}).get("nickname") or "").strip()

    with _open_runtime_db_readonly() as conn:
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()

        test_prefix_count = cursor.execute(
            """
            SELECT COUNT(*) FROM users
            WHERE (
                                LOWER(username) GLOB 'test_*'
                                OR LOWER(username) GLOB 'pytest_*'
            )
                            AND LOWER(email) LIKE '%@example.com'
              AND LOWER(username) != ?
            """,
            (protected_username,),
        ).fetchone()[0]
        assert test_prefix_count == 0, f"运行时 bills.db 仍残留 {test_prefix_count} 个测试前缀用户"

        suspicious_usernames = [
            dict(row)["username"]
            for row in cursor.execute(
                "SELECT username, email FROM users WHERE LOWER(email) LIKE '%@example.com' ORDER BY id"
            ).fetchall()
            if any(pattern.fullmatch(str(row["username"])) for pattern in SUSPICIOUS_NONPREFIXED_USER_PATTERNS)
        ]
        assert suspicious_usernames == [], f"运行时 bills.db 仍残留人工复核测试用户: {suspicious_usernames}"

        protected_user_row = cursor.execute(
            "SELECT username, email, nickname FROM users WHERE LOWER(username) = ? LIMIT 1",
            (protected_username,),
        ).fetchone()
        assert protected_user_row is not None, "受保护默认用户缺失"
        assert protected_user_row["email"] == protected_email
        assert protected_user_row["nickname"] == protected_nickname

        admin_test_like_counts = {
            "accounts": cursor.execute(
                                """
                                SELECT COUNT(*) FROM accounts
                                WHERE user_id = 1
                                    AND (
                                        LOWER(name) LIKE 'pytest%'
                                        OR LOWER(comment) LIKE '%pytest%'
                                        OR LOWER(comment) LIKE '%regression%'
                                        OR LOWER(name) LIKE 'test%'
                                    )
                                """
            ).fetchone()[0],
            "categories": cursor.execute(
                                """
                                SELECT COUNT(*) FROM categories
                                WHERE user_id = 1
                                    AND (
                                        LOWER(main_category) LIKE 'pytest%'
                                        OR LOWER(main_category) LIKE 'test%'
                                        OR LOWER(sub_category) LIKE 'pytest%'
                                        OR LOWER(sub_category) LIKE 'test%'
                                        OR LOWER(description) LIKE '%pytest%'
                                        OR LOWER(description) LIKE '%regression%'
                                    )
                                """
            ).fetchone()[0],
            "bills": cursor.execute(
                                """
                                SELECT COUNT(*) FROM bills
                                WHERE user_id = 1
                                    AND (
                                        LOWER(COALESCE(description, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(counterparty, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(payment_method, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(main_category, '')) LIKE 'pytest%'
                                        OR LOWER(COALESCE(sub_category, '')) LIKE 'pytest%'
                                    )
                                """
            ).fetchone()[0],
            "recurring_bills": cursor.execute(
                                """
                                SELECT COUNT(*) FROM recurring_bills
                                WHERE user_id = 1
                                    AND (
                                        LOWER(COALESCE(name, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(description, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(comment, '')) LIKE '%pytest%'
                                    )
                                """
            ).fetchone()[0],
            "import_sessions": cursor.execute(
                "SELECT COUNT(*) FROM import_sessions WHERE user_id = 1 AND LOWER(session_id) LIKE 'pytest%'"
            ).fetchone()[0],
            "bills_parser_template": cursor.execute(
                                """
                                SELECT COUNT(*) FROM bills_parser_template
                                WHERE user_id = 1
                                    AND (
                                        LOWER(COALESCE(session_id, '')) LIKE 'pytest%'
                                        OR LOWER(COALESCE(parser_description, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(parser_counterparty, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(parser_payment_method, '')) LIKE '%pytest%'
                                    )
                                """
            ).fetchone()[0],
            "import_learning_rules": cursor.execute(
                                """
                                SELECT COUNT(*) FROM import_learning_rules
                                WHERE user_id = 1
                                    AND (
                                        LOWER(COALESCE(match_value, '')) LIKE '%pytest%'
                                        OR LOWER(COALESCE(source_session_id, '')) = 'pytest-session'
                                    )
                                """
            ).fetchone()[0],
        }
        assert all(count == 0 for count in admin_test_like_counts.values()), (
            f"受保护默认用户名下仍残留测试脏数据: {admin_test_like_counts}"
        )
