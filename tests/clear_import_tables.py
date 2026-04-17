#!/usr/bin/env python
"""清空导入相关临时表"""

import argparse
import sqlite3
from pathlib import Path

try:
	from tests.runtime_paths import build_sqlite_uri
except ModuleNotFoundError:  # pragma: no cover - direct script execution fallback
	from runtime_paths import build_sqlite_uri


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="清空指定数据库中的导入相关临时表")
    parser.add_argument("--db-path", type=Path, required=True, help="要清理的数据库路径")
    parser.add_argument("--apply", action="store_true", help="真正执行删除；默认仅 dry-run")
    return parser


def main() -> int:
    args = _build_parser().parse_args()
    db_path = args.db_path.resolve()
    connection_target = build_sqlite_uri(db_path, readonly=not args.apply)
    conn = sqlite3.connect(connection_target, uri=True)
    c = conn.cursor()

    try:
        if not args.apply:
            print("当前为 dry-run；未修改数据库。")
            print(f"import_sessions: {c.execute('SELECT COUNT(*) FROM import_sessions').fetchone()[0]}")
            print(f"bills_parser_template: {c.execute('SELECT COUNT(*) FROM bills_parser_template').fetchone()[0]}")
            print(f"bills_preview: {c.execute('SELECT COUNT(*) FROM bills_preview').fetchone()[0]}")
            return 0

        c.execute("DELETE FROM import_sessions")
        c.execute("DELETE FROM bills_parser_template")
        c.execute("DELETE FROM bills_preview")
        conn.commit()

        print("已清空导入相关的临时表")
        print(f"import_sessions: {c.execute('SELECT COUNT(*) FROM import_sessions').fetchone()[0]}")
        print(f"bills_parser_template: {c.execute('SELECT COUNT(*) FROM bills_parser_template').fetchone()[0]}")
        print(f"bills_preview: {c.execute('SELECT COUNT(*) FROM bills_preview').fetchone()[0]}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
