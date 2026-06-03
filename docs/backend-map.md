# Backend Map

Bill Analyser backend is a Rust-only Axum service. `bill_http_server` owns every runtime HTTP route under `/api/...`; PostgreSQL is the only business database and Weaviate is required for derived vector recall.

## Crate Layout

- `src/backend/http`: Axum routers, authentication context, DTO parsing, response envelopes, upload handling, and route facades.
- `src/backend/core`: domain contracts for money, dates, import decisions, matching, budgets, statistics, OCR/LLM, auth, backup, and governance.
- `src/backend/db`: SQLx PostgreSQL pool, schema scripts, repositories, user-scope helpers, import staging, settings bundle persistence, backup metadata, and vector outbox.
- `src/backend/parsers`: dedicated bill parsers and standard bill normalization.

## Request Lifecycle

1. Axum extracts the bearer token and user context.
2. Route handlers validate request DTOs and user-scope identifiers.
3. Domain logic calls PostgreSQL repositories through `HttpAppState`.
4. Import learning paths call Weaviate after deterministic rules.
5. Responses are serialized through the current API envelope.

## Runtime Invariants

- `BILL_ANALYSER_DATABASE_BACKEND` accepts only `postgres` / `postgresql`.
- `BILL_ANALYSER_POSTGRES_URL` must be configured and reachable.
- Weaviate must pass the ready probe for `/api/health` to be `ok`.
- Route handlers must not open alternate database runtimes.
- Backup file operations create, list, verify, download, delete, cleanup, schedule, and sync backup files. They do not rebuild application data.

## Import Pipeline

Upload routes parse files, create import sessions, stage normalized rows, run dedup and transfer materialization, apply category/account rules, request Weaviate recall when deterministic learning misses, render preview pages, persist manual edits, and confirm selected rows in PostgreSQL transactions.

## Verification Matrix

- Rust route ownership: `tests/backend/core/*governance*_contracts.rs` plus generated frontend route fixtures.
- Database/runtime: PostgreSQL repository tests and HTTP runtime contracts.
- Import: parser, staging, learning, LLM/OCR, preview, and confirm contracts.
- Frontend: service/model/component tests under `tests/web`.
