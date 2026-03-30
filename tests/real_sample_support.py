"""真实样本回归的共享清单与工具。"""

from __future__ import annotations

import json
import re
from functools import lru_cache
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SAMPLE_BILLS_DIR = PROJECT_ROOT / 'tests' / 'fixtures' / 'import_samples'
LOCAL_ORIGINAL_BILLS_DIR = PROJECT_ROOT / 'bills'
MANIFEST_PATH = Path(__file__).with_name('real_sample_manifest.json')

LOCAL_SUFFIX_PREFERENCE_BY_PARSER: dict[str, list[str]] = {
    'wechat': ['.csv', '.xlsx'],
    'alipay': ['.csv'],
    'icbc': ['.csv', '.xlsx', '.xls'],
    'cmbc': ['.csv', '.xls', '.xlsx'],
    'abc': ['.csv', '.xlsx', '.xls'],
    'ccb': ['.xlsx', '.xls', '.csv'],
}


@lru_cache(maxsize=1)
def load_real_sample_manifest() -> list[dict[str, Any]]:
    return json.loads(MANIFEST_PATH.read_text(encoding='utf-8'))


def discover_real_sample_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []

    for spec in load_real_sample_manifest():
        matched_paths: list[Path] = []
        seen_paths: set[Path] = set()

        for pattern in spec['patterns']:
            for path in sorted(SAMPLE_BILLS_DIR.glob(pattern)):
                if path in seen_paths:
                    continue
                seen_paths.add(path)
                matched_paths.append(path)

        assert matched_paths, f"未发现样本族 {spec['id']} 的账单文件"

        for path in matched_paths:
            cases.append(
                {
                    **spec,
                    'path': path,
                    'id': f"{spec['id']}::{path.name}",
                }
            )

    return cases


def discover_parser_comparison_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []

    for spec in load_real_sample_manifest():
        comparison_sample = str(spec.get('comparison_sample') or '').strip()
        dedicated_parser = str(spec.get('dedicated_parser') or '').strip()
        if not comparison_sample or not dedicated_parser:
            continue

        sample_path = SAMPLE_BILLS_DIR / comparison_sample
        assert sample_path.exists(), f'对比样本不存在: {sample_path}'
        cases.append(
            {
                **spec,
                'path': sample_path,
                'id': f"{spec['id']}::{sample_path.name}",
            }
        )

    return cases


def _get_local_suffix_rank(parser_id: str, suffix: str) -> int:
    preferences = LOCAL_SUFFIX_PREFERENCE_BY_PARSER.get(parser_id, [])
    try:
        return preferences.index(suffix)
    except ValueError:
        return len(preferences)


def discover_local_original_bill_cases() -> list[dict[str, Any]]:
    """从本地 bills/ 原始账单中按 parser 家族与扩展名各挑一份代表样本。"""
    if not LOCAL_ORIGINAL_BILLS_DIR.is_dir():
        return []

    from bill_analyser.parsers.factory import ParserFactory

    factory = ParserFactory()
    selected_by_family: dict[tuple[str, str], Path] = {}

    for path in sorted(LOCAL_ORIGINAL_BILLS_DIR.iterdir()):
        if not path.is_file():
            continue

        parser_info = factory.detect_parser(str(path))
        if not parser_info:
            continue

        parser_id = str(parser_info.get('id') or '').strip()
        if not parser_id:
            continue

        family_key = (parser_id, path.suffix.lower())
        current = selected_by_family.get(family_key)

        if current is None or path.name < current.name:
            selected_by_family[family_key] = path

    cases: list[dict[str, Any]] = []
    for (parser_id, suffix), path in sorted(
        selected_by_family.items(),
        key=lambda item: (
            item[0][0],
            _get_local_suffix_rank(item[0][0], item[0][1]),
            item[1].name,
        ),
    ):
        cases.append(
            {
                'id': f'local_{parser_id}_{suffix.lstrip(".")}::{path.name}',
                'path': path,
                'parser_id': parser_id,
                'sample_source': 'local_original_bill',
            }
        )

    return cases


def extract_real_sample_family(nodeid: str) -> str | None:
    match = re.search(r'\[([^\]]+?)::', nodeid)
    if match:
        return match.group(1)
    return None