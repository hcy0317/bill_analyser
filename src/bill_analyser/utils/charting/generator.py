"""Chart generator public implementation."""

from pathlib import Path
from typing import Any

from ..logger import get_logger
from .budgets import BudgetProgressChartMixin
from .categories import CategoryPieChartMixin
from .comparison import ComparisonBarChartMixin
from .dashboard import DashboardChartMixin
from .heatmap import HeatmapChartMixin
from .ranking import TopMerchantsChartMixin
from .trends import TrendChartMixin


class ChartGenerator(
    TrendChartMixin,
    CategoryPieChartMixin,
    TopMerchantsChartMixin,
    ComparisonBarChartMixin,
    HeatmapChartMixin,
    BudgetProgressChartMixin,
    DashboardChartMixin,
):
    """图表生成器"""

    def __init__(self, output_dir: Path | None = None, dpi: int = 100):
        """
        初始化图表生成器

        Args:
            output_dir: 输出目录
            dpi: 图片分辨率
        """
        self.logger = get_logger("ChartGenerator")
        self.output_dir = output_dir or Path("output/charts")
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.dpi = dpi


# 全局实例
_chart_generator = ChartGenerator()


def generate_all_charts(
    report_data: dict[str, Any],
    output_dir: Path | None = None,
) -> dict[str, Path | None]:
    """
    生成所有图表

    Args:
        report_data: 报告数据
        output_dir: 输出目录

    Returns:
        Dict[str, Path]: 图表文件路径字典
    """
    generator = ChartGenerator(output_dir=output_dir)
    charts = {}

    # 生成各类图表
    if report_data.get("trend"):
        charts["trend"] = generator.generate_trend_chart(report_data["trend"])

    if report_data.get("by_category"):
        charts["category_pie"] = generator.generate_category_pie_chart(
            report_data["by_category"],
            chart_type="expense",
        )

    if report_data.get("top_expenses"):
        charts["top_expenses"] = generator.generate_top_merchants_chart(
            report_data["top_expenses"], title="最大支出 Top 10"
        )

    if report_data.get("summary"):
        charts["comparison"] = generator.generate_comparison_bar_chart(report_data["summary"])

    # 生成综合仪表盘
    charts["dashboard"] = generator.generate_comprehensive_dashboard(report_data)

    return charts


__all__ = ["ChartGenerator", "_chart_generator", "generate_all_charts"]
