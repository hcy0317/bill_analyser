# PostgreSQL migration tooling

当前 PostgreSQL 迁移工具负责 SQLite 源库的 dry-run、标准化导出、导入前校验和事务性 PostgreSQL 导入；它不会切换业务 repository，也不会在 dry-run/export/import-check 阶段写入 PostgreSQL。

## Modes

```powershell
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode dry-run --sqlite data\bills.db --output migration-report.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode export --sqlite data\bills.db --output migration-bundle.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode import-check --bundle migration-bundle.json --output migration-import-check.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode import --bundle migration-bundle.json --postgres-url $env:BILL_ANALYSER_POSTGRES_URL --output migration-import-report.json
```

- `dry-run` reads SQLite, validates required legacy columns, counts rows, and emits table checksums.
- `export` normalizes supported SQLite tables into PostgreSQL target-row payloads with deterministic checksums.
- `import-check` replays an export bundle into a counting sink and verifies expected row counts/checksums before any real target writer is introduced.
- `import` writes the bundle to PostgreSQL in a single transaction, records `migration_audit_events`, validates row counts/checksums, and refreshes identity sequences after explicit ID inserts.

Supported source tables in this slice:

- `users`
- `accounts`
- `categories`
- `tags`
- `bills`
- `app_settings`
- `bills_parser_template`
- `category_rules`

The export also generates `account_rules` target rows from legacy account aliases. Amount values from SQLite yuan fields are converted into target `*_cents` integer fields during export.

## Retry

1. Run `dry-run` and fix any `missing_table` or `missing_columns` entries before exporting.
2. Re-run `export`; the bundle checksum is deterministic for the same SQLite contents.
3. Run `import-check` against the exported bundle. A row-count mismatch means the bundle is invalid and should be regenerated.
4. Run `import` against an already migrated PostgreSQL database. A failed import rolls back the target transaction and records a retryable failure audit event when PostgreSQL remains reachable.

## Rollback

Dry-run/export/import-check rollback is file cleanup only: discard the generated report/bundle/check files and rerun from the unchanged SQLite source. Import rollback is transaction rollback; no target rows become visible if a table insert or row-count check fails. After a successful import, rollback is an operator database restore/drop of the target PostgreSQL database plus rerun from the unchanged SQLite source.
