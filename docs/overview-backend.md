# 后端模块

后端由 Rust workspace 承载，主入口是 `src/backend/http/bin/bill_http_server.rs`。

## HTTP

`src/backend/http` 负责 Axum route、认证上下文、request DTO、response envelope、上传处理、runtime state、route ownership 和业务 facade。handler 只编排请求，不直接散落复杂 SQL。

## Core

`src/backend/core` 负责金额、时间、分类、预算、统计、导入、matching、LLM/OCR、认证和备份的业务合同。跨 route 的规则先放在 core，再由 db/http 调用。

## DB

`src/backend/db` 负责 SQLx PostgreSQL pool、schema scripts、repositories、transactions、user scope、import staging、settings bundle、backup metadata 和 vector outbox。所有业务 repository 都以当前用户为边界。

## Parsers

`src/backend/parsers` 负责 dedicated parser 检测和标准账单归一化。导入 route 只接收 parser 输出，不在 HTTP 层重复解析账单业务字段。
