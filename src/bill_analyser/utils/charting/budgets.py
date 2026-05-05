"""Budget chart generation."""

# pylint: disable=too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

import numpy as np

from ..logger import log_method
from .matplotlib_setup import plt


class BudgetProgressChartMixin:
    """Generate budget progress charts."""

    @log_method
    def generate_budget_progress_chart(
        self,
        budget_data: dict[str, Any],
        title: str = "预算执行进度",
        filename: str | None = None,
    ) -> Path | None:
        """
        生成预算执行进度图

        Args:
            budget_data: 预算数据，包含各分类的预算和实际支出
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        # pylint: disable=too-many-locals
        if not budget_data:
            self.logger.warning("预算数据为空")
            return None

        # 提取数据
        categories = []
        budgets = []
        actuals = []
        for category, data in budget_data.items():
            if isinstance(data, dict) and "budget" in data and "actual" in data:
                categories.append(category)
                budgets.append(data["budget"])
                actuals.append(data["actual"])

        if not categories:
            self.logger.warning("没有可显示的预算数据")
            return None

        # 创建图表
        _, ax = plt.subplots(
            figsize=(12, len(categories) * 0.8 + 2),
            facecolor="#d0d0d0",
        )
        ax.set_facecolor("#d0d0d0")

        # 设置位置
        y_pos = np.arange(len(categories))
        bar_height = 0.35

        # 绘制柱状图
        ax.barh(
            y_pos - bar_height / 2,
            budgets,
            bar_height,
            label="预算",
            color="#90CAF9",
            alpha=0.8,
        )
        bars2 = ax.barh(y_pos + bar_height / 2, actuals, bar_height, label="实际", alpha=0.8)

        # 根据超支情况着色
        colors = []
        for budget, actual in zip(budgets, actuals):
            if actual > budget:
                colors.append("#F44336")  # 红色 - 超支
            elif actual > budget * 0.9:
                colors.append("#FF9800")  # 橙色 - 接近预算
            else:
                colors.append("#4CAF50")  # 绿色 - 良好

        for bar_patch, color in zip(bars2, colors):
            bar_patch.set_color(color)

        # 添加百分比标签
        for i, (budget, actual) in enumerate(zip(budgets, actuals)):
            if budget > 0:
                percentage = (actual / budget) * 100
                ax.text(
                    max(budget, actual),
                    y_pos[i],
                    f" {percentage:.1f}%",
                    va="center",
                    ha="left",
                    fontsize=9,
                )

        # 设置标签
        ax.set_yticks(y_pos)
        ax.set_yticklabels(categories)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)
        ax.set_xlabel("金额 (元)", fontsize=12)

        # 添加图例
        ax.legend(loc="best", fontsize=10)

        # 添加网格
        ax.grid(True, axis="x", alpha=0.3, linestyle="--")

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"budget_progress_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("预算进度图已保存: %s", filepath)
        return filepath
