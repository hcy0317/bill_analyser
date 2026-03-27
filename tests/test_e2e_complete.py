"""Legacy manual E2E script placeholder.

The original file depended on a running local server and deprecated v1 API
shapes. Current automated end-to-end REST coverage lives in ``tests/new_ui``.
"""

import pytest


@pytest.mark.skip(reason='遗留手工脚本：当前账单/账户工作流已由 tests/new_ui 套件覆盖')
def test_legacy_e2e_script_placeholder() -> None:
    """Retire deprecated live-server checks from automated pytest runs."""
    pass
