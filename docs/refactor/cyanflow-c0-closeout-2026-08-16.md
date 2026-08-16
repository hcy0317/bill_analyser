# Bill Analyser C0 关闭与 C1 准入记录

> 状态：`C0_EXIT_PASS / C1_ENTRY_OPEN`
> 日期：2026-08-16
> 工作流：`Cyaness -> Cyanflow`
> C0 起始主线：`main@9d043748ae17d7e5fc3304d521de8b2a32ecbec8`
> 识别合同提交：`60da580f9accd6127607e3e188ac62e30b731548`
> OMX：未使用，也不是后续执行依赖

## 1. 结论

C0 的架构、性能、运行 provenance 和识别质量工件已经齐全，C1 两项独立 canary 可以开始。C0 不宣称底层重构已经完成，只证明后续切片有当前/目标 owner、唯一 writer、回滚或 forward-fix 点、删除门禁以及可复跑的真实验收基线。

`cyanflow-project-modular-architecture-consensus-2026-08-15.md` 形成时记录的“实现授权：false”只描述当时的 proposal 阶段。用户在 2026-08-16 明确授权按 Cyanflow 自治实现、提交、创建 PR、观察 CI、合并并继续后续切片，因此该旧元数据不再阻止 C1；业务和架构约束仍继续有效。

## 2. C0 固定工件

| 工件 | 证据 | 状态 |
| --- | --- | --- |
| Ownership matrix、架构债务基线、C1 transition ledger | `docs/refactor/cyanflow-c0-architecture-baseline-2026-08-15.md` | PASS |
| 64 文件 corpus、binary/config/browser provenance | `docs/refactor/evidence/cyanflow-c0-64file-evidence-manifest-2026-08-15.json` | PASS |
| Stage 1/2 性能 profile | `docs/refactor/evidence/cyanflow-c0-performance-profile-2026-08-15.json` | PASS |
| 单次表格物化与 exact-main 复验 | `docs/refactor/cyanflow-c0-xls-single-materialization-2026-08-15.md` | PASS |
| 识别与信号语义 fixture | `docs/refactor/evidence/cyanflow-c0-recognition-quality-fixture-2026-08-15.json` | PASS |

## 3. 性能退出证据

固定 64 文件 corpus manifest 为 `0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838`。`main@80269554153b21f122fa03b4d9a9484134439e8c` 的三次 exact-main 浏览器验收结果如下：

| Run | Parsed | Preview | Unmatched | 首屏可操作 |
| --- | ---: | ---: | ---: | ---: |
| 1 | 15,041 | 13,082 | 0 | 12,560ms |
| 2 | 15,041 | 13,082 | 0 | 22,860ms |
| 3 | 15,041 | 13,082 | 0 | 21,416ms |

三次均低于 60,000ms，且 parser 输出逐文件、逐字段 parity 为 0；C0 性能退出通过。后续切片不得通过延后首屏必需信号或跳过真实浏览器重新制造快结果。

## 4. 识别质量退出证据

识别 fixture 的 8 个场景现均为 `covered`。最后关闭的 `generic-unmatched-column-mapping` 使用真实 HTTP handler 和隔离 PostgreSQL 16，验证：

- dedicated parser 对通用 CSV 明确返回 `no_match`，不伪选 parser；
- unmatched 临时文件可继续进入 generic column mapping；
- 映射后返回 2 行且无 unmatched 文件；
- PostgreSQL standard rows 保留 source/row 顺序、`rust-import` parser id、描述、日期、收入 `1,250` 分和支出 `-800` 分；
- focused discovery 为 1，执行 1，通过 1，跳过 0。

因此 recognition fixture 的 `all_contracts_executable=true`，阻断场景为空。

## 5. C1 准入边界

C1 仍保持两个独立、可回滚 canary，不合并成一次大改：

1. Ledger list 边界：`HTTP mapper -> LedgerQueries::list -> PostgreSQL query module -> LedgerEntryPage`，证明 handler 不打开 DB runtime、不解析 DB row，并保持 REST parity 为 0。
2. Missing-category 止血：删除手工 revision/annotation cache 权威，只保留 `server issues + draft-only ProvisionalIssueDelta`；合法分类后 issue 必须在同一 tick、ack、翻页、刷新和重新进入时消失。

每个 canary 开始前都由 Cyanflow 重新裁决最小动作并走 TDD。C1 不改 import schema、不做 UI 视觉改版，也不预建全局 application/server crate。

## 6. 非阻断风险

- 最近精确 head 的 E2E 约 10 分钟，其中主要耗时为依赖、Chromium 和 supervisor 环境准备；它不构成本次 import 业务阻断，但后续 CI 若在缓存命中后单阶段超过 10 分钟，必须按测试拓扑异常处理。
- C1 两项 canary 必须分别保留 transition ledger、旧路径删除条件和 changed-line coverage，不能用 C0 全局 coverage 代替。
- C0 只关闭证据门禁；C1-C7 的模块所有权、typed API、schema expand/contract 和 confirm 净化仍需逐切片完成。
