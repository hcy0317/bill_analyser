# Bill Analyser 数据库与数据流

## 6.1 主要业务表（由 `db.py` façade 通过 `DatabaseSchemaMixin` 初始化）
- 交易域：`bills`
- 匹配域：`bill_pair_links`、`bill_pair_feedback`
- 分类域：`categories`
- 账户域：`accounts`、`account_types`、`account_transfers`
- 标签域：`tags`、`bill_tags`
- 模板域：`bill_templates`、`recurring_bills`
  - 普通文件库上的模板主数据 CRUD、排序、DTO 列表/详情和启用周期模板读取由 `core/template_rust_bridge.py` 调用 Rust `bill_taxonomy_bridge` 的 templates repository；settings bundle taxonomy sections 的 normalization、导出 DTO 构建和模板引用解析由 `core/settings_bundle_rust_bridge.py` 调用 Rust helper，最终 import SQL 仍留在 Python 的同一个 `aiosqlite` 事务里以保留 preview rollback / full import atomicity；`:memory:`、SQLCipher、settings bundle category-rule/LLM/OCR import、recurring suggestion/import-flow 写路径，以及 recurring 匹配/绑定推进仍走 Python。
- 预算域：`budgets`、`budget_history`
  - `import_db_runtime` 下预算 CRUD/export/execution/forecast/history/snapshot/import 已由 Rust `crates/bill-analyser-db/src/budgets.rs` 访问普通应用 SQLite 文件，保持 `user_id` 隔离、yuan-style numeric 金额、父子预算自动上卷、月度到季度/年度父周期同步、旧 `categories.type=1` 支出归一、列表 category metadata 解析、export DTO、execution 只读聚合的日期窗口交集/账户标签过滤/`abs(sum(amount))`、forecast 历史窗口扩展/period grouping/当前周期花费/预算 primary-sub-total 映射/backtest MAPE、history canonical `filter_summary` 精确快照优先与 on-demand fallback、snapshot 对 `budget_history` 的同 user/filter/period replacement 写入，以及 import 按 `name + user_id` 更新或插入预算、单事务提交、逐项错误计数和默认 `period_type/alert_threshold/enabled` 处理。Rust `crates/bill-analyser-core/src/budgets.rs` 继续固定期间、金额口径、父子预算、历史过滤、导入校验和预测计算合同。
- 用户与安全：`users`、`sessions`、`auth_logs`、`audit_logs`、`user_two_factor_recovery_codes`
- 备份与恢复：`backup_records`、`backup_jobs`
  - `audit_logs`、`backup_records`、`backup_jobs` 的实际写入仍由 Python `core/database/audit_backup/` 在 `aiosqlite` 事务中执行；Rust `ops.rs` 固定备份记录投影、cleanup record-first 决策、用户数据清理审计 payload 和 backup job 默认值/校验合同。
- 导入三阶段：`import_sessions`、`bills_parser_template`、`bills_preview`
- 导入三阶段临时表当前还会持久化解析器元标签：`bills_parser_template.parser_tags_json` 保存解析阶段 tags，`bills_preview.preview_parser_tags_json` 保存预览阶段 tags
- 迁移与索引补齐由 schema 子模块统一编排；运行态调用方不直接依赖某个单独 schema 文件
- 领域持久化代码统一位于 `core/database/**`：runtime/shared/time/encryption 维护基础层，`schema/` 维护 schema 编排，accounts/bills/budgets/imports/llm/users 等语义 package 继续导出原 mixin 名称；`core/` 根层只保留 `db.py` 作为公共 Database façade
- Rust DB runtime foundation 位于 `crates/bill-analyser-db`，提供 SQLite 连接 guard、WAL/foreign_keys PRAGMA 初始化、事务 helper、schema dry-run scaffold 与 user_id 参数化 scope；`import_db_runtime` 下账单/交易核心 CRUD 已经由 Rust `bills` repository 写入普通应用 SQLite 文件，create/update/delete/batch mutations 在单个事务内覆盖 `bills`、`bill_tags`、hash/duplicate 语义、pair/suppression cleanup 与受影响账户余额重算，仍显式保持前端分与 DB 元的边界；预算 CRUD/export/import 已由 Rust `budgets` repository 写入普通应用 SQLite 文件，create/update/delete 在单个事务内覆盖预算行、一级预算补全/上卷、季度/年度父预算补全与列表 category metadata，import 在单个事务内按 `name + user_id` upsert 并累计 item-level errors，预算 execution/forecast/history/snapshot 由同一 repository 聚合或写入 `budgets`、`budget_history`、`bills`、`bill_tags` 和 `categories`；统计读取由 Rust `statistics` repository 读取 `bills/accounts/categories`，生成 category statistics/trends、asset trends、category pie、top merchants 和 amounts 的 Flask-compatible payload。Flask/Python Database façade 仍保留未迁移域入口。账户 master-data 的 list/get/create/update/delete/subAccounts/display-order 在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::accounts`，`:memory:`、SQLCipher 加密库、账户交易迁移/清空和账户审计动作继续使用 Python 路径。标签 master-data 的 list/get/create/update/delete/display-order 在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::tags`，`:memory:` 与 SQLCipher 加密库继续使用 Python aiosqlite 路径；核心账单 CRUD 的 `bill_tags` 关联写入由 Rust bills runtime 维护，其他标签域操作仍由 Python 标签 mixin 维护。分类 master-data 的 list/get/create/batch-ensure/update/delete/按主分类删除/按主分类批量改名在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::categories`，并保留 categories 为空时从 bills 派生分类的旧 fallback；settings bundle taxonomy section helper 位于 Rust `taxonomy::settings_bundle`，提供纯 JSON 规范化/引用解析/导出投影，不直接提交设置包导入事务；`:memory:`、SQLCipher、分类规则/匹配和设置包非 taxonomy import/upsert 继续使用 Python 路径。

## 6.1.1 模板域当前漂移清单
- 前端期望字段：`templateType/categoryId/sourceAccountId/destinationAccountId/sourceAmount/destinationAmount/hideAmount/tagIds/displayOrder/hidden/scheduled*`。
- 后端当前字段：`category/account/tag/description/is_favorite/use_count/last_used_at` 等旧模板语义。
- 结果：已通过 DTO → 数据库映射完成首轮收口，但模板双表语义仍比账户/标签复杂，兼容层清理应继续分阶段推进。

## 6.2 导入处理顺序（核心）
1. 解析器识别并标准化账单
2. 数据验证
3. 智能去重
4. 分类匹配（规则+类型过滤）
5. 账户匹配（源/目标账户）
6. 预览写入或正式入库
