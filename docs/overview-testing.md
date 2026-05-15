# Bill Analyser 测试结构与质量门禁

## 8.1 测试分布（`tests/`）
- Rust workspace 测试：`crates/*/tests/` 覆盖当前主 HTTP/DB/core/parser runtime 合同
- 核心测试：`test_import.py`、`test_smart_dedup.py`、`test_category_engine_v2.py`、`test_db.py`
- API 测试：`test_v1_routes.py`、`test_statistics_*`、`new_ui/` 下接口测试
- 领域化测试：`tests/domains/` 采用 `domain/(unit|integration)` 结构，当前已覆盖认证、统计、导入主链、数据库深水区等高价值路径
- 回归脚本/诊断脚本：`check_*`、`diagnose_*`、`debug_*`
- 前端 Jest 测试：`tests/web/`（与 Python 测试同仓库级根目录并行管理）
- 前端-Rust 集成契约：`tests/web/contracts/frontendRustRouteContract.test.ts` 会校验由 Rust route ownership 生成的前端 fixture 未过期，并扫描 Vue/TS axios/fetch 调用，确保当前前端 `/api/...` 请求不依赖 `PythonProxied` 或 `/api/v1/*` 路由。

## 8.2 推荐验证命令
- Rust 主后端：
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- 残留 Python sidecar 单元/集成：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pytest tests/ -v -n auto --dist loadfile`
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v -n auto --dist loadfile`
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/scripts/trim_ci_caches.ps1`
- 后端结构门禁：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe scripts/backend_structure_gate.py`
  - 该 gate 扫描 `src/bill_analyser/**/*.py`，对新增未知超大文件/函数直接失败，并用 `scripts/backend_structure_baseline.json` 约束历史热点不能继续膨胀。
- 代码质量：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pylint src/bill_analyser/core src/bill_analyser/api/routes`
- 前端契约：
  - `Set-Location src/web; node scripts/generate-rust-route-fixture.mjs --check`
  - `Set-Location src/web; npx jest --runTestsByPath ../../tests/web/contracts/frontendRustRouteContract.test.ts --maxWorkers=50%`
