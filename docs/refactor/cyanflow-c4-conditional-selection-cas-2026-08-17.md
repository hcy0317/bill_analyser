# Cyanflow C4 conditional selection CAS

日期：2026-08-17

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把条件选择所携带的行草稿纳入 `selection_hash` 与 `row_version` 组合并发控制。后端在同一个 active-session 锁事务内先校验完整选择集合，再逐行 CAS 草稿，最后执行 query selection；任一 token 过期都会整体零写入。纯 selection writer 不再推进业务行版本，避免选择操作制造虚假的行冲突。

桌面跨页选择改用 `importPreviewSelection.ts` 的 typed service。请求携带当前 `selection_hash` 与每条可用的 `expected_row_version`；成功后以完整权威行回执重基。集合冲突重基 metadata 与全部目标行，行冲突只重基冲突行并保留同批其他未写草稿；两类冲突都终止当前动作，不自动重试。

## 模块边界

- DB import staging repository 负责 session lock、集合快照校验、行 CAS、query selection 与事务原子性；HTTP handler 只解析 token、映射 typed 409 并投影完整行回执。
- `selection_hash` 只表达集合成员关系，`row_version` 只表达单行业务字段版本；纯 selection 不推进 `row_version`，业务草稿 patch 成功后只递增一次。
- `importPreviewSelection.ts` 持有 selection endpoint、wire payload 与 typed response；页面只负责乐观 UI、权威快照合并和草稿协调。
- 本切片不修改数据库 schema、金额单位、分类/学习/LLM 业务语义、direct decision-group 或 confirm 合同。

## TDD 与并发合同

RED 先证明四个缺口：过期集合 hash 仍会写草稿；过期行版本仍会执行选择；纯 selection writer 会推进 `row_version`；成功响应缺少权威 `previewItems`。前端 RED 还证明 typed service 尚不存在、页面仍走手写 fetch，以及行冲突会误删同批其他草稿。

GREEN 后，四条真实 PostgreSQL 场景全部通过：stale selection 在首个 patch 前失败；stale row version 不产生 selection 写入；四类纯 selection writer 都保持行版本；组合成功只递增一次并返回完整权威行。相关 Rust selection 过滤集 11/11 通过，条件选择聚焦集 4/4 通过；前端受影响矩阵 71/71 通过。

## 完整审计

- 带真实 PostgreSQL 的 `cargo test --workspace` 通过，真实数据库用例没有 ignored 或静默 skip。
- 带真实 PostgreSQL 的 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过；Rust full-runtime 行覆盖率 `65.54%`。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust changed-line gate 匹配 47 个可执行变更行并全部覆盖，覆盖率 `100%`；缺失 LCOV 文件为 0。
- 前端 `npm run lint:ci` 以 0 error 通过，141 条 warning 为现有基线；`npm run build` 通过。
- 前端完整 coverage 为 `308/308` suites、`41,487/41,487` tests，总行覆盖率 `94.23%`、分支 `91.21%`、函数 `91.99%`。
- 前端 changed-line coverage 为 `33/35`，即 `94.29%`；所有业务文件均被覆盖率报告匹配。
- Rust-only source-tree、Rust backend structure、frontend structure 与 backend doc map gate 均通过；frontend 只有既有文件行数减少提示。
- `git diff --check` 通过；本切片没有 schema 或金额转换。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-conditional-selection-cas-2026-08-17.json`。
