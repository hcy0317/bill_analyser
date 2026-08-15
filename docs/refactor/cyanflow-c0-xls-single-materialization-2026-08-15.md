# Bill Analyser C0 表格单次物化验收

> 状态：`WORKTREE_VALIDATED / EXACT_MAIN_PENDING`
> 日期：2026-08-15
> 工作流：Cyaness -> Cyanflow `flow / performance / affected_surface`
> 基准主线：`ff29b7ebaa3173bb9869e0f12a47e873ac7f3d90`
> 切片：`C0-xls-single-materialization`

## 1. 结论

本切片消除了 dedicated parser 对同一 HTML/XLS/XLSX 文件的重复完整物化。selection 在文件级创建一个只读 `PreparedSpreadsheet`，所有 hinted 与 remaining parser 候选复用同一份 rows，仍完整执行 exactly-one/no_match/conflict 判定。

XLSX dedicated 路径先用现有受限 XML scanner 校验 ZIP entry、展开字节、worksheet、dimension、单元格和物化字节预算，再由 calamine 完整物化一次 rows，以保持旧 dedicated parser 的稳定单元格文本合同。generic spreadsheet preview/列映射继续使用原有受限 XML 物化，不改变其行为。

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
- exact-main：PENDING；合并后必须重新构建并运行。
- C0 退出：PENDING；仍需连续三次 exact-main `<=60,000ms` 及识别质量 gate。
