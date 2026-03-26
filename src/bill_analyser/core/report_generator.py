"""
Report Generator Module - 报表生成模块

生成HTML/PDF格式的账单分析报告。
"""

from datetime import datetime
from pathlib import Path
from typing import Dict, List, Any, Optional
import base64

from ..utils.logger import get_logger, log_method


class ReportGenerator:
    """报表生成器"""

    def __init__(self, output_dir: Optional[Path] = None):
        """
        初始化报表生成器

        Args:
            output_dir: 输出目录
        """
        self.logger = get_logger("ReportGenerator")
        self.output_dir = output_dir or Path("output/reports")
        self.output_dir.mkdir(parents=True, exist_ok=True)

    @log_method
    def generate_html_report(
        self, report_data: Dict[str, Any], filename: Optional[str] = None, include_charts: bool = True
    ) -> Path:
        """
        生成HTML报告

        Args:
            report_data: 报告数据
            filename: 输出文件名
            include_charts: 是否包含图表

        Returns:
            Path: HTML文件路径
        """
        if filename is None:
            period = report_data.get("period", "month")
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"bill_report_{period}_{timestamp}.html"

        filepath = self.output_dir / filename

        # 生成HTML内容
        html_content = self._generate_html_content(report_data, include_charts)

        # 写入文件
        with open(filepath, "w", encoding="utf-8") as f:
            f.write(html_content)

        self.logger.info("HTML报告已生成: %s", filepath)
        return filepath

    def _generate_html_content(self, report_data: Dict[str, Any], include_charts: bool) -> str:
        """生成HTML内容"""
        period = report_data.get("period", "month")
        period_name = {"month": "月度", "quarter": "季度", "year": "年度"}.get(period, "周期")

        start_date = report_data.get("start_date", "")
        end_date = report_data.get("end_date", "")
        generated_at = report_data.get("generated_at", datetime.now().isoformat())

        summary = report_data.get("summary", {})
        by_category = report_data.get("by_category", {})
        by_type = report_data.get("by_type", {})
        trend = report_data.get("trend", [])
        top_expenses = report_data.get("top_expenses", [])
        top_income = report_data.get("top_income", [])

        html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{period_name}账单分析报告</title>
    <style>
        {self._get_css_styles()}
    </style>
</head>
<body>
    <div class="container">
        <header class="report-header">
            <h1>{period_name}账单分析报告</h1>
            <p class="period">分析期间: {start_date} 至 {end_date}</p>
            <p class="generated">生成时间: {generated_at[:19]}</p>
        </header>

        <!-- 概览 -->
        <section class="summary-section">
            <h2>📊 财务概览</h2>
            <div class="summary-cards">
                <div class="card income">
                    <h3>总收入</h3>
                    <p class="amount">¥{summary.get("total_income", 0):,.2f}</p>
                </div>
                <div class="card expense">
                    <h3>总支出</h3>
                    <p class="amount">¥{summary.get("total_expense", 0):,.2f}</p>
                </div>
                <div class="card net">
                    <h3>净收入</h3>
                    <p class="amount">¥{summary.get("net_income", 0):,.2f}</p>
                </div>
                <div class="card total">
                    <h3>账单总数</h3>
                    <p class="amount">{report_data.get("total_records", 0)}</p>
                </div>
            </div>
        </section>

        <!-- 图表 -->
        {self._generate_charts_section(report_data) if include_charts else ""}

        <!-- 按类型统计 -->
        <section class="type-section">
            <h2>💳 交易类型统计</h2>
            <table class="data-table">
                <thead>
                    <tr>
                        <th>类型</th>
                        <th>笔数</th>
                        <th>总金额</th>
                        <th>平均金额</th>
                    </tr>
                </thead>
                <tbody>
                    {self._generate_type_rows(by_type)}
                </tbody>
            </table>
        </section>

        <!-- 按分类统计 -->
        <section class="category-section">
            <h2>🏷️ 分类统计</h2>
            <table class="data-table">
                <thead>
                    <tr>
                        <th>主分类</th>
                        <th>子分类</th>
                        <th>笔数</th>
                        <th>总金额</th>
                        <th>平均金额</th>
                    </tr>
                </thead>
                <tbody>
                    {self._generate_category_rows(by_category)}
                </tbody>
            </table>
        </section>

        <!-- Top支出 -->
        <section class="top-section">
            <h2>💰 最大支出 Top 10</h2>
            <table class="data-table">
                <thead>
                    <tr>
                        <th>日期</th>
                        <th>金额</th>
                        <th>交易对象</th>
                        <th>分类</th>
                        <th>描述</th>
                    </tr>
                </thead>
                <tbody>
                    {self._generate_top_expense_rows(top_expenses)}
                </tbody>
            </table>
        </section>

        <!-- Top收入 -->
        {self._generate_top_income_section(top_income)}

        <!-- 趋势数据 -->
        {self._generate_trend_section(trend)}

        <footer class="report-footer">
            <p>报告由账单管理系统自动生成</p>
        </footer>
    </div>
</body>
</html>
"""
        return html

    def _get_css_styles(self) -> str:
        """获取CSS样式"""
        return """
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Microsoft YaHei', 'SimHei', Arial, sans-serif;
            background-color: #f5f5f5;
            color: #333;
            line-height: 1.6;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #fff;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }

        .report-header {
            text-align: center;
            padding: 30px 0;
            border-bottom: 3px solid #1976D2;
            margin-bottom: 30px;
        }

        .report-header h1 {
            font-size: 32px;
            color: #1976D2;
            margin-bottom: 15px;
        }

        .period {
            font-size: 16px;
            color: #666;
            margin-bottom: 5px;
        }

        .generated {
            font-size: 14px;
            color: #999;
        }

        section {
            margin-bottom: 40px;
        }

        h2 {
            font-size: 24px;
            color: #333;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 2px solid #e0e0e0;
        }

        .summary-cards {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }

        .card {
            padding: 25px;
            border-radius: 8px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
            text-align: center;
        }

        .card h3 {
            font-size: 16px;
            color: #666;
            margin-bottom: 15px;
        }

        .card .amount {
            font-size: 28px;
            font-weight: bold;
        }

        .card.income {
            background: linear-gradient(135deg, #4CAF50 0%, #66BB6A 100%);
            color: white;
        }

        .card.expense {
            background: linear-gradient(135deg, #F44336 0%, #E57373 100%);
            color: white;
        }

        .card.net {
            background: linear-gradient(135deg, #2196F3 0%, #64B5F6 100%);
            color: white;
        }

        .card.total {
            background: linear-gradient(135deg, #FF9800 0%, #FFB74D 100%);
            color: white;
        }

        .card.income h3, .card.expense h3, .card.net h3, .card.total h3 {
            color: rgba(255,255,255,0.9);
        }

        .data-table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 15px;
        }

        .data-table thead {
            background-color: #1976D2;
            color: white;
        }

        .data-table th, .data-table td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #e0e0e0;
        }

        .data-table th {
            font-weight: bold;
            text-transform: uppercase;
            font-size: 13px;
        }

        .data-table tbody tr:hover {
            background-color: #f5f5f5;
        }

        .data-table .amount-col {
            text-align: right;
            font-weight: bold;
        }

        .data-table .income-amount {
            color: #4CAF50;
        }

        .data-table .expense-amount {
            color: #F44336;
        }

        .sub-category {
            padding-left: 30px;
            font-style: italic;
            color: #666;
            background-color: #fafafa;
        }

        .charts-section {
            margin-bottom: 40px;
        }

        .chart-container {
            margin-bottom: 30px;
            text-align: center;
        }

        .chart-container img {
            max-width: 100%;
            height: auto;
            border: 1px solid #e0e0e0;
            border-radius: 8px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
        }

        .chart-container h3 {
            margin-bottom: 15px;
            color: #666;
        }

        .report-footer {
            text-align: center;
            padding: 20px;
            margin-top: 40px;
            border-top: 2px solid #e0e0e0;
            color: #999;
            font-size: 14px;
        }

        .highlight {
            background-color: #FFF9C4;
            padding: 2px 5px;
            border-radius: 3px;
        }

        @media print {
            body {
                background-color: #fff;
            }
            .container {
                box-shadow: none;
            }
        }
        """

    def _generate_type_rows(self, by_type: Dict[str, Dict[str, Any]]) -> str:
        """生成交易类型表格行"""
        if not by_type:
            return '<tr><td colspan="4">暂无数据</td></tr>'

        rows = []
        for type_name, data in by_type.items():
            amount_class = "income-amount" if type_name == "收入" else "expense-amount"
            row = f"""
                <tr>
                    <td>{type_name}</td>
                    <td>{data.get("count", 0)}</td>
                    <td class="amount-col {amount_class}">¥{data.get("total", 0):,.2f}</td>
                    <td class="amount-col">¥{data.get("average", 0):,.2f}</td>
                </tr>
            """
            rows.append(row)

        return "".join(rows)

    def _generate_category_rows(self, by_category: Dict[str, Dict[str, Any]]) -> str:
        """生成分类表格行"""
        if not by_category:
            return '<tr><td colspan="5">暂无分类数据</td></tr>'

        rows = []
        for main_cat, data in by_category.items():
            # 主分类行
            row = f"""
                <tr>
                    <td><strong>{main_cat}</strong></td>
                    <td>-</td>
                    <td>{data.get("count", 0)}</td>
                    <td class="amount-col">¥{data.get("total", 0):,.2f}</td>
                    <td class="amount-col">¥{data.get("average", 0):,.2f}</td>
                </tr>
            """
            rows.append(row)

            # 子分类行
            sub_categories = data.get("sub_categories", {})
            for sub_cat, sub_data in sub_categories.items():
                sub_row = f"""
                    <tr class="sub-category">
                        <td></td>
                        <td>{sub_cat}</td>
                        <td>{sub_data.get("count", 0)}</td>
                        <td class="amount-col">¥{sub_data.get("total", 0):,.2f}</td>
                        <td class="amount-col">-</td>
                    </tr>
                """
                rows.append(sub_row)

        return "".join(rows)

    def _generate_top_expense_rows(self, top_expenses: List[Dict[str, Any]]) -> str:
        """生成最大支出表格行"""
        if not top_expenses:
            return '<tr><td colspan="5">暂无数据</td></tr>'

        rows = []
        for expense in top_expenses:
            row = f"""
                <tr>
                    <td>{expense.get("date", "")}</td>
                    <td class="amount-col expense-amount">¥{expense.get("amount", 0):,.2f}</td>
                    <td>{expense.get("counterparty", "")}</td>
                    <td>{expense.get("category", "-")}</td>
                    <td>{expense.get("description", "")[:50]}</td>
                </tr>
            """
            rows.append(row)

        return "".join(rows)

    def _generate_top_income_section(self, top_income: List[Dict[str, Any]]) -> str:
        """生成最大收入区块"""
        if not top_income:
            return ""

        rows = []
        for income in top_income:
            row = f"""
                <tr>
                    <td>{income.get("date", "")}</td>
                    <td class="amount-col income-amount">¥{income.get("amount", 0):,.2f}</td>
                    <td>{income.get("counterparty", "")}</td>
                    <td>{income.get("description", "")[:50]}</td>
                </tr>
            """
            rows.append(row)

        return f"""
        <section class="top-section">
            <h2>💵 最大收入 Top 10</h2>
            <table class="data-table">
                <thead>
                    <tr>
                        <th>日期</th>
                        <th>金额</th>
                        <th>来源</th>
                        <th>描述</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(rows)}
                </tbody>
            </table>
        </section>
        """

    def _generate_trend_section(self, trend: List[Dict[str, Any]]) -> str:
        """生成趋势数据区块"""
        if not trend:
            return ""

        rows = []
        for item in trend[:30]:  # 最多显示30条
            net_class = "income-amount" if item.get("net", 0) >= 0 else "expense-amount"
            row = f"""
                <tr>
                    <td>{item.get("date", "")}</td>
                    <td class="amount-col income-amount">¥{item.get("income", 0):,.2f}</td>
                    <td class="amount-col expense-amount">¥{item.get("expense", 0):,.2f}</td>
                    <td class="amount-col {net_class}">¥{item.get("net", 0):,.2f}</td>
                </tr>
            """
            rows.append(row)

        return f"""
        <section class="trend-section">
            <h2>📈 每日收支趋势</h2>
            <table class="data-table">
                <thead>
                    <tr>
                        <th>日期</th>
                        <th>收入</th>
                        <th>支出</th>
                        <th>净收入</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(rows)}
                </tbody>
            </table>
        </section>
        """

    def _generate_charts_section(self, report_data: Dict[str, Any]) -> str:
        """生成图表区块"""
        charts = report_data.get("charts", {})
        if not charts:
            return ""

        chart_sections = []

        # 添加仪表盘
        if "dashboard" in charts:
            chart_sections.append(f"""
                <section class="charts-section">
                    <h2>📊 数据可视化仪表盘</h2>
                    <div class="chart-container">
                        <img src="{charts["dashboard"]}" alt="综合仪表盘">
                    </div>
                </section>
            """)

        # 其他图表
        chart_titles = {
            "trend": "收支趋势图",
            "category_pie": "分类占比图",
            "top_expenses": "最大支出排行",
            "comparison": "收支对比图",
        }

        other_charts = []
        for key, title in chart_titles.items():
            if key in charts:
                other_charts.append(f"""
                    <div class="chart-container">
                        <h3>{title}</h3>
                        <img src="{charts[key]}" alt="{title}">
                    </div>
                """)

        if other_charts:
            chart_sections.append(f"""
                <section class="charts-section">
                    <h2>📈 详细图表</h2>
                    {"".join(other_charts)}
                </section>
            """)

        return "".join(chart_sections)

    @log_method
    def generate_markdown_report(self, report_data: Dict[str, Any], filename: Optional[str] = None) -> Path:
        """
        生成Markdown报告

        Args:
            report_data: 报告数据
            filename: 输出文件名

        Returns:
            Path: Markdown文件路径
        """
        if filename is None:
            period = report_data.get("period", "month")
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"bill_report_{period}_{timestamp}.md"

        filepath = self.output_dir / filename

        # 生成Markdown内容
        md_content = self._generate_markdown_content(report_data)

        # 写入文件
        with open(filepath, "w", encoding="utf-8") as f:
            f.write(md_content)

        self.logger.info("Markdown报告已生成: %s", filepath)
        return filepath

    def _generate_markdown_content(self, report_data: Dict[str, Any]) -> str:
        """生成Markdown内容"""
        period = report_data.get("period", "month")
        period_name = {"month": "月度", "quarter": "季度", "year": "年度"}.get(period, "周期")

        summary = report_data.get("summary", {})
        by_category = report_data.get("by_category", {})
        by_type = report_data.get("by_type", {})
        top_expenses = report_data.get("top_expenses", [])

        md = f"""# {period_name}账单分析报告

**分析期间**: {report_data.get("start_date", "")} 至 {report_data.get("end_date", "")}  
**生成时间**: {report_data.get("generated_at", "")[:19]}

---

## 📊 财务概览

| 项目 | 金额 |
|------|------|
| **总收入** | ¥{summary.get("total_income", 0):,.2f} |
| **总支出** | ¥{summary.get("total_expense", 0):,.2f} |
| **净收入** | ¥{summary.get("net_income", 0):,.2f} |
| **账单总数** | {report_data.get("total_records", 0)} 条 |

---

## 💳 交易类型统计

| 类型 | 笔数 | 总金额 | 平均金额 |
|------|------|--------|----------|
"""

        for type_name, data in by_type.items():
            md += f"| {type_name} | {data.get('count', 0)} | ¥{data.get('total', 0):,.2f} | ¥{data.get('average', 0):,.2f} |\n"

        md += "\n---\n\n## 🏷️ 分类统计\n\n"

        for main_cat, data in by_category.items():
            md += f"\n### {main_cat}\n\n"
            md += f"- **笔数**: {data.get('count', 0)}\n"
            md += f"- **总金额**: ¥{data.get('total', 0):,.2f}\n"
            md += f"- **平均金额**: ¥{data.get('average', 0):,.2f}\n"

            sub_categories = data.get("sub_categories", {})
            if sub_categories:
                md += "\n**子分类**:\n\n"
                for sub_cat, sub_data in sub_categories.items():
                    md += f"- {sub_cat}: {sub_data.get('count', 0)}笔, ¥{sub_data.get('total', 0):,.2f}\n"

        md += "\n---\n\n## 💰 最大支出 Top 10\n\n"
        md += "| 日期 | 金额 | 交易对象 | 分类 | 描述 |\n"
        md += "|------|------|----------|------|------|\n"

        for expense in top_expenses:
            md += f"| {expense.get('date', '')} | ¥{expense.get('amount', 0):,.2f} | {expense.get('counterparty', '')} | {expense.get('category', '-')} | {expense.get('description', '')[:30]} |\n"

        md += "\n---\n\n*报告由账单管理系统自动生成*\n"

        return md


# 全局实例
_report_generator = ReportGenerator()


def generate_html_report(report_data: Dict[str, Any], output_dir: Optional[Path] = None) -> Path:
    """
    生成HTML报告

    Args:
        report_data: 报告数据
        output_dir: 输出目录

    Returns:
        Path: HTML文件路径
    """
    generator = ReportGenerator(output_dir=output_dir)
    return generator.generate_html_report(report_data)


def generate_markdown_report(report_data: Dict[str, Any], output_dir: Optional[Path] = None) -> Path:
    """
    生成Markdown报告

    Args:
        report_data: 报告数据
        output_dir: 输出目录

    Returns:
        Path: Markdown文件路径
    """
    generator = ReportGenerator(output_dir=output_dir)
    return generator.generate_markdown_report(report_data)
