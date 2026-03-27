"""真实样本回归的共享清单与工具。"""

from __future__ import annotations

import json
import re
from functools import lru_cache
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
BILLS_DIR = PROJECT_ROOT / 'bills'
MANIFEST_PATH = Path(__file__).with_name('real_sample_manifest.json')


@lru_cache(maxsize=1)
def load_real_sample_manifest() -> list[dict[str, Any]]:
    return json.loads(MANIFEST_PATH.read_text(encoding='utf-8'))


def discover_real_sample_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []

    for spec in load_real_sample_manifest():
        matched_paths: list[Path] = []
        seen_paths: set[Path] = set()

        for pattern in spec['patterns']:
            for path in sorted(BILLS_DIR.glob(pattern)):
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

        sample_path = BILLS_DIR / comparison_sample
        assert sample_path.exists(), f'对比样本不存在: {sample_path}'
        cases.append(
            {
                **spec,
                'path': sample_path,
                'id': f"{spec['id']}::{sample_path.name}",
            }
        )

    return cases


def extract_real_sample_family(nodeid: str) -> str | None:
    match = re.search(r'\[([^\]]+?)::', nodeid)
    if match:
        return match.group(1)
    return None