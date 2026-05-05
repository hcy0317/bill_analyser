# Bill Analyser 测试结构与质量门禁

## 8.1 测试分布（`tests/`）
- 核心测试：`test_import.py`、`test_smart_dedup.py`、`test_category_engine_v2.py`、`test_db.py`
- API 测试：`test_v1_routes.py`、`test_statistics_*`、`new_ui/` 下接口测试
- 领域化测试：`tests/domains/` 采用 `domain/(unit|integration)` 结构，当前已覆盖认证、统计、导入主链、数据库深水区等高价值路径
- 回归脚本/诊断脚本：`check_*`、`diagnose_*`、`debug_*`
- 前端 Jest 测试：`tests/web/`（与 Python 测试同仓库级根目录并行管理）

## 8.2 推荐验证命令
- 单元/集成：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pytest tests/ -v`
- 后端结构门禁：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe scripts/backend_structure_gate.py`
  - 该 gate 扫描 `src/bill_analyser/**/*.py`，对新增未知超大文件/函数直接失败，并用 `scripts/backend_structure_baseline.json` 约束历史热点不能继续膨胀。
- 代码质量：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pylint src/bill_analyser/core src/bill_analyser/api/routes`
