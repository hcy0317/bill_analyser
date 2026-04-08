---
name: add-parser-standard-flow
description: Add a Bill Analyser dedicated parser under src/bill_analyser/parsers with ParserFactory wiring, collision-proof detection, StandardBill-compatible output, focused regression tests, parser tags design guardrails, and parser docs.
---

# Add Parser Standard Flow

Use this workflow when adding a new dedicated parser under `src/bill_analyser/parsers/`, or when tightening the detection and parse behavior of an existing parser.

This is a Bill Analyser-specific workflow. It is **not** a generic CSV parsing tutorial.

## When to Use

Use this skill when you need to:

- add a new runtime parser under `src/bill_analyser/parsers/*.py`
- extend `ParserFactory` and `PARSER_CLASS_REGISTRY` safely
- harden `can_parse()` so a parser stops colliding with a neighboring parser
- bring a parser output back to the `StandardBill` contract
- add parser-specific regression coverage before importing new statement formats

Do **not** use this skill for:

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

1. **Sample boundary**
   - Which bank / wallet / export format is in scope?
   - Which file extensions are real inputs?
   - Are there both CSV and Excel variants?

2. **Collision set**
   - Which existing parser is most likely to mis-detect the same sample?
   - Will this change require reordering `PARSER_CLASS_REGISTRY` in `src/bill_analyser/parsers/factory.py`?

3. **Fixture plan**
   - Prefer stable fixtures or builders under `tests/fixtures/import_samples/` and `tests/parser_test_support.py`
   - Do not rely only on ad-hoc local files under `bills/`

4. **Contract plan**
   - Keep the work inside parser runtime and parser regressions unless there is an explicitly approved follow-up slice
   - Avoid piggyback API or database contract changes

## Standard Implementation Order

1. Pick the closest existing parser as the comparison baseline.
2. Implement or tighten `can_parse()` first.
3. Implement or tighten `parse()` second.
4. Route every parsed row through `ParserBase` helpers and `post_process()` when practical.
5. Register the parser in `PARSER_CLASS_REGISTRY`.
6. Re-check `ParserFactory` detection order after registration.
7. Add regression tests before broadening sample coverage.
8. Update parser docs only after runtime and tests agree.

## Detection Collisions and Factory Priority

In Bill Analyser, parser work is not finished when a positive sample parses successfully.

You must also prove the parser does **not** incorrectly absorb neighboring formats.

Minimum collision checks:

- the target sample is accepted by the intended parser
- the closest neighboring parser rejects that sample
- `ParserFactory.detect_parser()` resolves to the intended `PARSER_ID`
- changing parser order inside `PARSER_CLASS_REGISTRY` is justified and documented in the PR/session summary

If a parser becomes broader, add a negative test against the nearest parser family before merging.

## StandardBill Output and Import-Preview Contract

All dedicated parsers must stay compatible with `StandardBill` in `src/bill_analyser/parsers/base.py`.

Important expectations:

- `date` must normalize to `YYYY-MM-DD HH:MM:SS`
- `amount` must use Bill Analyser sign rules
  - expense -> negative
  - income -> positive
- `type` should map through the existing normalized transaction families
- `description` should be rich enough to support categorization and preview review
- `source_account_id` should match the parser `PARSER_ID`
- parser output must keep working with `tests/test_parser_base_factory.py`
- parser output must stay aligned with import preview checks in `tests/new_ui/test_import_parser_alignment.py`

## Parser Tags Design Guardrails

The roadmap calls for `parser tags`, but the repository does **not** yet expose a runtime parser-tags schema in REST or DB.

For now, treat parser tags as a **design contract** for future matching / learning work.

Use controlled prefixes when documenting or testing candidate tags:

- `parser:<parser_id>`
- `record_origin:<wallet_statement|bank_statement>`
- `channel:<wallet|bank_card|credit_card>`
- `txn_family:<expense|income|transfer|investment|refund>`
- `institution:<wechat|alipay|icbc|cmbc|abc|ccb>`

Rules:

- keep the vocabulary controlled and reviewable
- document tag intent in parser docs or regression comments first
- do **not** add new REST fields, DB columns, or preview payload fields for parser tags in this workflow
- if runtime parser tags are needed, split that into a separate approved slice

## Required Regressions

At minimum, update or add the most relevant tests from this repository set:

- `tests/test_parser_base_factory.py`
  - `StandardBill` contract
  - parser helper behavior
  - `ParserFactory` detection and registry behavior
- dedicated parser regression, such as `tests/test_abc_parser.py`
  - positive recognition
  - nearest-parser negative cases
- `tests/new_ui/test_import_parser_alignment.py`
  - dedicated parser vs generic import alignment when the parser participates in import preview
- optional real-sample or fixture support updates
  - `tests/parser_test_support.py`
  - `tests/fixtures/import_samples/`
  - `tests/new_ui/test_original_local_bill_samples.py` only when the repository already treats the sample family as stable evidence

Do not stop at “parser can parse one file”.

## Docs and Follow-up Updates

Keep documentation additive and proportional.

Required updates for parser workflow changes:

- `docs/parsers/add-parser-standard-flow.md`

Update `docs/PROJECT_OVERVIEW.md` only when a stable runtime fact changes, such as:

- a new supported parser is officially added
- parser detection order changes in a lasting way
- import preview/parser alignment behavior changes as a stable system fact

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
- sneaking parser-tags runtime schema, API changes, or DB changes into the same slice
- relying only on local machine statement files instead of reproducible fixtures or builders
