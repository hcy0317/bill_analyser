# Rust Backend Migration Plan Baseline

S0 establishes the auditable contract used by later Python-to-Rust migration slices.
S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.

## Preservation Rules

- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.
- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.
- `/api/v1/*` is not revived as a runtime chain.
- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.

## Initial Migration Surface

- Python backend files to track: 321
- Current Rust backend files: 52
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

S1 added the internal Rust workspace boundary while keeping Flask REST as the runtime shell. The workspace contains `crates/bill-analyser-core` and `crates/bill-analyser-db`; Rust remains an internal library / bridge surface and 不接管任何业务 API.

## S2 Shared Primitives Mapping

S2 maps shared Python primitives to Rust primitives without changing the public REST contract:

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/utils/currency.py` | `crates/bill-analyser-core/src/primitives/money.rs` | Explicit yuan / cents conversion and no float-backed money primitive |
| `src/bill_analyser/core/bill_date_utils.py` | `crates/bill-analyser-core/src/primitives/date_time.rs` | Date/time parsing and serialization helpers |
| `src/bill_analyser/api/adapters/transaction_adapter.py` | `crates/bill-analyser-core/src/adapters/transactions.rs` | REST adapter parity for amount/time/category/account fields |

## S3 SQLite Schema Runtime Foundation

S3 added `crates/bill-analyser-db` as the Rust DB runtime foundation for schema/path checks and database guard logic. It is not Rust-primary for business writes: the Flask/Python Database facade remains the primary runtime write path until later domain slices prove and switch a specific business surface.

## S7 Parser Import Contracts

S7 starts the import parser domain in Rust without switching parser runtime ownership. It preserves the Python parser detection order (`wechat -> alipay -> icbc -> cmbc -> abc -> ccb`), parser metadata, source labels, parser tag normalization, generic raw-bill post-processing, and the JSON `StandardBill` key shape used by the existing Python import staging path.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/parsers/factory.py` | `crates/bill-analyser-core/src/parsers.rs` | Parser registry order and metadata contract |
| `src/bill_analyser/import_contracts/parser_tags.py` | `crates/bill-analyser-core/src/parsers.rs` | Parser/channel tag normalization and serialization |
| `src/bill_analyser/parsers/base.py` | `crates/bill-analyser-core/src/parsers.rs` | Raw parser output post-processing into StandardBill-shaped data |
| `tests/fixtures/import_samples/**` parser families | `crates/bill-analyser-core/tests/fixtures/parser_golden_contracts.json` | Deterministic golden contract cases for six parser families |

No Python parser implementation is removed in S7. The Rust parser module is an internal contract/oracle surface: Python parser tests continue to parse real CSV/XLS/XLSX samples, while Rust golden contract tests pin the normalized StandardBill, parser tag, and source-label output expected from each dedicated parser family.

## S8 Smart Dedup Contracts

S8 starts the smart-dedup domain in Rust without switching the Python import runtime. It preserves the current Python ordering for exact duplicate, platform-bank duplicate, same-batch transfer, similar duplicate, split bill, database duplicate, and cross-batch transfer semantics as a pure Rust contract layer.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/core/smart_dedup/models.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Deduplication type/result/group data contracts and Python hidden-field serde aliases |
| `src/bill_analyser/core/smart_dedup/exact.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Exact duplicate grouping and source-priority keep rule |
| `src/bill_analyser/core/smart_dedup/platform_bank.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Platform-bank amount/time/source, transfer-intent guard, merged fields, parser tags, and source-id provenance |
| `src/bill_analyser/core/smart_dedup/transfers.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Same-batch transfer pair mutation contract and cross-batch transfer marker contract |
| `src/bill_analyser/core/smart_dedup/grouping.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Similar duplicate merged-field/source-id contract and split bill grouping contract |
| `src/bill_analyser/core/smart_dedup/database.py` | `crates/bill-analyser-core/src/smart_dedup.rs` | Database duplicate marker contract without DB query ownership |
| `src/bill_analyser/core/smart_dedup/reconciliation.py` and `src/bill_analyser/core/database/reconciliation/**` | `crates/bill-analyser-core/src/smart_dedup.rs` | Import reconciliation candidate classification/key/group contract; persistence remains the existing Python DB boundary |

No Python smart-dedup implementation is removed in S8. The Rust module uses `Money` cents and shared bill datetime parsing to avoid float drift while Rust contract tests pin Python-shaped `_parser_id` / `_template_id` / `_parser_tags` JSON, dedup source IDs, kept/removed indices, transfer destination hints, split groups, database duplicate markers, cross-batch transfer markers, and import reconciliation candidates. Runtime DB querying/persistence remains Python-owned until the import pipeline slice takes over that boundary.

## S9a Bills Read Adapter Contracts

S9a starts the bills CRUD/media/reconciliation domain by pinning the read/list/export adapter contract in Rust without switching the Python runtime. It covers transaction-list JSON shape, amount yuan-to-cents presentation, transaction type codes, local bill date to Unix seconds conversion, by-month date ranges, type query filters, and CSV/Excel formula-cell escaping.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/api/adapters/transaction_adapter.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Backend bill to frontend transaction JSON contract |
| `src/bill_analyser/api/routes/bills/crud_query.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | List type filter, by-month range, and export cell serialization contract |
| `tests/new_ui/test_transaction_list_rest_api.py` and `tests/new_ui/test_bills_api.py` | `crates/bill-analyser-core/tests/transaction_adapter_contracts.rs` | Golden read/list adapter cases for cents/yuan, type codes, account ids, date fields, and formula escaping |

No Python bills route or database implementation is removed in S9a. Python remains the runtime owner for DB reads, writes, media, account balance sync, and reconciliation statements until later S9 sub-slices verify and switch those boundaries.

## S9b Bills Write and Balance Contracts

S9b extends the Rust bills contract to the write-side adapter and account balance synchronization rules without switching the Python runtime. It covers frontend mutation payload conversion, manual create fallback fields, category resolution/defaulting contracts, batch-create payload and route-envelope shapes, create/update field guards, batch-update response projection, affected account collection for create/update/delete/batch-delete, and the raw account balance formula used by the existing Python database layer.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/api/adapters/transaction_adapter.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Frontend write payload to backend bill field conversion, including cents-to-yuan, type mapping, tag IDs, account IDs, and local timestamp normalization |
| `src/bill_analyser/api/routes/bills/crud_prepare.py` and `src/bill_analyser/api/routes/bills/crud_create_update.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Manual-create description/counterparty/source-account fallbacks, legacy modify/delete response shapes, and batch-create request/response validation contract |
| `src/bill_analyser/api/routes/bills/category_actions.py` and `src/bill_analyser/core/database/bills/__init__.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Route-level and DB-level update field allowlists, batch-update response shape, create aliases, and the current no-immediate-balance-sync batch-update policy |
| `src/bill_analyser/api/routes/bills/support.py` and `src/bill_analyser/core/database/accounts/balances.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Affected account ID collection for create/update/delete/batch-delete and raw DB balance formula contract, including the frontend-signed outgoing amount convention |
| `tests/new_ui/test_bills_api.py`, `tests/domains/db/**`, and `tests/domains/import_flow/unit/test_bills_route_branches.py` | `crates/bill-analyser-core/tests/transaction_adapter_contracts.rs` | Golden write/batch/balance contract cases for later bridge replacement |

No Python bills write, delete, batch, or account balance implementation is removed in S9b. Pictures/media and reconciliation statement runtime behavior remain deferred to later S9 sub-slices.

## S9c Bills Picture Media Contracts

S9c extends the Rust bills contract to transaction-picture upload and unused-picture deletion without switching the Python runtime. It preserves the current REST-only picture surface: upload accepts multipart field `picture`, validates only the original filename extension allowlist, derives the saved suffix from Werkzeug-style `secure_filename(original_filename)`, stores the file under the fixed uploads directory as `<uuid4hex><derived lowercase suffix>` when a suffix remains, and returns an inline `originalUrl` data URL for preview. Unused-picture delete remains best-effort by sanitized picture id and returns success even when the file is already absent.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/api/config/bills.py` and `src/bill_analyser/api/routes/bills/support.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Transaction-picture extension allowlist and unsupported-type error message contract |
| `src/bill_analyser/api/routes/bills/crud_query.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Upload/delete REST status codes, success/error envelopes, UUID-based picture id suffix handling, and unused-delete best-effort response contract |
| `src/bill_analyser/api/routes/bills/import_review.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Data URL MIME projection and `application/octet-stream` fallback contract |
| `tests/new_ui/test_transaction_pictures_rest_api.py` and `tests/domains/import_flow/unit/test_bills_route_branches.py` | `crates/bill-analyser-core/tests/transaction_adapter_contracts.rs` | Golden picture API cases for upload/delete success, validation failures, 500 error envelopes, data URL projection, and filename sanitization |

No Python picture route or file-storage implementation is removed in S9c. Current Python does not persist `pictureIds` into bill DB rows and does not enforce per-bill picture ownership; S9c records that behavior instead of inventing attachment persistence. Legacy `/api/v1/transaction/pictures/*` 404 behavior stays Python routing-runtime owned and is not part of the Rust adapter contract. Import-session uploads, OCR receipt recognition, parse-import temp files, and reconciliation statements remain deferred to later dedicated slices.

## S9d Bills Reconciliation Statement Contracts

S9d completes the bills CRUD/media/reconciliation contract split by pinning the reconciliation statement route behavior in Rust without switching the Python runtime. It preserves the `GET /api/bills/reconciliation_statements` query contract, route envelopes, all-time versus filtered opening-balance fallback, local timestamp-to-date filters, category/type/keyword filter projection, ascending balance-trace calculation, descending transaction output, and cents-based API result fields.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/api/routes/bills/reconciliation.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Required query parameters, `1..4` reconciliation type-code mapping, all-time/date-range filter projection, and 400/404/500 route envelopes |
| `src/bill_analyser/core/database/bills/__init__.py` and account reads/mappings | `crates/bill-analyser-core/src/adapters/transaction.rs` | Account-id filter shape, category filter materialization, historical opening-balance snapshot selection, and all-time initial-balance fallback |
| `src/bill_analyser/api/adapters/transaction_adapter.py` | `crates/bill-analyser-core/src/adapters/transaction.rs` | Frontend transaction projection extension with `accountOpeningBalance` and `accountClosingBalance`, sorted by frontend `time` descending |
| `tests/domains/import_flow/unit/test_bills_route_branches.py`, `tests/test_reconciliation_fields.py`, `tests/test_reconciliation_amount_fix.py`, `tests/test_v6_1_fixes.py`, and `tests/test_v6_2_comprehensive.py` | `crates/bill-analyser-core/tests/transaction_adapter_contracts.rs` | Golden reconciliation cases for missing/invalid account parameters, account-not-found/500 envelopes, opening balance, amount-unit conversion, inflow/outflow direction, skipped unknown/foreign transfers, and result payload shape |

No Python reconciliation route, adapter, or database implementation is removed in S9d. The Rust layer is a contract and calculation surface only; Flask remains the route owner, Python still performs DB reads and adapter hydration, and later runtime-bridge slices must re-run API parity before replacing the live path.

## S10 Import V2 Pipeline Contracts

S10 starts the import-v2 pipeline domain in Rust without switching Flask routes, Python parser execution, aiosqlite staging writes, or BillService orchestration. It pins route envelope DTOs, import pipeline step ordering, staging table/field names, server-paged preview query normalization, `preview_ids` order-preserving selection, preview selected-flag coercion, lightweight preview filter-index projection, nested matching payload families, and `expectedState` conflict/error response semantics.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/api/routes/bills/v2_pipeline.py` | `crates/bill-analyser-core/src/import_pipeline.rs` | Stage parse/dedup/confirm success and error envelope contracts |
| `src/bill_analyser/api/routes/bills/v2_sessions.py` | `crates/bill-analyser-core/src/import_pipeline.rs` | Session summary, cancel message, preview page/index envelope contracts |
| `src/bill_analyser/api/routes/bills/v2_preview_actions.py` | `crates/bill-analyser-core/src/import_pipeline.rs` | `Invalid request`, stale preview conflict, and `expectedState` object contract |
| `src/bill_analyser/core/bills/service_parts/import_preview_paging.py` | `crates/bill-analyser-core/src/import_pipeline.rs` | Sort key allowlist, page/page_size clamp, `preview_ids` normalization, and preview filter index projection |
| `src/bill_analyser/import_contracts/preview_selection.py` | `crates/bill-analyser-core/src/import_pipeline.rs` | Confirm/update selected-key precedence and bool coercion contract |
| `tests/domains/import_flow/**` and `tests/new_ui/test_import_*.py` | `crates/bill-analyser-core/tests/import_pipeline_contracts.rs` | Golden contract cases for preview paging, stage envelopes, matching payload, expected state, and selection semantics |

No Python import-v2 route, staging database, parser factory, category/account matching, recurring lookup, learning replay, or confirm-to-bills cleanup implementation is removed in S10. Runtime takeover remains deferred until a later bridge slice can compare live Python and Rust paths against the same imported sample/session fixtures.

## S11 Import Learning and LLM Preview Contracts

S11 starts the import-learning / LLM preview memory domain in Rust without switching Flask routes, aiosqlite persistence, numpy model training, provider calls, or preview mutation runtime. It pins composite match hash normalization, feature payload and labels, token generation, green/blue policy thresholds, model registry/snapshot metadata, session-learning route envelopes, Learning Center paging/error envelopes, LLM stable error envelopes, preview memory event shape, blank-only LLM apply behavior, and reject-restore snapshot guards.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/core/database/imports/learning/base.py` | `crates/bill-analyser-core/src/import_learning.rs` | Composite match feature normalization, alias parsing, hash ordering, and preview id validation contract |
| `src/bill_analyser/core/import_learning/features.py` | `crates/bill-analyser-core/src/import_learning.rs` | Feature schema, labels, amount buckets, sample filtering, token generation, and confirmation-count keys |
| `src/bill_analyser/core/import_learning/model.py` and `src/bill_analyser/core/database/imports/learning/model.py` | `crates/bill-analyser-core/src/import_learning.rs` | Model key/family/dimensions, snapshot payload, active model version, metrics payload, and archive/active metadata contract |
| `src/bill_analyser/core/import_learning/policy.py` | `crates/bill-analyser-core/src/import_learning.rs` | Green/blue confidence, margin, confirmation, conflict, score, level, and auto-apply policy contract |
| `src/bill_analyser/api/routes/bills/import_learning.py` and `src/bill_analyser/api/routes/learning.py` | `crates/bill-analyser-core/src/import_learning.rs` | Session suggestions/promote errors, legacy rule page envelope, Learning Center page envelope, and batch validation surfaces |
| `src/bill_analyser/core/database/llm/candidates/preview_apply.py`, `preview_review.py`, and `src/bill_analyser/api/routes/llm/support.py` | `crates/bill-analyser-core/src/import_learning.rs` | LLM memory event DTO, blank-field-only apply semantics, reject restore guard, and stable `code`/`error_code` envelope |
| `tests/domains/import_flow/unit/test_import_learning_model_loop.py`, `tests/new_ui/test_bills_learning_suggestions_api.py`, `tests/new_ui/test_learning_suggestion_center_api.py`, and `tests/new_ui/test_llm_import_session_analysis_api.py` | `crates/bill-analyser-core/tests/import_learning_contracts.rs` | Golden contract cases for learning features, policy, model metadata, route envelopes, and LLM preview memory behavior |

No Python import-learning database, corpus writes, active model training/prediction, suggestion writeback, LLM provider prompt/generation, or live preview apply/reject implementation is removed in S11. Runtime takeover remains deferred until a later bridge slice can verify model/persistence parity against the same session and memory fixtures.

## S12 Matching, Investment, and Recurring Contracts

S12 starts the matching / investment / recurring domain in Rust without switching Flask routes, aiosqlite persistence, manual-pair writes, suppression writes, investment user settings storage, recurring suggestion persistence, or preview mutation runtime. It pins candidate id families, preview session candidate projection, manual pair request validation, reconciliation query normalization, matching action payload envelopes, transfer pair scoring, bill-pair feedback payload relationship filtering, investment keyword normalization/profile/scoring/PnL guards, and recurring pattern hash/frequency/suggestion calculations.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/core/matching/candidate_ids.py` | `crates/bill-analyser-core/src/matching.rs` | Preview/formal/reconciliation candidate id builders and parsers |
| `src/bill_analyser/api/routes/matching/support.py` and `src/bill_analyser/core/matching/session_candidates.py` | `crates/bill-analyser-core/src/matching.rs` | Matching candidate session projection, manual pair request validation, action payload, query normalization, and feedback payload contract |
| `src/bill_analyser/core/matching/transfer_candidates.py` and `src/bill_analyser/core/database/matching/**` | `crates/bill-analyser-core/src/matching.rs` | Transfer pair direction, amount/date/account eligibility, candidate id, score, suppression key, and manual pair DTO contract |
| `src/bill_analyser/core/investment/settings.py` and `src/bill_analyser/core/investment/matching.py` | `crates/bill-analyser-core/src/matching.rs` | User keyword normalization, deterministic investment profile/scoring, generic-service-fee guard, bank-interest guard, and PnL classification contract |
| `src/bill_analyser/core/recurring_detection.py`, `src/bill_analyser/api/routes/recurring.py`, and `src/bill_analyser/core/database/recurring_suggestions/**` | `crates/bill-analyser-core/src/matching.rs` | Recurring hash, frequency, next-date, confidence, and suggestion DTO contract |
| `tests/domains/import_flow/**`, `tests/domains/investment/**`, and `tests/domains/analytics/**` | `crates/bill-analyser-core/tests/matching_contracts.rs` | Golden contract cases for candidate ids, session projection, pair requests, transfer candidates, investment recognition, PnL, bank interest, and recurring detection |

No Python matching, investment, recurring route, database, or preview-action implementation is removed in S12. The old `/api/matching/investment-settings` write surface remains gone; S12 records deterministic keyword/settings behavior only. Runtime takeover remains deferred until a later bridge slice can compare live Python and Rust paths against the same matching sessions, manual pair, suppression, investment, and recurring fixtures.
