# Cyanflow C4 selection patch CAS

日期：2026-08-17

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把已有 `selection_hash` 接入显式 `selectionAction=patch`。后端 repository 在同一个 active-session 锁事务内读取完整已选 id 快照、校验可选集合 token、验证全部目标行的 user/session 归属，再原子应用 selected 与 deselected 两组更新。过期 token 返回 `409 PREVIEW_SELECTION_CONFLICT` 且零写入；两组任一写入失败时整批回滚；跨组重复 id 与 session 外 id 均失败关闭。

桌面跨页选择不再手写该 REST 请求，改由中立 services facade 发送当前 `selection_hash`。类型化冲突响应携带权威 metadata 和本次目标行快照，前端据此重基选择状态与 baseline，并终止当前 LLM/学习上层动作，不会用旧 token 自动重试。无 token 的旧调用仍可用，但复用同一个锁事务。

## 模块边界

- `patch_preview_selection` 位于 DB import staging repository，独占锁、快照 CAS、目标归属校验与双集合写事务；HTTP handler 只处理输入、错误码和响应投影。
- 集合并发只使用既有 `selection_hash`，不把行级 `row_version` 当集合 token，也不新增 `selection_version` 或数据库列。
- `importPreviewSelection.ts` 承担 endpoint、wire payload 与类型化冲突解析；`importPreview.ts` 继续作为兼容 facade 暴露能力。
- 冲突回读只返回当前用户、当前 session 内的请求目标行；回读失败按数据库错误失败关闭，不伪造空快照。
- 本切片只改变显式 patch。条件选择、recurring、decision group、reclassify、confirm、parser、dedup 与金额整数分合同保持不变。

## 原子性与兼容边界

- session parent lock 先于选择快照读取和任何行写入，和 confirm 的既有父锁顺序一致。
- expected hash 只接受 `fnv1a32:` 加八位十六进制；服务端归一为小写后与完整有序已选 id 快照比较。
- stale hash、非法 hash、跨组重复 id、session 外 id 都在首个 selection 写入前返回。
- selected 与 deselected 两组在一个 SQLx 事务内更新；第二组触发数据库错误时第一组也回滚。
- 无 hash 兼容调用仍有 user/session scope 和单事务保证，但不承诺并发冲突检测。

## TDD 与审计恢复

RED：真实 PostgreSQL 测试先建立选择快照，再模拟另一个 writer 改变集合，旧 handler 仍返回 200 并按过期视图修改选择；原子性测试通过第二组更新触发器证明旧实现的两次独立事务不能整体回滚。前端测试锁定旧页面直接 `fetch`，缺少类型化 token、409 解析和冲突重基。

GREEN：repository 单事务实现使 stale token 零写入并返回当前 hash；触发器失败后两行保持原状态，正常双集合 patch 一次提交；非法 hash、跨 session id、跨组重复与无 hash 兼容路径均有真实 PostgreSQL 回归。聚焦命令命中 3 条 Rust 测试，3 passed、0 failed、0 ignored；前端最终聚焦回归为 5 个 suite、84/84。

结构门禁第一次发现旧测试文件和 import preview service 超过新增文件上限。本切片将 CAS 集成测试拆到独立 runtime 分片，将 selection endpoint 拆到中立 service 子模块；生产 facade 与测试 include 保持稳定，随后前后端结构门禁通过。

## 完整审计

- 带真实 PostgreSQL 的 `cargo test --workspace` 通过；本切片 PostgreSQL 聚焦 discovery 命中 3 条且全部执行。
- 带真实 PostgreSQL 的 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过；Rust full-runtime 行覆盖率 `65.36%`。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust changed-line gate 匹配 4 个 coverage-eligible 业务文件、150 个可执行变更行，覆盖 145 行，即 `96.67%`；缺失 LCOV 文件为 0。
- 前端 `npm run lint:ci` 以 0 error 通过；141 条 warning 为现有基线。
- 前端完整 coverage 为 `308/308` suites、`41,482/41,482` tests，总行覆盖率 `94.23%`。
- 前端 changed-line coverage 为 `37/38`，即 `97.37%`；新增 selection service 为 `100%`，Vue 改动文件已匹配。
- Rust-only source-tree gate扫描最终 1,986 个 tracked path；Rust backend structure gate扫描 595 个文件、3 个 baseline entry；frontend structure gate扫描 617 个文件、39 个 baseline entry，全部通过。
- `git diff --check` 通过；本切片没有 schema 或金额转换。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-selection-patch-cas-2026-08-17.json`。
