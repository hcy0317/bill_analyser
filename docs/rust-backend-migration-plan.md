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
- Current Rust backend files: 43
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

S3 added `crates/bill-analyser-db` as the Rust DB runtime foundation for schema/path checks and database guard logic. It is not Rust-primary for business writes: the Flask/Python Database façade remains the primary runtime write path until later domain slices prove and switch a specific business surface.

## S7 Parser Import Contracts

S7 starts the import parser domain in Rust without switching parser runtime ownership. It preserves the Python parser detection order (`wechat -> alipay -> icbc -> cmbc -> abc -> ccb`), parser metadata, source labels, parser tag normalization, generic raw-bill post-processing, and the JSON `StandardBill` key shape used by the existing Python import staging path.

| Python source | Rust source | Boundary |
| --- | --- | --- |
| `src/bill_analyser/parsers/factory.py` | `crates/bill-analyser-core/src/parsers.rs` | Parser registry order and metadata contract |
| `src/bill_analyser/import_contracts/parser_tags.py` | `crates/bill-analyser-core/src/parsers.rs` | Parser/channel tag normalization and serialization |
| `src/bill_analyser/parsers/base.py` | `crates/bill-analyser-core/src/parsers.rs` | Raw parser output post-processing into StandardBill-shaped data |
| `tests/fixtures/import_samples/**` parser families | `crates/bill-analyser-core/tests/fixtures/parser_golden_contracts.json` | Deterministic golden contract cases for six parser families |

No Python parser implementation is removed in S7. The Rust parser module is an internal contract/oracle surface: Python parser tests continue to parse real CSV/XLS/XLSX samples, while Rust golden contract tests pin the normalized StandardBill, parser tag, and source-label output expected from each dedicated parser family.
