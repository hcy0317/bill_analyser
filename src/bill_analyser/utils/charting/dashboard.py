"""Dashboard chart generation."""

# pylint: disable=duplicate-code,too-few-public-methods

from datetime import datetime
from pathlib import Path
from typing import Any

import numpy as np

from ..logger import log_method
from .constants import CATEGORY_COLORS, COLORS
from .matplotlib_setup import mdates, plt


def _add_summary_subplot(ax: Any, summary: dict[str, Any]) -> None:
    ax.set_facecolor("#d0d0d0")
    if not summary:
        return

    categories = ["收入", "支出", "净收入"]
    values = [
        summary.get("total_income", 0),
        summary.get("total_expense", 0),
        summary.get("net_income", 0),
    ]
    colors = [COLORS["income"], COLORS["expense"], COLORS["net"]]
    bars = ax.bar(categories, values, color=colors, alpha=0.8)

    for bar_patch, value in zip(bars, values):
        height = bar_patch.get_height()
        ax.text(
            bar_patch.get_x() + bar_patch.get_width() / 2,
            height,
            f"¥{value:,.0f}",
            ha="center",
            va="bottom",
            fontsize=9,
        )

    ax.set_title("收支概览", fontsize=14, fontweight="bold")
    ax.set_ylabel("金额 (元)")
    ax.grid(True, axis="y", alpha=0.3)


def _add_category_subplot(ax: Any, by_category: dict[str, Any]) -> None:
    ax.set_facecolor("#d0d0d0")
    if not by_category:
        return

    labels = list(by_category.keys())[:8]  # 最多显示8个
    values = [by_category[cat]["total"] for cat in labels]
    colors_pie = CATEGORY_COLORS[: len(labels)]

    _wedges, _texts, autotexts = ax.pie(
        values,
        labels=labels,
        colors=colors_pie,
        autopct="%1.1f%%",
        startangle=90,
    )
    for autotext in autotexts:
        autotext.set_color("white")
        autotext.set_fontsize(8)

    ax.set_title("支出分类分布", fontsize=14, fontweight="bold")


def _add_trend_subplot(ax: Any, trend: list[dict[str, Any]]) -> None:
    ax.set_facecolor("#d0d0d0")
    if not trend:
        return

    dates = [datetime.strptime(d["date"], "%Y-%m-%d") for d in trend]
    income = [d["income"] for d in trend]
    expense = [d["expense"] for d in trend]

    ax.plot(dates, income, label="收入", color=COLORS["income"], linewidth=2)
    ax.plot(dates, expense, label="支出", color=COLORS["expense"], linewidth=2)

    ax.xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))
    plt.setp(ax.xaxis.get_majorticklabels(), rotation=45, ha="right")

    ax.set_title("收支趋势", fontsize=14, fontweight="bold")
    ax.set_ylabel("金额 (元)")
    ax.legend(fontsize=9)
    ax.grid(True, alpha=0.3)


def _add_top_expenses_subplot(ax: Any, top_expenses: list[dict[str, Any]]) -> None:
    ax.set_facecolor("#d0d0d0")
    if not top_expenses:
        return

    merchants = [
        e["counterparty"][:12] + "..." if len(e["counterparty"]) > 12 else e["counterparty"]
        for e in top_expenses
    ]
    amounts = [e["amount"] for e in top_expenses]

    y_pos = np.arange(len(merchants))
    bars = ax.barh(y_pos, amounts, color=COLORS["expense"], alpha=0.7)

    for bar_patch, amount in zip(bars, amounts):
        width = bar_patch.get_width()
        ax.text(
            width,
            bar_patch.get_y() + bar_patch.get_height() / 2,
            f" ¥{amount:,.0f}",
            va="center",
            fontsize=8,
        )

    ax.set_yticks(y_pos)
    ax.set_yticklabels(merchants, fontsize=9)
    ax.invert_yaxis()
    ax.set_title("最大支出 Top 8", fontsize=14, fontweight="bold")
    ax.set_xlabel("金额 (元)")
    ax.grid(True, axis="x", alpha=0.3)


class DashboardChartMixin:
    """Generate dashboard charts."""

    @log_method
    def generate_comprehensive_dashboard(
        self,
        report_data: dict[str, Any],
        filename: str | None = None,
    ) -> Path | None:
        """
        生成综合仪表盘（多图组合）

        Args:
            report_data: 完整的报告数据
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not report_data:
            self.logger.warning("报告数据为空")
            return None

        # 创建2x2的子图布局
        fig = plt.figure(figsize=(16, 12), facecolor="#d0d0d0")
        gs = fig.add_gridspec(2, 2, hspace=0.3, wspace=0.3)

        _add_summary_subplot(fig.add_subplot(gs[0, 0]), report_data.get("summary", {}))
        _add_category_subplot(fig.add_subplot(gs[0, 1]), report_data.get("by_category", {}))
        _add_trend_subplot(fig.add_subplot(gs[1, 0]), report_data.get("trend", []))
        _add_top_expenses_subplot(
            fig.add_subplot(gs[1, 1]),
            report_data.get("top_expenses", [])[:8],
        )

        # 总标题
        period = report_data.get("period", "month")
        period_name = {"month": "月度", "quarter": "季度", "year": "年度"}.get(period, "")
        fig.suptitle(f"{period_name}账单分析仪表盘", fontsize=18, fontweight="bold", y=0.98)

        # 保存图片
        if filename is None:
            filename = f"dashboard_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches="tight")
        plt.close()

        self.logger.info("综合仪表盘已保存: %s", filepath)
        return filepath
