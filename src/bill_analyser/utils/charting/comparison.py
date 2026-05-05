"""Comparison chart generation."""

# pylint: disable=duplicate-code,too-few-public-methods

from datetime import datetime
from pathlib import Path

import numpy as np

from ..logger import log_method
from .constants import COLORS
from .matplotlib_setup import plt


class ComparisonBarChartMixin:
    """Generate income and expense comparison charts."""

    @log_method
    def generate_comparison_bar_chart(
        self,
        summary_data: dict[str, float],
        title: str = "收支对比",
        filename: str | None = None,
    ) -> Path | None:
        """
        生成收支对比柱状图

        Args:
            summary_data: 汇总数据，包含 total_income, total_expense, net_income
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not summary_data:
            self.logger.warning("汇总数据为空")
            return None

        # 提取数据
        categories = ["收入", "支出", "净收入"]
        values = [
            summary_data.get("total_income", 0),
            summary_data.get("total_expense", 0),
            summary_data.get("net_income", 0),
        ]
        colors = [COLORS["income"], COLORS["expense"], COLORS["net"]]

        # 创建图表
        _, ax = plt.subplots(figsize=(8, 6), facecolor="#d0d0d0")
        ax.set_facecolor("#d0d0d0")

        # 绘制柱状图
        x_pos = np.arange(len(categories))
        bars = ax.bar(x_pos, values, color=colors, alpha=0.8, width=0.6)

        # 在柱子上显示金额
        for bar_patch, value in zip(bars, values):
            height = bar_patch.get_height()
            ax.text(
                bar_patch.get_x() + bar_patch.get_width() / 2,
                height,
                f"¥{value:,.2f}",
                ha="center",
                va="bottom" if value >= 0 else "top",
                fontsize=11,
                fontweight="bold",
            )

        # 设置标签
        ax.set_xticks(x_pos)
        ax.set_xticklabels(categories, fontsize=12)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)
        ax.set_ylabel("金额 (元)", fontsize=12)

        # 添加零线
        ax.axhline(y=0, color="gray", linestyle="-", linewidth=1, alpha=0.5)

        # 添加网格
        ax.grid(True, axis="y", alpha=0.3, linestyle="--")

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("对比图已保存: %s", filepath)
        return filepath
