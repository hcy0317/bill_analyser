"""Ranking chart generation."""

# pylint: disable=duplicate-code,too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

import numpy as np

from ..logger import log_method
from .matplotlib_setup import matplotlib, plt


class TopMerchantsChartMixin:
    """Generate top merchant charts."""

    @log_method
    def generate_top_merchants_chart(
        self,
        top_data: list[dict[str, Any]],
        title: str = "消费排行榜 Top 10",
        filename: str | None = None,
    ) -> Path | None:
        """
        生成Top商户/交易对象排行榜

        Args:
            top_data: 排行数据，包含 counterparty, amount 等字段
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        # pylint: disable=too-many-locals
        if not top_data:
            self.logger.warning("排行数据为空")
            return None

        # 提取数据
        merchants = [
            d["counterparty"][:15] + "..." if len(d["counterparty"]) > 15 else d["counterparty"]
            for d in top_data
        ]
        amounts = [d["amount"] for d in top_data]

        # 创建图表
        _, ax = plt.subplots(figsize=(10, 6), facecolor="#d0d0d0")
        ax.set_facecolor("#d0d0d0")

        # 绘制水平柱状图
        y_pos = np.arange(len(merchants))
        colors_gradient = matplotlib.colormaps["Reds"](np.linspace(0.4, 0.8, len(merchants)))

        bars = ax.barh(y_pos, amounts, color=colors_gradient, alpha=0.8)

        # 在柱子上显示金额
        for bar_patch, amount in zip(bars, amounts):
            width = bar_patch.get_width()
            ax.text(
                width,
                bar_patch.get_y() + bar_patch.get_height() / 2,
                f"¥{amount:,.2f}",
                ha="left",
                va="center",
                fontsize=9,
                fontweight="bold",
            )

        # 设置标签
        ax.set_yticks(y_pos)
        ax.set_yticklabels(merchants)
        ax.invert_yaxis()  # 最大值在顶部

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)
        ax.set_xlabel("金额 (元)", fontsize=12)

        # 添加网格
        ax.grid(True, axis="x", alpha=0.3, linestyle="--")

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"top_merchants_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("排行榜已保存: %s", filepath)
        return filepath
