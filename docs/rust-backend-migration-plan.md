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

S9e makes Rust the runtime owner for core bills/transactions CRUD routes in `import_db_runtime`, including `GET/POST /api/bills`, by-month/get aliases, single update/delete, legacy modify/delete, and batch create/update/delete. Bills export, reconciliation statements, recurring candidates/match, and category actions stay Python-proxied. Python bills CRUD/import deletion remains blocked because residual route, frontend, test, and docs references still exist.

## S9f Transaction Picture Runtime

S9f makes Rust the runtime owner for `POST /api/bills/pictures` and `POST /api/bills/pictures/unused` in `import_db_runtime`. The Rust path preserves Flask-compatible `success/result` envelopes, `pictureId/originalUrl` data URL responses, extension allow-list errors, secure filename cleanup, and configurable upload storage through `BILL_ANALYSER_UPLOADS_DIR` with `data/uploads` as the default. Bills export, reconciliation statements, recurring candidates/match, and category actions remain Python-proxied.

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

S14a makes Rust the runtime owner for DB-backed statistics read routes in `import_db_runtime`: `GET /api/statistics/category-statistics`, `GET /api/statistics/category-statistics/trends`, `GET /api/statistics/asset-trends`, `GET /api/statistics/category-pie`, `GET /api/statistics/top-merchants`, and `GET /api/statistics/amounts`. The Rust path reads `bills/accounts/categories` directly, keeps `user_id` scope, timestamp/year-month/all-mode parsing, keyword/date/type filters, asset-trends 365-day guard, category-statistics and amounts cents output, and category-pie/top-merchants yuan output. Analyzer overview/trends/comparison/category/trend plus live exchange-rate provider/custom-rate routes remain Python-proxied until the Analyzer/provider execution boundary is ported as a complete follow-up.

Runtime metadata now reports `business_migration=import-db-runtime+bills-crud-runtime+bills-picture-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial`; the `partial` suffix remains because Analyzer and exchange-rate provider/custom-rate routes are still Python-proxied.

## Deletion Gate

Python import route disabling/deletion is blocked unless all five evidence gates pass together: Rust route runtime, DB write semantics, frontend import flow, full coverage, and no residual references. Current import and budget runtime slices pass the Rust route/runtime and DB evidence for their owned endpoints, but deletion stays blocked where residual Python route imports, frontend calls, tests, or docs still reference the Python path.
