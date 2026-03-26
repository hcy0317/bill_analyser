"""前端源码不再直接引用 legacy v1 路径的回归测试。"""

from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
FRONTEND_SRC_ROOT = WORKSPACE_ROOT / 'src' / 'web' / 'src'
LEGACY_MARKERS = ('v1/', '/api/v1/')


def _collect_legacy_hits() -> list[tuple[str, int, str]]:
    """扫描前端源码中的 legacy v1 路径引用。"""
    hits: list[tuple[str, int, str]] = []

    for file_path in FRONTEND_SRC_ROOT.rglob('*'):
        if not file_path.is_file():
            continue
        if file_path.suffix not in {'.ts', '.tsx', '.js', '.jsx', '.vue'}:
            continue

        relative_path = file_path.relative_to(WORKSPACE_ROOT).as_posix()
        content = file_path.read_text(encoding='utf-8')

        for line_number, line in enumerate(content.splitlines(), start=1):
            if any(marker in line for marker in LEGACY_MARKERS):
                hits.append((relative_path, line_number, line.strip()))

    return hits


def test_frontend_source_has_no_direct_legacy_v1_routes():
    """当前运行态前端源码不应再直接请求 legacy v1 路径。"""
    hits = _collect_legacy_hits()

    assert not hits, '发现前端源码仍直接引用 legacy v1 路径: ' + '; '.join(
        f'{path}:{line} -> {text}' for path, line, text in hits
    )