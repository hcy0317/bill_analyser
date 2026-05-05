"""Category chart generation."""

# pylint: disable=too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

from ..logger import log_method
from .constants import CATEGORY_COLORS
from .matplotlib_setup import plt


class CategoryPieChartMixin:
    """Generate category pie charts."""

    @log_method
    def generate_category_pie_chart(
        self,
        category_data: dict[str, dict[str, Any]],
        chart_type: str = "expense",
        title: str | None = None,
        filename: str | None = None,
    ) -> Path | None:
        """
        生成分类占比饼图

        Args:
            category_data: 分类数据
            chart_type: 类型 ('expense' 或 'income')
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        # pylint: disable=too-many-locals
        if not category_data:
            self.logger.warning("分类数据为空")
            return None

        # 提取数据
        labels = []
        values = []
        for category, data in category_data.items():
            if data["total"] > 0:  # 只显示正值
                labels.append(category)
                values.append(data["total"])

        if not values:
            self.logger.warning("没有可显示的分类数据")
            return None

        # 创建图表
        _, ax = plt.subplots(figsize=(10, 8), facecolor="#d0d0d0")
        ax.set_facecolor("#d0d0d0")

        # 绘制饼图
        colors = CATEGORY_COLORS[: len(labels)]
        _wedges, _texts, autotexts = ax.pie(
            values,
            labels=labels,
            colors=colors,
            autopct="%1.1f%%",
            startangle=90,
            textprops={"fontsize": 10},
        )

        # 美化标签
        for autotext in autotexts:
            autotext.set_color("white")
            autotext.set_fontweight("bold")
            autotext.set_fontsize(9)

        # 设置标题
        if title is None:
            title = f"{'支出' if chart_type == 'expense' else '收入'}分类占比"
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)

        # 添加图例（显示金额）
        legend_labels = [f"{label}: ¥{value:,.2f}" for label, value in zip(labels, values)]
        ax.legend(legend_labels, loc="center left", bbox_to_anchor=(1, 0, 0.5, 1), fontsize=9)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"category_pie_{chart_type}_{timestamp}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("饼图已保存: %s", filepath)
        return filepath
