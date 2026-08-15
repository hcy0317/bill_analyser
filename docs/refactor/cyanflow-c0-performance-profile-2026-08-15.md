# Bill Analyser C0 导入性能 Profile

> 状态：`PROFILE_CAPTURED / C0_EXIT_BLOCKED`
> 日期：2026-08-15
> 工作流：Cyaness -> Cyanflow `flow / performance / affected_surface`
> 基准主线：`ed401d1894f35c74c2e47a099831cf3b4c4bc7fc`
> 范围：Stage 1/2 运行态观测、真实 64 文件 browser profile 与下一优化边界

## 1. 结论

当前 64 文件导入的首要瓶颈不是 API 上传、PostgreSQL staging、preview GET 或前端渲染，而是 Stage 1 dedicated parser。正常日志级别的 instrumentation worktree 运行中：

| 指标 | 耗时 | 占比/解释 |
| --- | ---: | --- |
| 首屏可操作 | 151,627ms | C0 的 60,000ms 目标仍失败 |
| Stage 1 total | 136,865ms | browser parse 完成前的主等待窗口 |
| Stage 1 parser | 132,883ms | Stage 1 的 97.09% |
| Stage 1 multipart | 43ms | 不是主瓶颈 |
| Stage 1 PostgreSQL staging | 3,291ms | Stage 1 的 2.40% |
| Stage 2 total | 13,899ms | dedup 6,599ms、intelligence 1,292ms、preview insert 4,180ms |
| preview GET | 348ms / 444ms | 不是首屏主瓶颈 |

同一进程、同一 corpus 的前一次 Stage 1 为 71,850ms，紧接着的第二次为 136,865ms，放大 1.90 倍；Stage 2 同时从 7,682ms 放大到 13,899ms。性能不仅不达标，而且重复运行稳定性不足。

## 2. 调用链证据

codebase-memory 与当前源码共同确认以下运行链：

```text
browser upload
  -> POST /api/bills/import/v2/parse
  -> import_parse_multipart_runtime_response
  -> parse_multipart_import_files_bounded
  -> parse_dedicated_import_bytes_with_decision
  -> validate_dedicated_spreadsheet_payload
  -> hinted parser + remaining registry parsers
  -> stage_import_parser_templates_with_sources
  -> POST /api/bills/import/v2/dedup
  -> import_dedup_runtime_handler
  -> preview page
```

结构性根因有三层：

1. `parse_multipart_import_files_bounded` 在 `for` 循环内对每个 `spawn_blocking` 立即 `await`，64 个文件保持严格串行；该边界保护压缩表格内存，但目前没有按格式或预算拆分调度策略。
2. spreadsheet 文件先由 `validate_dedicated_spreadsheet_payload` 打开并遍历一次；进入具体 parser 后又通过 `sheet_or_html_rows` 再次打开和物化。
3. auto hint 缩小候选后，selection 为保留 exactly-one/conflict 语义仍继续运行其余 registry parser；二进制 XLS 的非目标 parser 无法从原始字节轻量排除，因而各自再次打开同一工作簿。

debug 级全模块日志运行不用于性能基线：它产生 1MiB 以上日志并改变计时。当前结论只采用默认日志级别的标准 `Server-Timing` 和已脱敏 info summary。

## 3. 本切片观测合同

Stage 1 parse 成功响应以标准 `Server-Timing` 暴露：

```text
multipart;dur=..., parser;dur=..., staging;dur=..., total;dur=...
```

Stage 2 dedup 成功响应暴露：

```text
dedup;dur=..., intelligence;dur=..., preview-insert;dur=..., response;dur=..., total;dur=...
```

真实 64 文件 Playwright flow 解析并校验必需 metric，把结构化数值写入 run-scoped `.cyaness/evidence`。这些 header 只承担 HTTP 观测职责，不参与识别、筛选、validity、confirm 或持久化分支。

## 4. 下一优化切片

下一切片冻结为 `C0-xls-single-materialization`：

1. 对受限 HTML/XLS/XLSX 输入建立一次请求内 immutable prepared spreadsheet；预检、source detection 与命中 parser 共享同一份受预算约束的 rows。
2. registry 仍检查 exactly-one/no_match/conflict；不得因为 filename/content hint 命中就跳过冲突判定。
3. 不并发展开多个压缩工作簿；若后续引入并发，必须先有 spawn 前内存 permit、取消与背压合同。
4. CSV/TXT 快路径、原始金额/日期、parser tags、64 MiB 请求物化预算和 hostile spreadsheet fail-closed 行为保持。
5. 优化先以 parser crate fixture 和真实 64 文件 browser flow 证明 recognition parity，再比较性能。

## 5. Gate

- Profile 证据：PASS。
- 根因归属：PASS，主导项为 spreadsheet dedicated parser 的重复解析/物化。
- 60 秒目标：FAIL。
- 连续三次 exact-main `<=60,000ms`：未开始。
- recognition fixture：仍有 5 个 partial 与 1 个 missing，C2 cutover 继续阻断。
- C1：不放行；先完成 `C0-xls-single-materialization` 并重跑 exact-main。

机器可读证据：`docs/refactor/evidence/cyanflow-c0-performance-profile-2026-08-15.json`。
