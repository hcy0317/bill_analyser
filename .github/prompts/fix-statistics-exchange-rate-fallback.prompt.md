---
name: Fix Statistics Exchange Rate Fallback
description: '排查并修复 Bill Analyser 统计域的汇率获取、用户自定义汇率、provider 回退链、内置汇率兜底，以及所有依赖汇率的 statistics 展示问题。适用于 exchange-rates 接口失败、总是回退到内置汇率、自定义汇率不生效、SSL 或 provider 解析异常、以及统计页金额展示未使用最终汇率结果。'
argument-hint: '描述汇率或依赖汇率的统计展示问题，例如“统计页总是使用内置汇率而不是用户自定义汇率”'
agent: 'agent'
---

排查并修复当前工作区中的 statistics 汇率链路，以及所有依赖汇率结果的统计展示问题。

优先参考 `AGENTS.md`、`docs/PROJECT_OVERVIEW.md` 与 `.agents/skills/bill-analyser-conventions/SKILL.md`，但本次任务只聚焦“汇率链路 + 依赖汇率的 statistics 展示”。不要扩散到无关的预算、账单导入或纯样式问题。

## 输入

- 用户提供的症状描述
- 当前工作区代码
- 如果相关，当前请求参数、日志片段、返回 JSON 或页面表现

## 任务要求

1. 先判断问题属于哪一类：
   - `/api/statistics/exchange-rates` 或相关 route 返回错误
   - 用户自定义汇率没有优先生效
   - provider 抓取失败、解析失败或 SSL 失败
   - provider 全部失败后没有正确回退到内置汇率
   - 前端 services/store/页面没有正确消费最终汇率结果
   - 某些统计展示仍然使用旧汇率、默认汇率或未换算金额
2. 明确回退链路：
   - 是否先命中用户自定义汇率
   - 是否正确进入 provider 抓取流程
   - provider 失败后是否回退到内置汇率
   - 返回结果里是否仍包含 `baseCurrency=1.0` 和必要币种
3. 明确消费链路：
   - `services.ts` 如何转换汇率结果
   - `stores/statistics.ts` 如何保存并分发汇率相关状态
   - 哪些 statistics 页面、图表或摘要金额依赖这份汇率结果
4. 优先修根因，不要只在前端吞掉错误提示、硬编码汇率，或只改单个展示点掩盖统一换算错误。
5. 如果问题涉及 SSL 或第三方提供者异常，优先检查 Rust statistics runtime/provider 集成的 provider 顺序、证书处理和解析逻辑。
6. 完成修改后验证接口返回、回退顺序和所有依赖汇率的 statistics 展示，而不是只做静态代码修改。

## 输出格式

按这个结构完成任务：

1. 问题阶段
2. 根本原因
3. 修改文件
4. 验证结果
5. 剩余风险

## 最少检查项

- `src/backend/http/statistics_routes.rs`
- `src/backend/db/statistics.rs`
- `src/backend/core/statistics.rs`
- `src/web/src/lib/services.ts`
- `src/web/src/stores/statistics.ts`
- `src/web/src/views/desktop/statistics/TransactionPage.vue`
- 如有必要，检查具体统计图表或汇率展示组件

## 完成标准

- 用户自定义汇率优先级正确
- provider 抓取与回退顺序正确
- provider 全部失败时仍能正确回退到内置汇率
- 返回结果包含 `baseCurrency=1.0` 和必要汇率字段
- statistics 页所有依赖汇率的金额展示都使用最终生效的汇率结果
- 不存在“接口已回退成功但页面仍显示旧值或错误换算结果”的残留问题
