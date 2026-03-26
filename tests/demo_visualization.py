"""
数据可视化功能演示

展示图表生成和报表功能的使用方法。
"""

import asyncio
import sys
from pathlib import Path
from datetime import datetime, timedelta

# 添加项目根目录到路径
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.utils.charts import ChartGenerator
from src.core.report_generator import ReportGenerator


def create_mock_report_data():
    """创建模拟报告数据"""
    # 生成30天的趋势数据
    base_date = datetime.now() - timedelta(days=30)
    trend_data = []
    for i in range(30):
        date = base_date + timedelta(days=i)
        income = 500 + i * 50 + (i % 7) * 200  # 模拟周末收入增加
        expense = 400 + i * 30 + (i % 5) * 100  # 模拟消费波动
        trend_data.append({
            'date': date.strftime('%Y-%m-%d'),
            'income': income,
            'expense': expense,
            'net': income - expense
        })

    # 分类数据
    category_data = {
        '餐饮': {'total': 4500, 'count': 60, 'average': 75, 'sub_categories': {
            '外卖': {'count': 35, 'total': 2800},
            '餐厅': {'count': 25, 'total': 1700}
        }},
        '交通': {'total': 1800, 'count': 45, 'average': 40, 'sub_categories': {
            '地铁': {'count': 30, 'total': 900},
            '网约车': {'count': 15, 'total': 900}
        }},
        '购物': {'total': 3200, 'count': 18, 'average': 178, 'sub_categories': {
            '服装': {'count': 8, 'total': 1600},
            '日用品': {'count': 10, 'total': 1600}
        }},
        '娱乐': {'total': 1200, 'count': 15, 'average': 80, 'sub_categories': {
            '电影': {'count': 8, 'total': 640},
            '游戏': {'count': 7, 'total': 560}
        }},
        '居住': {'total': 3500, 'count': 3, 'average': 1167, 'sub_categories': {
            '房租': {'count': 1, 'total': 3000},
            '水电': {'count': 2, 'total': 500}
        }}
    }

    # Top支出
    top_expenses = [
        {'date': '2024-11-01', 'amount': 3000, 'counterparty': '房东', 'description': '十一月房租', 'category': '居住'},
        {'date': '2024-11-05', 'amount': 1200, 'counterparty': '商场', 'description': '购买冬装', 'category': '购物'},
        {'date': '2024-11-12', 'amount': 800, 'counterparty': '超市', 'description': '大采购', 'category': '餐饮'},
        {'date': '2024-11-15', 'amount': 600, 'counterparty': '加油站', 'description': '加油', 'category': '交通'},
        {'date': '2024-11-20', 'amount': 450, 'counterparty': '餐厅', 'description': '聚餐', 'category': '餐饮'}
    ]

    # 汇总数据
    total_income = sum(d['income'] for d in trend_data)
    total_expense = sum(d['expense'] for d in trend_data)

    report_data = {
        'period': 'month',
        'start_date': trend_data[0]['date'],
        'end_date': trend_data[-1]['date'],
        'total_records': 180,
        'generated_at': datetime.now().isoformat(),
        'summary': {
            'total_income': total_income,
            'total_expense': total_expense,
            'net_income': total_income - total_expense
        },
        'by_category': category_data,
        'by_type': {
            '收入': {'count': 30, 'total': total_income, 'average': total_income / 30},
            '支出': {'count': 150, 'total': total_expense, 'average': total_expense / 150}
        },
        'trend': trend_data,
        'top_expenses': top_expenses,
        'top_income': [
            {'date': '2024-11-25', 'amount': 10000, 'counterparty': '公司', 'description': '工资'}
        ]
    }

    return report_data


def demonstrate_charts():
    """演示图表生成"""
    print("\n" + "="*60)
    print("📊 图表生成功能演示")
    print("="*60)

    # 创建输出目录
    output_dir = Path('output/demo_charts')
    output_dir.mkdir(parents=True, exist_ok=True)

    # 创建图表生成器
    generator = ChartGenerator(output_dir=output_dir, dpi=100)

    # 准备数据
    report_data = create_mock_report_data()

    print("\n正在生成各类图表...")

    # 1. 趋势图
    print("\n1️⃣ 生成收支趋势图...")
    trend_file = generator.generate_trend_chart(
        report_data['trend'],
        title="月度收支趋势图"
    )
    print(f"   ✓ 保存到: {trend_file}")

    # 2. 分类饼图
    print("\n2️⃣ 生成分类饼图...")
    pie_file = generator.generate_category_pie_chart(
        report_data['by_category'],
        chart_type='expense',
        title="支出分类分布"
    )
    print(f"   ✓ 保存到: {pie_file}")

    # 3. Top排行榜
    print("\n3️⃣ 生成Top支出排行榜...")
    top_file = generator.generate_top_merchants_chart(
        report_data['top_expenses'],
        title="最大支出 Top 5"
    )
    print(f"   ✓ 保存到: {top_file}")

    # 4. 收支对比
    print("\n4️⃣ 生成收支对比图...")
    comparison_file = generator.generate_comparison_bar_chart(
        report_data['summary'],
        title="月度收支对比"
    )
    print(f"   ✓ 保存到: {comparison_file}")

    # 5. 综合仪表盘
    print("\n5️⃣ 生成综合仪表盘...")
    dashboard_file = generator.generate_comprehensive_dashboard(report_data)
    print(f"   ✓ 保存到: {dashboard_file}")

    # 更新报告数据中的图表路径
    report_data['charts'] = {
        'trend': str(trend_file),
        'category_pie': str(pie_file),
        'top_expenses': str(top_file),
        'comparison': str(comparison_file),
        'dashboard': str(dashboard_file)
    }

    print(f"\n✅ 共生成 5 个图表文件")
    print(f"📁 保存位置: {output_dir.absolute()}")

    return report_data


def demonstrate_reports(report_data):
    """演示报表生成"""
    print("\n" + "="*60)
    print("📄 报表生成功能演示")
    print("="*60)

    # 创建输出目录
    output_dir = Path('output/demo_reports')
    output_dir.mkdir(parents=True, exist_ok=True)

    # 创建报表生成器
    generator = ReportGenerator(output_dir=output_dir)

    print("\n正在生成报表文件...")

    # 1. HTML报表(含图表)
    print("\n1️⃣ 生成HTML报表(含图表)...")
    html_file = generator.generate_html_report(
        report_data,
        filename='demo_report_with_charts.html',
        include_charts=True
    )
    print(f"   ✓ 保存到: {html_file}")
    print(f"   💡 可用浏览器打开查看")

    # 2. HTML报表(不含图表)
    print("\n2️⃣ 生成HTML报表(不含图表)...")
    html_simple_file = generator.generate_html_report(
        report_data,
        filename='demo_report_simple.html',
        include_charts=False
    )
    print(f"   ✓ 保存到: {html_simple_file}")

    # 3. Markdown报表
    print("\n3️⃣ 生成Markdown报表...")
    md_file = generator.generate_markdown_report(
        report_data,
        filename='demo_report.md'
    )
    print(f"   ✓ 保存到: {md_file}")
    print(f"   💡 可用文本编辑器打开")

    print(f"\n✅ 共生成 3 个报表文件")
    print(f"📁 保存位置: {output_dir.absolute()}")


def show_summary(report_data):
    """显示数据摘要"""
    print("\n" + "="*60)
    print("📊 数据摘要")
    print("="*60)

    summary = report_data['summary']
    print(f"\n时间范围: {report_data['start_date']} 至 {report_data['end_date']}")
    print(f"账单总数: {report_data['total_records']} 条")
    print(f"\n💰 财务概览:")
    print(f"   总收入: ¥{summary['total_income']:,.2f}")
    print(f"   总支出: ¥{summary['total_expense']:,.2f}")
    print(f"   净收入: ¥{summary['net_income']:,.2f}")

    print(f"\n🏷️ 支出分类(Top 5):")
    sorted_categories = sorted(
        report_data['by_category'].items(),
        key=lambda x: x[1]['total'],
        reverse=True
    )
    for i, (cat, data) in enumerate(sorted_categories[:5], 1):
        percentage = (data['total'] / summary['total_expense']) * 100
        print(f"   {i}. {cat:6s}: ¥{data['total']:7,.2f} ({percentage:5.1f}%)")


def main():
    """主函数"""
    print("\n" + "="*60)
    print("🎨 账单管理系统 - 数据可视化功能演示")
    print("="*60)

    try:
        # 准备数据并生成图表
        report_data = demonstrate_charts()

        # 生成报表
        demonstrate_reports(report_data)

        # 显示摘要
        show_summary(report_data)

        print("\n" + "="*60)
        print("✨ 演示完成!")
        print("="*60)
        print("\n💡 提示:")
        print("   - 查看 output/demo_charts/ 目录的图表文件")
        print("   - 用浏览器打开 HTML 报表查看完整效果")
        print("   - Markdown 报表可用任意文本编辑器查看")
        print("\n📚 更多信息请查看: docs/VISUALIZATION_GUIDE.md")

    except Exception as e:  # pylint: disable=broad-except
        print(f"\n❌ 错误: {e}")
        import traceback
        traceback.print_exc()
        return 1

    return 0


if __name__ == '__main__':
    sys.exit(main())
