# Bill Analyser C3 Stage 2 性能与资源画像

> 状态：`PROFILE_PASS / C3_EXIT_PENDING_BATCH_LIFECYCLE_READ`
> 日期：2026-08-16
> 工作流：`Cyaness -> Cyanflow`
> exact main：`244c30f952f82426b76936375151c739aa10f5cf`
> 固定 corpus：64 文件，manifest `0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838`

## 1. 结论

当前主线的 Stage 1/2 已满足重新分配的阶段预算，且四次有效真实 browser run 的数量与信号路径一致。最慢首屏可操作耗时为 `12,526ms`，Stage 1 最慢 `4,265ms`，Stage 2 最慢 `7,906ms`。因此没有证据支持在 Stage 1 增加并发；压缩工作簿继续采用单 worker 顺序准入，避免扩大 RSS、取消和有序归并风险。

C3 尚有一个明确阻断项：learning 匹配在逐行循环中调用 `get_import_learning_lifecycle_view`。固定 corpus 没有 learning 命中，所以当前 `600-657ms` intelligence 指标没有覆盖该 N+1 查询路径。下一最小切片必须把 lifecycle 读取迁入 user-scoped batch repository，并保持缺失、suppressed、pending、accepted 与 auto-apply 语义不变，完成后重跑本画像才能关闭 C3。

## 2. 调用图前后对比

重构前 `ca1c08f0f5423fe76127a904d3c7d7b64996e508`：

```text
dedup handler -----------+
direct reclassify -------+--> apply_import_intelligence_chain
decision-group reclassify+      -> HTTP stage2_chain 中 7 个 production SQL query
                               -> 每个 caller 自行承担 context/rule loading
```

当前 `244c30f952f82426b76936375151c739aa10f5cf`：

```text
dedup handler -----------+
direct reclassify -------+--> ImportStage2::evaluate
decision-group reclassify+      -> load_import_stage2_context once per batch
                               -> ImportStage2ContextSnapshot
                               -> precompiled category/account rules and reused maps/values
                               -> fixed-order Stage 2 projection

HTTP production SQL: 0
DB repository: src/backend/db/import_stage2.rs
```

`tests/backend/http/import_runtime_contract.rs` 对三个调用方、固定阶段顺序和 HTTP SQL=0 提供结构门禁；本次 focused discovery 命中 3 项并全部通过。

## 3. exact-main Stage 1/2 profile

四次有效 run 均使用全新隔离 PostgreSQL database、run-scoped Weaviate prefix、dedicated E2E defaults、同一 debug binary SHA `eaa28a9359120f71f4f7fd4f99810cefbe8fdace0949be49bf74b918c5381f6e`，并在结束后完成 strict business/Weaviate/database cleanup。

| Run | Browser parse | Stage 1 total / parser | Stage 2 total | dedup / intelligence / insert | 首屏可操作 |
| --- | ---: | ---: | ---: | ---: | ---: |
| r2 | 4,690ms | 4,265 / 1,621ms | 7,457ms | 2,988 / 636 / 2,914ms | 12,275ms |
| r3 | 4,483ms | 4,048 / 1,615ms | 7,906ms | 3,022 / 600 / 3,312ms | 12,516ms |
| r4 | 4,594ms | 4,170 / 1,595ms | 7,515ms | 2,994 / 602 / 2,999ms | 12,250ms |
| r5 | 4,587ms | 4,158 / 1,638ms | 7,809ms | 2,985 / 657 / 3,260ms | 12,526ms |

每次均为 64 文件、15,041 parsed、13,082 after dedup、0 unmatched；canonical parser filter request 为 1，legacy filter-index request 为 0。从 dedup response 到首屏 operable marker 为 `61-67ms`。

r1 没有进入应用：Windows 排除端口范围覆盖 `55432`，导致 PostgreSQL dependency readiness fail-closed。数据库容器随后以同一命名卷改发未排除端口 `54321`；r2-r5 均通过。r1 不计入性能样本，且残留隔离数据库数量为 0。

## 4. 资源、预算与取消

r5 以 250ms 周期对实际 `bill_http_server` PID 做 94 次外部采样：

| 指标 | 结果 | 口径 |
| --- | ---: | --- |
| peak working set | 430,399,488 bytes（410.46 MiB） | 整个真实 E2E 后端进程生命周期，不是请求预算 |
| peak private bytes | 413,417,472 bytes（394.27 MiB） | 同上 |
| backend CPU | 28,640.625ms | 同一进程累计 CPU |
| process read/write transfer | 1,018,185 / 1,769 bytes | Windows process counters；不含 PostgreSQL 容器 I/O |
| PostgreSQL temp bytes | 181,023,414 bytes | 隔离库全 run；包含两次 real64 desktop case、mobile smoke 与 cleanup |
| PostgreSQL blocks read/hit | 146 / 2,607,442 | `pg_stat_database`，不是 SQL statement count |

请求级合同与进程 RSS 必须分开解释：HTTP 默认 body limit 为 10 MiB；标准账单字符串/容器估算的请求级物化上限为 64 MiB。峰值进程 RSS 还包含 Rust runtime、parser/workbook、JSON、15,041 行 staging draft、连接池与 allocator 保留内存，不能拿 64 MiB 直接比较。

预算与恶意输入 focused gate：

- 聚合 `StandardBill` 物化超过 64 MiB 返回 413：1/1 通过。
- hostile XLSX 在 dedicated parser 物化前拒绝：HTTP 1/1 通过。
- XLSX archive/expanded cell budget：parser 2/2 通过。
- 当前 Stage 1 文件 worker 并发为 1；handler future 取消后不会准入后续文件，但已经进入 `spawn_blocking` 的单个 worker 只能有界完成。
- 只有全部文件 parse 成功并通过请求预算后才进入 staging；worker 失败、预算超限或 hostile input 不持久化部分 Stage 1 结果。

单 worker 的取消边界在当前 `1.6s` parser 窗口可接受。任何未来 `concurrency > 1` 的变更必须先增加 spawn 前内存 permit、压缩 worker 独立限额、取消 token 和输入顺序归并测试；本切片不提前引入。

## 5. 互斥阶段预算

| 互斥窗口 | 预算 | 当前最慢 | 余量 |
| --- | ---: | ---: | ---: |
| Stage 1：multipart + parser + staging | 12,000ms | 4,265ms | 7,735ms |
| Stage 2：dedup + intelligence + preview insert + response | 20,000ms | 7,906ms | 12,094ms |
| preview query + 首屏可编辑/信号/筛选 | 5,000ms | 67ms（dedup 后） | 4,933ms |
| upload/browser/network 调度抖动 | 15,000ms | 不与 server stage 重叠计入 | 动态 |
| 分配合计 | 52,000ms | - | 为 60,000ms 总 SLA 保留 8,000ms |

完整 `<=60s` 仍属于 C6 终验；这里仅证明 Stage 1/2 在相同 corpus 上满足互斥预算，不把单阶段结果替代最终 SLA。

## 6. Recognition parity 与下一动作

本次四个 run 均保持 15,041 parsed、13,082 preview、0 unmatched；C0 recognition fixture 的 8 个场景仍全部 `covered`。没有减少 parser registry、延后首屏 signal、复用业务 session 或开启 same-file cache。

下一动作只有一个：以 TDD 将 learning lifecycle point read 改为一次批量 repository read，并增加多行同/异 recommendation key 的真实 PostgreSQL query-count/behavior fixture。该切片完成、画像复跑且 parity=0 后，才能把 C3 标记为 exit pass。

机器可读证据：`docs/refactor/evidence/cyanflow-c3-stage2-profile-2026-08-16.json`。
