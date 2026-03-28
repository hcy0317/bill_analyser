"""阻止缺失的 ML 分类器模块残影继续滞留在运行时代码与依赖清单中。"""

import ast
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = WORKSPACE_ROOT / 'src'
RUNTIME_SRC_ROOT = SRC_ROOT / 'bill_analyser'
PYPROJECT_PATH = WORKSPACE_ROOT / 'pyproject.toml'
ORPHAN_ML_MODULE = 'bill_analyser.core.ml_classifier'
ORPHAN_ML_FILE = RUNTIME_SRC_ROOT / 'core' / 'ml_classifier.py'
SKLEARN_DEP_MARKER = 'scikit-learn'


def _iter_runtime_python_files():
    """遍历运行时代码树中的 Python 文件。"""
    yield from RUNTIME_SRC_ROOT.rglob('*.py')



def _read_lines(file_path: Path) -> list[str]:
    """读取源码文件并按行返回，便于拼接错误信息。"""
    return file_path.read_text(encoding='utf-8').splitlines()



def _scan_orphan_ml_import_hits() -> list[tuple[str, int, str]]:
    """通过 AST 扫描运行时代码中对缺失 ML 分类器模块的引用。"""
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
                    if alias.name == ORPHAN_ML_MODULE:
                        hits.append((relative_path, line_number, line_text or alias.name))
            elif isinstance(node, ast.ImportFrom):
                module_name = node.module or ''
                if module_name == ORPHAN_ML_MODULE:
                    hits.append((relative_path, line_number, line_text or module_name))
            elif (
                isinstance(node, ast.Call)
                and isinstance(node.func, ast.Name)
                and node.func.id == 'import_module'
                and node.args
                and isinstance(node.args[0], ast.Constant)
                and node.args[0].value == ORPHAN_ML_MODULE
            ):
                hits.append((relative_path, line_number, line_text or ORPHAN_ML_MODULE))

    return hits


def test_orphan_ml_classifier_module_file_absent():
    """缺失的 ML 分类器源码文件应仍然不存在；如需恢复应一并恢复实现。"""
    assert not ORPHAN_ML_FILE.exists(), f'发现缺失的 ML 分类器源码文件重新出现: {ORPHAN_ML_FILE}'



def test_runtime_code_does_not_reference_orphan_ml_classifier_module():
    """运行时代码树中不应再引用缺失的 ML 分类器模块。"""
    hits = _scan_orphan_ml_import_hits()

    assert not hits, '发现运行时代码仍引用缺失的 ML 分类器模块: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )



def test_pyproject_does_not_keep_orphan_ml_dependency():
    """依赖清单中不应继续保留仅服务于缺失 ML 分类器的 scikit-learn。"""
    pyproject_text = PYPROJECT_PATH.read_text(encoding='utf-8')
    assert SKLEARN_DEP_MARKER not in pyproject_text, 'pyproject.toml 仍保留 orphan ML 依赖 scikit-learn'
