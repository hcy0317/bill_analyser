---
name: add-parser-standard-flow
description: Add or tighten a Bill Analyser dedicated parser in the Rust parser-first import runtime, with collision-proof detection, StandardBill-compatible output, focused Rust regression tests, parser tags design guardrails, and parser docs.
---

# Add Parser Standard Flow

Use this workflow when adding a new dedicated parser under `src/backend/parsers/`, or when tightening detection and parse behavior for an existing parser.

This is a Bill Analyser-specific workflow. It is not a generic CSV parsing tutorial.

## When to Use

Use this skill when you need to:

- add a new runtime parser under `src/backend/parsers/`
- wire Rust parser-first upload handling through `src/backend/http/import_routes/`
- harden `can_parse()` so a parser stops colliding with a neighboring parser
- bring parser output back to the `StandardBill` contract
- add parser-specific regression coverage before importing new statement formats

Do not use this skill for:

- generic import-column mapping fixes that do not add or change a dedicated parser
- unrelated REST/API contract work outside parser-first import
- DB schema expansion for parser metadata
- one-off debugging in local scratch files

## Read First

Before editing anything, read these files:

- `AGENTS.md`
- `docs/PROJECT_OVERVIEW.md`
- `src/backend/parsers/lib.rs`
- `tests/backend/parsers/parser_contracts.rs`
- `src/backend/http/import_routes/`
- `tests/backend/core/import_pipeline_contracts.rs`
- the closest existing parser module and its dedicated Rust regression coverage

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
5. Re-check Rust parser order and adjacent-parser negative guards.
6. Add regression tests before broadening sample coverage.
7. Update parser docs only after runtime and tests agree.

## Detection Collisions and Factory Priority

Parser work is not finished when a positive sample parses successfully.

Minimum collision checks:

- the target sample is accepted by the intended parser
- the closest neighboring parser rejects that sample
- Rust `parse_dedicated_import_bytes()` resolves to the intended `PARSER_ID`
- changing parser registry order is justified and documented

If a parser becomes broader, add a negative test against the nearest parser family before merging.

## StandardBill Output and Import-Preview Contract

All dedicated parsers must stay compatible with Rust `StandardBill` in `src/backend/parsers/lib.rs`.

Important expectations:

- `date` normalizes to `YYYY-MM-DD HH:MM:SS`
- expense amounts are negative and income amounts are positive
- `type` maps through the existing transaction families
- `description` is rich enough for categorization and preview review
- `source_account_id` matches the parser `PARSER_ID`
- output stays aligned with parser-first import runtime contract tests

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

- `tests/backend/parsers/parser_contracts.rs`
- `tests/backend/core/import_pipeline_contracts.rs`
- optional fixture support in `tests/fixtures/import_samples/`

Do not stop at "parser can parse one file".

## Docs and Follow-up Updates

Required updates for parser workflow changes:

- `docs/parsers/add-parser-standard-flow.md`

Update `docs/PROJECT_OVERVIEW.md` only when a stable runtime fact changes, such as a new supported parser, stable detection-order change, or import-preview/parser alignment change.

## Verification

Minimum parser-focused verification:

- `cargo test -p bill-analyser-parsers`
- `cargo test -p bill-analyser-core --test import_pipeline_contracts`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`

## Anti-patterns

Avoid these mistakes:

- adding a parser with only positive tests
- broadening `can_parse()` without nearest-parser negative coverage
- writing runtime parser files into shadow directories like `src/parsers/`
- inventing parser output fields that drift away from `StandardBill`
- mixing parser-tags runtime schema, API changes, or DB changes into the same slice
