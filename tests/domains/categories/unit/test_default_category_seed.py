"""Default category seed branch coverage."""

from __future__ import annotations

from typing import Any

import pytest

from bill_analyser.core.default_category_seed import ensure_default_categories


class _LegacyCategorySeedDB:
    """DB double that models callers without the batch ensure_categories API."""

    def __init__(self) -> None:
        self.payloads: list[dict[str, Any]] = []

    async def create_category(self, payload: dict[str, Any], *, user_id: int) -> int | None:
        self.payloads.append({"payload": payload, "user_id": user_id})
        if len(self.payloads) == 1:
            return 1
        return None


@pytest.mark.asyncio
async def test_default_categories_support_legacy_db_without_batch_ensure() -> None:
    """Default seed should keep the old create_category loop for non-Database doubles."""
    db = _LegacyCategorySeedDB()

    result = await ensure_default_categories(db, user_id=7)

    assert result["created"] == 1
    assert result["skipped"] > 0
    assert db.payloads[0]["user_id"] == 7
    assert db.payloads[0]["payload"]["main_category"]
