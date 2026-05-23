# 测试结构

当前测试基线分为 Rust 后端、前端和仓库治理三类。

按后端变更类型选择验证命令时，优先查看 [Rust 后端导航图的验证矩阵](backend-map.md#verification-matrix)。

## Rust 后端

- `cargo test --workspace` 覆盖 Rust workspace 的单元、集成和契约测试。
- Rust 集成/契约测试源码集中在 `tests/backend/core`、`tests/backend/db`、`tests/backend/http`、`tests/backend/parsers`，并通过各 crate `Cargo.toml` 的 `[[test]]` 目标纳入 workspace。
- `cargo clippy --workspace --all-targets -- -D warnings` 是共享 runtime 改动的静态门禁。
- `node scripts/check-rust-backend-structure.mjs` 是 Rust 后端结构体量门禁，按已追踪的 `src/backend/**/*.rs` 文件和 `scripts/rust-backend-structure-baseline.json` 做 ratchet-only 检查；生成或复核 baseline 使用 `node scripts/check-rust-backend-structure.mjs --print-baseline`。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` 是业务代码最终覆盖率门禁。
- `node scripts/check-backend-doc-map.mjs` 校验 `docs/backend-map.md` 与静态 HTML 入口的 canonical marker、锚点和关键路径索引。

## 前端

- `src/web` 下运行 `npm run lint` 或 CI 等价的 `npm run lint:ci`。
- 前端代码交付补 `npm run test:coverage` 和 `npm run build`。
- `tests/backend/core/migration_governance_contracts.rs` 校验前端 route ownership JSON 快照与 Rust `governance_manifest_snapshot().routes` 同步；`tests/web/contracts/frontendRustRouteContract.test.ts` 校验由该快照生成的前端 fixture 未过期，并扫描 Vue/TS axios/fetch 调用，确保当前前端 `/api/...` 请求不依赖旧运行时边界或 `/api/v1/*` 路由。

## 仓库治理

- CI 的 `repo-governance` job 校验受追踪源码树保持 Rust-only。
- Gitea Actions 的 push / pull_request path filter 覆盖 `src/backend/**`、`tests/backend/**`、`src/web/**`、`tests/web/**`、`tests/fixtures/**` 与治理/文档入口；`tests/backend/core/migration_governance_contracts.rs` 断言后端契约测试路径不会从 CI 过滤器中漂移。
- 调整启动、CI、agent 或 docs 入口时，先跑 `git ls-files *.py` 确认没有重新加入已删除源码类型。
- 改动 `.gitea/workflows/ci.yml` 时同步检查 Gitea Actions 语法与缓存体量。
