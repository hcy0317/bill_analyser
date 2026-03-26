"""
测试图表生成模块
"""

import pytest
from pathlib import Path
from datetime import datetime, timedelta
import sys

# 添加项目根目录到路径
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.utils.charts import ChartGenerator


@pytest.fixture
def chart_generator(tmp_path):
    """创建图表生成器实例"""
    return ChartGenerator(output_dir=tmp_path, dpi=72)


@pytest.fixture
def sample_trend_data():
    """示例趋势数据"""
    base_date = datetime(2024, 1, 1)
    data = []
    for i in range(30):
        date = base_date + timedelta(days=i)
        data.append({
            'date': date.strftime('%Y-%m-%d'),
            'income': 1000 + i * 100,
            'expense': 800 + i * 80,
            'net': 200 + i * 20
        })
    return data


@pytest.fixture
def sample_category_data():
    """示例分类数据"""
    return {
        '餐饮': {'total': 3500, 'count': 45, 'average': 77.78},
        '交通': {'total': 1200, 'count': 30, 'average': 40},
        '购物': {'total': 2800, 'count': 15, 'average': 186.67},
        '娱乐': {'total': 800, 'count': 10, 'average': 80},
        '居住': {'total': 4500, 'count': 5, 'average': 900}
    }


@pytest.fixture
def sample_top_expenses():
    """示例Top支出数据"""
    return [
        {
            'date': '2024-01-15',
            'amount': 3500,
            'counterparty': '房租',
            'description': '一月房租',
            'category': '居住'
        },
        {
            'date': '2024-01-10',
            'amount': 1200,
            'counterparty': '超市',
            'description': '采购食材',
            'category': '餐饮'
        },
        {
            'date': '2024-01-05',
            'amount': 800,
            'counterparty': '电影院',
            'description': '观影消费',
            'category': '娱乐'
        }
    ]


@pytest.fixture
def sample_summary():
    """示例汇总数据"""
    return {
        'total_income': 15000,
        'total_expense': 12800,
        'net_income': 2200
    }


@pytest.fixture
def sample_bills():
    """示例账单数据"""
    base_date = datetime(2024, 1, 1)
    bills = []
    for i in range(50):
        date = base_date + timedelta(hours=i * 12)
        bills.append({
            'date': date.strftime('%Y-%m-%d %H:%M:%S'),
            'type': '支出' if i % 3 != 0 else '收入',
            'amount': 100 + i * 10,
            'counterparty': f'商户{i % 10}',
            'description': f'交易描述{i}',
            'channel': '支付宝'
        })
    return bills


def test_generate_trend_chart(chart_generator, sample_trend_data):
    """测试生成趋势图"""
    filepath = chart_generator.generate_trend_chart(sample_trend_data)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_generate_category_pie_chart(chart_generator, sample_category_data):
    """测试生成分类饼图"""
    filepath = chart_generator.generate_category_pie_chart(
        sample_category_data,
        chart_type='expense'
    )

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_generate_top_merchants_chart(chart_generator, sample_top_expenses):
    """测试生成Top商户排行榜"""
    filepath = chart_generator.generate_top_merchants_chart(sample_top_expenses)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_generate_comparison_bar_chart(chart_generator, sample_summary):
    """测试生成收支对比图"""
    filepath = chart_generator.generate_comparison_bar_chart(sample_summary)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_generate_heatmap(chart_generator, sample_bills):
    """测试生成热力图"""
    filepath = chart_generator.generate_heatmap(sample_bills)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_generate_comprehensive_dashboard(chart_generator):
    """测试生成综合仪表盘"""
    report_data = {
        'period': 'month',
        'summary': {
            'total_income': 15000,
            'total_expense': 12800,
            'net_income': 2200
        },
        'by_category': {
            '餐饮': {'total': 3500, 'count': 45},
            '交通': {'total': 1200, 'count': 30},
            '购物': {'total': 2800, 'count': 15}
        },
        'trend': [
            {'date': '2024-01-01', 'income': 1000, 'expense': 800, 'net': 200},
            {'date': '2024-01-02', 'income': 1100, 'expense': 900, 'net': 200}
        ],
        'top_expenses': [
            {
                'date': '2024-01-15',
                'amount': 3500,
                'counterparty': '房租',
                'description': '一月房租',
                'category': '居住'
            }
        ]
    }

    filepath = chart_generator.generate_comprehensive_dashboard(report_data)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.png'
    assert filepath.stat().st_size > 0


def test_empty_data_handling(chart_generator):
    """测试空数据处理"""
    # 空趋势数据
    filepath = chart_generator.generate_trend_chart([])
    assert filepath is None

    # 空分类数据
    filepath = chart_generator.generate_category_pie_chart({})
    assert filepath is None

    # 空Top数据
    filepath = chart_generator.generate_top_merchants_chart([])
    assert filepath is None


def test_custom_filename(chart_generator, sample_trend_data):
    """测试自定义文件名"""
    custom_name = "test_trend_chart.png"
    filepath = chart_generator.generate_trend_chart(
        sample_trend_data,
        filename=custom_name
    )

    assert filepath is not None
    assert filepath.name == custom_name


def test_output_directory_creation(tmp_path):
    """测试输出目录自动创建"""
    output_dir = tmp_path / "charts" / "subfolder"
    generator = ChartGenerator(output_dir=output_dir)

    assert generator.output_dir.exists()
    assert generator.output_dir.is_dir()


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
