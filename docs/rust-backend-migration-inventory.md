# Rust Backend Migration Inventory

The migration inventory is complete. Runtime source now lives under the Rust workspace:

- `src/backend/http`
- `src/backend/core`
- `src/backend/db`
- `src/backend/parsers`

Frontend source remains under `src/web`, with contract tests under `tests/web`.

Use `cargo metadata --locked --format-version 1` for workspace dependency inventory and `cargo test --workspace` for current contract coverage.
