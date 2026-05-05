# Rust Backend Migration Plan Baseline

S0 establishes the auditable contract used by later Python-to-Rust migration slices.
S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.

## Preservation Rules

- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.
- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.
- `/api/v1/*` is not revived as a runtime chain.
- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.

## Initial Migration Surface

- Python backend files to track: 317
- Current Rust backend files: 37 across the `bill-analyser-core` and `bill-analyser-db` internal crates.
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

## S4b Auth Runtime Bridge

S4b establishes the first Python-to-Rust auth runtime bridge while preserving the Flask REST shell. Python still decodes the refresh JWT and keeps all token/session/database writes; only the decoded refresh-claim shape validation in `POST /api/tokens/refresh` is delegated to Rust.

| Python responsibility | Rust S4b mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/api/routes/auth/tokens.py` post-decode refresh claim validation for `type`, `user_id`, and `username` | `bill_auth_bridge` calls `bill_analyser_core::auth::validate_refresh_token_claims`; `src/bill_analyser/core/auth_rust_bridge.py` invokes the bridge through stdin/stdout JSON and maps Rust validation errors back to the existing REST status/message contract | runtime bridge |
| `src/bill_analyser/api/routes/auth/tokens.py` refresh JWT decode, token signing, user lookup, session creation, cloud settings projection, and response envelope | Python remains the runtime owner; Rust receives only decoded claims and never sees token secrets or writes the database | retained |

S4b uses a Rust CLI bridge instead of PyO3 or C FFI: the repository still uses `uv_build`, earlier runtime tests require the core/db crates to stay internal library boundaries, and the workspace forbids unsafe code. The CLI bridge avoids Python packaging backend changes and unsafe FFI while giving normal Python route tests a real Rust execution path. Runtime startup via `start_backend.ps1` and backend CI build `bill_auth_bridge` before Python serves the auth route; package-style deployments that do not use this startup path must provide `BILL_ANALYSER_RUST_AUTH_BRIDGE` pointing at a prebuilt bridge executable.

Explicit S4b deferrals: full login/register/logout takeover, password verification, JWT signing/verification, API/MCP token writes, refresh session creation semantics, 2FA/step-up/profile/cloud-settings/external-auth/user-data routes, auth DB helpers, audit log writes, and Python business cleanup remain deferred. No compatibility shell is removed because Python remains the auth runtime owner outside the selected validation function.

## S5a Tags Master Data Runtime Bridge

S5a migrates only tag master-data CRUD and display-order persistence to Rust for regular file-backed SQLite databases. Flask REST remains the route shell, and Python still owns batch orchestration, request validation, response envelopes, SQLCipher setup, in-memory databases, settings bundle import/export, and all `bill_tags` relationship helpers.

| Python responsibility | Rust S5a mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/core/database/tags/__init__.py` `get_all_tags`, `get_tag_by_id`, `create_tag`, `update_tag`, `delete_tag`, `update_tag_display_orders` for file DBs | `crates/bill-analyser-db/src/taxonomy/tags.rs` implements user-scoped repository methods with `ORDER BY display_order, created_at DESC`; `crates/bill-analyser-db/src/bin/bill_taxonomy_bridge.rs` exposes stdin/stdout JSON commands; `src/bill_analyser/core/tag_rust_bridge.py` invokes the prebuilt bridge | runtime bridge |
| `src/bill_analyser/core/database/tags/__init__.py` `:memory:` and SQLCipher tag CRUD/display-order behavior | Python aiosqlite fallback remains active because a Rust subprocess opens a separate connection and cannot share in-memory state or SQLCipher pragmas | retained |
| `src/bill_analyser/core/database/tags/__init__.py` `add_tags_to_bill`, `get_tags_for_bill`, `get_tags_for_bills`, `update_bill_tags` | Python remains the owner; relationship table semantics are explicitly deferred to the later bill/tag relationship slice | retained |
| `src/bill_analyser/api/routes/tags.py` list/get/create/update/delete/batch/display-orders REST contract | Route code remains unchanged; existing REST status/error/envelope behavior is exercised through the same DB facade methods | facade retained |
| Settings bundle transaction tag export/import | Export reads tag master data through the public `get_all_tags()` façade, so file DB export now observes the Rust-backed list path; import/upsert remains direct Python SQL and is deferred to the settings-bundle/taxonomy integration slice | mixed: export via façade, import retained |

Runtime startup via `start_backend.ps1` and backend CI now build `bill_taxonomy_bridge` alongside `bill_auth_bridge`. Package-style deployments that do not use this startup path must provide `BILL_ANALYSER_RUST_TAXONOMY_BRIDGE` pointing at a prebuilt bridge executable.

Explicit S5a deferrals: `bill_tags` relationship read/write helpers, settings bundle tag import/upsert, accounts/categories/templates taxonomy domains, SQLCipher Rust access, and any Python route-shell cleanup remain deferred. No Python business code is deleted in S5a because the route shell, in-memory fallback, SQLCipher fallback, batch orchestration, settings import, and relationship helpers remain active owners.

## S5b Accounts Master Data Runtime Bridge

S5b migrates only account master-data persistence to Rust for regular file-backed SQLite databases. Flask REST remains the route shell, and Python still owns account balance synchronization, account transaction move/clear operations, audit/password side effects, SQLCipher setup, in-memory databases, and settings bundle account import/upsert; settings bundle account export reads accounts through the Database façade, so file-backed export observes the Rust-backed list path.

| Python responsibility | Rust S5b mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/core/database/accounts/reads.py` `get_all_accounts`, `get_account_by_id`, `get_sub_accounts` for file DBs | `crates/bill-analyser-db/src/taxonomy/accounts.rs` implements user-scoped list/get/subaccount reads with existing account table fields and list ordering; `bill_taxonomy_bridge` exposes stdin/stdout JSON commands; `core/account_rust_bridge.py` invokes the prebuilt bridge | runtime bridge |
| `src/bill_analyser/core/database/accounts/mutations.py` `create_account`, `update_account`, `delete_account`, `update_account_display_orders` for file DBs | Rust repository writes the same `accounts` columns, preserves parent_id/subAccounts creation, hidden/display_order, aliases JSON, balance/initial_balance, currency/icon/color/comment/category/type, and user_id-scoped mutations | runtime bridge |
| `src/bill_analyser/core/database/accounts/**` `:memory:` and SQLCipher account CRUD/display-order behavior | Python aiosqlite fallback remains active because a Rust subprocess cannot share in-memory state or SQLCipher pragmas safely | retained |
| Legacy file-backed databases and tests that hold account rows before matching user rows | Rust account connection keeps Python's account-path foreign-key PRAGMA behavior instead of making S5b stricter than the previous aiosqlite account connection | retained parity |
| Account balance synchronization, account transaction move/clear, operation password checks, audit logs, and settings bundle account import/upsert | Python remains the owner; these side-effecting operation paths are deferred to later accounts-operations or settings-bundle taxonomy slices. Settings bundle account export uses the façade read path and therefore uses Rust-backed list on file DBs | retained / export via façade |
| `src/bill_analyser/api/routes/accounts/**` REST contract | Route shell, URL/status/envelope, and frontend camelCase/snake_case compatibility remain in Python; display-order REST orchestration now calls the same Database façade batch method that bridges to Rust on file DBs | facade retained |

Explicit S5b deferrals: account balance sync, account transaction move/clear, audit/security password actions, alias-learning helpers, historical account suggestions, settings bundle account import/upsert, SQLCipher Rust access, and any Python route-shell cleanup remain deferred. No Python business code is deleted in S5b because the route shell, in-memory fallback, SQLCipher fallback, balance/operation helpers, audit side effects, account-learning helpers, and settings import/upsert remain active owners.

## S5c Categories Master Data Runtime Bridge

S5c migrates category master-data persistence to Rust for regular file-backed SQLite databases. Flask REST remains the route shell, and Python still owns category route validation/envelopes, category statistics, category rule CRUD/matching, settings bundle category import/upsert, SQLCipher setup, and in-memory databases. Default category seed category creation now uses a batch `ensure_categories()` façade call so file-backed registration/default-seed flows cross the Rust taxonomy runtime once per seed run instead of once per category; default category-rule creation remains Python-owned until S6.

| Python responsibility | Rust S5c mapping | Status |
| --- | --- | --- |
| `src/bill_analyser/core/database/categories/__init__.py` `get_all_categories`, `get_category_by_id`, `get_category_by_name`, `create_category`, `ensure_categories`, `update_category`, `delete_category`, `delete_categories_by_main_category`, and `update_main_category_name` for file DBs | `crates/bill-analyser-db/src/taxonomy/categories.rs` implements user-scoped CRUD, batch missing-category ensure, parent delete cascade by `main_category`, main-category bulk rename, unique-conflict parity, bills-derived fallback when `categories` is empty, and stable `ORDER BY priority ASC, main_category, sub_category`; `bill_taxonomy_bridge` exposes stdin/stdout JSON commands; `core/category_rust_bridge.py` invokes the prebuilt bridge | runtime bridge |
| `src/bill_analyser/core/database/categories/__init__.py` `:memory:` and SQLCipher category CRUD/tree behavior | Python aiosqlite fallback remains active because a Rust subprocess cannot share in-memory state or SQLCipher pragmas safely | retained |
| `src/bill_analyser/core/default_category_seed.py` default category master data | Category seed constants/orchestration remain Python, but file-backed `db.ensure_categories()` persists missing default category rows through one Rust bridge process; default category-rule creation stays Python and now preloads category/rule maps to avoid per-rule taxonomy bridge calls | mixed: master data via façade, rules retained |
| Settings bundle transaction category export/import | Export reads categories through `get_all_categories()`, so file DB export observes the Rust-backed list path; import/upsert remains direct Python SQL and is deferred to S5e settings-bundle taxonomy integration | mixed: export via façade, import retained |
| `src/bill_analyser/api/routes/categories/**` REST contract | Route shell, URL/status/envelope, virtual parent IDs, tree/list formatting, import/export route orchestration, and category rule route behavior remain in Python and are exercised through the same Database façade methods | facade retained |

Explicit S5c deferrals: category rules and matcher, category statistics, settings bundle category import/upsert, SQLCipher Rust access, route-shell cleanup, and Python seed/rule orchestration remain deferred. No Python business code is deleted in S5c except now-unused private seed/rule lookup helpers removed after replacing them with batch category payload generation and rule-name preloading; all retained paths are still active owners.

## Domain Review Baseline

| Domain | Port | Facade | Deferred |
| --- | ---: | ---: | ---: |
| accounts | 7 | 2 | 0 |
| ai-learning-llm | 24 | 6 | 0 |
| ai-ocr | 4 | 1 | 0 |
| api-contract-adapters | 0 | 4 | 0 |
| api-runtime-shell | 3 | 8 | 0 |
| auth-security | 19 | 2 | 0 |
| backup-operations | 5 | 1 | 0 |
| bills-import | 53 | 7 | 0 |
| budgets | 16 | 6 | 0 |
| classification-rules | 17 | 2 | 0 |
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
| tags-templates | 9 | 1 | 0 |

## Review Contract

- Code-bug reviews compare changed inventory tooling and docs against this deterministic scan.
- Feature-gap reviews use the Python File Matrix to prove each backend item is ported, facade-only, deferred with reason, or verified-dead with evidence.
- A later slice cannot claim a domain complete while any file in that domain is unmapped.
