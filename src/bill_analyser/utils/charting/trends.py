"""Trend chart generation."""

# pylint: disable=too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

from ..logger import log_method
from .constants import COLORS
from .matplotlib_setup import mdates, plt


class TrendChartMixin:
    """Generate income and expense trend charts."""

    @log_method
    def generate_trend_chart(
        self,
        trend_data: list[dict[str, Any]],
        title: str = "收支趋势图",
        filename: str | None = None,
    ) -> Path | None:
        """
        生成收支趋势图

        Args:
            trend_data: 趋势数据，包含 date, income, expense, net 字段
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not trend_data:
            self.logger.warning("趋势数据为空")
            return None

        _, ax = plt.subplots(figsize=(12, 6), facecolor="#d0d0d0")
        ax.set_facecolor("#d0d0d0")

        # 提取数据
        dates = [datetime.strptime(d["date"], "%Y-%m-%d") for d in trend_data]
        income = [d["income"] for d in trend_data]
        expense = [d["expense"] for d in trend_data]
        net = [d["net"] for d in trend_data]

        # 绘制折线图
        ax.plot(
            dates,
            income,
            label="收入",
            color=COLORS["income"],
            linewidth=2,
            marker="o",
            markersize=4,
        )
        ax.plot(
            dates,
            expense,
            label="支出",
            color=COLORS["expense"],
            linewidth=2,
            marker="s",
            markersize=4,
        )
        ax.plot(
            dates,
            net,
            label="净收入",
            color=COLORS["net"],
            linewidth=2,
            marker="^",
            markersize=4,
            linestyle="--",
        )

        # 添加零线
        ax.axhline(y=0, color="gray", linestyle="-", linewidth=0.5, alpha=0.5)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)
        ax.set_xlabel("日期", fontsize=12)
        ax.set_ylabel("金额 (元)", fontsize=12)

        # 设置日期格式
        ax.xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))
        if len(dates) > 30:
            ax.xaxis.set_major_locator(mdates.WeekdayLocator(interval=1))
        plt.setp(ax.xaxis.get_majorticklabels(), rotation=45, ha="right")

        # 添加网格
        ax.grid(True, alpha=0.3, linestyle="--", color=COLORS["grid"])

        # 添加图例
        ax.legend(loc="best", fontsize=10, framealpha=0.9)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"trend_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("趋势图已保存: %s", filepath)
        return filepath
