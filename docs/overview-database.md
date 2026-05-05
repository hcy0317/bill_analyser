# Bill Analyser 数据库与数据流

## 6.1 主要业务表（由 `db.py` façade 通过 `DatabaseSchemaMixin` 初始化）
- 交易域：`bills`
- 匹配域：`bill_pair_links`、`bill_pair_feedback`
- 分类域：`categories`
- 账户域：`accounts`、`account_types`、`account_transfers`
- 标签域：`tags`、`bill_tags`
- 模板域：`bill_templates`、`recurring_bills`
- 预算域：`budgets`、`budget_history`
- 用户与安全：`users`、`sessions`、`auth_logs`、`audit_logs`、`user_two_factor_recovery_codes`
- 备份与恢复：`backup_records`、`backup_jobs`
- 导入三阶段：`import_sessions`、`bills_parser_template`、`bills_preview`
- 导入三阶段临时表当前还会持久化解析器元标签：`bills_parser_template.parser_tags_json` 保存解析阶段 tags，`bills_preview.preview_parser_tags_json` 保存预览阶段 tags
- 迁移与索引补齐由 schema 子模块统一编排；运行态调用方不直接依赖某个单独 schema 文件
- 领域持久化代码统一位于 `core/database/**`：runtime/shared/time/encryption 维护基础层，`schema/` 维护 schema 编排，accounts/bills/budgets/imports/llm/users 等语义 package 继续导出原 mixin 名称；`core/` 根层只保留 `db.py` 作为公共 Database façade
- Rust DB runtime foundation 位于 `crates/bill-analyser-db`，提供 SQLite 连接 guard、WAL/foreign_keys PRAGMA 初始化、事务 helper、schema dry-run scaffold 与 user_id 参数化 scope；Flask/Python Database façade 仍是业务运行时入口。账户 master-data 的 list/get/create/update/delete/subAccounts/display-order 在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::accounts`，`:memory:`、SQLCipher 加密库、余额同步、账户交易迁移/清空和账户审计动作继续使用 Python 路径。标签 master-data 的 list/get/create/update/delete/display-order 在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::tags`，`:memory:` 与 SQLCipher 加密库继续使用 Python aiosqlite 路径，`bill_tags` 账单关联读写仍由 Python 标签 mixin 维护。分类 master-data 的 list/get/create/batch-ensure/update/delete/按主分类删除/按主分类批量改名在普通文件 SQLite 库上通过 `bill_taxonomy_bridge` 调用 Rust `taxonomy::categories`，并保留 categories 为空时从 bills 派生分类的旧 fallback；`:memory:`、SQLCipher、分类统计、分类规则/匹配和设置包分类 import/upsert 继续使用 Python 路径。

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
