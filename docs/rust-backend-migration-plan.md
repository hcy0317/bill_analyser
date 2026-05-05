# Rust Backend Migration Plan Baseline

S0 establishes the auditable contract used by later Python-to-Rust migration slices.
S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.

## Preservation Rules

- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.
- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.
- `/api/v1/*` is not revived as a runtime chain.
- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.

## Initial Migration Surface

- Python backend files to track: 313
- Current Rust backend files: S1 introduces the `bill-analyser-core` internal runtime shell crate only.
- Initial verified-dead files: 0

## S1 Rust Runtime Shell

- `bill-analyser-core` 是 Rust 内部库边界，当前只包含 runtime identity、health check、统一错误结构、serde JSON 和 API response envelope 基础类型。
- Flask REST 外壳继续作为运行时入口；S1 不接管任何业务 API，不新增 `/api/v1/*`，不迁移任何业务域。
- S1 的 `ApiResponse` 只代表 Rust runtime shell foundation；后续业务 API 迁移必须为对应 endpoint 增加现有 Flask envelope parity adapter 或测试，不能把 S1 envelope 直接当成全站业务响应替代品。
- 后续功能域切片必须在各自 feature-gap review 中逐项声明 Python business item 的 `ported`、`facade_only`、`verified_dead` 或 `deferred_with_reason` 状态；S1 的 `business_migration` 保持 `none`。

## Domain Review Baseline

| Domain | Port | Facade | Deferred |
| --- | ---: | ---: | ---: |
| accounts | 6 | 2 | 0 |
| ai-learning-llm | 24 | 6 | 0 |
| ai-ocr | 4 | 1 | 0 |
| api-contract-adapters | 0 | 4 | 0 |
| api-runtime-shell | 3 | 8 | 0 |
| auth-security | 18 | 2 | 0 |
| backup-operations | 5 | 1 | 0 |
| bills-import | 53 | 7 | 0 |
| budgets | 16 | 6 | 0 |
| classification-rules | 16 | 2 | 0 |
| database-facade | 3 | 2 | 0 |
| database-schema | 11 | 3 | 0 |
| import-contracts | 0 | 2 | 0 |
| import-parsers | 8 | 2 | 0 |
| matching-reconciliation | 22 | 5 | 0 |
| recurring-calendar | 4 | 0 | 0 |
| settings-bundle | 8 | 1 | 0 |
| shared-primitives | 5 | 3 | 0 |
| smart-dedup | 9 | 1 | 0 |
| statistics-reporting | 26 | 4 | 0 |
| sync-runtime | 1 | 0 | 0 |
| tags-templates | 8 | 1 | 0 |

## Review Contract

- Code-bug reviews compare changed inventory tooling and docs against this deterministic scan.
- Feature-gap reviews use the Python File Matrix to prove each backend item is ported, facade-only, deferred with reason, or verified-dead with evidence.
- A later slice cannot claim a domain complete while any file in that domain is unmapped.
