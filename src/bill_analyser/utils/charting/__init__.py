"""Chart generation package."""

# pylint: disable=unused-import
from .constants import CATEGORY_COLORS, COLORS
from .generator import ChartGenerator, generate_all_charts

__all__ = [
    "CATEGORY_COLORS",
    "COLORS",
    "ChartGenerator",
    "generate_all_charts",
]
