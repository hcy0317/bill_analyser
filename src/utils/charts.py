"""
Charts Module - 图表生成模块

提供各类数据可视化图表生成功能。
"""

from datetime import datetime
from pathlib import Path
from typing import Dict, List, Any, Optional, Tuple
import matplotlib
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
from matplotlib.patches import Patch
import numpy as np
import pandas as pd

from .logger import get_logger, log_method

# 使用非交互式后端
matplotlib.use('Agg')

# 设置中文字体
plt.rcParams['font.sans-serif'] = ['SimHei', 'Microsoft YaHei', 'Arial Unicode MS']
plt.rcParams['axes.unicode_minus'] = False  # 解决负号显示问题

# 颜色方案
COLORS = {
    'income': '#4CAF50',      # 绿色 - 收入
    'expense': '#F44336',     # 红色 - 支出
    'net': '#2196F3',         # 蓝色 - 净收入
    'primary': '#1976D2',     # 主色
    'secondary': '#FF9800',   # 辅色
    'background': '#FFFFFF',  # 背景色
    'grid': '#E0E0E0',        # 网格线
    'text': '#333333'         # 文字颜色
}

# 分类配色（循环使用）
CATEGORY_COLORS = [
    '#FF6B6B', '#4ECDC4', '#45B7D1', '#FFA07A', '#98D8C8',
    '#F7DC6F', '#BB8FCE', '#85C1E2', '#F8B195', '#C06C84',
    '#6C5B7B', '#355C7D', '#F67280', '#C06C84', '#6C5B7B'
]


class ChartGenerator:
    """图表生成器"""

    def __init__(self, output_dir: Optional[Path] = None, dpi: int = 100):
        """
        初始化图表生成器

        Args:
            output_dir: 输出目录
            dpi: 图片分辨率
        """
        self.logger = get_logger('ChartGenerator')
        self.output_dir = output_dir or Path('output/charts')
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.dpi = dpi

    @log_method
    def generate_trend_chart(
        self,
        trend_data: List[Dict[str, Any]],
        title: str = "收支趋势图",
        filename: Optional[str] = None
    ) -> Path:
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

        fig, ax = plt.subplots(figsize=(12, 6), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 提取数据
        dates = [datetime.strptime(d['date'], '%Y-%m-%d') for d in trend_data]
        income = [d['income'] for d in trend_data]
        expense = [d['expense'] for d in trend_data]
        net = [d['net'] for d in trend_data]

        # 绘制折线图
        ax.plot(dates, income, label='收入', color=COLORS['income'],
                linewidth=2, marker='o', markersize=4)
        ax.plot(dates, expense, label='支出', color=COLORS['expense'],
                linewidth=2, marker='s', markersize=4)
        ax.plot(dates, net, label='净收入', color=COLORS['net'],
                linewidth=2, marker='^', markersize=4, linestyle='--')

        # 添加零线
        ax.axhline(y=0, color='gray', linestyle='-', linewidth=0.5, alpha=0.5)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
        ax.set_xlabel('日期', fontsize=12)
        ax.set_ylabel('金额 (元)', fontsize=12)

        # 设置日期格式
        ax.xaxis.set_major_formatter(mdates.DateFormatter('%m-%d'))
        if len(dates) > 30:
            ax.xaxis.set_major_locator(mdates.WeekdayLocator(interval=1))
        plt.setp(ax.xaxis.get_majorticklabels(), rotation=45, ha='right')

        # 添加网格
        ax.grid(True, alpha=0.3, linestyle='--', color=COLORS['grid'])

        # 添加图例
        ax.legend(loc='best', fontsize=10, framealpha=0.9)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"trend_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("趋势图已保存: %s", filepath)
        return filepath

    @log_method
    def generate_category_pie_chart(
        self,
        category_data: Dict[str, Dict[str, Any]],
        chart_type: str = 'expense',
        title: Optional[str] = None,
        filename: Optional[str] = None
    ) -> Path:
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
        if not category_data:
            self.logger.warning("分类数据为空")
            return None

        # 提取数据
        labels = []
        values = []
        for category, data in category_data.items():
            if data['total'] > 0:  # 只显示正值
                labels.append(category)
                values.append(data['total'])

        if not values:
            self.logger.warning("没有可显示的分类数据")
            return None

        # 创建图表
        fig, ax = plt.subplots(figsize=(10, 8), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 计算百分比
        total = sum(values)
        percentages = [v / total * 100 for v in values]

        # 绘制饼图
        colors = CATEGORY_COLORS[:len(labels)]
        wedges, texts, autotexts = ax.pie(
            values,
            labels=labels,
            colors=colors,
            autopct='%1.1f%%',
            startangle=90,
            textprops={'fontsize': 10}
        )

        # 美化标签
        for autotext in autotexts:
            autotext.set_color('white')
            autotext.set_fontweight('bold')
            autotext.set_fontsize(9)

        # 设置标题
        if title is None:
            title = f"{'支出' if chart_type == 'expense' else '收入'}分类占比"
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)

        # 添加图例（显示金额）
        legend_labels = [f"{label}: ¥{value:,.2f}" for label, value in zip(labels, values)]
        ax.legend(legend_labels, loc='center left', bbox_to_anchor=(1, 0, 0.5, 1),
                  fontsize=9)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"category_pie_{chart_type}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("饼图已保存: %s", filepath)
        return filepath

    @log_method
    def generate_top_merchants_chart(
        self,
        top_data: List[Dict[str, Any]],
        title: str = "消费排行榜 Top 10",
        filename: Optional[str] = None
    ) -> Path:
        """
        生成Top商户/交易对象排行榜

        Args:
            top_data: 排行数据，包含 counterparty, amount 等字段
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not top_data:
            self.logger.warning("排行数据为空")
            return None

        # 提取数据
        merchants = [d['counterparty'][:15] + '...' if len(d['counterparty']) > 15
                     else d['counterparty'] for d in top_data]
        amounts = [d['amount'] for d in top_data]

        # 创建图表
        fig, ax = plt.subplots(figsize=(10, 6), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 绘制水平柱状图
        y_pos = np.arange(len(merchants))
        colors_gradient = plt.cm.Reds(np.linspace(0.4, 0.8, len(merchants)))

        bars = ax.barh(y_pos, amounts, color=colors_gradient, alpha=0.8)

        # 在柱子上显示金额
        for i, (bar, amount) in enumerate(zip(bars, amounts)):
            width = bar.get_width()
            ax.text(width, bar.get_y() + bar.get_height() / 2,
                    f'¥{amount:,.2f}',
                    ha='left', va='center', fontsize=9, fontweight='bold')

        # 设置标签
        ax.set_yticks(y_pos)
        ax.set_yticklabels(merchants)
        ax.invert_yaxis()  # 最大值在顶部

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
        ax.set_xlabel('金额 (元)', fontsize=12)

        # 添加网格
        ax.grid(True, axis='x', alpha=0.3, linestyle='--')

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"top_merchants_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("排行榜已保存: %s", filepath)
        return filepath

    @log_method
    def generate_comparison_bar_chart(
        self,
        summary_data: Dict[str, float],
        title: str = "收支对比",
        filename: Optional[str] = None
    ) -> Path:
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
        categories = ['收入', '支出', '净收入']
        values = [
            summary_data.get('total_income', 0),
            summary_data.get('total_expense', 0),
            summary_data.get('net_income', 0)
        ]
        colors = [COLORS['income'], COLORS['expense'], COLORS['net']]

        # 创建图表
        fig, ax = plt.subplots(figsize=(8, 6), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 绘制柱状图
        x_pos = np.arange(len(categories))
        bars = ax.bar(x_pos, values, color=colors, alpha=0.8, width=0.6)

        # 在柱子上显示金额
        for bar, value in zip(bars, values):
            height = bar.get_height()
            ax.text(bar.get_x() + bar.get_width() / 2, height,
                    f'¥{value:,.2f}',
                    ha='center', va='bottom' if value >= 0 else 'top',
                    fontsize=11, fontweight='bold')

        # 设置标签
        ax.set_xticks(x_pos)
        ax.set_xticklabels(categories, fontsize=12)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
        ax.set_ylabel('金额 (元)', fontsize=12)

        # 添加零线
        ax.axhline(y=0, color='gray', linestyle='-', linewidth=1, alpha=0.5)

        # 添加网格
        ax.grid(True, axis='y', alpha=0.3, linestyle='--')

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("对比图已保存: %s", filepath)
        return filepath

    @log_method
    def generate_heatmap(
        self,
        bills_data: List[Dict[str, Any]],
        title: str = "消费热力图",
        filename: Optional[str] = None
    ) -> Path:
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
        df['date'] = pd.to_datetime(df['date'])
        df['amount'] = pd.to_numeric(df['amount'], errors='coerce')

        # 只统计支出
        df_expense = df[df['type'] == '支出'].copy()

        if df_expense.empty:
            self.logger.warning("没有支出数据")
            return None

        # 添加星期和小时字段
        df_expense['weekday'] = df_expense['date'].dt.dayofweek
        df_expense['hour'] = df_expense['date'].dt.hour

        # 按星期和小时分组统计
        pivot_data = df_expense.groupby(['weekday', 'hour'])['amount'].sum().unstack(fill_value=0)

        # 确保所有小时都有数据
        for hour in range(24):
            if hour not in pivot_data.columns:
                pivot_data[hour] = 0
        pivot_data = pivot_data.sort_index(axis=1)

        # 创建图表
        fig, ax = plt.subplots(figsize=(14, 6), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 绘制热力图
        im = ax.imshow(pivot_data.values, cmap='YlOrRd', aspect='auto', alpha=0.8)

        # 设置坐标轴
        weekdays = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']
        ax.set_yticks(np.arange(len(weekdays)))
        ax.set_yticklabels(weekdays)

        hours = list(range(24))
        ax.set_xticks(np.arange(len(hours)))
        ax.set_xticklabels(hours)
        ax.set_xlabel('小时', fontsize=12)

        # 设置标题
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)

        # 添加颜色条
        cbar = plt.colorbar(im, ax=ax)
        cbar.set_label('消费金额 (元)', rotation=270, labelpad=20, fontsize=10)

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"heatmap_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("热力图已保存: %s", filepath)
        return filepath

    @log_method
    def generate_budget_progress_chart(
        self,
        budget_data: Dict[str, Any],
        title: str = "预算执行进度",
        filename: Optional[str] = None
    ) -> Path:
        """
        生成预算执行进度图

        Args:
            budget_data: 预算数据，包含各分类的预算和实际支出
            title: 图表标题
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        if not budget_data:
            self.logger.warning("预算数据为空")
            return None

        # 提取数据
        categories = []
        budgets = []
        actuals = []
        for category, data in budget_data.items():
            if isinstance(data, dict) and 'budget' in data and 'actual' in data:
                categories.append(category)
                budgets.append(data['budget'])
                actuals.append(data['actual'])

        if not categories:
            self.logger.warning("没有可显示的预算数据")
            return None

        # 创建图表
        fig, ax = plt.subplots(figsize=(12, len(categories) * 0.8 + 2), facecolor='#d0d0d0')
        ax.set_facecolor('#d0d0d0')

        # 设置位置
        y_pos = np.arange(len(categories))
        bar_height = 0.35

        # 绘制柱状图
        bars1 = ax.barh(y_pos - bar_height / 2, budgets, bar_height,
                        label='预算', color='#90CAF9', alpha=0.8)
        bars2 = ax.barh(y_pos + bar_height / 2, actuals, bar_height,
                        label='实际', alpha=0.8)

        # 根据超支情况着色
        colors = []
        for budget, actual in zip(budgets, actuals):
            if actual > budget:
                colors.append('#F44336')  # 红色 - 超支
            elif actual > budget * 0.9:
                colors.append('#FF9800')  # 橙色 - 接近预算
            else:
                colors.append('#4CAF50')  # 绿色 - 良好

        for bar, color in zip(bars2, colors):
            bar.set_color(color)

        # 添加百分比标签
        for i, (budget, actual) in enumerate(zip(budgets, actuals)):
            if budget > 0:
                percentage = (actual / budget) * 100
                ax.text(max(budget, actual), y_pos[i],
                        f' {percentage:.1f}%',
                        va='center', ha='left', fontsize=9)

        # 设置标签
        ax.set_yticks(y_pos)
        ax.set_yticklabels(categories)

        # 设置标题和标签
        ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
        ax.set_xlabel('金额 (元)', fontsize=12)

        # 添加图例
        ax.legend(loc='best', fontsize=10)

        # 添加网格
        ax.grid(True, axis='x', alpha=0.3, linestyle='--')

        # 调整布局
        plt.tight_layout()

        # 保存图片
        if filename is None:
            filename = f"budget_progress_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("预算进度图已保存: %s", filepath)
        return filepath

    @log_method
    def generate_comprehensive_dashboard(
        self,
        report_data: Dict[str, Any],
        filename: Optional[str] = None
    ) -> Path:
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
        fig = plt.figure(figsize=(16, 12), facecolor='#d0d0d0')
        gs = fig.add_gridspec(2, 2, hspace=0.3, wspace=0.3)

        # 1. 收支对比 (左上)
        ax1 = fig.add_subplot(gs[0, 0])
        ax1.set_facecolor('#d0d0d0')
        summary = report_data.get('summary', {})
        if summary:
            categories = ['收入', '支出', '净收入']
            values = [
                summary.get('total_income', 0),
                summary.get('total_expense', 0),
                summary.get('net_income', 0)
            ]
            colors = [COLORS['income'], COLORS['expense'], COLORS['net']]
            bars = ax1.bar(categories, values, color=colors, alpha=0.8)

            for bar, value in zip(bars, values):
                height = bar.get_height()
                ax1.text(bar.get_x() + bar.get_width() / 2, height,
                        f'¥{value:,.0f}', ha='center', va='bottom', fontsize=9)

            ax1.set_title('收支概览', fontsize=14, fontweight='bold')
            ax1.set_ylabel('金额 (元)')
            ax1.grid(True, axis='y', alpha=0.3)

        # 2. 分类饼图 (右上)
        ax2 = fig.add_subplot(gs[0, 1])
        ax2.set_facecolor('#d0d0d0')
        by_category = report_data.get('by_category', {})
        if by_category:
            labels = list(by_category.keys())[:8]  # 最多显示8个
            values = [by_category[cat]['total'] for cat in labels]
            colors_pie = CATEGORY_COLORS[:len(labels)]

            wedges, texts, autotexts = ax2.pie(
                values, labels=labels, colors=colors_pie,
                autopct='%1.1f%%', startangle=90
            )
            for autotext in autotexts:
                autotext.set_color('white')
                autotext.set_fontsize(8)

            ax2.set_title('支出分类分布', fontsize=14, fontweight='bold')

        # 3. 趋势图 (左下)
        ax3 = fig.add_subplot(gs[1, 0])
        ax3.set_facecolor('#d0d0d0')
        trend = report_data.get('trend', [])
        if trend:
            dates = [datetime.strptime(d['date'], '%Y-%m-%d') for d in trend]
            income = [d['income'] for d in trend]
            expense = [d['expense'] for d in trend]

            ax3.plot(dates, income, label='收入', color=COLORS['income'], linewidth=2)
            ax3.plot(dates, expense, label='支出', color=COLORS['expense'], linewidth=2)

            ax3.xaxis.set_major_formatter(mdates.DateFormatter('%m-%d'))
            plt.setp(ax3.xaxis.get_majorticklabels(), rotation=45, ha='right')

            ax3.set_title('收支趋势', fontsize=14, fontweight='bold')
            ax3.set_ylabel('金额 (元)')
            ax3.legend(fontsize=9)
            ax3.grid(True, alpha=0.3)

        # 4. Top支出 (右下)
        ax4 = fig.add_subplot(gs[1, 1])
        ax4.set_facecolor('#d0d0d0')
        top_expenses = report_data.get('top_expenses', [])[:8]
        if top_expenses:
            merchants = [e['counterparty'][:12] + '...' if len(e['counterparty']) > 12
                        else e['counterparty'] for e in top_expenses]
            amounts = [e['amount'] for e in top_expenses]

            y_pos = np.arange(len(merchants))
            bars = ax4.barh(y_pos, amounts, color=COLORS['expense'], alpha=0.7)

            for bar, amount in zip(bars, amounts):
                width = bar.get_width()
                ax4.text(width, bar.get_y() + bar.get_height() / 2,
                        f' ¥{amount:,.0f}', va='center', fontsize=8)

            ax4.set_yticks(y_pos)
            ax4.set_yticklabels(merchants, fontsize=9)
            ax4.invert_yaxis()
            ax4.set_title('最大支出 Top 8', fontsize=14, fontweight='bold')
            ax4.set_xlabel('金额 (元)')
            ax4.grid(True, axis='x', alpha=0.3)

        # 总标题
        period = report_data.get('period', 'month')
        period_name = {'month': '月度', 'quarter': '季度', 'year': '年度'}.get(period, '')
        fig.suptitle(f'{period_name}账单分析仪表盘', fontsize=18, fontweight='bold', y=0.98)

        # 保存图片
        if filename is None:
            filename = f"dashboard_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"

        filepath = self.output_dir / filename
        plt.savefig(filepath, dpi=self.dpi, bbox_inches='tight')
        plt.close()

        self.logger.info("综合仪表盘已保存: %s", filepath)
        return filepath


# 全局实例
_chart_generator = ChartGenerator()


def generate_all_charts(report_data: Dict[str, Any], output_dir: Optional[Path] = None) -> Dict[str, Path]:
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
    if report_data.get('trend'):
        charts['trend'] = generator.generate_trend_chart(report_data['trend'])

    if report_data.get('by_category'):
        charts['category_pie'] = generator.generate_category_pie_chart(
            report_data['by_category'], chart_type='expense'
        )

    if report_data.get('top_expenses'):
        charts['top_expenses'] = generator.generate_top_merchants_chart(
            report_data['top_expenses'], title='最大支出 Top 10'
        )

    if report_data.get('summary'):
        charts['comparison'] = generator.generate_comparison_bar_chart(
            report_data['summary']
        )

    # 生成综合仪表盘
    charts['dashboard'] = generator.generate_comprehensive_dashboard(report_data)

    return charts
