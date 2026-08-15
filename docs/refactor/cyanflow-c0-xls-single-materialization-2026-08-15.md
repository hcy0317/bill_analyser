# Bill Analyser C0 表格单次物化验收

> 状态：`EXACT_MAIN_VALIDATED / PERFORMANCE_GATE_PASS / RECOGNITION_GATE_PENDING`
> 日期：2026-08-15
> 工作流：Cyaness -> Cyanflow `flow / performance / affected_surface`
> 基准主线：`ff29b7ebaa3173bb9869e0f12a47e873ac7f3d90`
> 切片：`C0-xls-single-materialization`
> 合并：PR #281 -> `main@80269554153b21f122fa03b4d9a9484134439e8c`

## 1. 结论

本切片消除了 dedicated parser 对同一 HTML/XLS/XLSX 文件的重复完整物化。selection 在文件级创建一个只读 `PreparedSpreadsheet`，所有 hinted 与 remaining parser 候选复用同一份 rows，仍完整执行 exactly-one/no_match/conflict 判定。

XLSX dedicated 路径先用现有受限 XML scanner 校验 ZIP entry、展开字节、worksheet、dimension、单元格和物化字节预算，再由 calamine 完整物化一次 rows，以保持旧 dedicated parser 的稳定单元格文本合同。generic spreadsheet preview/列映射继续使用原有受限 XML 物化，不改变其行为。

PR #281 已通过精确 head CI 并 squash 合并。合并后的同一 `main` SHA 连续三次完成真实 64 文件浏览器验收，行数与识别结果完全一致，首屏均低于 60,000ms；C0 性能门禁因此关闭。整体 C0 Exit 仍等待独立的识别质量门禁，不因性能通过而提前放行。

## 2. 行为一致性

同一 64 文件 corpus 在旧 main 与本切片之间执行脱敏逐文件、逐字段 parser 输出指纹对比：

| 合同 | 旧 main | 本切片 |
| --- | ---: | ---: |
| 文件数 | 64 | 64 |
| 解析行数 | 15,041 | 15,041 |
| dedup 后行数 | 13,082 | 13,082 |
| 未匹配文件 | 0 | 0 |
| 输出指纹不同文件 | - | 0 |

对比覆盖 parser id、账单数量及全部序列化字段；诊断脚本和 detached worktree 已在验证后删除，不进入仓库。

## 3. 性能

worktree 真实浏览器 run `c0-xls-worktree-parity-20260815-2` 使用相同 corpus manifest `0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838`：

| 指标 | 旧基线 | 本切片 | 变化 |
| --- | ---: | ---: | ---: |
| 首屏可操作 | 151,627ms | 12,221ms | -91.94% |
| Stage 1 total | 136,865ms | 4,116ms | -96.99% |
| Stage 1 parser | 132,883ms | 1,600ms | -98.80% |
| Stage 1 staging | 3,291ms | 2,162ms | -34.31% |
| Stage 2 total | 13,899ms | 7,568ms | -45.55% |

本次 worktree run 已低于 60,000ms 总目标，但不能替代 exact-main 连续验收。

合并后对 `main@80269554153b21f122fa03b4d9a9484134439e8c` 连续执行三次 exact-main：

| Run | Parsed | Preview | Unmatched | Stage 1 parser | Stage 1 total | Stage 2 total | 首屏可操作 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `c0-exact-main-8026955-1-20260815` | 15,041 | 13,082 | 0 | 1,581ms | 4,254ms | 7,763ms | 12,560ms |
| `c0-exact-main-8026955-2-20260815` | 15,041 | 13,082 | 0 | 3,395ms | 7,578ms | 14,480ms | 22,860ms |
| `c0-exact-main-8026955-3-20260815` | 15,041 | 13,082 | 0 | 3,133ms | 7,055ms | 13,566ms | 21,416ms |

三次运行均使用 64 文件 corpus manifest `0a3b595c268413d7f404d823c6a0d0456d0e81b655e47c3c3f92e412b2c4c838`，并完成 strict cleanup、独立数据库删除和 Weaviate run-scoped cleanup。最慢首屏为 22,860ms，距离 60,000ms 门槛仍有 37,140ms 余量。

## 4. 安全与资源边界

- XLS 仍先执行 OLE/BIFF preflight，再进入单次 calamine rows 物化。
- XLSX 仍先执行 ZIP/dimension/cell/byte 全量安全扫描；校验通过后只进行一次完整 calamine rows 物化。
- HTML 仍由受限 HTML parser 单次物化。
- 多文件 blocking worker 仍按文件顺序等待，不在本切片引入压缩表格并发。
- hostile spreadsheet 合同、64 MiB 请求级输出预算和失败关闭错误码保持。
- run-scoped PostgreSQL、业务数据、Weaviate collection 与浏览器认证状态均严格清理成功。

## 5. Gate

- parser crate：PASS，30 unit + 22 contract。
- 64 文件输出 parity：PASS，64/64 文件逐字段一致。
- worktree 真实浏览器：PASS，首屏 12,221ms。
- exact-main：PASS，连续三次首屏为 12,560ms / 22,860ms / 21,416ms。
- C0 性能退出：PASS，三次均 `<=60,000ms`，最慢运行保留 37,140ms 余量。
- C0 识别质量：PENDING，仍有 5 个 partial 与 1 个 missing，必须由后续独立切片关闭。
- C0 整体退出：PENDING，不进入 C1。
