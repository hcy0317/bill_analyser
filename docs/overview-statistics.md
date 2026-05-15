# 统计与汇率

统计域由 Rust runtime 和 Rust repository 直接处理。
HTTP route 层在 `statistics_routes/` 下按读取、Analyzer、汇率处理、汇率 provider 解析和查询/响应 helper 拆分；provider 解析保持纯函数测试，不依赖真实网络。

## API

- `GET /api/statistics/category-statistics`
- `GET /api/statistics/category-statistics/trends`
- `GET /api/statistics/asset-trends`
- `GET /api/statistics/category-pie`
- `GET /api/statistics/top-merchants`
- `GET /api/statistics/amounts`
- `GET /api/statistics/overview`
- `GET /api/statistics/trends`
- `GET /api/statistics/comparison`
- `GET /api/statistics/category`
- `GET /api/statistics/trend`
- `GET /api/insights/anomalies`
- `GET /api/statistics/exchange-rates`
- `PUT /api/statistics/exchange-rates/custom`
- `DELETE /api/statistics/exchange-rates/custom/{currency}`

## 数据边界

- 统计读取按当前用户过滤 bills/accounts/categories。
- 汇率 custom rate 写入按当前用户隔离。
- 金额聚合必须保持元/分语义明确，面向前端的字段按既有 contract 输出。
