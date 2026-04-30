---
name: Fix Bill Import Preview
description: '排查并修复 Bill Analyser 导入预览页问题。适用于预览表格字段为空、金额或类型显示错误、编辑交互异常、重新分类不刷新、选择性导入失效、取消未清理会话。'
argument-hint: '描述导入预览问题，例如"预览表格全空但后端 preview 有数据"'
---

排查并修复当前工作区中的账单导入预览问题。

优先参考 `AGENTS.md`、`docs/PROJECT_OVERVIEW.md` 与 `.agents/skills/bill-analyser-conventions/SKILL.md`，但本次任务聚焦"前端导入预览"这一件事，不扩散到无关模块。

## 输入

- 用户提供的症状描述
- 当前工作区代码
- 如果相关，当前选中的文件或正在打开的导入预览文件

## 任务要求

1. 先判断问题属于哪一类：
   - 后端 preview 数据生成错误
   - `ImportDialog.vue` 转换错误
   - `ImportTransactionCheckDataTab.vue` 渲染或编辑逻辑错误
   - 重新分类、选择性导入、取消清理等交互逻辑错误
2. 明确数据边界：
   - 后端返回了什么字段
   - 前端转换成了什么模型
   - 表格和编辑态实际读取了什么字段
3. 只修根因，不做无依据的前端兜底映射。
4. 如果涉及类型、分类、账户、目标金额联动，确保相关字段一起校正。
5. 完成修改后验证相关链路，而不是只看代码表面正确。

## 输出格式

按这个结构完成任务：

1. 问题阶段
2. 根本原因
3. 修改文件
4. 验证结果
5. 剩余风险

## 最少检查项

- `src/web/src/views/desktop/transactions/import/ImportDialog.vue`
- `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue`
- `src/web/src/models/transaction.ts`
- 如有必要，检查 `src/bill_analyser/core/bill_service.py` 和 `src/bill_analyser/api/routes/bills.py`

## 完成标准

- 预览页关键字段显示正常
- 编辑交互不崩溃
- 重新分类、选择性导入或取消清理中与本问题相关的行为正确
- 前端构建通过；如改到后端，同时跑相关导入测试
