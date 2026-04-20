"""Decrypt a SQLCipher encrypted database back to plain SQLite format.

Usage:
    python scripts/decrypt_database.py --key YOUR_KEY [--source data/bills.db] [--output data/bills_decrypted.db]

This is the reverse operation of encrypt_database.py, useful for rollback.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "src"))


def decrypt_database(source_path: str, output_path: str, key: str) -> None:
    """Convert SQLCipher encrypted DB to plain unencrypted SQLite DB."""
    try:
        import sqlcipher3  # noqa: F401
    except ImportError:
        print("ERROR: sqlcipher3 not installed. Run: pip install sqlcipher3-binary")
        sys.exit(1)

    if not os.path.exists(source_path):
        print(f"ERROR: Source database not found: {source_path}")
        sys.exit(1)

    if os.path.exists(output_path):
        print(f"WARNING: Output file exists: {output_path}")
        resp = input("Overwrite? [y/N] ")
        if resp.lower() != "y":
            print("Aborted.")
            sys.exit(0)
        os.remove(output_path)

    print(f"Decrypting: {source_path} -> {output_path}")

    # Open encrypted source with key
    conn = sqlcipher3.connect(source_path)
    conn.execute(f"PRAGMA key = '{key}'")
    conn.execute("PRAGMA cipher_page_size = 4096")
    conn.execute("PRAGMA kdf_iter = 256000")
    conn.execute("PRAGMA cipher_compatibility = 4")

    # Verify we can read the encrypted database
    try:
        conn.execute("SELECT count(*) FROM sqlite_master")
    except Exception as e:
        print(f"ERROR: Cannot read encrypted database (wrong key?): {e}")
        conn.close()
        sys.exit(1)

    # Attach new plaintext DB (empty key = no encryption)
    conn.execute(f"ATTACH DATABASE '{output_path}' AS plaintext KEY ''")
    conn.execute("SELECT sqlcipher_export('plaintext')")
    conn.execute("DETACH DATABASE plaintext")
    conn.close()

    # Verify the decrypted database with standard sqlite3
    import sqlite3

    verify_conn = sqlite3.connect(output_path)
    row = verify_conn.execute(
        "SELECT count(*) FROM sqlite_master WHERE type='table'"
    ).fetchone()
    tables = row[0] if row else 0
    verify_conn.close()

    print(f"SUCCESS: Decrypted database created with {tables} tables")
    print()
    print("Next steps:")
    print(f"  1. Replace the encrypted DB: move {output_path} {source_path}")
    print("  2. Remove encryption env vars:")
    print("     unset BILL_DB_ENCRYPT")
    print("     unset BILL_DB_KEY")


def main() -> None:
    """CLI entry point."""
    parser = argparse.ArgumentParser(
        description="Decrypt a SQLCipher encrypted database to plain SQLite"
    )
    parser.add_argument(
        "--key",
        required=True,
        help="Encryption passphrase used to encrypt the database",
    )
    parser.add_argument(
        "--source",
        default=str(PROJECT_ROOT / "data" / "bills.db"),
        help="Path to encrypted source database (default: data/bills.db)",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="Path for decrypted output (default: source path + _decrypted suffix)",
    )
    args = parser.parse_args()

    output = args.output
    if output is None:
        src = Path(args.source)
        output = str(src.parent / f"{src.stem}_decrypted{src.suffix}")

    decrypt_database(args.source, output, args.key)


if __name__ == "__main__":
    main()
