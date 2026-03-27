"""Legacy manual tag smoke script placeholder.

The original module exercised removed v1 tag routes against a separately
running local server. Equivalent automated coverage now lives in
``tests/new_ui/test_accounts_tags_rest_api.py``.
"""

import pytest


@pytest.mark.skip(reason='遗留手工脚本：标签 REST 能力已由 tests/new_ui/test_accounts_tags_rest_api.py 覆盖')
def test_legacy_tag_workflow_placeholder():
    """Keep the historical filename without running deprecated live-server checks."""
