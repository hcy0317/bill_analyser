from __future__ import annotations

import sqlite3
from typing import TYPE_CHECKING, Any, Self, cast

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_user(db: Database, username: str) -> int:
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "pytest-hash",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


async def _get_table_columns(db: Database, table_name: str) -> set[str]:
    conn = await db._get_connection()
    async with conn.execute(f"PRAGMA table_info({table_name})") as cursor:
        rows = await cursor.fetchall()
    return {row[1] for row in rows}


class _FakeCursor:
    def __init__(self, rows: list[tuple[Any, ...]]) -> None:
        self._rows = rows

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(self, exc_type, exc, tb) -> bool:
        return False

    async def fetchall(self) -> list[tuple[Any, ...]]:
        return self._rows


class _FakeExecuteResult:
    def __init__(self, cursor: _FakeCursor) -> None:
        self._cursor = cursor

    def __await__(self):
        async def _resolve() -> _FakeCursor:
            return self._cursor

        return _resolve().__await__()

    async def __aenter__(self) -> _FakeCursor:
        return self._cursor

    async def __aexit__(self, exc_type, exc, tb) -> bool:
        return False


class _FakeConnection:
    def __init__(self, responses: dict[str, list[tuple[Any, ...]]]) -> None:
        self._responses = responses
        self.statements: list[str] = []

    def execute(self, sql: str, *args: Any, **kwargs: Any) -> _FakeExecuteResult:
        del args, kwargs
        self.statements.append(sql)
        return _FakeExecuteResult(_FakeCursor(self._responses.get(sql, [])))


def _seed_legacy_core_schema(db_path: Path) -> None:
    connection = sqlite3.connect(db_path)
    try:
        connection.executescript(
            """
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(main_category, sub_category)
            );

            INSERT INTO categories (user_id, main_category, sub_category, description, created_at)
            VALUES
                (1, '收入', '工资', 'legacy-income', '2026-04-01T08:00:00'),
                (1, '转账', '', 'legacy-transfer', '2026-04-01T08:00:00');

            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                category INTEGER,
                icon TEXT,
                color TEXT,
                balance REAL DEFAULT 0,
                initial_balance REAL DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                display_order INTEGER DEFAULT 0,
                comment TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            INSERT INTO accounts (
                user_id, name, type, category, icon, color, balance, initial_balance,
                hidden, display_order, comment, created_at, updated_at
            ) VALUES (
                1, 'legacy-account', 1, 1, '', '', 0, 0,
                0, 0, 'legacy account', '2026-04-01T08:00:00', '2026-04-01T08:00:00'
            );

            CREATE TABLE tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                color TEXT,
                icon TEXT,
                display_order INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(name)
            );

            INSERT INTO tags (user_id, name, color, icon, display_order, created_at, updated_at)
            VALUES (1, 'legacy-tag', '#123456', 'mdi-tag', 0, '2026-04-01T08:00:00', '2026-04-01T08:00:00');

            CREATE TABLE budget_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                budget_id INTEGER NOT NULL,
                period_start TEXT NOT NULL,
                period_end TEXT NOT NULL,
                spent_amount REAL DEFAULT 0,
                remaining_amount REAL,
                status TEXT,
                calculated_at TEXT NOT NULL
            );

            INSERT INTO budget_history (
                user_id, budget_id, period_start, period_end, spent_amount, remaining_amount, status, calculated_at
            ) VALUES (1, 1, '2026-04-01', '2026-04-30', 88.0, 12.0, 'legacy', '2026-04-30T08:00:00');

            CREATE TABLE bills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                date TEXT NOT NULL,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                counterparty TEXT NOT NULL,
                description TEXT NOT NULL,
                main_category TEXT,
                sub_category TEXT,
                batch_id TEXT,
                hash TEXT UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            INSERT INTO bills (
                user_id, date, type, amount, counterparty, description,
                main_category, sub_category, batch_id, hash, created_at, updated_at
            ) VALUES (
                1, '2026-04-01 09:00:00', '支出', -18.5, 'legacy shop', 'legacy bill',
                '餐饮', '早餐', 'legacy-batch', 'legacy-hash', '2026-04-01T09:00:00', '2026-04-01T09:00:00'
            );

            CREATE TABLE user_exchange_rates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                from_currency TEXT NOT NULL,
                to_currency TEXT NOT NULL,
                rate REAL NOT NULL,
                source TEXT DEFAULT 'manual',
                effective_date TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(from_currency, to_currency, effective_date)
            );

            INSERT INTO user_exchange_rates (
                user_id, from_currency, to_currency, rate, source, effective_date, created_at, updated_at
            ) VALUES (
                1, 'CNY', 'USD', 7.21, 'manual', '2026-04-01', '2026-04-01T08:00:00', '2026-04-01T08:00:00'
            );
            """
        )
        connection.commit()
    finally:
        connection.close()


@pytest.mark.asyncio
async def test_init_db_backfills_legacy_core_tables_and_user_scoped_unique_constraints(
    tmp_path: Path,
) -> None:
    """init_db() 应补齐 legacy core schema 缺列，并把旧唯一约束迁移为 user-scoped。"""
    db_path = tmp_path / "test_db_schema_core_paths.db"
    _seed_legacy_core_schema(db_path)

    db = Database(str(db_path))
    try:
        await db.init_db()

        category_columns = await _get_table_columns(db, "categories")
        account_columns = await _get_table_columns(db, "accounts")
        tag_columns = await _get_table_columns(db, "tags")
        budget_history_columns = await _get_table_columns(db, "budget_history")
        bill_columns = await _get_table_columns(db, "bills")
        exchange_rate_columns = await _get_table_columns(db, "user_exchange_rates")

        assert {
            "user_id",
            "type",
            "priority",
            "keywords",
            "hidden",
            "icon",
            "color",
        }.issubset(category_columns)
        assert {"user_id", "currency", "parent_id", "aliases"}.issubset(account_columns)
        assert {"user_id", "hidden"}.issubset(tag_columns)
        assert {"user_id", "budget_amount", "execution_rate", "filter_summary"}.issubset(budget_history_columns)
        assert {
            "user_id",
            "payment_method",
            "source_account_id",
            "destination_account_id",
            "destination_amount",
            "created_from_template",
            "created_from_recurring",
            "import_history_id",
        }.issubset(bill_columns)
        assert {"user_id"}.issubset(exchange_rate_columns)

        conn = await db._get_connection()
        async with conn.execute(
            "SELECT main_category, sub_category, type, priority, hidden, icon, color, user_id "
            "FROM categories ORDER BY id"
        ) as cursor:
            category_rows = await cursor.fetchall()
        assert [tuple(row) for row in category_rows] == [
            ("收入", "工资", 2, 0, 0, None, None, 1),
            ("转账", "", 3, 0, 0, None, None, 1),
        ]

        async with conn.execute(
            "SELECT currency, parent_id, aliases, user_id FROM accounts WHERE name = ?",
            ("legacy-account",),
        ) as cursor:
            account_row = await cursor.fetchone()
        assert account_row is not None
        assert tuple(account_row) == ("CNY", 0, None, 1)

        async with conn.execute(
            "SELECT hidden, user_id FROM tags WHERE name = ?",
            ("legacy-tag",),
        ) as cursor:
            tag_row = await cursor.fetchone()
        assert tag_row is not None
        assert tuple(tag_row) == (0, 1)

        async with conn.execute(
            "SELECT budget_amount, execution_rate, filter_summary, user_id FROM budget_history WHERE id = 1"
        ) as cursor:
            budget_history_row = await cursor.fetchone()
        assert budget_history_row is not None
        assert tuple(budget_history_row) == (0.0, 0.0, "", 1)

        async with conn.execute(
            """
            SELECT payment_method, source_account_id, destination_account_id,
                   destination_amount, created_from_template, created_from_recurring,
                   import_history_id, user_id
            FROM bills
            WHERE hash = ?
            """,
            ("legacy-hash",),
        ) as cursor:
            bill_row = await cursor.fetchone()
        assert bill_row is not None
        assert tuple(bill_row) == ("", 0, 0, 0.0, None, None, None, 1)

        first_user_id = await _create_user(db, "schema_core_user_one")
        second_user_id = await _create_user(db, "schema_core_user_two")
        assert first_user_id == 1
        assert second_user_id == 2

        await conn.execute(
            """
            INSERT INTO categories (
                user_id, type, main_category, sub_category, description,
                priority, keywords, hidden, icon, color, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                second_user_id,
                2,
                "收入",
                "工资",
                "user-scoped duplicate",
                0,
                None,
                0,
                None,
                None,
                "2026-04-07T09:00:00",
            ),
        )
        await conn.execute(
            """
            INSERT INTO user_exchange_rates (
                user_id, from_currency, to_currency, rate, source,
                effective_date, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                second_user_id,
                "CNY",
                "USD",
                7.11,
                "manual",
                "2026-04-01",
                "2026-04-07T09:00:00",
                "2026-04-07T09:00:00",
            ),
        )
        await conn.commit()

        async with conn.execute(
            "SELECT COUNT(*) FROM categories WHERE main_category = ? AND sub_category = ?",
            ("收入", "工资"),
        ) as cursor:
            category_count_row = await cursor.fetchone()
        async with conn.execute(
            "SELECT COUNT(*) FROM user_exchange_rates WHERE from_currency = ? "
            "AND to_currency = ? AND effective_date = ?",
            ("CNY", "USD", "2026-04-01"),
        ) as cursor:
            exchange_rate_count_row = await cursor.fetchone()

        assert category_count_row is not None
        assert category_count_row[0] == 2
        assert exchange_rate_count_row is not None
        assert exchange_rate_count_row[0] == 2
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_migrate_user_id_field_adds_index_and_ignores_missing_table(tmp_path: Path) -> None:
    """user_id helper 应为 legacy 表补列补索引，并在缺表时安全返回。"""
    db = Database(str(tmp_path / "test_db_schema_core_user_id_helper.db"))
    try:
        conn = await db._get_connection()
        await conn.execute("CREATE TABLE legacy_items (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
        await conn.commit()

        await db._migrate_user_id_field(conn, "legacy_items")
        await db._migrate_user_id_field(conn, "missing_table")

        async with conn.execute("PRAGMA table_info(legacy_items)") as cursor:
            columns = {row[1] for row in await cursor.fetchall()}
        async with conn.execute("PRAGMA index_list(legacy_items)") as cursor:
            indexes = {row[1] for row in await cursor.fetchall()}

        assert "user_id" in columns
        assert "idx_legacy_items_user_id" in indexes
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_migrate_categories_unique_constraint_noops_without_unique_indexes(tmp_path: Path) -> None:
    """categories unique migration 在没有 legacy unique index 时应直接返回。"""
    db = Database(str(tmp_path / "test_db_schema_core_categories_no_unique.db"))
    try:
        conn = await db._get_connection()
        await conn.execute(
            """
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                type INTEGER DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                priority INTEGER DEFAULT 0,
                keywords TEXT,
                hidden BOOLEAN DEFAULT 0,
                icon TEXT,
                color TEXT,
                created_at TEXT NOT NULL
            )
            """
        )
        await conn.execute(
            "INSERT INTO categories (user_id, main_category, sub_category, created_at) VALUES (?, ?, ?, ?)",
            (1, "餐饮", "午餐", "2026-04-07T10:00:00"),
        )
        await conn.commit()

        await db._migrate_categories_unique_constraint(conn)

        async with conn.execute("SELECT COUNT(*) FROM categories") as cursor:
            row = await cursor.fetchone()
        async with conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'categories_new'"
        ) as cursor:
            categories_new_row = await cursor.fetchone()

        assert row is not None
        assert row[0] == 1
        assert categories_new_row is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_migrate_user_exchange_rates_unique_constraint_noops_without_unique_indexes(
    tmp_path: Path,
) -> None:
    """exchange-rate unique migration 在没有 legacy unique index 时应直接返回。"""
    db = Database(str(tmp_path / "test_db_schema_core_exchange_no_unique.db"))
    try:
        conn = await db._get_connection()
        await conn.execute(
            """
            CREATE TABLE user_exchange_rates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                from_currency TEXT NOT NULL,
                to_currency TEXT NOT NULL,
                rate REAL NOT NULL,
                source TEXT DEFAULT 'manual',
                effective_date TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        await conn.execute(
            """
            INSERT INTO user_exchange_rates (
                user_id, from_currency, to_currency, rate, source, effective_date, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (1, "CNY", "USD", 7.25, "manual", "2026-04-07", "2026-04-07T10:00:00", "2026-04-07T10:00:00"),
        )
        await conn.commit()

        await db._migrate_user_exchange_rates_unique_constraint(conn)

        async with conn.execute("SELECT COUNT(*) FROM user_exchange_rates") as cursor:
            row = await cursor.fetchone()
        async with conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'user_exchange_rates_new'"
        ) as cursor:
            recreated_table_row = await cursor.fetchone()

        assert row is not None
        assert row[0] == 1
        assert recreated_table_row is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_init_db_skips_existing_budget_history_and_bill_extension_columns(tmp_path: Path) -> None:
    """已有 legacy 列时，schema 初始化应走 continue/except 分支而不是重复补列。"""
    db_path = tmp_path / "test_db_schema_core_existing_columns.db"
    connection = sqlite3.connect(db_path)
    try:
        connection.executescript(
            """
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                type INTEGER DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                priority INTEGER DEFAULT 0,
                keywords TEXT,
                hidden BOOLEAN DEFAULT 0,
                icon TEXT,
                color TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, main_category, sub_category)
            );

            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                category INTEGER,
                currency TEXT DEFAULT 'CNY',
                icon TEXT,
                color TEXT,
                balance REAL DEFAULT 0,
                initial_balance REAL DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                display_order INTEGER DEFAULT 0,
                comment TEXT,
                aliases TEXT,
                parent_id INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                color TEXT,
                icon TEXT,
                display_order INTEGER DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(name)
            );

            CREATE TABLE budget_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                budget_id INTEGER NOT NULL,
                period_start TEXT NOT NULL,
                period_end TEXT NOT NULL,
                budget_amount REAL DEFAULT 0,
                spent_amount REAL DEFAULT 0,
                remaining_amount REAL,
                status TEXT,
                calculated_at TEXT NOT NULL
            );

            CREATE TABLE bills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                date TEXT NOT NULL,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                counterparty TEXT NOT NULL,
                description TEXT NOT NULL,
                payment_method TEXT DEFAULT '',
                main_category TEXT,
                sub_category TEXT,
                batch_id TEXT,
                hash TEXT UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source_account_id INTEGER DEFAULT 0,
                destination_account_id INTEGER DEFAULT 0,
                destination_amount REAL DEFAULT 0,
                created_from_template INTEGER,
                created_from_recurring INTEGER,
                import_history_id INTEGER
            );

            CREATE TABLE user_exchange_rates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                from_currency TEXT NOT NULL,
                to_currency TEXT NOT NULL,
                rate REAL NOT NULL,
                source TEXT DEFAULT 'manual',
                effective_date TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(user_id, from_currency, to_currency, effective_date)
            );
            """
        )
        connection.commit()
    finally:
        connection.close()

    db = Database(str(db_path))
    try:
        await db.init_db()

        budget_history_columns = await _get_table_columns(db, "budget_history")
        assert "budget_amount" in budget_history_columns
        assert "execution_rate" in budget_history_columns
        assert "filter_summary" in budget_history_columns

        account_columns = await _get_table_columns(db, "accounts")
        bill_columns = await _get_table_columns(db, "bills")
        assert "currency" in account_columns
        assert {
            "created_from_template",
            "created_from_recurring",
            "import_history_id",
            "destination_amount",
            "destination_account_id",
            "source_account_id",
            "payment_method",
        }.issubset(bill_columns)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_unique_constraint_helpers_append_missing_user_id_in_generated_selects(tmp_path: Path) -> None:
    """重建 legacy unique 约束时，helper 应把缺失的 user_id 追加到复制列。"""
    db = Database(str(tmp_path / "test_db_schema_core_fake_unique_helpers.db"))
    try:
        categories_conn = _FakeConnection(
            {
                "PRAGMA index_list(categories)": [(0, "legacy_categories_unique", 1, "c", 0)],
                "PRAGMA table_info(categories)": [(0, "id"), (1, "main_category"), (2, "sub_category")],
                "PRAGMA index_info(legacy_categories_unique)": [(0, 1, "main_category"), (1, 2, "sub_category")],
            }
        )
        exchange_conn = _FakeConnection(
            {
                "PRAGMA index_list(user_exchange_rates)": [(0, "legacy_exchange_unique", 1, "c", 0)],
                "PRAGMA table_info(user_exchange_rates)": [
                    (0, "id"),
                    (1, "from_currency"),
                    (2, "to_currency"),
                    (3, "effective_date"),
                ],
                "PRAGMA index_info(legacy_exchange_unique)": [
                    (0, 1, "from_currency"),
                    (1, 2, "to_currency"),
                    (2, 3, "effective_date"),
                ],
            }
        )

        await db._migrate_categories_unique_constraint(cast("Any", categories_conn))
        await db._migrate_user_exchange_rates_unique_constraint(cast("Any", exchange_conn))

        assert any(
            "INSERT OR IGNORE INTO categories_new (main_category, sub_category, user_id) "
            "SELECT main_category, sub_category, user_id FROM categories" in statement
            for statement in categories_conn.statements
        )
        assert any(
            "INSERT OR IGNORE INTO user_exchange_rates_new (user_id, from_currency, to_currency, effective_date) "
            "SELECT user_id, from_currency, to_currency, effective_date FROM user_exchange_rates" in statement
            for statement in exchange_conn.statements
        )
    finally:
        await db.close()
