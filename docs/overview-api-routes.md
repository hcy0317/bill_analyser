# Bill Analyser API 路由与 REST 收口

## 5.1 Flask 蓝图注册（`src/bill_analyser/api/app.py`）
- RESTful 蓝图：
  - `/api/bills`
  - `/api/accounts`
  - `/api/categories`
  - `/api/matching`
  - `/api/statistics`
  - `/api/tags`
  - `/api/templates`
  - `/api/budgets`
  - `/api/backup`
- `app.py` 直接提供 ML 小票识图入口：`POST /api/ml/receipt-recognition` 与 `GET/PUT /api/ml/receipt-recognition/config`

## 5.1.1 当前 REST 收口进展（2026-03-06）
- 账户域首批 legacy action 已收口到 REST：
  - `PUT /api/accounts/<id>` 支持 `hidden` 可见性更新
  - `PUT /api/accounts/display-orders` 支持批量排序
  - 子账户删除统一通过 `DELETE /api/accounts/<id>`
  - `POST /api/accounts/<id>/transactions/move` 支持账户间批量迁移交易
  - `POST /api/accounts/<id>/transactions/clear` 支持按账户清空交易
- 标签域首批 legacy action 已收口到 REST：
  - `PUT /api/tags/<id>` 同时承担内容更新与 `hidden` 可见性更新
  - `POST /api/tags/batch` 支持批量创建
  - `PUT /api/tags/display-orders` 支持批量排序
- 前端 `services.ts` 与标签 store 已移除对应直连 v1 排序路径，改为统一走 REST 入口
- 模板域已完成契约梳理，但被重新评估为非低风险域：
  - 该域已完成首轮收口：前端模板调用统一走 REST，后端补齐 `templateType` 过滤、双表 DTO 映射、隐藏/排序与 `user_id` 收口；
  - 模板 rewrite 映射、`templates.bp_v1` 注册与实现均已移除；
  - 模板域当前仅保留 REST 主链路与对应回归测试。
- 预算域主链已完成 REST 收口：
  - 前端 `services.ts` 已将列表/详情/创建/更新/删除/执行统计/预测/导入导出统一切换到 `/api/budgets/*`；
  - 服务层新增预算 REST ↔ 前端旧结构兼容映射，避免直接重写 `budget store` 与页面；
  - `db.py` 已为预算 `forecast/import/export` 补齐 `user_id` 隔离；
  - `app.py` 已移除 `budgets.bp_v1` 注册；
  - `src/bill_analyser/api/routes/budgets.py` 内预算旧 `bp_v1` 实现已完成物理删除；
  - 已补 `tests/new_ui/test_budgets_rest_api.py` 覆盖 REST 主链与 legacy 404 回归。
- 账户域历史 rewrite 与旧 `/get` `/modify` `/hide` `/delete` `/move` 兼容路由已移除。
- 标签域 `bp_v1` 注册与全部 v1 兼容实现已移除。
- 账单/分类/账户路由层已完成一轮适配器收敛：
  - `bills.py` 已拆分基础上下文与 adapter 上下文，非转换型路由不再默认构造交易适配器；
  - `accounts.py` / `categories.py` / `bills.py` 已统一改从中性适配器模块导入。
