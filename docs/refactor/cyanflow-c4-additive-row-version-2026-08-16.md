# Cyanflow C4 additive preview row version

日期：2026-08-16

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把 PostgreSQL 已有的 `import_preview_rows.version` 接入统一预览行合同：Rust repository DTO 映射该列，共享 row presenter 以 additive `row_version` 序列化，TypeScript 中立 preview 模型和 row-to-draft adapter 继续携带该 token。

这不是 CAS 切片。普通 preview update 尚未接受 `expected_row_version`，SQL 也尚未使用 `WHERE version = expected`。当前客户端不带版本时行为保持不变，旧服务响应缺少 `row_version` 时前端仍可读取。

## 模块边界

- PostgreSQL schema 不变；`version BIGINT NOT NULL DEFAULT 1` 仍是单行版本权威。
- `ImportPreviewRow` 是 repository 到 HTTP presenter 的 typed row 合同，JSON 字段固定为 `row_version`。
- 分页、mutation refresh、decision refresh 与 matching candidate refresh 复用现有 presenter，不增加第二套 JSON 拼接逻辑。
- `src/web/src/models/import_preview.ts` 持有中立 row/page DTO；原 view-local import path 通过 type re-export 保持兼容。
- row-to-draft adapter 把 token 保存为 `_rowVersion`，为下一切片提交 `expected_row_version` 做准备。
- 轻量 filter index 不承载行 mutation，因此本切片不向 filter index 复制 `row_version`。

## 兼容与不变量

- `row_version` 在 Rust 当前响应中总是整数；TypeScript 声明为 optional，只为兼容迁移窗口中的旧响应。
- 金额继续使用 `*_amount_cents` 整数分，本切片没有新增元/分转换。
- `selection_hash` 仍只保护 selected preview id 集合，不能替代 row version。
- session version、decision group version 与 history bill version 的职责不变。
- confirm、receipt replay、history rewrite、parser、dedup、Stage 2 与信号状态机语义不变。

## TDD 证据

RED：先让 Rust presenter 测试要求 `row_version`，编译因 `ImportPreviewRow` 结构体尚无 `version` 字段而失败；前端测试随后要求 `row_version -> _rowVersion`，类型检查因 row DTO 与 draft 均无对应字段而失败。

GREEN：repository mapper、共享 presenter、中立 TypeScript 模型、service response 类型与 draft adapter 完成最小实现；真实 PostgreSQL 测试证明新插入行版本为 1、patch 后为 2。

已通过的局部命令：

```powershell
cargo test -p bill-analyser-http canonical_preview_response_includes_the_typed_state_snapshot --lib
cargo test -p bill-analyser-http --test import_runtime_contract
$env:BILL_ANALYSER_TEST_POSTGRES_URL='postgresql://bill_analyser:bill_analyser_dev@127.0.0.1:54321/bill_analyser'
cargo test -p bill-analyser-db --test import_staging import_preview_runtime_facets_identity_validation_and_confirm_are_db_backed -- --exact --nocapture
Set-Location src\web
npm test -- ../../tests/web/views/desktop/transactions/import/importPreviewTransaction.test.ts
npm test -- ../../tests/web/coverage-wave10-import-business/import-preview-services.behavior.test.ts
npx vue-tsc --noEmit
```

局部结果：Rust presenter `1 passed`；Rust contract `10 passed`；真实 PostgreSQL `1 passed / 0 ignored`；前端 row adapter `12 passed`；前端 service `11 passed`；Vue typecheck 通过。

## 完整审计

- `cargo fmt --all -- --check` 通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 带真实 PostgreSQL 的 `cargo test --workspace` 通过；数据库合同没有静默 skip。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过；workspace 行覆盖率 `65.07%`。
- Rust changed-line coverage 为 `11/11`，即 `100%`。
- `npm run lint` 以 0 error 通过；141 条 `no-explicit-any` warning 为仓库既有 warning。
- `npm run test:coverage` 通过：`308/308` suites、`41,468/41,468` tests，前端总行覆盖率 `94.22%`。
- 前端 changed-line coverage 为 `5/5`，即 `100%`，包含 mobile preview reload 与 desktop row-to-draft token 传播。
- Rust-only source-tree gate 通过，扫描 1,971 个 tracked path；frontend structure gate 扫描 616 个文件通过。

首次执行全量 Rust 测试时，未设置 PostgreSQL URL 的运行按合同 fail-closed；设置隔离数据库后又暴露 3 个手工 projector `SELECT` 未包含 `version`。测试查询随后改为显式投影并断言版本 3/4/5，mapper 仍对缺列失败，不使用默认值掩盖 schema/query 漂移。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-additive-row-version-2026-08-16.json`。
