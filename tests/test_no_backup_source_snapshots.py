"""阻止源码树中的 .backup 历史快照文件回归。"""

from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = WORKSPACE_ROOT / 'src'


def test_source_tree_contains_no_backup_snapshots():
    """src 目录下不应再保留 .backup 历史快照文件。"""
    backup_files = sorted(path.relative_to(WORKSPACE_ROOT).as_posix() for path in SOURCE_ROOT.rglob('*.backup'))

    assert not backup_files, '发现源码树仍存在 .backup 历史快照文件: ' + '; '.join(backup_files)
