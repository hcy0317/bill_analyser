# C2 PreviewStateKernel Foundation

## 交付边界

本切片建立 C2 的 Rust canonical foundation，不提前完成整个 C2 cutover：

- `PreviewStateKernel::derive(PreviewStateInput) -> PreviewState` 是公开 typed API，不接受 `Map<String, Value>`；
- `PreviewState` 固定 `projection_version = 1`，包含六类 `SignalSet`、阻塞 `ReviewIssueSet`、四个 lifecycle 的 evidence/decision 只读视图和合法 identity 的 effective fields；
- `ImportPreviewFilterIndexItem` 新增 `preview_state`，现有 legacy row 只通过一个集中 compatibility adapter 进入 kernel；
- transfer、learning、LLM 与 history 的旧扁平状态字段由同一 snapshot 回投，`auto_applied` 在 typed decision view 中保留，旧字段继续兼容为 `accepted`；
- PostgreSQL `import_preview_signal_flags` 不改规则、不获得写权，只作为 C5 reader cutover 前的只读 shadow oracle；
- 本切片不改 import schema、不切 SQL filter reader、不切 desktop/mobile mapping，也不改变 confirm writer。

## 固定业务合同

`tests/fixtures/import_preview_state_kernel_v1.json` 是跨投影 canonical fixture，当前固定七种边界：

1. parser-only 只属于 `parser`；
2. 另外五类出现时 parser membership 被排除；
3. 非 parser family 可重叠；
4. likely-transfer 同时属于 `transfer` 与 `learning`；
5. transfer/history/learning/LLM 未知非空状态全部隐藏并产生阻塞 issue；
6. 缺失或非法 identity 是 issue，不是 signal；
7. 转账来源/目标账户相同会清除 effective destination 并阻止 confirm。

同一 fixture 由纯 Rust kernel、legacy-row compatibility adapter 和真实 PostgreSQL shadow oracle 消费；三者的六类 membership 未解释差异必须为 0。

## 所有权与删除门禁

| Surface | 当前角色 | Writer | 删除/切换门禁 |
| --- | --- | --- | --- |
| `PreviewStateKernel` | canonical typed projector | Rust core | 不删除；后续扩展只能先改 fixture 和 typed input |
| filter-index compatibility adapter | legacy JSON evidence adapter | 无持久 writer | 新写路径直接产出 typed input 后删除 JSON adapter |
| `import_preview_signal_flags` | SQL read-only shadow oracle | 无 | C5 typed projection reader 上线、真实 parity=0、无 active legacy session 后删除重算逻辑 |
| desktop/mobile signal projection | 旧 UI read-only oracle | 无 | typed API/TS DTO 与 desktop/mobile selector parity 完成后删除 JSON 状态机 |

## 当前证据

- TDD RED：公开 kernel 类型不存在时 `import_pipeline_contracts` 编译失败；filter index 尚无 `preview_state` 时第二个合同测试编译失败；`is_confirmable` 尚不存在时 fixture 合同编译失败。
- Rust canonical fixture：7/7 case 通过。
- legacy-row adapter parity：7/7 case 通过，未解释 membership diff 为 0。
- 真实 PostgreSQL shadow parity：`real_postgres_signal_flags_match_preview_state_kernel_v1_fixture` 1/1 通过，0 skip，七个 case 未解释 diff 为 0。
- core affected surface：`import_pipeline_contracts` 27/27 通过。
- 受影响集成面：真实 PostgreSQL `import_staging` 35/35、`preview_metadata_postgres` 6/6、HTTP `import_runtime_contract` 5/5 通过，0 skip。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 完整 Rust workspace coverage 命令通过并生成 `workspace.lcov`；本切片 changed-line coverage 为 329/343，可执行改动行覆盖率 95.92%。

## 后续开放门禁

- 下一切片把 typed snapshot 接入 preview row 完整响应和 TypeScript DTO；desktop/mobile 只能映射 snapshot，不再解析 JSON 决定 membership。
- SQL page/filter/count/facet reader 在 benchmark 和 typed persisted projection schema 冻结前保持现状；不得向 legacy SQL 添加新业务规则。
- identity category type compatibility、manual ownership 和完整 effective row 仍由现有 staging identity validation 负责；迁入 kernel 前必须先补 typed facts 和真实 PostgreSQL parity，不能从展示文本推断 canonical ID。
