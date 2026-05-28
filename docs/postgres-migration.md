# PostgreSQL migration tooling

当前 PostgreSQL 迁移工具负责 SQLite 源库的 dry-run、标准化导出、导入前校验和事务性 PostgreSQL 导入；它不会切换业务 repository，也不会在 dry-run/export/import-check 阶段写入 PostgreSQL。

账号业务数据恢复使用独立工具 `bill_postgres_account_recovery`，不复用整库导入命令。该工具面向单个 SQLite 源用户和显式 Postgres 目标用户，默认只做只读 preflight/dry-run；`apply` 必须带 manifest id、目标用户三元确认和 git-ignored snapshot 目录，才会在单个目标用户事务中清空并重建业务表。Postgres URL 优先从进程内的 `BILL_ANALYSER_POSTGRES_URL` 读取，避免把带密码的连接串放进命令行参数。

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

Amount values from SQLite yuan fields are converted into target `*_cents` integer fields during export. Legacy account aliases are not exported as target runtime data by the whole-database migration command.

## Account recovery

```powershell
cargo run -p bill-analyser-db --bin bill_postgres_account_recovery -- --mode preflight --sqlite data\backups\bills-pre-import-staging-cleanup-20260517_190849.db --source-user-id 5
$env:BILL_ANALYSER_POSTGRES_URL = "postgres://..."
cargo run -p bill-analyser-db --bin bill_postgres_account_recovery -- --mode dry-run --sqlite data\backups\bills-pre-import-staging-cleanup-20260517_190849.db --source-user-id 5 --target-user <id-or-username-or-email> --output .git\ai\recovery-snapshots\dry-run-manifest.redacted.json
cargo run -p bill-analyser-db --bin bill_postgres_account_recovery -- --mode apply --sqlite data\backups\bills-pre-import-staging-cleanup-20260517_190849.db --source-user-id 5 --target-user <id-or-username-or-email> --manifest-id <manifest-id> --confirm-target-user-id <id> --confirm-target-username <username> --confirm-target-email <email> --snapshot-dir .git\ai\recovery-snapshots\<manifest-id> --output .git\ai\recovery-snapshots\<manifest-id>\apply.redacted.json
```

- `preflight` reads SQLite only, hashes the source file and source user identity, reports user-scoped counts, verifies the expected recovery shape, and never prints raw usernames, emails, transaction descriptions, or aliases.
- `dry-run` additionally resolves the explicit Postgres target user and emits a pseudonymous sensitive manifest with `manifest_id`, source checksum/counts, target pre-counts, target triplet hash, table allowlist/denylist, id remap plan, snapshot default path, and `dry_run_hash`.
- `apply` recomputes the dry-run manifest, rejects manifest or target-confirmation mismatch, requires the source counts to match the expected recovery shape, verifies the snapshot directory is outside the worktree or git-ignored/under `.git`, writes a redacted snapshot manifest, then clears and rebuilds only the target user's accounts, categories, tags, bills, budget rows, category rules, and account rules inside one Postgres transaction with an advisory lock.
- `--output` is allowed only outside the worktree, under `.git`, or at a git-ignored path. Manifest identity hashes and source checksums are for operator confirmation and audit correlation, not anonymous public artifacts.
- Recovery preserves `users`, auth/session/2FA/external-auth rows, parser templates, transaction templates, operation-password settings, backup/cloud/LLM/OCR credentials, audit history, and all other non-business runtime state.
- Legacy budgets whose `name` is blank are recovered with a deterministic display name derived from category, sub-category, period, and start date; explicit source names are preserved unchanged.
- Legacy budget history rows whose `budget_id` no longer exists in the recovered user's budgets are reported as orphaned source rows and skipped, because PostgreSQL budget history requires a live budget foreign key.
- Settings recovery is default-deny. Only non-sensitive allowlisted preference keys are counted as recoverable; keys containing `auth`, `2fa`, `operation_password`, `cloud`, `backup`, `llm`, `ocr`, `provider`, `api_key`, `token`, `secret`, or `password` are denied.
- Legacy SQLite account `aliases` are consumed only by this recovery tool and are converted directly into `account_rules` with `source=sqlite_account_recovery`; the tool does not recreate account alias runtime/API compatibility.

## Retry

1. Run `dry-run` and fix any `missing_table` or `missing_columns` entries before exporting.
2. Re-run `export`; the bundle checksum is deterministic for the same SQLite contents.
3. Run `import-check` against the exported bundle. A row-count mismatch means the bundle is invalid and should be regenerated.
4. Run `import` against an already migrated PostgreSQL database. A failed import rolls back the target transaction and records a retryable failure audit event when PostgreSQL remains reachable.

## Rollback

Dry-run/export/import-check rollback is file cleanup only: discard the generated report/bundle/check files and rerun from the unchanged SQLite source. Import rollback is transaction rollback; no target rows become visible if a table insert or row-count check fails. After a successful import, rollback is an operator database restore/drop of the target PostgreSQL database plus rerun from the unchanged SQLite source.
