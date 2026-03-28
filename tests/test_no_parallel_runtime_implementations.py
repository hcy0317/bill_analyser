"""阻止已确认移除的平行实现重新进入运行时代码树。"""

import ast
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = WORKSPACE_ROOT / 'src'
RUNTIME_SRC_ROOT = SRC_ROOT / 'bill_analyser'
REMOVED_RUNTIME_FILES = (
    RUNTIME_SRC_ROOT / 'core' / 'report_generator.py',
    RUNTIME_SRC_ROOT / 'utils' / 'deduplication.py',
    RUNTIME_SRC_ROOT / 'utils' / 'advanced_logger.py',
)
REMOVED_RUNTIME_MODULES = {
    'bill_analyser.core.report_generator',
    'bill_analyser.utils.deduplication',
    'bill_analyser.utils.advanced_logger',
}
REMOVED_RUNTIME_BASENAMES = {
    'report_generator',
    'deduplication',
    'advanced_logger',
}
PACKAGE_ROOTS = {
    'bill_analyser.core',
    'bill_analyser.utils',
}


def _iter_runtime_python_files():
    yield from RUNTIME_SRC_ROOT.rglob('*.py')


def _read_lines(file_path: Path) -> list[str]:
    return file_path.read_text(encoding='utf-8').splitlines()


def _module_name_for(file_path: Path) -> str:
    return '.'.join(file_path.relative_to(SRC_ROOT).with_suffix('').parts)


def _relative_import_target(file_path: Path, node: ast.ImportFrom) -> str:
    current_package_parts = _module_name_for(file_path).split('.')[:-1]
    if node.level:
        base_parts = current_package_parts[: len(current_package_parts) - node.level + 1]
    else:
        base_parts = []

    if node.module:
        base_parts = [*base_parts, *node.module.split('.')]

    return '.'.join(base_parts)


def _scan_removed_runtime_import_hits() -> list[tuple[str, int, str]]:
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
                    if alias.name in REMOVED_RUNTIME_MODULES:
                        hits.append((relative_path, line_number, line_text or alias.name))
            elif isinstance(node, ast.ImportFrom):
                module_name = _relative_import_target(file_path, node)
                if module_name in REMOVED_RUNTIME_MODULES:
                    hits.append((relative_path, line_number, line_text or module_name))
                    continue

                if module_name not in PACKAGE_ROOTS:
                    continue

                if any(alias.name in REMOVED_RUNTIME_BASENAMES for alias in node.names):
                    hits.append((relative_path, line_number, line_text or module_name))

    return hits


def test_removed_parallel_runtime_files_stay_deleted():
    existing_files = [path.as_posix() for path in REMOVED_RUNTIME_FILES if path.exists()]
    assert not existing_files, '发现已移除的平行实现文件重新出现: ' + '; '.join(existing_files)



def test_runtime_code_does_not_import_removed_parallel_implementations():
    hits = _scan_removed_runtime_import_hits()
    assert not hits, '发现运行时代码重新导入已移除的平行实现: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )
