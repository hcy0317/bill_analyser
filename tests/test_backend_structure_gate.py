"""Regression tests for the backend structure size gate."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
GATE_PATH = REPO_ROOT / "scripts" / "backend_structure_gate.py"


def _load_gate_module():
    spec = importlib.util.spec_from_file_location("backend_structure_gate", GATE_PATH)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_backend_structure_gate_current_checkout_passes_with_baseline() -> None:
    gate = _load_gate_module()

    result = gate.run_gate(REPO_ROOT)

    assert result.ok, [finding.message for finding in result.failures]
    assert result.files_scanned > 0


def test_backend_structure_gate_rejects_unknown_large_route(tmp_path: Path) -> None:
    gate = _load_gate_module()
    route_dir = tmp_path / "src" / "bill_analyser" / "api" / "routes"
    route_dir.mkdir(parents=True)
    (route_dir / "oversized.py").write_text("\n".join(f"line_{idx} = {idx}" for idx in range(510)), encoding="utf-8")
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "file-hard-limit" for finding in result.failures)


def test_backend_structure_gate_classifies_database_packages() -> None:
    gate = _load_gate_module()

    assert gate._categorize(Path("core/db.py")) == "database"
    assert gate._categorize(Path("core/database/runtime.py")) == "database"
    assert gate._categorize(Path("core/database/bills/mixin.py")) == "database"


def test_backend_structure_gate_classifies_ai_packages() -> None:
    gate = _load_gate_module()

    assert gate._categorize(Path("core/ai/llm/provider.py")) == "core_service"
    assert gate._categorize(Path("core/ai/ocr/service.py")) == "core_service"


def test_backend_structure_gate_classifies_domain_packages() -> None:
    gate = _load_gate_module()

    assert gate._categorize(Path("core/bills/service_parts/facade.py")) == "core_service"
    assert gate._categorize(Path("core/budgets/manager.py")) == "core_service"
    assert gate._categorize(Path("core/investment/matching.py")) == "core_service"


def test_backend_structure_gate_rejects_legacy_db_root_artifacts(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "db_legacy.py").write_text("VALUE = 1\n", encoding="utf-8")
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-db-root-artifact" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_db_imports(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "service.py").write_text(
        "from bill_analyser.core.db_bills import DatabaseBillsMixin\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-db-import" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_relative_db_imports(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "service.py").write_text(
        "from .db_bills import DatabaseBillsMixin\nfrom .. import db_runtime\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-db-import" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_ai_root_artifacts(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "llm_provider.py").write_text("VALUE = 1\n", encoding="utf-8")
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-ai-root-artifact" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_ai_imports(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "service.py").write_text(
        "from bill_analyser.core.ocr_service import normalize_ocr_config\n"
        "from .llm_provider import ProviderFactory\n"
        "from .. import llm_learning_service\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-ai-import" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_ai_relative_imports_inside_ai_package(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    ai_ocr_dir = tmp_path / "src" / "bill_analyser" / "core" / "ai" / "ocr"
    ai_ocr_dir.mkdir(parents=True)
    (ai_ocr_dir / "bad.py").write_text(
        "from ...ocr_service import normalize_ocr_config\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-ai-import" for finding in result.failures)


def test_backend_structure_gate_allows_ai_package_local_relative_imports(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    ai_ocr_dir = tmp_path / "src" / "bill_analyser" / "core" / "ai" / "ocr"
    ai_ocr_dir.mkdir(parents=True)
    (ai_ocr_dir / "service.py").write_text(
        "from .payment_screenshot_parser import parse_payment_screenshot_text\n",
        encoding="utf-8",
    )
    (ai_ocr_dir / "payment_screenshot_parser.py").write_text(
        "def parse_payment_screenshot_text(text):\n    return text\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert result.ok, [finding.message for finding in result.failures]


def test_backend_structure_gate_ignores_legacy_ai_import_text_in_strings(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "notes.py").write_text(
        '"""Example: from bill_analyser.core.ocr_service import normalize_ocr_config"""\n',
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert result.ok, [finding.message for finding in result.failures]


def test_backend_structure_gate_rejects_legacy_domain_root_artifacts(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "bill_service.py").write_text("VALUE = 1\n", encoding="utf-8")
    (core_dir / "bill_service_parts").mkdir()
    (core_dir / "budget.py").write_text("VALUE = 1\n", encoding="utf-8")
    (core_dir / "budget_execution_summary.py").write_text("VALUE = 1\n", encoding="utf-8")
    (core_dir / "investment_matching.py").write_text("VALUE = 1\n", encoding="utf-8")
    (core_dir / "investment_settings.py").write_text("VALUE = 1\n", encoding="utf-8")
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-domain-root-artifact" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_domain_imports(tmp_path: Path) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "service.py").write_text(
        "from bill_analyser.core.bill_service import BillService\n"
        "from bill_analyser.core.budget import BudgetManager\n"
        "from bill_analyser.core.investment_matching import score_investment_candidate\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-domain-import" for finding in result.failures)


def test_backend_structure_gate_rejects_legacy_domain_relative_imports(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    service_parts_dir = tmp_path / "src" / "bill_analyser" / "core" / "bills" / "service_parts"
    service_parts_dir.mkdir(parents=True)
    (service_parts_dir / "bad.py").write_text(
        "from ...investment_settings import build_user_investment_keyword_settings\n"
        "from ...bill_service_parts.common import StandardBill\n",
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert not result.ok
    assert any(finding.code == "legacy-domain-import" for finding in result.failures)


def test_backend_structure_gate_allows_domain_package_local_relative_imports(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    service_parts_dir = tmp_path / "src" / "bill_analyser" / "core" / "bills" / "service_parts"
    service_parts_dir.mkdir(parents=True)
    (service_parts_dir / "facade.py").write_text(
        "from .common import StandardBill\n",
        encoding="utf-8",
    )
    (service_parts_dir / "common.py").write_text("StandardBill = dict\n", encoding="utf-8")
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert result.ok, [finding.message for finding in result.failures]


def test_backend_structure_gate_ignores_legacy_domain_import_text_in_strings(
    tmp_path: Path,
) -> None:
    gate = _load_gate_module()
    core_dir = tmp_path / "src" / "bill_analyser" / "core"
    core_dir.mkdir(parents=True)
    (core_dir / "notes.py").write_text(
        '"""Example: from bill_analyser.core.bill_service import BillService"""\n',
        encoding="utf-8",
    )
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps({"files": {}, "functions": {}}), encoding="utf-8")

    result = gate.run_gate(tmp_path, baseline_path)

    assert result.ok, [finding.message for finding in result.failures]
