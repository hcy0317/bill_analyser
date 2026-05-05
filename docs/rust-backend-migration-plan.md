# Rust Backend Migration Plan Baseline

S0 establishes the auditable contract used by later Python-to-Rust migration slices.
S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.

## Preservation Rules

- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.
- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.
- `/api/v1/*` is not revived as a runtime chain.
- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.

## Initial Migration Surface

- Python backend files to track: 313
- Current Rust backend files: S1 introduces the `bill-analyser-core` internal runtime shell crate only.
- Initial verified-dead files: 0

## S1 Rust Runtime Shell

- `bill-analyser-core` 是 Rust 内部库边界，当前只包含 runtime identity、health check、统一错误结构、serde JSON 和 API response envelope 基础类型。
- Flask REST 外壳继续作为运行时入口；S1 不接管任何业务 API，不新增 `/api/v1/*`，不迁移任何业务域。
- S1 的 `ApiResponse` 只代表 Rust runtime shell foundation；后续业务 API 迁移必须为对应 endpoint 增加现有 Flask envelope parity adapter 或测试，不能把 S1 envelope 直接当成全站业务响应替代品。
- 后续功能域切片必须在各自 feature-gap review 中逐项声明 Python business item 的 `ported`、`facade_only`、`verified_dead` 或 `deferred_with_reason` 状态；S1 的 `business_migration` 保持 `none`。

## S2 Shared Primitives Mapping

S2 adds foundational Rust primitives and adapter helpers only. Flask REST remains the runtime shell, and no Python business route or service is retired in this slice.

| Python responsibility | Rust S2 mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/utils/currency.py` cents/yuan conversion, symbols, amount validation baseline | `crates/bill-analyser-core/src/primitives/money.rs` and `crates/bill-analyser-core/src/primitives/currency.rs` provide exact cent storage, explicit yuan text conversion, half-up rounding, transaction amount range validation, default `CNY`, and symbol lookup | foundational port |
| `src/bill_analyser/utils/constants.py` transaction IDs, default currency, default/max pagination, sort field/order constants | `crates/bill-analyser-core/src/primitives/transaction_type.rs`, `pagination.rs`, `sorting.rs`, `ids.rs`, and `currency.rs` define typed reusable constants and parsers | foundational port |
| `src/bill_analyser/core/bill_date_utils.py` supported bill date parsing and normalized `YYYY-MM-DD HH:MM:SS` text | `crates/bill-analyser-core/src/primitives/date_time.rs` supports existing dash, slash, Chinese, date-only, minute, second, and ISO-prefix inputs | foundational port |
| `src/bill_analyser/api/adapters/account_adapter.py` alias parsing and hierarchy constants | `crates/bill-analyser-core/src/adapters/account.rs` and `category.rs` provide shared alias parsing and `0`/`virtual_*` helpers | foundational port |
| `src/bill_analyser/api/adapters/category_adapter.py` category virtual parent conventions | `crates/bill-analyser-core/src/adapters/category.rs` records the `virtual_<main>` parent ID convention | foundational port |
| `src/bill_analyser/api/adapters/transaction_adapter.py` type-name mapping, sign convention, timestamp normalization, and page response shape | `crates/bill-analyser-core/src/adapters/transaction.rs`, `adapters/api.rs`, and `primitives/date_time.rs` provide typed mappings and response pagination helpers | foundational port |
| `src/bill_analyser/api/routes/request_context_helpers.py` positive `user_id` request context expectation | `crates/bill-analyser-core/src/primitives/ids.rs` and `auth.rs` provide positive `UserId` parsing plus camelCase serialized `AuthContext` | foundational port |

Explicit S2 deferrals: account category/type display maps, category type maps, tag filters, amount filter operators, error-code message localization, and historical dedup mode constants remain with their owning future business/API domains; `format_currency_display` UI formatting remains in Python until a display/reporting slice needs it; full account/category/transaction object projection, database-backed enrichment, `run_async_in_new_loop`, route status/envelope parity per endpoint, and any Python runtime cleanup are also deferred.

## S3 SQLite Schema Runtime Foundation

S3 adds `crates/bill-analyser-db` as an internal Rust DB runtime foundation only. Flask/Python remains the Database façade and no business DB write path is Rust-primary in this slice.

| Python responsibility | Rust S3 mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/core/database/runtime.py` database path resolution and connection lifecycle guardrails | `crates/bill-analyser-db/src/path.rs` and `connection.rs` provide explicit temp/copy DB path guards, `data/bills.db` rejection, WAL/foreign_keys/synchronous/cache/temp-store PRAGMA setup, and busy timeout configuration for Rust-side dry-run use | foundational port |
| `src/bill_analyser/core/database/shared.py` default positive `user_id` contract | `crates/bill-analyser-db/src/user_scope.rs` wraps core `UserId` into parameterized `user_id = ?` SQL scope helpers without inline values | foundational port |
| `src/bill_analyser/core/database/schema/__init__.py` schema orchestration boundary | `crates/bill-analyser-db/src/schema.rs` records schema responsibility mapping and supports copied-fixture schema validation dry-runs | foundational port |
| `src/bill_analyser/core/database/schema/core/*`, `templates_imports/*`, `users_security.py` table DDL and legacy migrations | Rust records these as deferred; Python schema initialization remains the runtime owner until the corresponding business/auth/import slices migrate table ownership with parity tests | deferred |
| `src/bill_analyser/core/database/encryption.py` SQLCipher opt-in behavior | Deferred; Python remains the only SQLCipher runtime owner in S3 | deferred |

S3 uses `rusqlite` with the bundled SQLite feature rather than `sqlx`: this keeps the first DB layer synchronous, small, and internal while avoiding a Tokio runtime or Python bridge before any business write path is ready. The crate is tested only against temporary/copy databases and refuses the repository `data/bills.db` path.

Explicit S3 deferrals: full Python schema DDL porting, SQLCipher parity, aiosqlite async lifecycle parity, all business CRUD helpers, and any Python database cleanup remain with later domain slices. S3 does not delete or replace Python runtime files and does not introduce Rust-primary writes.

## S4 Auth/Security Core Foundation

S4 starts the auth-security migration with pure Rust contract helpers only. Flask/Python still owns every runtime auth route, password hash check, JWT encode/decode, 2FA write path, session DB write, and audit-log write. No auth endpoint is Rust-primary in this slice.

| Python responsibility | Rust S4 mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/api/middleware/auth.py` required/optional Bearer header format checks | `crates/bill-analyser-core/src/auth/mod.rs` provides `parse_bearer_authorization_header` and `extract_bearer_token_or_empty` with the existing missing/malformed header error messages | foundational port |
| `src/bill_analyser/api/routes/auth/tokens.py` token kind user-agent markers and token type inference | Rust `TokenKind`, token type constants, and `infer_token_type_from_user_agent` preserve session/API/MCP marker semantics | foundational port |
| `src/bill_analyser/api/routes/auth/tokens.py` refresh-token decoded claim shape checks | Rust `validate_refresh_token_claims` records the existing `Not a refresh token` vs `Invalid refresh token` REST error contract after Python JWT decode | foundational port |
| `src/bill_analyser/api/routes/auth/tokens.py` token list device-name projection | Rust `parse_user_agent_device_name` mirrors the existing Windows/iOS/macOS/Android/Linux and browser display fallback rules | foundational port |
| `src/bill_analyser/api/routes/auth/registration.py` password policy validation messages | Rust `PasswordPolicy` mirrors configured length, uppercase, lowercase, digit, and special-character validation messages | foundational port |
| `src/bill_analyser/core/database/users/auth/__init__.py` persistent 2FA recovery-code normalization pre-hash contract | Rust `normalize_recovery_code` and `recovery_code_hash_input` preserve uppercase whitespace-insensitive canonical input for a later hashing/DB takeover | foundational port |

Explicit S4 deferrals: login/register/logout route takeover, bcrypt password verification, JWT signing/verification, API/MCP token creation, refresh session creation, 2FA enable/disable/recovery writes, step-up token signing, profile/cloud-settings/external-auth/user-data routes, auth/session SQLite helpers, auth/audit log writes, Python bridge wiring, and Python business cleanup remain deferred. This slice intentionally avoids half-migrated runtime paths.

## Domain Review Baseline

| Domain | Port | Facade | Deferred |
| --- | ---: | ---: | ---: |
| accounts | 6 | 2 | 0 |
| ai-learning-llm | 24 | 6 | 0 |
| ai-ocr | 4 | 1 | 0 |
| api-contract-adapters | 0 | 4 | 0 |
| api-runtime-shell | 3 | 8 | 0 |
| auth-security | 18 | 2 | 0 |
| backup-operations | 5 | 1 | 0 |
| bills-import | 53 | 7 | 0 |
| budgets | 16 | 6 | 0 |
| classification-rules | 16 | 2 | 0 |
| database-facade | 3 | 2 | 0 |
| database-schema | 11 | 3 | 0 |
| import-contracts | 0 | 2 | 0 |
| import-parsers | 8 | 2 | 0 |
| matching-reconciliation | 22 | 5 | 0 |
| recurring-calendar | 4 | 0 | 0 |
| settings-bundle | 8 | 1 | 0 |
| shared-primitives | 5 | 3 | 0 |
| smart-dedup | 9 | 1 | 0 |
| statistics-reporting | 26 | 4 | 0 |
| sync-runtime | 1 | 0 | 0 |
| tags-templates | 8 | 1 | 0 |

## Review Contract

- Code-bug reviews compare changed inventory tooling and docs against this deterministic scan.
- Feature-gap reviews use the Python File Matrix to prove each backend item is ported, facade-only, deferred with reason, or verified-dead with evidence.
- A later slice cannot claim a domain complete while any file in that domain is unmapped.
