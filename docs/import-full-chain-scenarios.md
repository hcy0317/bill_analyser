# 导入全链路验收场景

本页记录当前导入链路的可复跑验收场景。运行态只有 PostgreSQL authority + 必需 Weaviate。

## 场景矩阵

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| parser-first 上传 | 单一微信/支付宝/银行卡样本 | 命中唯一 dedicated parser，生成标准账单行 |
| parser 冲突 | 同一文件命中多个 parser | 返回 unmatched 文件并携带 parser decision 证据 |
| stage2 规则匹配 | 已存在分类规则与账户规则 | 预览行填充分类、账户、转账账户和信号 |
| Weaviate recall | 确定性 learning 未命中且向量索引 ready | 返回 yellow learning 建议，不直接改写人工字段 |
| preview 编辑 | 修改分类、账户、类型或转账字段 | 更新当前 session 的预览状态并刷新待处理信号 |
| confirm | 选中预览行确认 | 在 PostgreSQL 事务中创建或更新账单并同步账户余额 |

## 验收命令

```powershell
cargo test -p bill-analyser-core import_pipeline_contracts
cargo test -p bill-analyser-http --lib import_routes
Set-Location src\web
npm run test:coverage
```
