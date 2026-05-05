"""Heatmap chart generation."""

# pylint: disable=too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

from ..logger import log_method
from .matplotlib_setup import plt


class HeatmapChartMixin:
    """Generate spending heatmaps."""

    @log_method
    def generate_heatmap(
        self,
        bills_data: list[dict[str, Any]],
        title: str = "消费热力图",
        filename: str | None = None,
    ) -> Path | None:
        """
        生成每日消费热力图

        Args:
            bills_data: 账单数据列表
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not bills_data:
            self.logger.warning("账单数据为空")
            return None

        # 转换为 DataFrame
        df = pd.DataFrame(bills_data)
        df["date"] = pd.to_datetime(df["date"])
        df["amount"] = pd.to_numeric(df["amount"], errors="coerce")

        # 只统计支出
        df_expense = df[df["type"] == "支出"].copy()

        if df_expense.empty:
            self.logger.warning("没有支出数据")
            return None

        # 添加星期和小时字段
        df_expense["weekday"] = df_expense["date"].dt.dayofweek
        df_expense["hour"] = df_expense["date"].dt.hour

        # 按星期和小时分组统计
        pivot_data = df_expense.groupby(["weekday", "hour"])["amount"].sum().unstack(fill_value=0)

        # 确保所有小时都有数据
        for hour in range(24):
            if hour not in pivot_data.columns:
                pivot_data[hour] = 0
        pivot_data = pivot_data.sort_index(axis=1)

        # 创建图表
        _, ax = plt.subplots(figsize=(14, 6), facecolor="#d0d0d0")
        ax.set_facecolor("#d0d0d0")

        # 绘制热力图
        im = ax.imshow(pivot_data.values, cmap="YlOrRd", aspect="auto", alpha=0.8)

        # 设置坐标轴
        weekdays = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"]
        ax.set_yticks(np.arange(len(weekdays)))
        ax.set_yticklabels(weekdays)

        hours = list(range(24))
        ax.set_xticks(np.arange(len(hours)))
        ax.set_xticklabels(hours)
        ax.set_xlabel("小时", fontsize=12)

        # 设置标题
        ax.set_title(title, fontsize=16, fontweight="bold", pad=20)

        # 添加颜色条
        cbar = plt.colorbar(im, ax=ax)
        cbar.set_label("消费金额 (元)", rotation=270, labelpad=20, fontsize=10)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"heatmap_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("热力图已保存: %s", filepath)
        return filepath
