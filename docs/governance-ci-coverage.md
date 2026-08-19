# CI 证据与覆盖率归一化

本页固定 B009 治理切片的证据格式。它只规范脚本、CI closeout 与 backlog 输入，不放宽现有结构或覆盖率门禁。

## Forge closeout 证据

每个需要 PR 的切片都记录同一组字段，便于 Ultragoal/PR fan-in 判断是否真的完成：

- `target_forge`、`remote`、`base_ref`、`head_ref`
- `pr.number`、`pr.url`、`pr.state`
- `ci.status`：只允许 `passed`、`running`、`failed`、`skipped`、`blocked`
- `merge.status`：只允许 `merged`、`skipped`、`blocked`
- `merge.method`、`merge.title`、`merge.commit`
- `divergence.status` 与必要说明

本地归一化命令：

```powershell
node scripts/governance-normalizers.mjs forge-evidence --input .omx\ultragoal\evidence\forge-input.json
```

`.gitea/workflows/ci.yml` 的 `repo-governance` job 会运行 `node scripts/check-governance-normalizers.mjs`，确保归一化合同持续可执行。

`ci.status=passed` 需要指向当前 PR/head commit 对应的 Gitea Actions 成功 run；只创建 PR、启用 auto-merge 或存在非关联成功 run 都不能算完成。

## Changed-line coverage

Rust full-runtime baseline 仍是：

```powershell
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
```

业务源码改动还需要按 changed executable line 计算 90% 目标覆盖率。归一化命令接受统一 diff 与 lcov：

```powershell
git -c gc.auto=0 diff --unified=0 main...HEAD > .omx\ultragoal\evidence\changed.diff
node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx\ultragoal\evidence\changed.diff --threshold 90
```

输出中的 `status` 只代表 changed-line 目标是否达标；如果本切片只改 docs/scripts/static governance，可在 PR 风险区明确跳过业务覆盖率并说明原因。

LCOV 只记录可执行 Rust 行。完整文件经保守解析确认仅包含属性、导入、模块/常量、宏及 `struct`/`enum`/`union` 数据声明时，归一化器将其标记为 `declaration_only_source`，不要求不存在的 LCOV 文件记录；文件中一旦出现 `fn`、`impl` 或其他未识别运行时代码仍会 fail closed，并继续要求 coverage 匹配。

## Structure gate queue

结构门禁失败是 backlog 输入，不是 baseline 放宽理由。当前 Rust 与前端结构输出可归一化为仓库根相对路径：

```powershell
node scripts/check-rust-backend-structure.mjs *> .omx\ultragoal\evidence\rust-structure.txt
Push-Location src\web
npm run structure:check *> ..\..\.omx\ultragoal\evidence\frontend-structure.txt
Pop-Location
node scripts/governance-normalizers.mjs structure-queue --rust-output .omx\ultragoal\evidence\rust-structure.txt --frontend-output .omx\ultragoal\evidence\frontend-structure.txt
```

`frontend-structure` 输出中的 `src/...` 会归一化为 `src/web/src/...`，避免与后端 `src/...` 混淆。
