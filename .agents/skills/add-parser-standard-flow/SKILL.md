---
name: add-parser-standard-flow
description: Add a Bill Analyser dedicated parser under src/bill_analyser/parsers with ParserFactory wiring, collision-proof detection, StandardBill-compatible output, focused regression tests, parser tags design guardrails, and parser docs.
---

# Add Parser Standard Flow

Use this workflow when adding a new dedicated parser under `src/bill_analyser/parsers/`, or when tightening detection and parse behavior for an existing parser.

This is a Bill Analyser-specific workflow. It is not a generic CSV parsing tutorial.

## When to Use

Use this skill when you need to:

- add a new runtime parser under `src/bill_analyser/parsers/*.py`
- extend `ParserFactory` and `PARSER_CLASS_REGISTRY` safely
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

1. Pick the closest existing parser as the comparison baseline.
2. Implement or tighten `can_parse()` first.
3. Implement or tighten `parse()` second.
4. Route parsed rows through `ParserBase` helpers and `post_process()` when practical.
5. Register the parser in `PARSER_CLASS_REGISTRY`.
6. Re-check `ParserFactory` detection order after registration.
7. Add regression tests before broadening sample coverage.
8. Update parser docs only after runtime and tests agree.

## Detection Collisions and Factory Priority

Parser work is not finished when a positive sample parses successfully.

Minimum collision checks:

- the target sample is accepted by the intended parser
- the closest neighboring parser rejects that sample
- `ParserFactory.detect_parser()` resolves to the intended `PARSER_ID`
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
