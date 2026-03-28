"""阻止 legacy adapter 壳文件与旧命名在运行时代码树中回归的静态回归测试。"""

import ast
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
SRC_ROOT = WORKSPACE_ROOT / 'src'
RUNTIME_SRC_ROOT = SRC_ROOT / 'bill_analyser'

LEGACY_ADAPTER_FILES = (
    'src/bill_analyser/api/adapters/v1_adapter.py',
    'src/bill_analyser/api/adapters/v1_account_adapter.py',
    'src/bill_analyser/api/adapters/v1_category_adapter.py',
)

LEGACY_CLASS_MARKERS = (
    'V1TransactionAdapter',
    'V1AccountAdapter',
    'V1CategoryAdapter',
    'V1ResponseBuilder',
)
LEGACY_MODULE_NAMES = (
    'bill_analyser.api.adapters.v1_adapter',
    'bill_analyser.api.adapters.v1_account_adapter',
    'bill_analyser.api.adapters.v1_category_adapter',
)
LEGACY_MODULE_BASENAMES = ('v1_adapter', 'v1_account_adapter', 'v1_category_adapter')
LEGACY_ADAPTER_PACKAGE = 'bill_analyser.api.adapters'

ALLOWED_CLASS_FILES: set[str] = set()


def _iter_runtime_python_files():
    """遍历运行时代码树中的 Python 文件。"""
    yield from RUNTIME_SRC_ROOT.rglob('*.py')


def _read_lines(file_path: Path) -> list[str]:
    """读取源码文件并按行返回，便于拼接错误信息。"""
    return file_path.read_text(encoding='utf-8').splitlines()


def _module_name_for(file_path: Path) -> str:
    """根据 src 下的相对路径推导 Python 模块名。"""
    return '.'.join(file_path.relative_to(SRC_ROOT).with_suffix('').parts)


def _relative_import_target(file_path: Path, node: ast.ImportFrom) -> str:
    """解析 ImportFrom 节点的绝对模块名。"""
    current_package_parts = _module_name_for(file_path).split('.')[:-1]
    if node.level:
        base_parts = current_package_parts[: len(current_package_parts) - node.level + 1]
    else:
        base_parts = []

    if node.module:
        base_parts = [*base_parts, *node.module.split('.')]

    return '.'.join(base_parts)


def _scan_legacy_symbol_hits(markers: tuple[str, ...], allowed_files: set[str]) -> list[tuple[str, int, str]]:
    """通过 AST 扫描运行时代码中对 legacy 标识符的直接引用。"""
    hits: list[tuple[str, int, str]] = []

    for file_path in _iter_runtime_python_files():
        relative_path = file_path.relative_to(WORKSPACE_ROOT).as_posix()
        if relative_path in allowed_files:
            continue

        source_lines = _read_lines(file_path)
        tree = ast.parse('\n'.join(source_lines), filename=str(file_path))
        for node in ast.walk(tree):
            marker = None
            if isinstance(node, ast.Name) and node.id in markers:
                marker = node.id
            elif isinstance(node, ast.Attribute) and node.attr in markers:
                marker = node.attr

            if marker is None:
                continue

            line_number = getattr(node, 'lineno', 1)
            line_text = source_lines[line_number - 1].strip()
            hits.append((relative_path, line_number, line_text or marker))

    return hits


def _scan_legacy_import_hits() -> list[tuple[str, int, str]]:
    """通过 AST 扫描运行时代码中对 legacy 模块的导入。"""
    hits: list[tuple[str, int, str]] = []

    for file_path in _iter_runtime_python_files():
        relative_path = file_path.relative_to(WORKSPACE_ROOT).as_posix()
        source_lines = _read_lines(file_path)
        tree = ast.parse('\n'.join(source_lines), filename=str(file_path))

        for node in ast.walk(tree):
            line_number = getattr(node, 'lineno', 1)
            line_text = source_lines[line_number - 1].strip()

            if isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name in LEGACY_MODULE_NAMES:
                        hits.append((relative_path, line_number, line_text or alias.name))
            elif isinstance(node, ast.ImportFrom):
                module_name = _relative_import_target(file_path, node)
                if module_name in LEGACY_MODULE_NAMES:
                    hits.append((relative_path, line_number, line_text or module_name))
                    continue

                if module_name != LEGACY_ADAPTER_PACKAGE:
                    continue

                if any(alias.name in LEGACY_MODULE_BASENAMES for alias in node.names):
                    hits.append((relative_path, line_number, line_text or module_name))

    return hits


def test_legacy_adapter_wrapper_files_removed():
    """legacy adapter wrapper 文件应已从运行时代码树中移除。"""
    existing_files = [path for path in LEGACY_ADAPTER_FILES if (WORKSPACE_ROOT / path).exists()]

    assert not existing_files, '发现 legacy adapter wrapper 文件仍存在: ' + '; '.join(existing_files)


def test_no_source_uses_legacy_adapter_class_names_directly():
    """运行时代码树中不应再直接引用 V1*Adapter / V1ResponseBuilder 旧命名。"""
    hits = _scan_legacy_symbol_hits(LEGACY_CLASS_MARKERS, ALLOWED_CLASS_FILES)

    assert not hits, '发现运行时代码仍直接引用 legacy 适配器类名: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )


def test_no_source_imports_legacy_adapter_modules():
    """运行时代码树中不应再直接导入 legacy v1_* adapter 模块。"""
    hits = _scan_legacy_import_hits()

    assert not hits, '发现运行时代码仍直接导入 legacy adapter 模块: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )
