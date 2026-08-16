# Bill Analyser C3 Stage 2 learning 批量读取

> 状态：`SLICE_PASS / C3_EXIT_PENDING_VECTOR_RECALL_BATCH_READ`
> 日期：2026-08-16
> 工作流：`Cyaness -> Cyanflow -> TDD`
> 基线 main：`244c30f952f82426b76936375151c739aa10f5cf`

## 1. 交付结论

`ImportStage2::evaluate` 的 deterministic learning 路径不再按 preview row 调用 lifecycle point read。应用层先完成分类和 recurring 投影、准备同批 recommendation key 并去重，DB repository 再用一次 user-scoped `ANY($2)` 查询读取 lifecycle view，随后按原顺序应用 suppressed、pending 与 auto-apply 语义，最后执行账户规则和 baseline 投影。

本切片没有改变 REST API、PostgreSQL schema、HTTP DTO、金额单位、前端模型或 confirm 协议。`stage_vector_recall/results.rs` 仍保留一处 `get_import_learning_lifecycle_view` 点查，属于下一独立切片；因此这里只关闭 deterministic Stage 2 子切片，不宣称 C3 总阶段完成。

## 2. TDD 与行为合同

RED 先由真实 PostgreSQL repository 测试证明批量 API 尚不存在，再由架构测试证明应用边界没有批量调用。GREEN 后固定以下合同：

- 空 key 集直接返回空列表，不访问 SQL。
- recommendation key 先去重并保持确定性顺序。
- 查询必须带当前 `user_id`，跨用户相同 key 不得泄漏。
- accepted、pending、suppressed、缺失 key 与重复 key 均保持既有行为。
- HTTP Stage 2 生产路径只调用一次批量 repository API，逐行 chain 和 learning rule 文件不得调用 lifecycle point read。
- 分类、recurring、learning、账户规则和 baseline 的有效语义顺序保持不变。

## 3. 验证证据

| 门禁 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `node scripts/check-rust-backend-structure.mjs` | 593 files，3 baseline entries，pass |
| Stage 2 architecture focused tests | 3 passed，0 failed，0 ignored |
| Stage 2 real PostgreSQL repository tests | 2 passed，0 failed，0 ignored |
| HTTP library tests | 205 passed，0 failed，0 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass，真实 PostgreSQL 场景未 skip |
| Rust workspace line coverage | 64.78%，门槛 35% |
| Rust changed-line coverage | 136/138，98.55%，门槛严格大于 90% |

## 4. 固定 64 文件画像

最终 worktree run `c3-stage2-learning-batch-worktree-20260816-r2` 通过：64 文件、15,041 parsed、13,082 after dedup、0 unmatched；Stage 1 为 4,313ms，Stage 2 为 8,165ms，首屏可操作为 13,025ms。canonical parser filter request 为 1，legacy filter-index request 为 0；业务数据、Weaviate collection 和隔离数据库均完成严格清理。

与 exact-main 四次基线相比，识别数量和信号请求路径 parity=0，Stage 1/2 继续低于 12s/20s 的互斥预算，没有证据支持扩大 Stage 1 并发。

机器可读证据：`docs/refactor/evidence/cyanflow-c3-stage2-learning-batch-read-2026-08-16.json`。

## 5. 下一切片

将 vector recall result projection 的 lifecycle point read 改为批量 user-scoped repository read，并继续保留缺失/suppressed/pending/auto-apply、转账保护、Weaviate 降级和输入顺序合同。完成后再评估 C3 exit，不把当前子切片结果外推为整个 Stage 2 已无 N+1。
