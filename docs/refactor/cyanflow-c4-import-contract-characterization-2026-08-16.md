# Cyanflow C4 导入契约现状与首个 additive 切片

日期：2026-08-16

状态：characterization 与本地门禁完成，等待 exact-head CI 关闭

范围：preview/query/mutation/decision/confirm 的 Rust、PostgreSQL 与 TypeScript 合同；本切片不改变生产行为

## 结论

C4 的首要问题不是缺少一个统一 OpenAPI 文件，而是同一个导入实体在存储、HTTP 与前端边界使用了不同的并发身份：

1. PostgreSQL `import_preview_rows.version` 已存在，普通 patch 也会执行 `version = version + 1`，但 `ImportPreviewRow` 和 PostgreSQL row mapper 不返回该值。
2. 普通单行 update 没有 `expected_row_version`，更新 SQL 也没有 `WHERE version = expected`；它只能保证“写过之后版本增加”，不能防止后写覆盖先写。
3. decision route 使用字段快照 `ImportPreviewExpectedState` 做局部冲突判断；移动端部分调用只传 `sessionId`，因此同一行并发编辑仍可能漏判。
4. confirm 后端已经支持 `expected_session_version`，并在事务内执行 session CAS；但 session GET、Rust summary、前端 service 和 desktop/mobile 调用都没有形成可传递的 session version 链路。
5. `selection_hash` 只证明跨页选择集合没有变化；decision-group version、history bill version、preview row version、session version 各自保护不同聚合根，不能合并成一个 `selection_version`。
6. `src/web/src/lib/services/importPreview.ts` 当前有 28 个 `Record<string, unknown>` 和 13 个词法 `any` 命中；桌面两个页面另有 13 个 `/api/bills/import/v2` 字符串命中，说明 typed service 与页面直连适配器仍在并行演化。

底层架构判断：数据库已有建立乐观并发控制所需的单调 token，问题主要位于 repository DTO、HTTP contract 和 client state 的断链。无需重建数据库或更换 REST 运行时；应按单一 typed contract 做 expand/cutover/enforce。

## 权威 token 边界

| Token | 当前 owner | 保护对象 | 当前缺口 | C4 规则 |
| --- | --- | --- | --- | --- |
| `import_preview_rows.version` | PostgreSQL preview row | 单行字段、matching 与 decision 投影 | API/前端不可见，普通更新无 CAS | additive 暴露为 `row_version`；竞争 mutation 使用 `expected_row_version` |
| `import_sessions.version` | PostgreSQL import session | confirm 事务与 terminal 状态 | GET/session summary 不返回，前端 confirm 不传 | additive 暴露为 `session_version`；confirm 继续使用 `expected_session_version` |
| `selection_hash` | preview query metadata | 当前 selected preview id 集合 | 仅 LLM/action scope 使用 | 保留集合快照语义，不替代 row/session CAS |
| decision group version | decision-group repository | 一组 transfer/history 等成员及决定 | 已有独立 expected versions | 保留，不折叠进 preview row token |
| history bill version | ledger/history repository | confirm-time 历史账单 rewrite | 已进入 acknowledgement | 保留，不折叠进 session token |

## 端点契约矩阵

| Surface | 当前请求 | 当前返回 | 当前并发保护 | 缺口与迁移优先级 |
| --- | --- | --- | --- | --- |
| `GET /preview/:session` | typed query filters 在 HTTP 内归一 | Rust `ImportPreviewPageData.preview: Vec<Value>`，row 无 version | `selection_hash` 只覆盖选择集合 | P0：先把 row DTO 与 page DTO typed 化并 additive 暴露 `row_version` |
| `PUT /preview/:session/update` | raw JSON -> `ImportPreviewPatch` | legacy `{success}` 或 `previewItem: Value` | 无 row CAS；SQL 只递增 version | P1：可选 `expected_row_version`，成功返回完整新 row，409 返回最新安全 snapshot |
| `PUT /preview/:session/selection` | selection action + 可选 preview patches | metadata + 计数 | query scope/selected ids，不是逐行 CAS | P2：patch 部分携带各自行版本；纯集合选择继续用 selection hash 语义 |
| transfer/learning/LLM decision | raw JSON + 部分字段 expected state | 多种 `Value` envelope | 字段快照或 group version；移动端保护更弱 | P2：统一 typed command；涉及行写入时带 `expected_row_version` |
| `GET /session/:session` | session id | `ImportSessionSummary` 无 version | 无 | P3：additive 暴露 `session_version` |
| `POST /confirm` | 后端接受可选 `expected_session_version` | DB-owned HTTP envelope | 后端已有 session CAS，当前 web 不传 | P3：当前 web 全部传 version；保留 terminal receipt replay 优先 |

## 可执行 characterization

`tests/backend/http/import_runtime_contract.rs` 增加三个 focused gate：

- 证明 preview row 的持久 version 和递增语句存在，同时锁定当前 public row、mapper 和 update SQL 的 CAS 缺口；
- 证明 confirm handler 已接受 session version，同时锁定 session DTO 与 web confirm facade 的传递缺口；
- 对 `importPreview.ts` 的弱类型和 desktop 直连 adapter 建立只降不升 ratchet，并要求 focused discovery 命中数大于零，避免扫描范围错误造成假绿。

这些是迁移期 characterization，不是永久认可缺口。后续生产切片每关闭一项，就必须把对应 absence assertion 改成正向 contract assertion；禁止删除整个 gate 来绕过。

## 首个 additive 实现切片

下一切片只完成 `row_version` 的端到端可见性，不同时强制 CAS：

1. DB `ImportPreviewRow` 增加 `version: i64`，row mapper 必须从既有列读取；所有现有构造器显式补齐版本。
2. preview page、mutation refresh、decision refresh 与 matching candidate 返回复用同一个 typed preview row presenter，并以 additive `row_version` 暴露。
3. TypeScript 中性 `ImportPreviewRecord`、page response 和 mutation response 增加明确的 `row_version` 类型；desktop/mobile 读取同一字段。
4. 增加 Rust serialization fixture 与 TypeScript fixture，冻结字段名、整数语义和缺失字段兼容策略。
5. 旧客户端缺少版本时行为不变；本切片不修改写入条件，不宣称已具备 CAS。

该切片完成后，第二个实现切片再为单行 update 加入可选 `expected_row_version`、原子 `WHERE version = expected` 和 typed 409；当前 web 全面发送 token 后，才讨论把缺失版本从 telemetry 兼容分支升级为拒绝。

## 不变量与风险

- 金额仍以 `*_amount_cents` 整数分传递，本切片不新增元/分转换。
- confirm 真实 effect 仍只执行一次；characterization 不触碰 receipt、history、bill 或 staging cleanup。
- terminal receipt replay 始终优先于新旧 binary 路由。
- 409 最新 snapshot 只能返回当前用户、当前 session 有权读取的行，不得借冲突响应扩大数据可见范围。
- 页面直连收敛必须按 endpoint 切换，不能一次移除所有 legacy fallback；每个 endpoint 的 typed facade、调用方与错误处理必须在同一切片关闭。
- `PROJECT_OVERVIEW.md` 本切片无需更新，因为没有稳定运行时行为、接口契约或模块关系变化。

## 验证命令

```powershell
cargo test -p bill-analyser-http --test import_runtime_contract
cargo fmt --all -- --check
node scripts/check-rust-only-source-tree.mjs
```

机器可读证据：`docs/refactor/evidence/cyanflow-c4-import-contract-characterization-2026-08-16.json`。

本地结果：focused contract `10 passed / 0 failed / 0 ignored`；格式门禁通过；Rust-only source-tree gate 扫描 1,969 个 tracked path 通过。
