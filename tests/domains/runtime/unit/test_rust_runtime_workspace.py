from __future__ import annotations

import re
import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[4]


def _load_toml(path: str) -> dict:
    with (REPO_ROOT / path).open("rb") as file:
        return tomllib.load(file)


def test_rust_workspace_declares_internal_runtime_crates() -> None:
    workspace = _load_toml("Cargo.toml")["workspace"]

    assert workspace["resolver"] == "2"
    assert workspace["members"] == [
        "crates/bill-analyser-core",
        "crates/bill-analyser-db",
        "crates/bill-analyser-http",
        "crates/bill-analyser-parsers",
    ]


def test_core_crate_keeps_rust_as_internal_library_boundary() -> None:
    manifest = _load_toml("crates/bill-analyser-core/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-core"
    assert manifest["lib"]["name"] == "bill_analyser_core"
    assert manifest["package"]["publish"] is False
    assert "pyo3" not in manifest.get("dependencies", {})


def test_http_crate_declares_rust_primary_proxy_runtime() -> None:
    manifest = _load_toml("crates/bill-analyser-http/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-http"
    assert manifest["lib"]["name"] == "bill_analyser_http"
    assert manifest["package"]["publish"] is False
    assert "axum" in manifest["dependencies"]
    assert "reqwest" in manifest["dependencies"]
    assert "pyo3" not in manifest.get("dependencies", {})


def test_parser_crate_declares_pure_parser_contract_boundary() -> None:
    manifest = _load_toml("crates/bill-analyser-parsers/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-parsers"
    assert manifest["lib"]["name"] == "bill_analyser_parsers"
    assert manifest["package"]["publish"] is False
    assert set(manifest["dependencies"]) == {
        "bill-analyser-core",
        "serde",
        "serde_json",
    }
    assert "pyo3" not in manifest.get("dependencies", {})
    assert "rusqlite" not in manifest.get("dependencies", {})
    assert "axum" not in manifest.get("dependencies", {})


def test_architecture_docs_record_rust_primary_http_and_import_runtime_gates() -> None:
    architecture = (REPO_ROOT / "docs/overview-architecture.md").read_text(encoding="utf-8")
    migration_plan = (REPO_ROOT / "docs/rust-backend-migration-plan.md").read_text(encoding="utf-8")
    project_overview = (REPO_ROOT / "docs/PROJECT_OVERVIEW.md").read_text(encoding="utf-8")
    api_routes = (REPO_ROOT / "docs/overview-api-routes.md").read_text(encoding="utf-8")

    assert "Rust HTTP 主入口" in architecture
    assert "`BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`" in architecture
    assert "`BILL_ANALYSER_API_PORT=5001`" in architecture
    assert "`BILL_ANALYSER_PYTHON_UPSTREAM=http://127.0.0.1:5001`" in architecture
    assert "`api_takeover=true`" in architecture
    assert "Python/Flask sidecar" in architecture
    assert "`BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE=import_route_skeleton`" in architecture
    assert "默认 `BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE=import_db_runtime`" in architecture
    assert "migration governance oracle" in architecture
    assert "当前治理矩阵把第一阶段导入与预览决策旁路、核心账单 CRUD、交易图片上传/未使用清理、账单 CSV/XLSX 导出、账单 recurring candidates/match、账户对账单、账单分类 quick actions、账户 CRUD/display-order/余额同步/交易迁移清空、标签 CRUD/display-order/batch-create、分类主数据/分类账单统计/批量重分类、legacy 分类规则 config/cache、分类规则列表/创建/更新/删除/重排/defaults/migrate/测试、规则中心概览、设置包导入/预览/导出、预算 CRUD/export/execution/forecast/history/import、DB-backed statistics read routes 与 statistics exchange routes 标为 Rust-owned" in architecture
    assert "分类规则列表 `GET /api/category-rules/`、创建/更新/删除/重排/defaults/migrate、测试 `POST /api/category-rules/{rule_id}/test`、规则中心概览 `GET /api/rules/overview`、批量重分类 `POST /api/categories/update-all`、legacy `GET|PUT /api/categories/rules` config/cache 与设置包导入/预览/导出当前由 Rust taxonomy runtime" in architecture
    assert "旧 Flask `category_rules.py` route shell 已删除" in architecture
    assert "同时把统计 Analyzer overview/trends/comparison/category/trend、真实 LLM/OCR provider 生成与全局 Learning Center suggestion/rules 决策标为 Python-proxied，OAuth2 provider exchange 在当前 workspace build 中是 Rust-owned disabled-safe/not-implemented 合同" in architecture
    assert "显式 `import_db_runtime` 已能用 Rust 处理 import v2 JSON parse/parse_generic/dedup/confirm、前端 FormData CSV 上传解析、未匹配文件 temp preview 与列映射 parse_generic、session/preview 读取清理、preview update/reclassify 显式 DB 更新、preview-item transfer/recurring 决策、learning 会话预览旁路、LLM accept/reject/memory 事件、OCR config app_settings、账单 CRUD、交易图片上传/未使用清理、账单 CSV/XLSX 导出、账单 recurring candidates/match、账户对账单、账单分类 quick actions、账户 CRUD/display-order/余额同步/交易迁移清空、标签 CRUD/display-order/batch-create、分类主数据与 `GET /api/categories/statistics`/`POST /api/categories/update-all`、legacy `GET|PUT /api/categories/rules` config/cache、分类规则创建/更新/删除/重排/defaults/migrate、设置包导入/预览/导出、预算 CRUD/export/execution/forecast/history/import，以及 category statistics/trends、asset trends、category pie、top merchants、amounts 统计读取与 exchange-rate provider/custom-rate runtime" in architecture
    assert "Rust DB writer policy 为 `RustDomainOwned`" in architecture
    assert "Rust import DB runtime 可校验前端 Bearer access token" in architecture
    assert "S1 Rust Runtime Shell" in migration_plan
    assert "S1b Opt-in Rust HTTP Ingress" in migration_plan
    assert "S2a Ownership Matrix, Envelope Oracle, and DB Writer Policy" in migration_plan
    assert "S3a Import Route Skeleton" in migration_plan
    assert "S4a Import Staging DB Primitives" in migration_plan
    assert "S5a Parser Template Staging DB Primitives" in migration_plan
    assert "S5b Confirm Preview-to-Bills DB Primitive" in migration_plan
    assert "S5c StandardBill-to-Parser-Template Staging Adapter" in migration_plan
    assert "S5d Parser-Template to Preview Staging Adapter" in migration_plan
    assert "S6a Preview Update, Reclassify, and Annotation DB Semantics" in migration_plan
    assert "S7a Preview-Item Transfer and Recurring Decision DB Semantics" in migration_plan
    assert "S8a Preview Learning Decision DB Semantics" in migration_plan
    assert "S8b Preview LLM Recommendation and Memory DB Semantics" in migration_plan
    assert "S8c OCR Config App Settings DB Semantics" in migration_plan
    assert "S9a Rust Import Route Runtime Wiring" in migration_plan
    assert "S9b Preview Update and Reclassify Runtime Wiring" in migration_plan
    assert "S9c Preview-Item, Learning, and LLM Runtime Wiring" in migration_plan
    assert "S9d Import Parse, Dedup, Confirm, and Frontend Upload Runtime Wiring" in migration_plan
    assert "S10 Rust Primary HTTP Runtime and Frontend Auth Bridge" in migration_plan
    assert "Python import route disabling/deletion is blocked unless all five evidence gates pass" in migration_plan
    assert "business_migration=import-route-skeleton-no-db" in migration_plan
    assert "`BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE=import_db_runtime`" in architecture
    assert "business_migration=import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime" in migration_plan
    assert "S15b.1 makes Rust the runtime owner for tag batch creation" in migration_plan
    assert "S15c makes Rust the runtime owner for category master-data routes" in migration_plan
    assert "S15c.1 makes Rust the runtime owner for `GET /api/categories/statistics`" in migration_plan
    assert "S15c.2 makes Rust the runtime owner for `GET /api/category-rules/`, `POST /api/category-rules/{rule_id}/test`, and `GET /api/rules/overview`" in migration_plan
    assert "Category rule create/update/delete/reorder/defaults/migrate is now also Rust-owned for ordinary SQLite runtime writes" in migration_plan
    assert "S15c.3 makes Rust the runtime owner for settings bundle export" in migration_plan
    assert "S15c.4 makes Rust the runtime owner for settings bundle import and preview" in migration_plan
    assert "S15c.5 makes Rust the runtime owner for `POST /api/categories/update-all`" in migration_plan
    assert "S15c.6 makes Rust the runtime owner for legacy `GET|PUT /api/categories/rules`" in migration_plan
    assert "S15c.7 makes Rust the runtime owner for `GET /api/settings/encryption/status`" in migration_plan
    assert "设置加密状态 `GET /api/settings/encryption/status` 已由 Rust taxonomy/settings runtime 直接响应" in api_routes
    assert "Rust taxonomy settings runtime 还直接负责无鉴权 `GET /api/settings/encryption/status`" in project_overview
    assert "标签 list/get/create/update/delete/display-order/batch-create 路由由 Rust HTTP taxonomy runtime 直接接管" in api_routes
    assert "旧 Flask `tags.py` route shell 已删除" in api_routes
    assert "旧 Flask `tags.py` route shell 已删除" in project_overview
    assert "批量创建 `POST /api/tags/batch` 仍由 Python 处理" not in api_routes
    assert "S10 switches the local startup boundary from Python-primary to Rust-primary HTTP" in migration_plan
    assert "BILL_ANALYSER_PYTHON_UPSTREAM=http://127.0.0.1:5001" in migration_plan
    assert "Authorization: Bearer" in migration_plan
    assert "导入 session、preview、parser template staging、StandardBill-to-parser-template adapter、parser-template-to-preview staging adapter、selected preview confirm-to-bills、preview update/reclassify、preview-item transfer/recurring/learning decision、learning promotion、preview LLM recommendation/memory event、OCR config app_settings 与 annotation sample" in project_overview
    assert "Rust 已成为主 HTTP 服务入口" in project_overview
    assert "可用前端 Bearer access token 校验 HS256/HS384/HS512 HMAC JWT" in project_overview
    assert "Rust 路由、DB 写入语义、前端导入流程、全量 coverage、无残留引用五项证据未同时通过前，Python import 删除仍阻塞" in project_overview
    assert "preview update/reclassify、preview-item transfer/recurring/learning decision、learning promotion、preview LLM recommendation/memory event、OCR config app_settings、annotation sample upsert/read" in architecture
    assert "当前通过显式 `import_db_runtime` 暴露 import v2 JSON parse/parse_generic/dedup/confirm、前端 FormData CSV 上传解析、未匹配文件 temp preview 与列映射 parse_generic、session/preview 读取清理、preview update/reclassify 显式 DB 更新、preview-item transfer/recurring 决策、learning 会话预览旁路与 promotion、LLM accept/reject/memory 事件、OCR config app_settings、账单 CRUD、交易图片上传/未使用清理、账单 CSV/XLSX 导出、账单 recurring candidates/match、账户对账单、账单分类 quick actions、账户 CRUD/display-order/余额同步/交易迁移清空、标签 CRUD/display-order/batch-create、分类主数据、分类账单统计、批量重分类、legacy 分类规则 config/cache、分类规则创建/更新/删除/重排/defaults/migrate、设置包导入/预览/导出、模板 CRUD/display-order、预算 CRUD/export/execution/forecast/history/import、统计读取与汇率 REST" in architecture
    assert "The S9b HTTP tests cover single preview-row edit persistence" in migration_plan
    assert "The S9c HTTP tests cover transfer accept projection" in migration_plan
    assert "The S9d HTTP tests cover JSON parse -> parse_generic -> dedup -> confirm DB writes" in migration_plan
    assert "Python-proxied provider generation routes, and Python-proxied global Learning Center suggestion/rule routes" in migration_plan
    assert "不接管任何业务 API" in migration_plan
    assert "proxied Python routes are not Rust business-owned" in migration_plan


def test_migration_governance_matrix_tracks_current_import_preview_adjacent_python_routes() -> None:
    expected_routes = set()
    bills_routes_dir = REPO_ROOT / "src/bill_analyser/api/routes/bills"
    for path in bills_routes_dir.glob("*.py"):
        for method, route_path in _extract_flask_route_pairs(path):
            if "/import" in route_path or route_path == "/parse_import":
                expected_routes.add((method, _normalize_route_pattern(f"/api/bills{route_path}")))

    for method, route_path in _extract_flask_route_pairs(REPO_ROOT / "src/bill_analyser/api/routes/learning.py"):
        expected_routes.add((method, _normalize_route_pattern(f"/api/learning{route_path}")))

    for method, route_path in _extract_flask_route_pairs(REPO_ROOT / "src/bill_analyser/api/routes/receipt_ocr.py"):
        if route_path.startswith("/receipt-recognition"):
            expected_routes.add((method, _normalize_route_pattern(f"/api/ml{route_path}")))

    for method, route_path in _extract_flask_route_pairs(REPO_ROOT / "src/bill_analyser/api/routes/llm/preview.py"):
        expected_routes.add((method, _normalize_route_pattern(f"/api/llm{route_path}")))
    for method, route_path in _extract_flask_route_pairs(REPO_ROOT / "src/bill_analyser/api/routes/llm/analysis.py"):
        if route_path in {"/analyze-transactions", "/rule-synthesis"}:
            expected_routes.add((method, _normalize_route_pattern(f"/api/llm{route_path}")))

    matrix_routes = _extract_rust_governance_routes()
    missing_routes = sorted(expected_routes - matrix_routes)

    assert not missing_routes


def _extract_flask_route_pairs(path: Path) -> set[tuple[str, str]]:
    source = path.read_text(encoding="utf-8")
    pairs: set[tuple[str, str]] = set()
    for match in re.finditer(
        r'@bp\.route\(\s*"(?P<route>[^"]+)"\s*,\s*methods=\[(?P<methods>[^\]]+)\]\s*\)',
        source,
    ):
        methods = re.findall(r'"([A-Z]+)"', match.group("methods"))
        pairs.update((method, match.group("route")) for method in methods)
    return pairs


def _normalize_route_pattern(route_path: str) -> str:
    return re.sub(r"<(?:int:)?([A-Za-z_][A-Za-z0-9_]*)>", r"{\1}", route_path)


def _extract_rust_governance_routes() -> set[tuple[str, str]]:
    source = (REPO_ROOT / "crates/bill-analyser-core/src/migration_governance.rs").read_text(encoding="utf-8")
    pairs: set[tuple[str, str]] = set()
    for match in re.finditer(r"EndpointOwnership\s*\{(?P<body>.*?)\n\s*\}", source, flags=re.S):
        body = match.group("body")
        method_match = re.search(r'method:\s*"([^"]+)"', body)
        pattern_match = re.search(r'pattern:\s*"([^"]+)"', body)
        if method_match and pattern_match:
            pairs.add((method_match.group(1), pattern_match.group(1)))
    return pairs


def test_auth_bridge_runtime_is_built_before_backend_pytest_and_startup() -> None:
    start_backend = (REPO_ROOT / "start_backend.ps1").read_text(encoding="utf-8")
    gitea_ci = (REPO_ROOT / ".gitea/workflows/ci.yml").read_text(encoding="utf-8")

    assert 'Package = "bill-analyser-core"' in start_backend
    assert 'Bin = "bill_auth_bridge"' in start_backend
    assert "$env:BILL_ANALYSER_RUST_AUTH_BRIDGE" in start_backend
    assert 'Package = "bill-analyser-db"' in start_backend
    assert 'Bin = "bill_taxonomy_bridge"' in start_backend
    assert "$env:BILL_ANALYSER_RUST_TAXONOMY_BRIDGE" in start_backend
    assert 'Package = "bill-analyser-http"' in start_backend
    assert 'Bin = "bill_http_server"' in start_backend
    assert "$env:BILL_ANALYSER_RUST_HTTP_SERVER" in start_backend
    assert "$env:BILL_ANALYSER_HTTP_BIND = \"127.0.0.1:5000\"" in start_backend
    assert "$env:BILL_ANALYSER_API_PORT = [string]$PythonFallbackPort" in start_backend
    assert "$env:BILL_ANALYSER_PYTHON_UPSTREAM" in start_backend
    assert "$DotenvPath" in start_backend
    assert "JWT_SECRET_KEY" in start_backend
    assert "JWT_ALGORITHM" in start_backend
    assert "Set-RustAuthEnvFromServerConfig" in start_backend
    assert "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION" in start_backend
    assert "BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION" in start_backend
    assert "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH" in start_backend
    assert "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_UPPERCASE" in start_backend
    assert "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS" in start_backend
    assert "Test-PythonFallbackHealth" in start_backend
    assert "Python fallback port already in use" in start_backend
    assert "Start-Process" in start_backend
    assert "-WindowStyle Hidden" in start_backend
    assert "Error: Rust toolchain not found" in start_backend
    assert "Configured Rust $($BridgeSpec.DisplayName) bridge not found" in start_backend
    assert "https://github.com/dtolnay/rust-toolchain@stable" in gitea_ci
    assert "cargo build -p bill-analyser-core --bin bill_auth_bridge" in gitea_ci
    assert "cargo build -p bill-analyser-db --bin bill_taxonomy_bridge" in gitea_ci
    assert "cargo build -p bill-analyser-http --bin bill_http_server" in gitea_ci
    assert "python scripts/check_rust_workspace_dependencies.py --json" in gitea_ci
    assert "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90" in gitea_ci
    assert "python -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v -n auto --dist loadfile" in gitea_ci
