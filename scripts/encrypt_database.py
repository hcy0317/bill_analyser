"""Migrate an unencrypted SQLite database to SQLCipher encrypted format.

Usage:
    python scripts/encrypt_database.py --key YOUR_KEY [--source data/bills.db] [--output data/bills_encrypted.db]

After migration:
    1. Back up the original database
    2. Replace data/bills.db with the encrypted version
    3. Set environment variables:
       BILL_DB_ENCRYPT=true
       BILL_DB_KEY=YOUR_KEY
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "src"))


def encrypt_database(source_path: str, output_path: str, key: str) -> None:
    """Convert unencrypted SQLite DB to SQLCipher encrypted DB."""
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

    print(f"Encrypting: {source_path} -> {output_path}")

    # Open source with sqlcipher3 (no key = reads unencrypted)
    conn = sqlcipher3.connect(source_path)

    # Attach new encrypted DB
    conn.execute("ATTACH DATABASE ? AS encrypted KEY ?", (output_path, key))
    conn.execute("PRAGMA encrypted.cipher_page_size = 4096")
    conn.execute("PRAGMA encrypted.kdf_iter = 256000")
    conn.execute("PRAGMA encrypted.cipher_compatibility = 4")
    conn.execute("SELECT sqlcipher_export('encrypted')")
    conn.execute("DETACH DATABASE encrypted")
    conn.close()

    # Verify the encrypted database
    enc = sqlcipher3.connect(output_path)
    enc.execute(f"PRAGMA key = '{key}'")
    enc.execute("PRAGMA cipher_page_size = 4096")
    enc.execute("PRAGMA kdf_iter = 256000")
    enc.execute("PRAGMA cipher_compatibility = 4")
    row = enc.execute("SELECT count(*) FROM sqlite_master WHERE type='table'").fetchone()
    tables = row[0] if row else 0
    enc.close()

    print(f"SUCCESS: Encrypted database created with {tables} tables")
    print()
    print("Next steps:")
    print(f"  1. Back up the original: copy {source_path} {source_path}.bak")
    print(f"  2. Replace original: move {output_path} {source_path}")
    print("  3. Set environment variables:")
    print("     BILL_DB_ENCRYPT=true")
    print(f"     BILL_DB_KEY={key}")


def main() -> None:
    """CLI entry point."""
    parser = argparse.ArgumentParser(
        description="Encrypt an existing SQLite database with SQLCipher"
    )
    parser.add_argument(
        "--key",
        required=True,
        help="Encryption passphrase",
    )
    parser.add_argument(
        "--source",
        default=str(PROJECT_ROOT / "data" / "bills.db"),
        help="Path to source unencrypted database (default: data/bills.db)",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="Path for encrypted output (default: source path + _encrypted suffix)",
    )
    args = parser.parse_args()

    output = args.output
    if output is None:
        src = Path(args.source)
        output = str(src.parent / f"{src.stem}_encrypted{src.suffix}")

    encrypt_database(args.source, output, args.key)


if __name__ == "__main__":
    main()
