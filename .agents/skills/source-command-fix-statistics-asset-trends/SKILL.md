---
name: "source-command-fix-statistics-asset-trends"
description: "排查并修复 Bill Analyser 资产趋势相关问题。适用于 asset-trends 接口时间范围异常、返回结构不匹配、前端图例包含空账户、净值/总资产/总负债图表空白或聚合错误。"
---

# source-command-fix-statistics-asset-trends

Use this skill when the user asks to run the migrated source command `fix-statistics-asset-trends`.

## Command Template

排查并修复当前工作区中的资产趋势问题。

优先参考 `AGENTS.md`、`docs/PROJECT_OVERVIEW.md` 与 `.agents/skills/bill-analyser-conventions/SKILL.md`，但本次任务只聚焦"资产趋势"这一条链路，不扩散到无关的分类统计、趋势统计或预算模块。

## 输入

- 用户提供的症状描述
- 当前工作区代码
- 如果相关，当前统计页路由参数、截图描述或报错信息

## 任务要求

1. 先判断问题属于哪一类：
   - `/api/statistics/asset-trends` 后端返回错误
   - services/store 映射错误
   - `assetTrendsDataWithAccountInfo` 或 `assetTrendsData` 聚合错误
   - 页面图例、图表或筛选交互错误
2. 明确数据边界：
   - 后端返回了什么字段
   - 前端服务层转换成了什么结构
   - store 最终给图表的 `items` 是什么
3. 优先修根因，不做无依据的页面层隐藏或假数据兜底。
4. 如果问题涉及空账户图例、净值或总资产/总负债，优先检查 store 聚合和过滤逻辑。
5. 完成修改后验证接口、store 结果和 statistics 页资产趋势模式，而不是只看代码表面正确。

## 输出格式

按这个结构完成任务：

1. 问题阶段
2. 根本原因
3. 修改文件
4. 验证结果
5. 剩余风险

## 最少检查项

- `src/bill_analyser/api/routes/statistics.py`
- `src/web/src/lib/services.ts`
- `src/web/src/stores/statistics.ts`
- `src/web/src/views/desktop/statistics/TransactionPage.vue`
- 如有必要，检查 `src/bill_analyser/core/db.py` 或相关汇率/账户模型

## 完成标准

- 资产趋势关键字段返回和映射正常
- 图例不显示无数据账户
- 资产趋势模式下图表和筛选交互正常
- 前端构建通过；如改到后端，同时跑相关 statistics 测试
