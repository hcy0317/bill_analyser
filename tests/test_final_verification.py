"""Legacy manual reconciliation verification placeholder.

The original file depended on a live local server and hard-coded bill IDs,
which makes it unsuitable for deterministic pytest automation.
"""

import pytest


@pytest.mark.skip(reason='遗留手工验证脚本：依赖本地服务与硬编码数据，不纳入自动化套件')
def test_legacy_reconciliation_verification_placeholder() -> None:
    """Preserve the historical filename while excluding manual verification from CI."""
    pass
