# Runtime Audit Archive

This audit is archived. The active runtime has completed the Rust-only cutover, and tracked backend source now lives under `src/backend/`.

Current audit checks:

```powershell
git ls-files *.py
cargo fmt --all -- --check
cargo test --workspace
```
