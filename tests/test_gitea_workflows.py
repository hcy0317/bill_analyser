from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_DIR = REPO_ROOT / '.gitea' / 'workflows'
EXPECTED_WORKFLOW_NAMES = {
    'backend-ci.yml',
    'frontend-ci.yml',
    'agent-stack-health.yml',
}
EXPECTED_GROUP = 'group: gitea-${{ gitea.workflow }}-${{ gitea.ref }}'


def _workflow_files() -> dict[str, Path]:
    workflow_paths = sorted(WORKFLOW_DIR.glob('*.yml')) + sorted(WORKFLOW_DIR.glob('*.yaml'))
    return {path.name: path for path in workflow_paths}


def _read(name: str) -> str:
    return _workflow_files()[name].read_text(encoding='utf-8')


def test_all_expected_gitea_workflows_exist() -> None:
    workflow_files = _workflow_files()
    assert EXPECTED_WORKFLOW_NAMES.issubset(workflow_files), 'missing expected Gitea workflows'
    for path in workflow_files.values():
        assert path.exists(), f'missing workflow: {path}'


def test_gitea_workflows_lock_supported_concurrency_contract() -> None:
    for name in _workflow_files():
        text = _read(name)
        assert 'concurrency:' in text, f'{name} must declare concurrency'
        assert EXPECTED_GROUP in text, f'{name} must keep the documented workflow+ref concurrency key'
        assert 'cancel-in-progress: true' in text, f'{name} must cancel in-progress runs in the same lane'
        assert 'timeout-minutes:' not in text, f'{name} must avoid unsupported timeout-minutes'
        assert 'continue-on-error:' not in text, f'{name} must avoid unsupported continue-on-error'
        assert re.search(r'(?m)^\s*environment:\s*', text) is None, f'{name} must avoid unsupported job environment'
        assert 'intentionally does not collapse push + PR lanes' in text, f'{name} should document its concurrency lane boundary'


def test_gitea_workflows_pin_read_only_contents_permissions() -> None:
    for name in _workflow_files():
        text = _read(name)
        assert 'permissions:' in text, f'{name} must declare least-privilege permissions'
        assert re.search(r'(?m)^permissions:\n  contents: read$', text), f'{name} must pin contents: read'


def test_gitea_workflows_use_absolute_action_urls() -> None:
    for name in _workflow_files():
        text = _read(name)
        assert 'uses: actions/' not in text, f'{name} should not rely on instance-default action source resolution'
        uses_lines = re.findall(r'(?m)^\s*uses:\s+(.+)$', text)
        assert uses_lines, f'{name} should declare explicit action sources'
        for uses in uses_lines:
            assert uses.startswith('https://github.com/'), f'{name} must use absolute GitHub action URLs on Gitea'


def test_agent_stack_health_workflow_covers_gitea_contract_regression() -> None:
    text = _read('agent-stack-health.yml')
    assert '.gitea/**' in text, 'agent-stack workflow should trigger on Gitea workflow changes'
    assert 'tests/test_gitea_workflows.py' in text, 'agent-stack workflow should cover the Gitea workflow contract test'
    assert 'python -m pytest tests/test_reviewer_agent_diff_contract.py tests/test_agent_stack_health.py tests/test_ai_workflow_docs.py tests/test_task_state.py tests/test_task_state_reader.py tests/test_gitea_workflows.py -v' in text
