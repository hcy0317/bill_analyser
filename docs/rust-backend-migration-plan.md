# Rust Backend Migration Plan

This document is now an archived migration note. The active runtime has reached the Rust-only cutover:

- `bill_http_server` is the only HTTP runtime entry.
- Business routes are owned by Rust route modules and Rust repositories.
- Startup, CI, and local verification use Rust/frontend gates only.
- Unknown `/api/...` requests receive a Rust-side structured 404.

Current backend verification:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

Current frontend verification:

```powershell
Set-Location src\web
npm run lint
npm run test:coverage
npm run build
```
