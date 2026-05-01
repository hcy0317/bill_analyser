"""User-scoped in-memory rate limiting for LLM calls."""
# pylint: disable=too-few-public-methods

from __future__ import annotations

import time

from .limits import _MAX_ANALYZE_LIMIT

_RATE_LIMIT_WINDOW = 60  # seconds
_RATE_LIMIT_MAX_CALLS = 10
_RATE_LIMIT_BUCKETS: dict[int, list[float]] = {}


class RateLimitMixin:
    """Shared rate limit behavior across LLM learning service domains."""

    @staticmethod
    def _normalize_limit(limit: int) -> int:
        """Validate and normalize the maximum number of items to analyze."""
        normalized_limit = int(limit)
        if normalized_limit <= 0:
            raise ValueError("limit must be a positive integer")
        if normalized_limit > _MAX_ANALYZE_LIMIT:
            raise ValueError(f"limit cannot exceed {_MAX_ANALYZE_LIMIT}")
        return normalized_limit

    def _reserve_rate_limit_slots(self, user_id: int, slots: int = 1) -> None:
        """Reserve real LLM call slots across service instances for the same user."""
        if slots <= 0:
            return

        now = time.time()
        recent_calls = [
            ts for ts in _RATE_LIMIT_BUCKETS.get(user_id, [])
            if now - ts < _RATE_LIMIT_WINDOW
        ]
        if len(recent_calls) + slots > _RATE_LIMIT_MAX_CALLS:
            raise RuntimeError(
                f"Rate limit exceeded: max {_RATE_LIMIT_MAX_CALLS} calls per {_RATE_LIMIT_WINDOW}s"
            )
        recent_calls.extend([now] * slots)
        _RATE_LIMIT_BUCKETS[user_id] = recent_calls
