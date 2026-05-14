# Bill Analyser 统计与汇率

## 5.3 统计与汇率
- 统计主链当前统一由 `GET /api/statistics/*` 提供
- 当前前端统计主链已切到：
  - `GET /api/statistics/category-statistics`
  - `GET /api/statistics/category-statistics/trends`
  - `GET /api/statistics/asset-trends`
- `crates/bill-analyser-core/src/statistics.rs` 当前固定统计/汇率/净值/洞察/日历的 Rust 合同层，覆盖反向区间 400、分/元换算、分类/账户聚合、资产趋势余额、空账户图例过滤、汇率 provider/custom/fallback、净值分组、洞察异常和日历 projection shape；`import_db_runtime` 已由 Rust 直接执行 DB-backed statistics read 与汇率 REST，旧 Flask statistics read/exchange route shell 已删除，统计 Analyzer report/chart 仍由 Python Flask/aiosqlite 保留。
- 移动端统计分析页使用移动专用 ECharts 饼图组件展示分类占比，容器和饼图半径按手机视口放大，避免沿用桌面或旧 SVG 尺寸导致图表过小。
- 旧 `transaction-statistics*` 兼容子路径已删除，当前通过 legacy 404 回归测试防止恢复
- 汇率主链已切到 `GET /api/statistics/exchange-rates`
- 用户自定义汇率写接口已收口到统计域 REST：`PUT /api/statistics/exchange-rates/custom`、`DELETE /api/statistics/exchange-rates/custom/<currency>`；旧 `v1/exchange_rates/user_custom/update.json` 与 `v1/exchange_rates/user_custom/delete.json` 已停止使用，并由 legacy 404 回归保护
- `GET /api/statistics/exchange-rates` 在用户存在自定义汇率时优先返回 `dataSource=user_custom` 的持久化结果，否则由 Rust 汇率 runtime 按 provider 候选获取实时汇率并在失败时返回内置 fallback
- Flask sidecar 中 `/api/statistics/category-statistics*`、`/asset-trends`、`/category-pie`、`/top-merchants`、`/amounts` 与 `/exchange-rates*` 不再注册；Analyzer 路由 `/overview`、`/trends`、`/comparison`、`/category`、`/trend` 仍由 Python sidecar 提供
- 用户资料页"数据统计"已切到认证域 REST 主链 `GET /api/data/statistics`，旧 `v1/data/statistics.json` 已删除并由 legacy 404 回归保护
- 用户数据管理已继续收口到认证域 REST：`GET /api/data/export.csv`、`GET /api/data/export.tsv`、`POST /api/data/clear/transactions`、`POST /api/data/clear/all`；旧 `v1/data/export.csv`、`v1/data/export.tsv`、`v1/data/clear/transactions.json`、`v1/data/clear/all.json` 已停止使用，并由 legacy 404 回归保护
- 系统版本检查已切到 `GET /api/system/version`；旧 `v1/systems/version.json` 已停止使用，并由 legacy 404 回归保护
- 导入解析辅助已切到 `POST /api/bills/parse_import`；前端 `services.ts` 不再使用旧 `v1/transactions/parse_import.json`
- 账单写入/导入辅助旧 rewrite 已移除：`v1/transactions/add.json`、`v1/transactions/modify.json`、`v1/transactions/delete.json`、`v1/transactions/import.json`、`v1/transactions/reconciliation_statements.json` 当前应返回 404
- 分类写入旧 rewrite 已移除：`v1/transaction/categories/list.json`、`v1/transaction/categories/add.json`、`v1/transaction/categories/add_batch.json` 当前应返回 404
- 导入提交前端已统一走 `POST /api/bills/import/v2/confirm`；旧 `v1/transactions/import/process.json` 与 `v1/transactions/parse_dsv_file.json` 已停止使用，并由 legacy 404 回归保护
- AI 小票识图前端已切到 `POST /api/ml/receipt-recognition`；后端通过 `GET/PUT /api/ml/receipt-recognition/config` 持久化当前 OCR provider/lang 配置，默认 disabled 时继续返回 `501 provider_unconfigured`，启用 `tesseract` 后按内存即用即弃语义识别并返回金额(元)、时间、描述与置信度；旧 `v1/llm/transactions/recognize_receipt_image.json` 已停止使用，并由 legacy 404 回归保护
- 交易列表前端主链已切到 `GET /api/bills/`；按月列表主链已切到 `GET /api/bills/by-month`
- 旧 `v1/transactions/list.json` 与 `v1/transactions/list/by_month.json` 已移除，并由 legacy 404 回归保护
- 交易图片前端主链已切到 `POST /api/bills/pictures` 与 `POST /api/bills/pictures/unused`
- 旧 `v1/transaction/pictures/upload.json` 与 `v1/transaction/pictures/remove_unused.json` 已停止使用，并由 legacy 404 回归保护
- 当前运行态前端源码（`src/web/src`）已不再直接引用 `v1/` 或 `/api/v1/` 路径；legacy 兼容仅保留在后端兼容层与历史 `ezbookkeeping/` 快照中
- `src/bill_analyser/api/app.py` 中历史 `URLRewriteMiddleware` 已移除；当前运行态不再通过 WSGI rewrite 兼容任何 `/api/v1/*` 路径
