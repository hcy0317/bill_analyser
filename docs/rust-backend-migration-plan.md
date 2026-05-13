# Rust Backend Migration Plan Baseline

S0 establishes the auditable contract used by later Python-to-Rust migration slices.
S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.
The active rewrite program is now governed by the `.omx/plans/rust-full-rewrite-total-plan.md` P0-P15 state machine; the historical S-sections below remain valid migration evidence, not the new governance source of truth.

## P0 Governance Contracts

- Cutover state machine: `PythonProxied -> RustImplemented -> RustOwnedVerified -> PythonDeleted`
- Machine-checkable route/domain manifest: `cargo run -p bill-analyser-core --bin bill_migration_manifest`
- Coverage evidence contract: `workspace.lcov`
- Mechanical dependency gate: `python scripts/check_rust_workspace_dependencies.py --json`
- Domain policies must carry owner files, migrated tests, fixture references, deletion blockers, and transition evidence before any Python deletion claim.

## Preservation Rules

- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.
- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.
- `/api/v1/*` is not revived as a runtime chain.
- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.

## Initial Migration Surface

- Python backend files to track: 321
- Current Rust backend files: 92
- Initial verified-dead files: 0

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
| classification-rules | 18 | 2 | 0 |
| database-facade | 3 | 2 | 0 |
| database-schema | 11 | 3 | 0 |
| import-contracts | 0 | 3 | 0 |
| import-parsers | 8 | 2 | 0 |
| matching-reconciliation | 22 | 5 | 0 |
| recurring-calendar | 4 | 0 | 0 |
| settings-bundle | 9 | 1 | 0 |
| shared-primitives | 5 | 3 | 0 |
| smart-dedup | 9 | 1 | 0 |
| statistics-reporting | 26 | 4 | 0 |
| sync-runtime | 1 | 0 | 0 |
| tags-templates | 6 | 5 | 0 |

## Review Contract

- Code-bug reviews compare changed inventory tooling and docs against this deterministic scan.
- Feature-gap reviews use the Python File Matrix to prove each backend item is ported, facade-only, deferred with reason, or verified-dead with evidence.
- A later slice cannot claim a domain complete while any file in that domain is unmapped.

## S1 Rust Runtime Shell

S1 added the internal Rust workspace boundary while keeping Flask REST as the original runtime shell. The workspace contains `crates/bill-analyser-core`, `crates/bill-analyser-db`, and `crates/bill-analyser-http`; early Rust crates remain internal library / bridge surfaces and 不接管任何业务 API.

## S1b Opt-in Rust HTTP Ingress

S1b adds `crates/bill-analyser-http` as an opt-in `rust-http-shell:proxy-only` ingress for migration verification. In proxy-only mode, proxied Python routes are not Rust business-owned, and the crate reports `api_takeover=false` with `business_migration=none`. The crate owns only health/runtime metadata routes plus catch-all reverse proxy behavior for unowned routes; it does not delete Python code, write the database, or take over import business handlers.

## S2 Shared Primitives Mapping

S2 maps shared Python primitives to Rust primitives without changing the public REST contract:

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/utils/currency.py` | `crates/bill-analyser-core/src/primitives/money.rs` | Explicit yuan / cents conversion and no float-backed money primitive |
| `src/bill_analyser/core/bill_date_utils.py` | `crates/bill-analyser-core/src/primitives/date_time.rs` | Date/time parsing and serialization helpers |
| `src/bill_analyser/api/adapters/transaction_adapter.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | REST adapter parity for amount/time/category/account fields |

## S2a Ownership Matrix, Envelope Oracle, and DB Writer Policy

S2a records the migration governance oracle that separates Rust-owned runtime endpoints from Python-proxied domains. It pins response envelope families, proxy infrastructure error wrapping, and DB writer policy so a route cannot claim Rust ownership without an explicit runtime and persistence contract.

## S3 SQLite Schema Runtime Foundation

S3 added `crates/bill-analyser-db` as the Rust DB runtime foundation for schema/path checks and database guard logic. It is not Rust-primary for business writes: the Flask/Python Database facade remains the primary runtime write path until later domain slices prove and switch a specific business surface.

## S3a Import Route Skeleton

S3a introduced `BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE=import_route_skeleton` to intercept first-phase import routes without claiming DB writes. The skeleton reported `business_migration=import-route-skeleton-no-db` and kept Python import route disabling/deletion blocked unless all five evidence gates pass.

## S4a Import Staging DB Primitives

S4a moved import session and preview staging persistence contracts into Rust SQLite primitives while keeping route runtime deletion blocked. The DB writer policy for the staged import runtime is Rust-owned only when the explicit runtime mode is enabled.

## S5a Parser Template Staging DB Primitives

S5a added parser-template staging primitives for import sessions, preserving user scope, processed status, rollback semantics, and Python-compatible staging field names.

## S5b Confirm Preview-to-Bills DB Primitive

S5b added selected preview confirm-to-bills DB semantics, including duplicate counting, idempotent completed-session behavior, transaction rollback, and bill insertion semantics shared with the Rust bills runtime.

## S5c StandardBill-to-Parser-Template Staging Adapter

S5c pins the StandardBill-to-parser-template adapter that keeps parser output shape stable before preview generation.

## S5d Parser-Template to Preview Staging Adapter

S5d pins parser-template to preview staging, keeping smart-dedup metadata, parser tags, source ids, and selection keys compatible with the Python import flow.

## S6a Preview Update, Reclassify, and Annotation DB Semantics

S6a adds Rust DB primitives for preview update/reclassify and annotation sample read/write. These are part of the import DB runtime but do not by themselves permit Python deletion.

## S7a Preview-Item Transfer and Recurring Decision DB Semantics

S7a adds preview-item transfer and recurring decision DB semantics with expected-state guards, snapshot restore, candidate-field mutation, and user/session scope.

## S8a Preview Learning Decision DB Semantics

S8a adds preview learning decision DB semantics for session-scoped accept/reject/clear decisions and learning promotion.

## S8b Preview LLM Recommendation and Memory DB Semantics

S8b adds preview LLM recommendation review and memory event DB semantics, including accept/reject replay and memory-only blank-field behavior.

## S8c OCR Config App Settings DB Semantics

S8c stores OCR config in Rust `app_settings` primitives and keeps real OCR image recognition proxied to Python until provider runtime is migrated.

## S9a Rust Import Route Runtime Wiring

S9a wires `BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE=import_db_runtime` so Rust can serve import v2 JSON parse/parse_generic/dedup/confirm, session/preview reads, and DB-backed staging routes. It reports `business_migration=import-db-runtime-partial` while Python deletion remains blocked by residual references.

## S9b Preview Update and Reclassify Runtime Wiring

S9b wires preview update and reclassify HTTP routes to the Rust DB primitives. The S9b HTTP tests cover single preview-row edit persistence and batch update rollback semantics.

## S9c Preview-Item, Learning, and LLM Runtime Wiring

S9c wires preview-item transfer/recurring decisions, session learning bypass/promotion, and LLM review/memory routes. The S9c HTTP tests cover transfer accept projection, recurring match updates, learning promotion, and LLM memory writes. Python-proxied provider generation routes, and Python-proxied global Learning Center suggestion/rule routes remain outside this Rust-owned import-session boundary.

## S9d Import Parse, Dedup, Confirm, and Frontend Upload Runtime Wiring

S9d wires JSON parse -> parse_generic -> dedup -> confirm and frontend FormData upload parsing through Rust, while parser-specific Excel/XLSX/provider generation remains proxied where not yet migrated. The S9d HTTP tests cover JSON parse -> parse_generic -> dedup -> confirm DB writes.

## S9e Bills and Transactions CRUD Runtime

S9e makes Rust the runtime owner for core bills/transactions CRUD routes in `import_db_runtime`, including `GET/POST /api/bills`, by-month/get aliases, single update/delete, legacy modify/delete, and batch create/update/delete. Recurring candidates/match, reconciliation statements, and category action helpers are Rust-owned after S14b-S14d. Python bills CRUD/import deletion remains blocked because residual route, frontend, test, and docs references still exist.

## S9f Transaction Picture Runtime

S9f makes Rust the runtime owner for `POST /api/bills/pictures` and `POST /api/bills/pictures/unused` in `import_db_runtime`. The Rust path preserves Flask-compatible `success/result` envelopes, `pictureId/originalUrl` data URL responses, extension allow-list errors, secure filename cleanup, and configurable upload storage through `BILL_ANALYSER_UPLOADS_DIR` with `data/uploads` as the default. Recurring candidates/match, reconciliation statements, and category action helpers are Rust-owned after S14b-S14d.

## S9g Bills Export Runtime

S9g makes Rust the runtime owner for `GET /api/bills/export` in `import_db_runtime`. The Rust path reads current-user bills through the bills DB runtime, preserves the legacy `format=csv|excel|xlsx|xls` contract, returns unsupported-format and empty-result errors before proxying, emits BOM-prefixed CSV with the stable export column order, generates XLSX responses with legacy filenames and MIME type, and escapes formula-like text cells for CSV/Excel clients. Recurring candidates/match, reconciliation statements, and category action helpers are Rust-owned after S14b-S14d.

## S10 Rust Primary HTTP Runtime and Frontend Auth Bridge

S10 switches the local startup boundary from Python-primary to Rust-primary HTTP. `bill_http_server` binds `BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`, Python sidecar runs on port 5001, and Rust proxies unmigrated domains through `BILL_ANALYSER_PYTHON_UPSTREAM=http://127.0.0.1:5001`. The runtime accepts frontend `Authorization: Bearer` access tokens and validates HS256/HS384/HS512 HMAC JWT plus session state before using the trusted user context for Rust-owned routes.

## S13e Budget CRUD Runtime

S13e makes Rust the runtime owner for budget CRUD/export routes in `import_db_runtime`: `GET/POST /api/budgets`, `GET/PUT/DELETE /api/budgets/{id}`, and `GET /api/budgets/export`. Follow-up budget runtime slices S13f-S13i now also own execution, forecast, history/snapshot, and import. Python budget deletion remains blocked because no-residual-reference evidence has not passed.

## S13f Budget Execution Runtime

S13f makes Rust the runtime owner for `GET /api/budgets/execution` in `import_db_runtime`. The Rust path reads `budgets`, `bills`, `bill_tags`, and `categories` directly through `crates/bill-analyser-db/src/budgets.rs`, preserving `user_id` scope, period overlap, date end-of-day normalization, category-id filtering, account/tag filters, legacy `categories.type=1` expense normalization, `abs(sum(amount))`, and the shared execution summary de-duplication contract. Budget import is Rust-owned after S13i.

## S13g Budget Forecast Runtime

S13g makes Rust the runtime owner for `GET /api/budgets/forecast` in `import_db_runtime`. The Rust path reads historical `bills`, enabled `budgets`, and `categories` through `crates/bill-analyser-db/src/budgets.rs`, preserving user scope, budget type filtering, history-window expansion, daily/weekly/monthly/quarterly/yearly grouping, current-period spend lookup, primary-budget-versus-sub-budget amount selection, forecast strategy, backtest MAPE, trend/confidence labels, period progress fields, and the Flask-compatible `success/result` envelope. Budget import is Rust-owned after S13i.

## S13h Budget History And Snapshot Runtime

S13h makes Rust the runtime owner for `GET /api/budgets/history` and `POST /api/budgets/history/snapshot` in `import_db_runtime`. The Rust path preserves canonical sorted `filter_summary`, user-scoped persisted `budget_history` lookup, exact-period snapshot preference, on-demand daily/weekly/monthly/quarterly/yearly fallback, category/type enrichment, snapshot replacement by `(user_id,budget_id,period_start,period_end,filter_summary)`, and Flask-compatible `success/result` envelopes. Budget import is Rust-owned after S13i.

## S13i Budget Import Runtime

S13i makes Rust the runtime owner for `POST /api/budgets/import` in `import_db_runtime`. The Rust path preserves Flask-compatible array payload validation, required `period_type/amount/start_date/category` checks, invalid item error messages, single-transaction import, per-row `error_details`, and upsert-by-`name + user_id` writes with Python-compatible default `period_type`, `alert_threshold`, and `enabled` handling. No route in the budget route set remains Python-proxied, but Python budget deletion remains blocked until residual Python/frontend/test/docs references are cleared and full gates pass.

## S14a Statistics Read Runtime

S14a makes Rust the runtime owner for DB-backed statistics read routes in `import_db_runtime`: `GET /api/statistics/category-statistics`, `GET /api/statistics/category-statistics/trends`, `GET /api/statistics/asset-trends`, `GET /api/statistics/category-pie`, `GET /api/statistics/top-merchants`, and `GET /api/statistics/amounts`. The Rust path reads `bills/accounts/categories` directly, keeps `user_id` scope, timestamp/year-month/all-mode parsing, keyword/date/type filters, asset-trends 365-day guard, category-statistics and amounts cents output, and category-pie/top-merchants yuan output. Analyzer overview/trends/comparison/category/trend remain Python-proxied; exchange-rate provider/custom-rate routes are handled by the Rust statistics-exchange runtime.

## S14b Bills Recurring Runtime

S14b makes Rust the runtime owner for formal bill recurring candidates and match writes in `import_db_runtime`: `GET /api/bills/{bill_id}/recurring-candidates`, `PUT /api/bills/{bill_id}/recurring-match`, and `DELETE /api/bills/{bill_id}/recurring-match`. The Rust path reads enabled `recurring_bills`, preserves tolerance-day clamping, type/amount/schedule/account scoring, linked recurring metadata, Flask-compatible `success/result` envelopes, `bills.created_from_recurring` writes, and `recurring_bills.next_date` recalculation after bind/unbind.

## S14c Bills Reconciliation Runtime

S14c makes Rust the runtime owner for account reconciliation statements in `import_db_runtime`: `GET /api/bills/reconciliation_statements`. The Rust path preserves required `account_id/start_time/end_time` validation, category ID mapping, type and keyword filters, account-scoped bill selection, Flask-compatible `success/result` and error envelopes, yuan-to-cents response amounts, and per-transaction opening/closing balance trace.

## S14d Bills Category Actions Runtime

S14d makes Rust the runtime owner for bill category action helpers in `import_db_runtime`: `POST /api/bills/category/quick-add-keyword` and `POST /api/bills/category/refresh`. The Rust path preserves Flask-compatible quick-add validation and messages, appends non-duplicate `categories.keywords`, reload-equivalent `category_rules` matching semantics, user-scoped bill refresh, and the `success/result` refresh counters.

Runtime metadata now reports `business_migration=import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-bundle-runtime`; the `partial` suffix remains because Analyzer overview/trends/comparison/category/trend routes are still Python-proxied. After S15c.6, the P4 taxonomy route set has no legacy category rules config/cache Python-proxy remainder.

After S15c.7, the `taxonomy-rules-settings` manifest bucket also has no settings encryption status Python-proxy remainder; backup file operations and real SQLCipher connection/migration behavior remain in later ops phases.

S15a makes Rust the runtime owner for account master-data routes in `import_db_runtime`: `GET|POST /api/accounts/`, `GET|PUT|DELETE /api/accounts/{account_id}`, `PUT /api/accounts/display-orders`, and `POST /api/accounts/sync-balances`. The Rust path uses the existing `bill-analyser-db` taxonomy account repository, keeps authenticated `user_id` scope, preserves the Flask `success/result` envelope, formats account IDs/parent IDs for the frontend DTO, builds direct subaccount hierarchy, converts frontend cents to SQLite yuan on writes, and recalculates current-user account balances from bill source/destination account links on sync.

S15a.1 makes Rust the runtime owner for account transaction batch actions in `import_db_runtime`: `POST /api/accounts/{account_id}/transactions/move` and `POST /api/accounts/{account_id}/transactions/clear`. The Rust path verifies the current password or operation-password fallback, keeps authenticated `user_id` scope, updates/deletes bills and `account_transfers`, removes bill side-effect rows on clear, resynchronizes account balances, preserves Flask-compatible top-level `success/result/moved_count/deleted_count` responses, and records best-effort `account` audit logs.

S15b makes Rust the runtime owner for tag master-data routes in `import_db_runtime`: `GET|POST /api/tags/`, `GET|PUT|DELETE /api/tags/{tag_id}`, and `PUT /api/tags/display-orders`. The Rust path uses the existing `bill-analyser-db` taxonomy tag repository, keeps authenticated `user_id` scope, preserves the Flask `success/result` envelope and validation messages, emits frontend `id`/`displayOrder`/`hidden` DTO fields, and originally left `POST /api/tags/batch` as the next batch duplicate-handling follow-up.

S15b.1 makes Rust the runtime owner for tag batch creation in `import_db_runtime`: `POST /api/tags/batch`. The Rust path reuses the taxonomy tag repository, keeps authenticated `user_id` scope, preserves Flask-compatible non-empty array and non-empty name validation, duplicate 409 responses, `skipExists` behavior, and returns the created or reused tag DTO list in a `201 success/result` envelope.

S15c makes Rust the runtime owner for category master-data routes in `import_db_runtime`: `GET|POST /api/categories`, `GET|POST /api/categories/`, `GET|PUT|DELETE /api/categories/{category_id}`, `GET /api/categories/tree`, `GET /api/categories/flat`, `GET|PUT /api/categories/all`, `POST /api/categories/batch`, `POST /api/categories/move`, `GET /api/categories/export`, and `POST /api/categories/import`. The Rust path uses the existing `bill-analyser-db` taxonomy category repository, keeps authenticated `user_id` scope, preserves Flask-compatible `success/result` envelopes and legacy `parentId`/`subCategories` DTOs, and handles virtual parent categories and import/export payloads.

S15c.1 makes Rust the runtime owner for `GET /api/categories/statistics`. The Rust path reads authenticated user bills directly, applies optional `start_date`/`end_date` filters, preserves the legacy main-category tree response with sub-category counters and absolute yuan totals, and keeps `period`/`type` query parameters as compatibility no-ops.

S15c.2 makes Rust the runtime owner for `GET /api/category-rules/`, `POST /api/category-rules/{rule_id}/test`, and `GET /api/rules/overview`. The Rust path reads `category_rules` joined to `categories` through `bill-analyser-db`, directly aggregates user-scoped `import_learning_rules` and `recurring_bills` for the rule-center overview, keeps authenticated `user_id` scope, preserves the Flask-compatible `success/data/total` envelope, supports `category_id` plus `enabled_only=false` query semantics, and tests stored rule expressions against request text without mutating rule state. Category rule create/update/delete/reorder/defaults/migrate is now also Rust-owned for ordinary SQLite runtime writes: defaults reuses the registration default category/rule seed idempotently, and migrate converts legacy `categories.keywords` plus investment keyword settings into canonical category rules while preserving user scope and duplicate skips. Settings bundle import/preview is now Rust-owned for current-user cross-section upsert and rollback preview semantics.

S15c.3 makes Rust the runtime owner for settings bundle export in `import_db_runtime`: `GET /api/settings/bundle/export` and `GET|POST /api/settings/bundle/sections/{section_key}/export`. The Rust path builds the current user's JSON attachment from taxonomy repositories, category rules, LLM config skeletons, and OCR app settings, preserves section filtering, LLM API Key redaction, and current-password verification for sensitive `llmConfigs`/`ocrConfig` section exports. Settings bundle import/preview is now Rust-owned: preview runs the same upsert path in a rollback-only transaction, while import commits current-user cross-section upserts without relying on Python service reloads.

S15c.4 makes Rust the runtime owner for settings bundle import and preview in `import_db_runtime`: `POST /api/settings/bundle/import`, `POST /api/settings/bundle/import/preview`, `POST /api/settings/bundle/sections/{section_key}/import`, and `POST /api/settings/bundle/sections/{section_key}/import/preview`. The Rust path validates the schema, normalizes section lists, applies account/category/tag/template/category-rule/LLM/OCR upserts in one SQLite transaction, rolls back preview, prevents external refs such as `category:123` from binding to unrelated local IDs, preserves existing LLM secrets for masked imports, and writes OCR config through `app_settings`.

S15c.5 makes Rust the runtime owner for `POST /api/categories/update-all`. The Rust path reads current-user bills and canonical enabled `category_rules` directly, preserves the legacy `force` flag semantics, and keeps the Flask-compatible `success/result.total/updated` envelope.

S15c.6 makes Rust the runtime owner for legacy `GET|PUT /api/categories/rules` in `import_db_runtime`. GET returns a user-scoped persisted legacy rules payload from `app_settings` when present, otherwise derives the old `CategoryEngine.rules` JSON shape from enabled canonical `category_rules` joined to `categories`; PUT persists the old `rules` payload into user-scoped `app_settings` and preserves the Flask-compatible success/message envelope. The route no longer falls through to the Python proxy, while Python `CategoryEngine` matcher cache remains a sidecar implementation detail for未迁移导入/学习 flows until those paths are ported or deleted.

S15c.7 makes Rust the runtime owner for `GET /api/settings/encryption/status` in `import_db_runtime`, and the old Flask route shell for that endpoint has been removed. The Rust path remains unauthenticated, returns the existing `success/data` SQLCipher status shape from `bill-analyser-core` ops contracts, reads `BILL_DB_ENCRYPT` and `BILL_DB_KEY`, and reports `sqlcipher_available=false`/`encrypted=false` because the current Rust SQLite runtime is not built with a SQLCipher provider. Real SQLCipher connection patching and migration behavior remain deferred to the later ops cutover.

## Deletion Gate

Python import route disabling/deletion is blocked unless all five evidence gates pass together: Rust route runtime, DB write semantics, frontend import flow, full coverage, and no residual references. Current import and budget runtime slices pass the Rust route/runtime and DB evidence for their owned endpoints, but deletion stays blocked where residual Python route imports, frontend calls, tests, or docs still reference the Python path.
