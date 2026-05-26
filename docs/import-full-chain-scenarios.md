# 导入全链路验收场景

本页记录导入主链路的可复跑验收场景。默认路径必须在 SQLite 和规则链路下可运行；PostgreSQL 与 Weaviate 是可选增强，不得成为导入预览、确认和 cleanup 的前置条件。

## 场景矩阵

| 场景 | 运行开关 | 覆盖链路 | 期望结果 |
| --- | --- | --- | --- |
| 默认 SQLite / 规则链路 | `BILL_ANALYSER_DATABASE_BACKEND` 未设置或为 `sqlite`；`BILL_ANALYSER_WEAVIATE_ENABLED=false` | parser-first multipart 上传、文件级 exactly-one parser 决策、standard rows、dedup、stage2 分类/账户、learning yellow、preview page signal filter、selected-only confirm、staging cleanup | 每个文件保留自己的 parser id / source signal；未被选中的预览行不会写入正式账单；confirm 后当前 session staging 被清理 |
| 历史改写/合并 | 默认 SQLite 或 PostgreSQL 权威库 | 历史重复、历史转账、`import_history_materializations`、preview 可见改写标记、acknowledgement payload、confirm 审计 | 预览行显式显示“将改写/合并历史账单”；confirm 校验 history bill id/version、operation id 与 acknowledgement token 后才改写历史账单、删除合并侧并同步账户余额 |
| PostgreSQL 权威库 | `BILL_ANALYSER_DATABASE_BACKEND=postgres`、`BILL_ANALYSER_POSTGRES_URL`、按 cutover 阶段设置 `BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER` | 迁移后 repository 边界、正式账单/反馈/审计事务、健康检查 | PostgreSQL 是业务主数据权威；SQLite 不作为静默回退；不支持的 repository 明确拒绝 |
| 可选 Weaviate 召回 | PostgreSQL 权威库基础上设置 `BILL_ANALYSER_WEAVIATE_ENABLED=true`、`BILL_ANALYSER_WEAVIATE_ENDPOINT`，可选 `BILL_ANALYSER_WEAVIATE_API_KEY` 和 `BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX` | deterministic stage2 之后的向量召回、metadata filter、derived index health、outbox/rebuild | Weaviate 只保存向量、特征副本和 metadata；召回建议仍写入本地 recommendation key / lifecycle；Weaviate 不可用时记录 degraded 并继续导入 |

## 本地合同测试

默认全链路场景由 `tests/backend/http/import_runtime_contract.rs` 的 multipart 合同测试覆盖：

```powershell
cargo test -p bill-analyser-http --test import_runtime_contract import_db_runtime_multipart_stage2_preview_confirm_covers_full_chain -- --nocapture
```

该测试使用支付宝与微信两个上传文件，覆盖：

- multipart 并发解析保持文件响应顺序，`alipay.csv` 命中 `alipay`，`wechat.csv` 命中 `wechat`。
- `import_sources` 和 `import_standard_rows` 保存文件级 parser signal、标准金额分单位和 parser tags。
- dedup stage2 生成 preview 后，支付宝咖啡账单命中分类规则和账户规则；微信超市账单命中 learning yellow 但不自动改写 preview 字段。
- preview page 支持按 `signal=parser` 过滤，并在 metadata counts 中暴露 learning 信号计数。
- confirm 只导入被选中的预览行，正式账单金额保持元单位，账户 id 与 stage2 结果一致，导入 session staging 被清理。

关联回归测试：

```powershell
cargo test -p bill-analyser-http --test import_runtime_contract import_db_runtime_parse_dedup_confirm_writes_import_chain -- --nocapture
cargo test -p bill-analyser-http --test import_runtime_contract import_db_runtime_parallel_parse_preserves_per_file_parser_identity -- --nocapture
```

提交前后端业务门禁仍以仓库总基线为准：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

## 运维说明

Weaviate 和 PostgreSQL 的本地运行入口为：

```powershell
docker compose -f docker-compose.postgres.yml up -d postgres weaviate
```

Weaviate schema bootstrap、outbox batch 和 rebuild 通过 `bill_weaviate_derived_index` CLI 执行。导入业务判断必须以 PostgreSQL 规则状态、反馈计数、lifecycle 和 suppression 为准；Weaviate metadata 只能用于召回过滤和重建派生索引。

历史改写确认的 UI/接口规则：preview 行需要带可见历史操作标记；confirm payload 需要包含服务端生成的 acknowledgement。用户确认导入即视为授权执行这些已标记的历史账单更新或合并操作。
