# 测试结构

当前测试基线分为 Rust 后端、前端和仓库治理三类。

## Rust 后端

- `cargo test --workspace` 覆盖 Rust workspace 的单元、集成和契约测试。
- `cargo clippy --workspace --all-targets -- -D warnings` 是共享 runtime 改动的静态门禁。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` 是业务代码最终覆盖率门禁。

## 前端

- `src/web` 下运行 `npm run lint` 或 CI 等价的 `npm run lint:ci`。
- 前端代码交付补 `npm run test:coverage` 和 `npm run build`。
- `tests/web/contracts/frontendRustRouteContract.test.ts` 校验由 Rust route ownership 生成的前端 fixture 未过期，并扫描 Vue/TS axios/fetch 调用，确保当前前端 `/api/...` 请求不依赖旧运行时边界或 `/api/v1/*` 路由。

## 仓库治理

- CI 的 `repo-governance` job 校验受追踪源码树保持 Rust-only。
- 调整启动、CI、agent 或 docs 入口时，先跑 `git ls-files *.py` 确认没有重新加入已删除源码类型。
- 改动 `.gitea/workflows/ci.yml` 时同步检查 Gitea Actions 语法与缓存体量。
