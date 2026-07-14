# 测试结构

当前测试基线分为 Rust 后端、前端和仓库治理三类。

## Rust 后端

- `cargo test --workspace` 覆盖 Rust workspace 的单元、集成和契约测试。
- Rust 集成/契约测试源码集中在 `tests/backend/core`、`tests/backend/db`、`tests/backend/http`、`tests/backend/parsers`，并通过各 crate `Cargo.toml` 的 `[[test]]` 目标纳入 workspace。
- 导入全链路验收场景见 [导入全链路验收场景](import-full-chain-scenarios.md)。
- `cargo clippy --workspace --all-targets -- -D warnings` 是共享 runtime 改动的静态门禁。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 是 Rust full-runtime baseline 门禁；它必须覆盖完整 Rust 工作区，不能通过 coverage cfg 隐藏运行时代码。
- 仓库规则要求被改业务代码的可执行行覆盖率严格高于 90%，但当前 Gitea CI 没有把 PR diff 与 LCOV 输入 changed-line 计算器作为验收 gate；CI 中运行的是 governance normalizer 自测试和 Rust workspace 35% 门禁。
- `node scripts/check-backend-doc-map.mjs` 校验 `docs/backend-map.md` 与静态 HTML 入口的关键路径索引。

## 前端

- `src/web` 下运行 `npm run lint` 或 CI 等价的 `npm run lint:ci`。
- `npm run test:coverage` 先运行全量 Jest 测试，再启动独立 coverage invocation。coverage invocation 的测试清单和 `collectCoverageFrom` 由 `jest.config.ts` 限定在导入、预算等选定模块，91% global threshold 只作用于该 curated denominator，不代表整个前端源码的覆盖率。
- 前端代码交付补 `npm run test:coverage` 和 `npm run build`。
- `tests/web/contracts/frontendRustRouteContract.test.ts` 校验生成的 Rust route fixture 未过期，并扫描 Vue/TS axios/fetch 调用，确保当前前端 `/api/...` 请求全部落到当前 Rust-owned route。
- 浏览器人工实测矩阵见 [浏览器功能实测矩阵](browser-functional-test-matrix.md)，覆盖桌面端、移动端、登录/健康检查/导航冒烟、账单导入预览确认、主数据 CRUD、预算、统计、设置、备份和外部能力边界。
- 核心浏览器自动化见 [浏览器功能自动化](browser-functional-test-automation.md)，采用“专用测试账号 + fixture 导入 + 开始/结束清理”的模式，优先覆盖登录、导入预览确认、账单列表与统计联动，并在桌面端和移动端分别运行。
- `src/web` 下存在 Playwright desktop/mobile projects 与 specs，通过 `npm run e2e` 运行；无后端时可先跑 `npm run e2e -- --list` 验证配置和测试发现，完整运行要求本地 Rust/PostgreSQL/Weaviate health 为 `ok`。当前 `.gitea/workflows/ci.yml` 的前端 job 运行 lint、Jest coverage 和 build，不运行 Playwright。

## 仓库治理

- CI 的 `repo-governance` job 校验受追踪源码树保持 Rust-only。
- Gitea Actions 的 push / pull_request path filter 覆盖 `src/backend/**`、`tests/backend/**`、`src/web/**`、`tests/web/**`、`tests/fixtures/**` 与治理/文档入口。
- Rust 结构 ratchet 由 `scripts/check-rust-backend-structure.mjs` 提供，前端结构 ratchet 由 `npm run structure:check` 提供；两者当前都不在 Gitea CI job 中。
- PR closeout、changed-line coverage 与结构门禁 backlog 证据格式见 [CI 证据与覆盖率归一化](governance-ci-coverage.md)。
- 调整启动、CI、agent 或 docs 入口时，先跑 `git ls-files *.py` 确认没有重新加入已删除源码类型。
- 改动 `.gitea/workflows/ci.yml` 时同步检查 Gitea Actions 语法与缓存体量。
