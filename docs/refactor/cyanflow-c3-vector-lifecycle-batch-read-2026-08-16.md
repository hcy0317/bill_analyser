# Bill Analyser C3 vector lifecycle 批量读取与阶段关闭

> 状态：`SLICE_PASS / C3_EXIT_PASS`
> 日期：2026-08-16
> 工作流：`Cyaness -> Cyanflow -> TDD`
> 基线 main：`f7524daa44990ea07a2e69ccb9d31b3a7d717a62`

## 1. 交付结论

vector recall result projection 不再对候选逐条调用 learning lifecycle point read。应用层先按既有 score、scope、distance、source 顺序准备候选及 recommendation key，对同批 key 去重后只调用一次 user-scoped batch repository；应用阶段从不可变 lifecycle snapshot 中跳过 missing 或 suppressed 候选并继续选择下一候选。

本切片保持转账保护、active category/account 校验、pending/accepted 状态展示和 Weaviate 降级语义。即使 lifecycle 已允许 auto apply，派生自 Weaviate metadata 的建议仍只产生黄色待审核信号，不改写 preview draft。REST API、PostgreSQL schema、HTTP DTO、金额单位、前端模型和 confirm 协议均未改变。

确定性 learning 批量读取已由前一切片关闭，本切片同时删除 vector recall 在 HTTP 生产路径中的最后一处 lifecycle point read。结合既有调用方、HTTP SQL、资源预算与真实浏览器证据，C3 的当前退出条件已满足。

## 2. TDD 与行为合同

RED 阶段先增加架构门禁，证明 vector result projection 尚未调用 batch repository；失败信息为期望 1 次、实际 0 次。GREEN 后固定以下合同：

- 非空 vector result 每批至多执行一次 lifecycle SQL；没有有效 recommendation key 时 repository 直接返回，不访问 SQL。
- recommendation key 去重并保持确定性候选顺序，查询严格绑定当前 `user_id`。
- 同 key 的其他用户 lifecycle 不得影响当前用户投影。
- missing 或 suppressed lifecycle 不终止整行候选选择，继续尝试下一候选。
- lifecycle batch read 失败时整批 vector 建议降级为未应用并记录 warning，不产生部分 derived suggestion。
- Weaviate metadata 永不自动改写 preview；auto-eligible lifecycle 仍输出黄色 pending 建议。
- HTTP vector result production 文件只允许一个 batch repository 调用点，禁止重新引入 lifecycle point read。

## 3. 验证证据

| 门禁 | 结果 |
| --- | --- |
| RED architecture test | 1 failed，期望 batch 调用 1、实际 0 |
| `cargo fmt --all -- --check` | pass |
| `node scripts/check-rust-backend-structure.mjs` | 593 files，3 baseline entries，pass |
| Import runtime contract tests | 7 passed，0 failed，0 ignored |
| Stage 2 real PostgreSQL repository tests | 2 passed，0 failed，0 ignored |
| Vector recall focused tests | 10 passed，0 failed，0 ignored；required PostgreSQL 场景未 skip |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass；HTTP library 208 passed，真实 PostgreSQL 场景未 skip |
| Rust workspace line coverage | 65.06%，门槛 35% |
| Rust changed-line coverage | 94/101，93.07%，门槛严格大于 90% |
| `git diff --check` | pass |

## 4. 固定 64 文件画像

worktree run `c3-vector-lifecycle-batch-worktree-20260816-r1` 通过：64 文件、15,041 parsed、13,082 after dedup、0 unmatched；Stage 1 为 8,084ms，Stage 2 为 14,909ms，首屏可操作为 23,862ms。canonical parser filter request 为 1，legacy filter-index request 为 0；业务数据、run-scoped Weaviate collection 和隔离数据库均严格清理成功。

本次单次画像比 C3 exact-main 四次样本慢，Stage 1、Stage 2 和首屏分别仍低于 12s、20s 和 60s 门槛。它能证明当前 worktree 未越过预算，不能证明本切片带来稳定加速；固定 corpus 没有 learning/vector 命中，因此 N+1 删除由 source architecture gate 和真实 PostgreSQL user-scope 行为测试证明，不用本次 intelligence 时间代替调用次数证据。

runner 在未提交 worktree 上执行，因此 artifact 的 `headSha` 是基线提交；实际被测二进制由 runtime provenance 的 SHA-256 `87242a1b39fad3115a462d9ea14efc42224b090c77cba71b86e7adfee3781366` 标识。corpus manifest 仍为 `0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838`。

机器可读证据：`docs/refactor/evidence/cyanflow-c3-vector-lifecycle-batch-read-2026-08-16.json`。

## 5. C3 关闭与下一阶段

C3 关闭依据：三个调用方只进入 `ImportStage2::evaluate`；HTTP production 相关 SQL 为 0；Stage 2 context 每批读取一次；deterministic learning 与 vector recall lifecycle 均为 user-scoped batch read；Stage 1/2 和完整 browser 未越过既定预算；recognition parity 为 0。

下一阶段进入 C4 Typed Import API 与前端状态。首个切片必须先盘点 preview/query/mutation/decision/confirm 的 weak type 与 row-version 契约，形成 characterization 和 additive contract，不直接进行破坏性 API 切换。
