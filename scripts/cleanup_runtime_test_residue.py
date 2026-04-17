"""Clean test-created residue from the real runtime bills.db safely."""

from __future__ import annotations

# pylint: disable=line-too-long,missing-function-docstring
import argparse
import asyncio
import re
import sqlite3
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from bill_analyser.constants import DATA_DIR
from bill_analyser.utils.config import load_default_user_settings

REVIEWED_NONPREFIXED_PATTERNS = (
    re.compile(r"^cat_reg_\d+$"),
    re.compile(r"^comprehensive_test$"),
)


def _load_protected_defaults() -> dict[str, str]:
    default_user = load_default_user_settings() or {}
    return {
        "username": str(default_user.get("username") or "admin").strip(),
        "email": str(default_user.get("email") or "").strip(),
        "nickname": str(default_user.get("nickname") or "").strip(),
    }


def _is_auto_candidate(username: str, email: str, protected_usernames: set[str]) -> bool:
    normalized_username = username.strip().lower()
    normalized_email = email.strip().lower()
    if normalized_username in protected_usernames:
        return False
    if normalized_username.startswith(("test_", "pytest_")) and normalized_email.endswith("@example.com"):
        return True
    if normalized_email.endswith("@example.com") and any(
        pattern.fullmatch(normalized_username) for pattern in REVIEWED_NONPREFIXED_PATTERNS
    ):
        return True
    return False


def _backup_runtime_database_family(db_path: Path) -> list[Path]:
    timestamp = datetime.now(UTC).strftime("%Y%m%d_%H%M%S")
    backup_path = db_path.with_name(f"{db_path.stem}_backup_before_test_cleanup_{timestamp}{db_path.suffix}")

    source_conn = sqlite3.connect(f"file:{db_path.as_posix()}?mode=ro", uri=True)
    backup_conn = sqlite3.connect(backup_path)
    try:
        source_conn.backup(backup_conn)
    finally:
        backup_conn.close()
        source_conn.close()

    return [backup_path]


async def _collect_user_scoped_counts(conn: sqlite3.Connection, user_id: int) -> dict[str, int]:
    counts: dict[str, int] = {}
    table_rows = conn.execute(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name ASC"
    ).fetchall()
    for (table_name,) in table_rows:
        columns = {row[1] for row in conn.execute(f'PRAGMA table_info("{table_name}")').fetchall()}
        if "user_id" in columns:
            counts[table_name] = int(
                conn.execute(f'SELECT COUNT(*) FROM "{table_name}" WHERE user_id = ?', (user_id,)).fetchone()[0]
            )

    counts["budget_history"] = int(
        conn.execute(
            "SELECT COUNT(*) FROM budget_history WHERE budget_id IN (SELECT id FROM budgets WHERE user_id = ?)",
            (user_id,),
        ).fetchone()[0]
    )
    counts["bill_tags"] = int(
        conn.execute(
            "SELECT COUNT(*) FROM bill_tags WHERE bill_id IN (SELECT id FROM bills WHERE user_id = ?)",
            (user_id,),
        ).fetchone()[0]
    )
    return counts


def _list_user_scoped_tables(conn: sqlite3.Connection) -> list[str]:
    """Return all application tables that expose a user_id column."""
    user_scoped_tables: list[str] = []
    for (table_name,) in conn.execute(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name ASC"
    ).fetchall():
        columns = {row[1] for row in conn.execute(f'PRAGMA table_info("{table_name}")').fetchall()}
        if "user_id" in columns:
            user_scoped_tables.append(table_name)
    return user_scoped_tables


async def _collect_candidate_users(conn: sqlite3.Connection, protected_usernames: set[str]) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    rows = conn.execute(
        "SELECT id, username, email, nickname, created_at, last_login_at FROM users ORDER BY id ASC"
    ).fetchall()
    for row in rows:
        row_dict = dict(row)
        if _is_auto_candidate(str(row_dict["username"] or ""), str(row_dict["email"] or ""), protected_usernames):
            candidates.append(row_dict)
    return candidates


def _cleanup_protected_admin_rows(conn: sqlite3.Connection, protected_defaults: dict[str, str]) -> dict[str, int]:
    username = protected_defaults["username"].strip().lower()
    user_row = conn.execute(
        "SELECT id, email, nickname FROM users WHERE LOWER(username) = ? LIMIT 1",
        (username,),
    ).fetchone()
    if user_row is None:
        return {}

    user_id = int(user_row[0])
    summary: dict[str, int] = {}

    update_cursor = conn.execute(
        "UPDATE users SET email = ?, nickname = ? WHERE id = ? AND (email != ? OR nickname != ?)",
        (
            protected_defaults["email"],
            protected_defaults["nickname"],
            user_id,
            protected_defaults["email"],
            protected_defaults["nickname"],
        ),
    )
    summary["protected_user_fields_restored"] = int(update_cursor.rowcount or 0)

    test_like_bill_ids = [
        row[0]
        for row in conn.execute(
            """
            SELECT id FROM bills
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(counterparty, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(payment_method, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(main_category, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(sub_category, '')) LIKE 'pytest%'
              )
            """,
            (user_id,),
        ).fetchall()
    ]
    if test_like_bill_ids:
        placeholders = ", ".join("?" for _ in test_like_bill_ids)
        summary["protected_user_bill_tags_deleted"] = int(
            conn.execute(
                f"DELETE FROM bill_tags WHERE bill_id IN ({placeholders})",
                tuple(test_like_bill_ids),
            ).rowcount
            or 0
        )
    else:
        summary["protected_user_bill_tags_deleted"] = 0

    summary["protected_user_bills_deleted"] = int(
        conn.execute(
            """
            DELETE FROM bills
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(counterparty, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(payment_method, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(main_category, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(sub_category, '')) LIKE 'pytest%'
              )
            """,
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_recurring_deleted"] = int(
        conn.execute(
            """
            DELETE FROM recurring_bills
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(name, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(comment, '')) LIKE '%pytest%'
              )
            """,
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_import_sessions_deleted"] = int(
        conn.execute(
            "DELETE FROM import_sessions WHERE user_id = ? AND LOWER(session_id) LIKE 'pytest%'",
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_parser_templates_deleted"] = int(
        conn.execute(
            """
            DELETE FROM bills_parser_template
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(session_id, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(parser_description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(parser_counterparty, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(parser_payment_method, '')) LIKE '%pytest%'
              )
            """,
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_previews_deleted"] = int(
        conn.execute(
            """
            DELETE FROM bills_preview
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(session_id, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(preview_description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(preview_counterparty, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(preview_payment_method, '')) LIKE '%pytest%'
              )
            """,
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_learning_rules_deleted"] = int(
        conn.execute(
            """
            DELETE FROM import_learning_rules
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(match_value, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(source_session_id, '')) = 'pytest-session'
              )
            """,
            (user_id,),
        ).rowcount
        or 0
    )
    summary["protected_user_categories_deleted"] = int(
        conn.execute(
            """
            DELETE FROM categories
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(main_category, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(main_category, '')) LIKE 'test%'
                OR LOWER(COALESCE(sub_category, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(sub_category, '')) LIKE 'test%'
                OR LOWER(COALESCE(description, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(description, '')) LIKE '%regression%'
              )
              AND id NOT IN (
                SELECT CAST(category AS INTEGER) FROM recurring_bills
                WHERE user_id = ? AND category GLOB '[0-9]*'
                UNION
                SELECT CAST(category AS INTEGER) FROM bill_templates
                WHERE user_id = ? AND category GLOB '[0-9]*'
                UNION
                SELECT learned_category_id FROM import_learning_rules
                WHERE user_id = ? AND learned_category_id IS NOT NULL
              )
            """,
            (user_id, user_id, user_id, user_id),
        ).rowcount
        or 0
    )
    summary["protected_user_accounts_deleted"] = int(
        conn.execute(
            """
            DELETE FROM accounts
            WHERE user_id = ?
              AND (
                LOWER(COALESCE(name, '')) LIKE 'pytest%'
                OR LOWER(COALESCE(name, '')) LIKE 'test%'
                OR LOWER(COALESCE(comment, '')) LIKE '%pytest%'
                OR LOWER(COALESCE(comment, '')) LIKE '%regression%'
              )
              AND id NOT IN (
                SELECT source_account_id FROM bills
                WHERE user_id = ? AND source_account_id IS NOT NULL
                UNION
                SELECT destination_account_id FROM bills
                WHERE user_id = ? AND destination_account_id IS NOT NULL
                UNION
                SELECT CAST(account AS INTEGER) FROM recurring_bills
                WHERE user_id = ? AND account GLOB '[0-9]*'
                UNION
                SELECT CAST(account AS INTEGER) FROM bill_templates
                WHERE user_id = ? AND account GLOB '[0-9]*'
                UNION
                SELECT learned_source_account_id FROM import_learning_rules
                WHERE user_id = ? AND learned_source_account_id IS NOT NULL
                UNION
                SELECT learned_destination_account_id FROM import_learning_rules
                WHERE user_id = ? AND learned_destination_account_id IS NOT NULL
              )
            """,
            (user_id, user_id, user_id, user_id, user_id, user_id, user_id),
        ).rowcount
        or 0
    )

    return summary


def _bulk_delete_candidate_users(conn: sqlite3.Connection, candidate_ids: list[int]) -> dict[str, int]:
    """Delete all candidate users and their user-scoped residue in bulk."""
    summary: dict[str, int] = {}
    if not candidate_ids:
        return summary

    conn.execute("DROP TABLE IF EXISTS temp.candidate_users")
    conn.execute("CREATE TEMP TABLE candidate_users (id INTEGER PRIMARY KEY)")
    conn.executemany("INSERT INTO candidate_users (id) VALUES (?)", [(candidate_id,) for candidate_id in candidate_ids])
    conn.execute("DROP TABLE IF EXISTS temp.candidate_bill_ids")
    conn.execute(
        "CREATE TEMP TABLE candidate_bill_ids AS "
        "SELECT id FROM bills WHERE user_id IN (SELECT id FROM candidate_users)"
    )
    conn.execute("DROP TABLE IF EXISTS temp.candidate_budget_ids")
    conn.execute(
        "CREATE TEMP TABLE candidate_budget_ids AS "
        "SELECT id FROM budgets WHERE user_id IN (SELECT id FROM candidate_users)"
    )

    summary["bill_tags"] = int(
        conn.execute(
            "DELETE FROM bill_tags WHERE bill_id IN (SELECT id FROM candidate_bill_ids)"
        ).rowcount
        or 0
    )
    summary["budget_history"] = int(
        conn.execute(
            "DELETE FROM budget_history WHERE budget_id IN (SELECT id FROM candidate_budget_ids)"
        ).rowcount
        or 0
    )

    for table_name in _list_user_scoped_tables(conn):
        if table_name == "users":
            continue
        summary[table_name] = int(
            conn.execute(
                f'DELETE FROM "{table_name}" WHERE user_id IN (SELECT id FROM candidate_users)'
            ).rowcount
            or 0
        )

    summary["users"] = int(
        conn.execute("DELETE FROM users WHERE id IN (SELECT id FROM candidate_users)").rowcount
        or 0
    )
    return summary


async def _run_cleanup(db_path: Path, apply_changes: bool) -> int:
    protected_defaults = _load_protected_defaults()
    protected_usernames = {protected_defaults["username"].strip().lower(), "admin"}

    runtime_conn = sqlite3.connect(db_path)
    runtime_conn.row_factory = sqlite3.Row
    try:
        candidates = await _collect_candidate_users(runtime_conn, protected_usernames)
        print(f"候选测试用户: {len(candidates)} 个")
        for candidate in candidates[:10]:
            preview_counts = await _collect_user_scoped_counts(runtime_conn, int(candidate["id"]))
            print(
                f"- id={candidate['id']} username={candidate['username']} email={candidate['email']} counts={preview_counts}"
            )
        if len(candidates) > 10:
            print(f"... 其余 {len(candidates) - 10} 个候选已省略")

        if not apply_changes:
            print("\n当前为 dry-run；未修改运行时 bills.db。")
            return 0
    finally:
        runtime_conn.close()

    backups = _backup_runtime_database_family(db_path)
    print("已创建备份:")
    for backup_path in backups:
        print(f"- {backup_path}")

    candidate_ids = [int(candidate["id"]) for candidate in candidates]
    runtime_conn = sqlite3.connect(db_path)
    runtime_conn.row_factory = sqlite3.Row
    try:
        deleted_summary = _bulk_delete_candidate_users(runtime_conn, candidate_ids)
        print(f"批量清理测试用户结果: {deleted_summary}")
        summary = _cleanup_protected_admin_rows(runtime_conn, protected_defaults)
        runtime_conn.commit()
        print(f"受保护默认用户定点去污结果: {summary}")
    finally:
        runtime_conn.close()

    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="清理运行时 bills.db 中的测试残留数据")
    parser.add_argument(
        "--db-path",
        type=Path,
        default=DATA_DIR / "bills.db",
        help="运行时数据库路径，默认 data/bills.db",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="真正执行清理；未传时仅做 dry-run 预览",
    )
    args = parser.parse_args()

    if not args.db_path.exists():
        raise FileNotFoundError(f"数据库不存在: {args.db_path}")

    return asyncio.run(_run_cleanup(args.db_path, args.apply))


if __name__ == "__main__":
    raise SystemExit(main())
