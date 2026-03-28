"""阻止旧版逻辑表达式解析器模块在运行时代码树中回归。"""

import ast
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = WORKSPACE_ROOT / 'src'
RUNTIME_SRC_ROOT = SRC_ROOT / 'bill_analyser'
LEGACY_LOGIC_FILE = RUNTIME_SRC_ROOT / 'utils' / 'logic.py'
LEGACY_LOGIC_MODULE = 'bill_analyser.utils.logic'
LEGACY_LOGIC_BASENAME = 'logic'
UTILS_PACKAGE = 'bill_analyser.utils'


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



def _scan_legacy_logic_import_hits() -> list[tuple[str, int, str]]:
    """通过 AST 扫描运行时代码中对旧逻辑模块的导入。"""
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
                    if alias.name == LEGACY_LOGIC_MODULE:
                        hits.append((relative_path, line_number, line_text or alias.name))
            elif isinstance(node, ast.ImportFrom):
                module_name = _relative_import_target(file_path, node)
                if module_name == LEGACY_LOGIC_MODULE:
                    hits.append((relative_path, line_number, line_text or module_name))
                    continue

                if module_name != UTILS_PACKAGE:
                    continue

                if any(alias.name == LEGACY_LOGIC_BASENAME for alias in node.names):
                    hits.append((relative_path, line_number, line_text or module_name))

    return hits



def test_legacy_logic_parser_module_removed():
    """旧版逻辑表达式解析器文件应已从运行时代码树中移除。"""
    assert not LEGACY_LOGIC_FILE.exists(), f'发现旧逻辑解析器文件仍存在: {LEGACY_LOGIC_FILE}'



def test_runtime_code_does_not_import_legacy_logic_parser():
    """运行时代码树中不应再直接导入旧逻辑表达式解析器。"""
    hits = _scan_legacy_logic_import_hits()

    assert not hits, '发现运行时代码仍直接导入旧逻辑表达式解析器: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )
