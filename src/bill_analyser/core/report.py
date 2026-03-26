"""
Report Module - 报告导出模块

使用 matplotlib 生成图表，支持导出为 PDF/Excel/HTML。
"""

from datetime import datetime
from pathlib import Path
from typing import Dict, Any, Optional
import matplotlib
matplotlib.use('Agg')  # 使用非交互式后端
import matplotlib.pyplot as plt
import pandas as pd

from ..utils.logger import get_logger, log_method, log_step


class ReportGenerator:
    """报告生成器"""

    def __init__(self, output_dir: Optional[str] = None):
        """初始化"""
        self.logger = get_logger('ReportGenerator')

        if output_dir:
            self.output_dir = Path(output_dir)
        else:
            # 输出目录指向项目根目录的 output 文件夹
            self.output_dir = Path(__file__).parent.parent.parent / "output"

        self.output_dir.mkdir(parents=True, exist_ok=True)

        # 设置中文字体
        self._setup_matplotlib()

    def _setup_matplotlib(self):
        """配置 matplotlib"""
        plt.rcParams['font.sans-serif'] = ['SimHei', 'Microsoft YaHei', 'Arial Unicode MS']
        plt.rcParams['axes.unicode_minus'] = False
        plt.rcParams['figure.figsize'] = (12, 8)

    @log_method
    @log_step("生成收支趋势图")
    def _generate_trend_chart(self, data: Dict[str, Any], ax):
        """生成趋势图"""
        trend = data.get('trend', [])
        if not trend:
            ax.text(0.5, 0.5, '无趋势数据', ha='center', va='center')
            return

        df = pd.DataFrame(trend)
        df['date'] = pd.to_datetime(df['date'])

        ax.plot(df['date'], df['income'], marker='o', label='收入', linewidth=2)
        ax.plot(df['date'], df['expense'], marker='s', label='支出', linewidth=2)
        ax.plot(df['date'], df['net'], marker='^', label='净收入', linewidth=2)

        ax.set_title('收支趋势图', fontsize=16, fontweight='bold')
        ax.set_xlabel('日期')
        ax.set_ylabel('金额（元）')
        ax.legend()
        ax.grid(True, alpha=0.3)
        plt.setp(ax.xaxis.get_majorticklabels(), rotation=45)

    @log_method
    @log_step("生成分类饼图")
    def _generate_category_pie(self, data: Dict[str, Any], ax):
        """生成分类饼图"""
        by_category = data.get('by_category', {})
        if not by_category:
            ax.text(0.5, 0.5, '无分类数据', ha='center', va='center')
            return

        labels = list(by_category.keys())
        sizes = [cat['total'] for cat in by_category.values()]

        ax.pie(sizes, labels=labels, autopct='%1.1f%%', startangle=90)
        ax.set_title('支出分类占比', fontsize=16, fontweight='bold')

    @log_method
    @log_step("生成类型对比图")
    def _generate_type_bar(self, data: Dict[str, Any], ax):
        """生成类型对比柱状图"""
        by_type = data.get('by_type', {})
        if not by_type:
            ax.text(0.5, 0.5, '无类型数据', ha='center', va='center')
            return

        types = list(by_type.keys())
        totals = [t['total'] for t in by_type.values()]
        counts = [t['count'] for t in by_type.values()]

        x = range(len(types))
        width = 0.35

        ax.bar([i - width/2 for i in x], totals, width, label='总金额')
        ax2 = ax.twinx()
        ax2.bar([i + width/2 for i in x], counts, width, label='交易笔数', color='orange')

        ax.set_title('交易类型统计', fontsize=16, fontweight='bold')
        ax.set_xlabel('交易类型')
        ax.set_ylabel('总金额（元）')
        ax2.set_ylabel('交易笔数')
        ax.set_xticks(x)
        ax.set_xticklabels(types)
        ax.legend(loc='upper left')
        ax2.legend(loc='upper right')

    @log_method
    @log_step("导出报告")
    async def export_report(self, data: Dict[str, Any],
                           format_type: str = 'pdf',
                           filename: Optional[str] = None) -> str:
        """
        导出报告

        Args:
            data: 分析数据
            format_type: 导出格式 ('pdf', 'excel', 'html')
            filename: 文件名（不含扩展名）

        Returns:
            str: 导出文件路径
        """
        if not filename:
            timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
            filename = f"report_{timestamp}"

        if format_type == 'pdf':
            return await self._export_pdf(data, filename)
        elif format_type == 'excel':
            return await self._export_excel(data, filename)
        elif format_type == 'html':
            return await self._export_html(data, filename)
        else:
            self.logger.error("不支持的格式: %s", format_type)
            raise ValueError(f"不支持的格式: {format_type}")

    async def _export_pdf(self, data: Dict[str, Any], filename: str) -> str:
        """导出为PDF"""
        output_path = self.output_dir / f"{filename}.pdf"

        fig, axes = plt.subplots(2, 2, figsize=(16, 12))
        fig.suptitle(f"账单分析报告 - {data.get('period', '')}", fontsize=20, fontweight='bold')

        # 趋势图
        self._generate_trend_chart(data, axes[0, 0])

        # 饼图
        self._generate_category_pie(data, axes[0, 1])

        # 柱状图
        self._generate_type_bar(data, axes[1, 0])

        # 摘要文本
        summary = data.get('summary', {})
        axes[1, 1].axis('off')
        summary_text = f"""
        统计摘要

        总收入: ¥{summary.get('total_income', 0):,.2f}
        总支出: ¥{summary.get('total_expense', 0):,.2f}
        净收入: ¥{summary.get('net_income', 0):,.2f}

        记录总数: {data.get('total_records', 0)}
        统计周期: {data.get('start_date', '')} ~ {data.get('end_date', '')}
        生成时间: {data.get('generated_at', '')}
        """
        axes[1, 1].text(0.1, 0.5, summary_text, fontsize=12, verticalalignment='center')

        plt.tight_layout()
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()

        self.logger.info("PDF报告已生成: %s", output_path)
        return str(output_path)

    async def _export_excel(self, data: Dict[str, Any], filename: str) -> str:
        """导出为Excel"""
        output_path = self.output_dir / f"{filename}.xlsx"

        with pd.ExcelWriter(output_path, engine='openpyxl') as writer:
            # 摘要
            summary_df = pd.DataFrame([data.get('summary', {})])
            summary_df.to_excel(writer, sheet_name='摘要', index=False)

            # 趋势
            if data.get('trend'):
                trend_df = pd.DataFrame(data['trend'])
                trend_df.to_excel(writer, sheet_name='趋势', index=False)

            # 分类统计
            if data.get('by_category'):
                category_data = []
                for cat, values in data['by_category'].items():
                    category_data.append({
                        '分类': cat,
                        '笔数': values['count'],
                        '总金额': values['total'],
                        '平均金额': values['average']
                    })
                cat_df = pd.DataFrame(category_data)
                cat_df.to_excel(writer, sheet_name='分类统计', index=False)

        self.logger.info("Excel报告已生成: %s", output_path)
        return str(output_path)

    async def _export_html(self, data: Dict[str, Any], filename: str) -> str:
        """导出为HTML"""
        output_path = self.output_dir / f"{filename}.html"

        html = f"""
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="UTF-8">
            <title>账单分析报告</title>
            <style>
                body {{ font-family: Arial, sans-serif; margin: 20px; }}
                h1 {{ color: #333; }}
                table {{ border-collapse: collapse; width: 100%; margin: 20px 0; }}
                th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
                th {{ background-color: #4CAF50; color: white; }}
            </style>
        </head>
        <body>
            <h1>账单分析报告 - {data.get('period', '')}</h1>
            <h2>统计摘要</h2>
            <table>
                <tr><th>项目</th><th>金额</th></tr>
                <tr><td>总收入</td><td>¥{data.get('summary', {}).get('total_income', 0):,.2f}</td></tr>
                <tr><td>总支出</td><td>¥{data.get('summary', {}).get('total_expense', 0):,.2f}</td></tr>
                <tr><td>净收入</td><td>¥{data.get('summary', {}).get('net_income', 0):,.2f}</td></tr>
            </table>
            <p>生成时间: {data.get('generated_at', '')}</p>
        </body>
        </html>
        """

        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(html)

        self.logger.info("HTML报告已生成: %s", output_path)
        return str(output_path)
