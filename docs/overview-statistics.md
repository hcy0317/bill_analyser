# 统计与汇率

统计链路由 Rust route 读取 PostgreSQL 中的账户、分类、账单、预算和汇率数据，并返回前端图表 DTO。

覆盖范围：

- 金额概览
- 分类统计
- 分类趋势
- 资产趋势
- 分类饼图
- 商户排行
- Analyzer overview/trends/comparison/category/trend
- insights anomaly data
- 汇率 provider fallback
- 用户自定义汇率

金额聚合使用数据库 minor units 字段，返回前端前按 API 契约转换。汇率链路优先使用用户自定义汇率，其次 provider fallback，再到内置兜底值。
