# 测试结构

当前测试基线分为 Rust 后端、前端和仓库治理三类。

## Rust 后端

- `cargo test --workspace` 覆盖 Rust workspace 的单元、集成和契约测试。
- Rust 集成/契约测试源码集中在 `tests/backend/core`、`tests/backend/db`、`tests/backend/http`、`tests/backend/parsers`，并通过各 crate `Cargo.toml` 的 `[[test]]` 目标纳入 workspace。
- 导入全链路验收场景见 [导入全链路验收场景](import-full-chain-scenarios.md)。
- `cargo clippy --workspace --all-targets -- -D warnings` 是共享 runtime 改动的静态门禁。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` 是业务代码最终覆盖率门禁。
- `node scripts/check-backend-doc-map.mjs` 校验 `docs/backend-map.md` 与静态 HTML 入口的关键路径索引。

## 前端

- `src/web` 下运行 `npm run lint` 或 CI 等价的 `npm run lint:ci`。
- 前端代码交付补 `npm run test:coverage` 和 `npm run build`。
- `tests/web/contracts/frontendRustRouteContract.test.ts` 校验生成的 Rust route fixture 未过期，并扫描 Vue/TS axios/fetch 调用，确保当前前端 `/api/...` 请求全部落到当前 Rust-owned route。

## 仓库治理

- CI 的 `repo-governance` job 校验受追踪源码树保持 Rust-only。
- Gitea Actions 的 push / pull_request path filter 覆盖 `src/backend/**`、`tests/backend/**`、`src/web/**`、`tests/web/**`、`tests/fixtures/**` 与治理/文档入口。
- 调整启动、CI、agent 或 docs 入口时，先跑 `git ls-files *.py` 确认没有重新加入已删除源码类型。
- 改动 `.gitea/workflows/ci.yml` 时同步检查 Gitea Actions 语法与缓存体量。
