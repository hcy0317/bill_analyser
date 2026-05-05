"""
Charts Module - 图表生成模块公共 facade。

保留历史公共导入路径，实际实现位于 :mod:`bill_analyser.utils.charting`。
"""

# pylint: disable=unused-import
from . import charting as _charting
from .charting import CATEGORY_COLORS, COLORS, ChartGenerator, generate_all_charts
from .charting.generator import _chart_generator

__all__ = list(_charting.__all__)
