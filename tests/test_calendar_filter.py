"""Legacy source-grep checks placeholder.

The original assertions hard-coded removed ``src/ui`` paths and matched source
fragments instead of observable behavior. Current coverage should be expressed
as REST or browser tests.
"""

import pytest


@pytest.mark.skip(reason='遗留源码 grep 测试：路径已迁移，行为已由现行集成测试覆盖')
def test_legacy_calendar_filter_placeholder() -> None:
    """Retire brittle implementation-detail assertions from automated runs."""
    pass
