# Bill Analyser 项目总览

多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析全栈系统。后端运行入口仍是 Python/Flask/aiosqlite，账户/标签/分类/模板、设置包 taxonomy helper 与分类规则表达式编译等路径已逐步通过内部 Rust crates/bridge 接管，导入/学习/matching/recurring 等路径也在 Rust crate 中固定合同层；前端 Vue 3/TypeScript/Vite，数据库 SQLite WAL 模式。

## 目录

- [项目定位](overview-positioning.md) — 系统目标、核心数据源（微信/支付宝/多家银行）
- [总体架构](overview-architecture.md) — 分层结构（API/Core/Database/Frontend）、异步桥接、REST 主链模式
- [后端模块](overview-backend.md) — 按功能域拆分的 API 路由包、Core 业务逻辑、解析器工厂、Database façade 与 mixin 包结构
- [前端模块](overview-frontend.md) — 视图（desktop/mobile）、Pinia stores、统一 axios 服务层、设置 JSON 导入导出入口
- [API 路由与 REST 收口](overview-api-routes.md) — Flask 蓝图注册、账户/标签/模板/预算/账单/分类/设置包/OCR 配置各域 REST 收口进展
- [导入链路](overview-import.md) — v2 三阶段导入、预览校验交互、图片 OCR 回填、学习域（corpus/dual-head model/overlay）、LLM 推荐
- [Matching 域](overview-matching.md) — 转账/投资/learning 配对候选、generic accept/reject/clear、manual pair 管理、reconcile-history
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST、用户数据管理、legacy 收口条目
- [认证与安全](overview-auth-security.md) — 认证/2FA/token/backup/step-up/OAuth2 全链路 REST 收口与审计
- [数据库与数据流](overview-database.md) — 主要业务表、导入三阶段临时表、导入处理顺序
- [日志与运维](overview-ops.md) — 统一日志入口、启停脚本
- [测试结构](overview-testing.md) — 测试分布、结构体量门禁、推荐验证命令
