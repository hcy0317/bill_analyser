"""适配层不再被上层代码直接以 legacy 命名引用的静态回归测试。"""

from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
SRC_ROOT = WORKSPACE_ROOT / 'src'

LEGACY_CLASS_MARKERS = (
    'V1TransactionAdapter',
    'V1AccountAdapter',
    'V1CategoryAdapter',
)
LEGACY_MODULE_MARKERS = (
    'src.api.adapters.v1_adapter',
    'src.api.adapters.v1_account_adapter',
    'src.api.adapters.v1_category_adapter',
)

ALLOWED_CLASS_FILES = {
    'src/bill_analyser/api/adapters/v1_adapter.py',
    'src/bill_analyser/api/adapters/v1_account_adapter.py',
    'src/bill_analyser/api/adapters/v1_category_adapter.py',
}

ALLOWED_MODULE_IMPORT_FILES = {
    'src/bill_analyser/api/adapters/account_adapter.py',
    'src/bill_analyser/api/adapters/category_adapter.py',
    'src/bill_analyser/api/adapters/transaction_adapter.py',
    'src/bill_analyser/api/adapters/v1_adapter.py',
    'src/bill_analyser/api/adapters/v1_account_adapter.py',
    'src/bill_analyser/api/adapters/v1_category_adapter.py',
}


def _scan_hits(markers: tuple[str, ...], allowed_files: set[str]) -> list[tuple[str, int, str]]:
    """扫描 Python 源码中不允许出现的标记。"""
    hits: list[tuple[str, int, str]] = []

    for file_path in SRC_ROOT.rglob('*.py'):
        relative_path = file_path.relative_to(WORKSPACE_ROOT).as_posix()
        if relative_path in allowed_files:
            continue

        content = file_path.read_text(encoding='utf-8')
        for line_number, line in enumerate(content.splitlines(), start=1):
            if any(marker in line for marker in markers):
                hits.append((relative_path, line_number, line.strip()))

    return hits


def test_no_upper_layers_use_legacy_adapter_class_names_directly():
    """除 legacy 实现文件外，不应再直接引用 V1*Adapter 类名。"""
    hits = _scan_hits(LEGACY_CLASS_MARKERS, ALLOWED_CLASS_FILES)

    assert not hits, '发现上层代码仍直接引用 legacy 适配器类名: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )


def test_only_neutral_adapter_modules_import_legacy_adapter_modules():
    """除中性出口与 legacy 实现文件外，不应再直接导入 v1_* adapter 模块。"""
    hits = _scan_hits(LEGACY_MODULE_MARKERS, ALLOWED_MODULE_IMPORT_FILES)

    assert not hits, '发现非适配器出口代码仍直接导入 legacy adapter 模块: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )
