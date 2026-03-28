"""阻止旧版 parser factory 链路在运行时代码树中回归。"""

import ast
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = WORKSPACE_ROOT / 'src'
RUNTIME_SRC_ROOT = SRC_ROOT / 'bill_analyser'
LEGACY_PARSER_FILES = (
    RUNTIME_SRC_ROOT / 'parsers' / 'base_parser.py',
    RUNTIME_SRC_ROOT / 'parsers' / 'wechat_parser.py',
    RUNTIME_SRC_ROOT / 'parsers' / 'alipay_parser.py',
    RUNTIME_SRC_ROOT / 'parsers' / 'csv_parser.py',
    RUNTIME_SRC_ROOT / 'parsers' / 'excel_parser.py',
)
LEGACY_PARSER_MODULES = {
    'bill_analyser.parsers.base_parser',
    'bill_analyser.parsers.wechat_parser',
    'bill_analyser.parsers.alipay_parser',
    'bill_analyser.parsers.csv_parser',
    'bill_analyser.parsers.excel_parser',
}
LEGACY_PARSER_BASENAMES = {
    'base_parser',
    'wechat_parser',
    'alipay_parser',
    'csv_parser',
    'excel_parser',
}
PARSERS_PACKAGE = 'bill_analyser.parsers'


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


def _scan_legacy_parser_factory_import_hits() -> list[tuple[str, int, str]]:
    """通过 AST 扫描运行时代码中对旧 parser factory 链路的导入。"""
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
                    if alias.name in LEGACY_PARSER_MODULES:
                        hits.append((relative_path, line_number, line_text or alias.name))
            elif isinstance(node, ast.ImportFrom):
                module_name = _relative_import_target(file_path, node)
                if module_name in LEGACY_PARSER_MODULES:
                    hits.append((relative_path, line_number, line_text or module_name))
                    continue

                if module_name != PARSERS_PACKAGE:
                    continue

                if any(alias.name in LEGACY_PARSER_BASENAMES for alias in node.names):
                    hits.append((relative_path, line_number, line_text or module_name))

    return hits


def test_legacy_parser_factory_stack_only_exists_as_compatibility_shims():
    """旧模块如仍存在，也只能作为兼容 shim，不能被运行时代码主动依赖。"""
    existing_files = [path.as_posix() for path in LEGACY_PARSER_FILES if path.exists()]
    assert existing_files, '兼容 shim 缺失：旧 parser 导入路径需要保留薄兼容层'


def test_runtime_code_does_not_import_legacy_parser_factory_stack():
    """运行时代码树中不应再直接导入旧版 parser factory 链路。"""
    hits = _scan_legacy_parser_factory_import_hits()

    assert not hits, '发现运行时代码仍直接导入旧版 parser factory 链路: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )
