# Cyanflow C4 reclassify row CAS

日期：2026-08-16

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把已有 preview `row_version` 接入桌面重新分类。前端只在 reclassify 的 `preview_updates` 中发送当前正整数 `expected_row_version`；后端把 token 写入已有 `ImportPreviewPatch`，并复用 repository 的批量事务逐行 CAS。任一输入行版本过期时，整批输入 patch 零写入回滚，HTTP 返回 `409 PREVIEW_ROW_VERSION_CONFLICT` 以及当前用户、当前 session 的冲突行最新完整快照。

桌面调用方不再为重新分类手写 `fetch`，而是复用 services facade、统一 Axios 认证/错误边界和类型化冲突解析器。冲突后直接以响应快照重基行版本、可编辑文本和信号状态，不追加 page/candidate reload。不携带 token 的旧客户端继续沿用既有 last-write-wins 输入 patch 行为。

## 模块边界

- `ImportPreviewPatchPayload` 位于中立 `models/import_preview.ts`，描述 API 可接受的局部 preview patch；页面构造器的 `ImportPreviewUpdatePayload` 在此基础上收紧为字段齐全的草稿写入合同。
- `reclassifyImportPreview` 位于共享 services facade，负责路径编码、`preview_updates` envelope 与 typed result；页面不再直接拼接该 REST 请求。
- 公共草稿构造器默认不发送行版本，只有重新分类显式开启 `includeExpectedRowVersion`。confirm、LLM 生成和 selection mutation 不会被隐式加入尚未执行的 token。
- 后端继续复用正整数 token parser、`DbError::PreviewVersionConflict` 和统一 conflict presenter；最新行回读必须同时满足 user/session scope。
- 两条 CAS 集成测试独立放在 `preview_reclassify_row_cas.rs`，避免扩大 mutation 测试分片；生产 handler 仍留在既有 mutation module。

## 原子性与兼容边界

- 输入 `preview_updates` 由 `update_preview_bills_batch` 在一个 SQLx 事务内处理；第二行冲突会回滚此前已执行的第一行 patch。
- 输入 patch 提交成功后，Stage 2 intelligence evaluate 和随后 intelligence patch 仍是后续阶段，不属于同一 CAS 事务。本切片不宣称整个 reclassify 调用具备端到端原子性。
- 空 `preview_updates` 保留整 session 重新分类语义；旧客户端不传 token 时保持兼容。
- PostgreSQL schema、金额整数分、parser、dedup、selection、直接 decision-group command 和 confirm 均未改变。

## TDD 与审计恢复

RED：真实 PostgreSQL 测试先构造两行 batch，在第二行被并发 writer 提升版本后提交旧 token；旧 handler 返回 200 且两行都被写入。前端测试随后锁定 reclassify service 路径/envelope、行 token 发线和类型化 409 快照重基。

GREEN：handler 解析每行 token 并接入已有批量 CAS，冲突后回读权威行；当前 token 成功返回更新后的更大 `row_version`，过期 token 返回 409 且第一行也保持原值。真实 PostgreSQL 聚焦命令命中 2 条测试，2 passed、0 failed、0 ignored；legacy 无 token 人工分类保护测试另行命中 1 条并通过。前端四个聚焦 suite 为 63/63。

第一次完整 Rust coverage 暴露 `weak_import_api_adapters_are_discoverable_and_cannot_grow` 失败：新增 service 参数使用 `Record<string, unknown>`，使 C4 弱类型计数超过上限。修复将局部 patch 合同下沉到中立模型，service 弱类型计数恢复为 20，治理合同单跑 1/1 通过，随后完整 workspace coverage 从头重跑并通过。

交付前按 CI 精确参数复现 changed-line gate 时，LLVM 将 async handler 新增行折叠到 await 状态机，导致 `--require-executable-lines` fail-closed。修复把单行更新与批量重新分类重复的可选 token 附加规则提取为同步 helper；helper 的 `Some`/`None` 合同先 RED 后 GREEN，完整 coverage 再次从头生成，精确 PR diff 最终命中 8 个可执行行并全部覆盖。

## 完整审计

- 带真实 PostgreSQL 的 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过；完整 Rust workspace 与数据库场景 0 failed。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust full-runtime 行覆盖率 `65.24%`。CI 同参数 changed-line gate 匹配 1 个业务文件、8 个可执行变更行，`8/8` 覆盖，即 `100%`；`--require-matched-files` 与 `--require-executable-lines` 均满足。
- 前端 `npm run lint:ci` 以 0 error 通过；141 条 warning 为现有基线。生产构建通过。
- 前端 `npm run test:coverage -- --silent` 通过：`308/308` suites、`41,480/41,480` tests，总行覆盖率 `94.23%`。
- 前端 changed-line coverage 为 `22/22`，即 `100%`；类型-only 中立模型不计可执行行。
- Rust-only source-tree gate 扫描 1,982 个 tracked path；Rust backend structure gate 扫描 594 个文件、3 个 baseline entry；frontend structure gate 扫描 616 个文件、39 个大文件 allowlist entry，全部通过。
- `git diff --check` 通过；本切片没有 schema 或金额转换。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-reclassify-row-cas-2026-08-16.json`。
