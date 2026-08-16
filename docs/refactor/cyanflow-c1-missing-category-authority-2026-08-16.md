# C1 Missing-category 权威收敛合同

> 状态：`MERGED / MISSING_CATEGORY_CANARY_COMPLETE`
> 交付：PR `#291`，squash commit `f7fe42a55dcb2af26a09a313ac6629a8f2fe4108`

本 canary 只收敛前端 missing-category issue 的读取权威，不改导入 schema、HTTP endpoint、服务端 writer 或视觉交互。数据库 patch transaction 继续是唯一持久 writer；本地分类草稿只作为当前 effective row 的临时输入。

## 边界选择

Codebase Memory 调用图显示，missing-category 派生函数只有两个直接调用者，旧权威集中在 `useImportCheckDataAnnotations` 的手工 revision 与 `annotationIssuesByIndex` 快照。对照的 Ledger list handler 有 23 个直接依赖，覆盖 DB runtime、filter lookup、query、tag presenter 和 HTTP envelope。为保持首个 C1 切片最小且可逆，本轮先关闭 missing-category 缓存权威；Ledger list 继续作为独立 canary。

## Transition ledger

| 字段 | 本 canary 合同 |
| --- | --- |
| Current owner | 服务端 row/matching 提供 baseline；当前 effective row 决定分类与账户 issue |
| Old read | effective row -> computed selection summary -> `annotationIssuesByIndex[item.index]` |
| New read | effective row -> `collectAnnotationIssues(item)`；selection summary 仅做响应式聚合 |
| Writer | DB patch transaction 是唯一持久 writer；本轮无新增 writer |
| Compatibility | endpoint、response envelope、筛选、选择、desktop/mobile 数据模型保持 |
| Rollback | 单个前端 caller 级回退；无 schema 或数据回滚 |
| Delete gate | `importTransactionSelectionRevision`、`refreshImportTransactionSelectionSummary`、`annotationIssuesByIndex` 全仓为 0 |

## 行为门禁

- 合法分类写入当前行后，不调用 refresh API，`getAnnotationIssues` 与 `needsAnnotation` 在同一交互帧返回已解决状态。
- selection summary 从当前 tracked rows 重新派生，不持有另一份按 index issue 真相。
- 既有浏览器合同继续覆盖 server ack、翻页、刷新与重新进入；任一必需场景 skip 或失败均不得交付。
- changed-line coverage 单独核算本 canary，不用 C0 全局覆盖率代替。

## 删除后的所有权

`matching.annotation` 只保留服务端 baseline/evidence 作用；已解决的已知 identity annotation 会按当前行字段判定为无效，未知状态仍 fail-closed 并保留证据。前端不再通过手工 revision、额外 refresh 或按 index cache 决定 issue 是否存在。

## 本地验收证据

- Red：新增“无需手工 refresh 即从当前行消除 missing-category”测试后，旧实现返回 `['Missing Category']`，目标断言为 `[]`。
- Green：目标 composable suite 10/10 通过；selection/paging、signal/history、review-state 与 SFC 矩阵 54/54 通过。
- 前端全量 coverage：307 suites、41,457 tests 全部通过；statements 94.12%、branches 91.26%、functions 91.96%、lines 94.23%。
- 本切片 changed-line coverage：2/2 可执行变更行覆盖，100%。
- `npm run lint` 与 `npm run build` 通过；lint 仅保留仓库既有 warnings。
- 真实 desktop Playwright：missing-category 同 tick、ack、翻页、刷新与重进场景 2/2 通过，0 skip，严格 cleanup 通过。
