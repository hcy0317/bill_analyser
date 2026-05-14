---
name: add-parser-standard-flow
description: Add or tighten a Bill Analyser dedicated parser in the Rust parser-first import runtime, with migration-period Python parity checks, collision-proof detection, StandardBill-compatible output, focused regression tests, parser tags design guardrails, and parser docs.
---

# Add Parser Standard Flow

Use this workflow when adding a new dedicated parser under `crates/bill-analyser-parsers/`, or when tightening detection and parse behavior for an existing parser. Python parser files under `src/bill_analyser/parsers/` are migration-period parity and comparison surfaces, not the parser-first upload runtime.

This is a Bill Analyser-specific workflow. It is not a generic CSV parsing tutorial.

## When to Use

Use this skill when you need to:

- add a new runtime parser under `crates/bill-analyser-parsers/src/`
- wire Rust parser-first upload handling through `crates/bill-analyser-http/src/import_routes.rs`
- keep migration-period `ParserFactory` and `PARSER_CLASS_REGISTRY` parity safe when Python sidecar coverage is also touched
- harden `can_parse()` so a parser stops colliding with a neighboring parser
- bring parser output back to the `StandardBill` contract
- add parser-specific regression coverage before importing new statement formats

Do not use this skill for:

- generic import-column mapping fixes that do not add or change a dedicated parser
- REST/API contract work in `src/bill_analyser/api/routes/**`
- DB schema expansion for parser metadata
- one-off debugging in local scratch files

## Read First

Before editing anything, read these files:

- `AGENTS.md`
- `docs/PROJECT_OVERVIEW.md`
- `crates/bill-analyser-parsers/src/lib.rs`
- `crates/bill-analyser-parsers/tests/parser_contracts.rs`
- `crates/bill-analyser-http/src/import_routes.rs`
- `crates/bill-analyser-http/tests/import_runtime_contract.rs`
- `src/bill_analyser/parsers/base.py`
- `src/bill_analyser/parsers/factory.py`
- `tests/test_parser_base_factory.py`
- `tests/new_ui/test_import_parser_alignment.py`
- the closest existing parser module and its dedicated regression tests, such as `tests/test_abc_parser.py`

## Pre-flight Scope

Confirm these inputs before implementation:

1. Sample boundary: which bank, wallet, export format, and real file extensions are in scope.
2. Collision set: which existing parser is most likely to mis-detect the same sample.
3. Fixture plan: prefer stable fixtures or builders under `tests/fixtures/import_samples/` and `tests/parser_test_support.py`.
4. Contract plan: keep parser runtime and parser regressions separate from REST/API/DB changes unless explicitly approved.

## Standard Implementation Order

1. Pick the closest existing Rust parser as the comparison baseline.
2. Implement or tighten Rust detection first, including negative guards for adjacent parsers.
3. Implement or tighten Rust parsing second, then route rows through `post_process_raw_bills()`.
4. Wire parser-first upload handling only when the HTTP import path needs a new entry point or behavior.
5. Keep Python `can_parse()` / `parse()` / `PARSER_CLASS_REGISTRY` parity only when sidecar behavior or comparison tests are part of the slice.
6. Re-check Rust parser order and, if touched, Python `ParserFactory` order.
7. Add regression tests before broadening sample coverage.
8. Update parser docs only after runtime and tests agree.

## Detection Collisions and Factory Priority

Parser work is not finished when a positive sample parses successfully.

Minimum collision checks:

- the target sample is accepted by the intended parser
- the closest neighboring parser rejects that sample
- Rust `parse_dedicated_import_bytes()` resolves to the intended `PARSER_ID`
- migration-period `ParserFactory.detect_parser()` still resolves to the intended `PARSER_ID` when Python parity is in scope
- changing parser order inside `PARSER_CLASS_REGISTRY` is justified and documented

If a parser becomes broader, add a negative test against the nearest parser family before merging.

## StandardBill Output and Import-Preview Contract

All dedicated parsers must stay compatible with `StandardBill` in `src/bill_analyser/parsers/base.py`.

Important expectations:

- `date` normalizes to `YYYY-MM-DD HH:MM:SS`
- expense amounts are negative and income amounts are positive
- `type` maps through the existing transaction families
- `description` is rich enough for categorization and preview review
- `source_account_id` matches the parser `PARSER_ID`
- output keeps working with `tests/test_parser_base_factory.py`
- output stays aligned with `tests/new_ui/test_import_parser_alignment.py`

## Parser Tags Design Guardrails

The roadmap includes parser tags, but this workflow does not add REST fields, DB columns, or preview payload fields for parser tags.

Use controlled prefixes only in docs or tests:

- `parser:<parser_id>`
- `record_origin:<wallet_statement|bank_statement>`
- `channel:<wallet|bank_card|credit_card>`
- `txn_family:<expense|income|transfer|investment|refund>`
- `institution:<wechat|alipay|icbc|cmbc|abc|ccb>`

## Required Regressions

At minimum, update or add the most relevant tests from this repository set:

- `crates/bill-analyser-parsers/tests/parser_contracts.rs`
- `crates/bill-analyser-http/tests/import_runtime_contract.rs`
- `tests/test_parser_base_factory.py`
- dedicated parser regression, such as `tests/test_abc_parser.py`
- `tests/new_ui/test_import_parser_alignment.py`
- optional fixture support in `tests/parser_test_support.py` or `tests/fixtures/import_samples/`

Do not stop at "parser can parse one file".

## Docs and Follow-up Updates

Required updates for parser workflow changes:

- `docs/parsers/add-parser-standard-flow.md`

Update `docs/PROJECT_OVERVIEW.md` only when a stable runtime fact changes, such as a new supported parser, stable detection-order change, or import-preview/parser alignment change.

## Verification

Minimum parser-focused verification:

- `cargo test -p bill-analyser-parsers`
- `cargo test -p bill-analyser-http --test import_runtime_contract`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- `./.venv/Scripts/python.exe -m pytest tests/test_parser_base_factory.py -v`
- `./.venv/Scripts/python.exe -m pytest tests/test_<parser>.py -v`
- `./.venv/Scripts/python.exe -m pytest tests/new_ui/test_import_parser_alignment.py -v`

If runtime parser behavior under `src/bill_analyser/**` changed, final acceptance still requires:

- `./.venv/Scripts/python.exe -m pytest tests/ -v`

## Anti-patterns

Avoid these mistakes:

- adding a parser with only positive tests
- broadening `can_parse()` without nearest-parser negative coverage
- bypassing `ParserFactory` or `PARSER_CLASS_REGISTRY`
- writing runtime parser files into shadow directories like `src/parsers/`
- inventing parser output fields that drift away from `StandardBill`
- mixing parser-tags runtime schema, API changes, or DB changes into the same slice
