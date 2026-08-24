# Cyanflow C4 Version Precondition Enforcement

## 结论

C4 已从 additive/telemetry 阶段切到强制前置条件。所有会修改 import preview 业务字段的公开请求必须携带当前行版本；active confirm 必须携带当前 session 版本。缺失 token 统一返回类型化 HTTP 428，并在进入业务 writer 前结束，因此不会形成 last-write-wins 或部分写入。

terminal confirm receipt replay 保持最高优先级：同 fingerprint 的已确认 session 即使没有新的 session token，仍直接返回既有 durable receipt，不重复执行账单、history effect、receipt 或 cleanup。

## 合同

| 边界 | 必需 token | 缺失响应 | stale 响应 |
| --- | --- | --- | --- |
| 单行 update、非空 reclassify、conditional selection/action preview patch | `expected_row_version` | `428 PREVIEW_ROW_VERSION_REQUIRED` | `409 PREVIEW_ROW_VERSION_CONFLICT` + 权威 preview row |
| recurring、transfer、learning、LLM 与 preview matching decision | `expectedState.rowVersion` | `428 PREVIEW_ROW_VERSION_REQUIRED` | 同一 409 snapshot 合同 |
| active confirm | `expected_session_version` | `428 IMPORT_SESSION_VERSION_REQUIRED` | `409 IMPORT_SESSION_VERSION_CONFLICT` + 最新 session summary |
| terminal confirm replay | 不要求新 token | 不适用 | canonical fingerprint 决定 replay/conflict |

空 `preview_updates` 的 reclassify 不再解释为整 session 写入，而是以 428 失败关闭。纯 selection-only mutation 不修改 preview 业务字段，继续只使用独立 `selection_hash`。

## 边界与可维护性

- 没有新增 schema、crate、trait、通用 UnitOfWork、第二 writer 或 sidecar。
- Rust 复用一个 row-version payload parser 和一个类型化 428 response projector；confirm 只在既有 repository port 增加 HTTP versioned wrapper，内部兼容调用不改变。
- desktop/mobile 共享 `ImportPreviewRecord`、`ImportPreviewPatchPayload`、`ImportPreviewExpectedState` 与 `ImportPreviewConfirmCommand`；公共 row/session token 在 TypeScript 模型中为必需字段。
- telemetry 只记录合同类别、token kind 与 required/present/missing 计数；现有 operation-specific version telemetry 对成功携带 token 的路径保持不变，两者都不记录用户、session、账单、fingerprint 或凭据。
- matching 的普通 bill candidate 仍不是 preview mutation，不会被错误要求 import row token；只有已解析为 preview context 的 candidate 才执行该前置条件。

## 批准与外部影响

本地运行日志共检查 40 个文件，未观察到 `legacy_missing_version` 或 import contract 命中；这只是“当前没有观察到旧流量”，不是对未知外部客户端的证明。用户随后明确批准 breaking-contract enforcement，并授权自治提交、PR、CI、合并与分支清理。旧客户端若仍省略版本会收到稳定 428，需要先读取当前 preview/session snapshot 后重试。

## 验证

- Codebase Memory 图谱：`bill_analyser-d7309f0e9eab-r1452b742b0eb`，七类只读查询均执行成功。
- Rust HTTP library：274 passed，0 failed；覆盖 update/reclassify/selection/recurring/transfer/learning/LLM/matching/confirm 与零写入断言。
- 前端 TypeScript/ESLint：通过；仓库既有 128 条 warning、0 error。
- Rust workspace coverage：81,097 lines found、56,461 hit、69.6216%；changed executable lines 104、covered 94、90.38%，严格通过 `>90%` 门禁。
- 前端 coverage：308 suites、41,500 tests 全部通过；全量 lines 94.2428%，changed executable lines 21、covered 20、95.24%；production build 通过。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过；PR #361 实现 head `6c823c81f` 的 run `16454` 四个 required jobs 全部成功，其中 repo-governance 首次 checkout 遇到 TLS timeout，failed-job 原地重跑后通过。

## 回滚

本切片没有数据迁移。若必须临时回滚 API enforcement，应回滚本切片代码并同步回滚 web client 类型合同；不得只在单一路由恢复缺 token 写入。C5/C6 已完成的 typed reader/目标库物化不依赖本切片，也不应随 C4 回滚。
