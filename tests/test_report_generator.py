"""
测试报表生成模块
"""

import pytest
from pathlib import Path
from datetime import datetime
import sys

# 添加项目根目录到路径
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.core.report_generator import ReportGenerator


@pytest.fixture
def report_generator(tmp_path):
    """创建报表生成器实例"""
    return ReportGenerator(output_dir=tmp_path)


@pytest.fixture
def sample_report_data():
    """示例报告数据"""
    return {
        'period': 'month',
        'start_date': '2024-01-01',
        'end_date': '2024-01-31',
        'total_records': 150,
        'generated_at': datetime.now().isoformat(),
        'summary': {
            'total_income': 15000.00,
            'total_expense': 12800.50,
            'net_income': 2199.50
        },
        'by_category': {
            '餐饮': {
                'count': 45,
                'total': 3500.00,
                'average': 77.78,
                'sub_categories': {
                    '外卖': {'count': 25, 'total': 2000.00},
                    '餐厅': {'count': 20, 'total': 1500.00}
                }
            },
            '交通': {
                'count': 30,
                'total': 1200.00,
                'average': 40.00,
                'sub_categories': {
                    '地铁': {'count': 20, 'total': 600.00},
                    '网约车': {'count': 10, 'total': 600.00}
                }
            },
            '购物': {
                'count': 15,
                'total': 2800.00,
                'average': 186.67,
                'sub_categories': {}
            }
        },
        'by_type': {
            '收入': {
                'count': 10,
                'total': 15000.00,
                'average': 1500.00
            },
            '支出': {
                'count': 140,
                'total': 12800.50,
                'average': 91.43
            }
        },
        'trend': [
            {'date': '2024-01-01', 'income': 1000, 'expense': 800, 'net': 200},
            {'date': '2024-01-02', 'income': 1100, 'expense': 900, 'net': 200},
            {'date': '2024-01-03', 'income': 900, 'expense': 850, 'net': 50}
        ],
        'top_expenses': [
            {
                'date': '2024-01-15',
                'amount': 3500.00,
                'counterparty': '房租',
                'description': '一月房租',
                'category': '居住'
            },
            {
                'date': '2024-01-10',
                'amount': 1200.00,
                'counterparty': '超市',
                'description': '采购食材',
                'category': '餐饮'
            },
            {
                'date': '2024-01-05',
                'amount': 800.00,
                'counterparty': '电影院',
                'description': '观影消费',
                'category': '娱乐'
            }
        ],
        'top_income': [
            {
                'date': '2024-01-25',
                'amount': 8000.00,
                'counterparty': '公司',
                'description': '工资'
            },
            {
                'date': '2024-01-15',
                'amount': 5000.00,
                'counterparty': '项目方',
                'description': '项目款'
            }
        ]
    }


def test_generate_html_report(report_generator, sample_report_data):
    """测试生成HTML报告"""
    filepath = report_generator.generate_html_report(sample_report_data)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.html'
    assert filepath.stat().st_size > 0

    # 检查HTML内容
    content = filepath.read_text(encoding='utf-8')
    assert '<!DOCTYPE html>' in content
    assert '账单分析报告' in content
    assert '财务概览' in content
    assert '¥15,000.00' in content  # 总收入
    assert '¥12,800.50' in content  # 总支出


def test_generate_html_report_without_charts(report_generator, sample_report_data):
    """测试生成不含图表的HTML报告"""
    filepath = report_generator.generate_html_report(
        sample_report_data,
        include_charts=False
    )

    assert filepath is not None
    assert filepath.exists()

    content = filepath.read_text(encoding='utf-8')
    # 检查没有charts-section章节
    assert 'charts-section' not in content or '<section class="charts-section">' not in content


def test_generate_markdown_report(report_generator, sample_report_data):
    """测试生成Markdown报告"""
    filepath = report_generator.generate_markdown_report(sample_report_data)

    assert filepath is not None
    assert filepath.exists()
    assert filepath.suffix == '.md'
    assert filepath.stat().st_size > 0

    # 检查Markdown内容
    content = filepath.read_text(encoding='utf-8')
    assert '# 月度账单分析报告' in content
    assert '## 📊 财务概览' in content
    assert '| **总收入**' in content
    assert '¥15,000.00' in content


def test_html_report_with_charts(report_generator, sample_report_data):
    """测试包含图表的HTML报告"""
    # 添加图表路径
    sample_report_data['charts'] = {
        'dashboard': 'output/charts/dashboard.png',
        'trend': 'output/charts/trend.png',
        'category_pie': 'output/charts/category_pie.png'
    }

    filepath = report_generator.generate_html_report(
        sample_report_data,
        include_charts=True
    )

    assert filepath is not None
    content = filepath.read_text(encoding='utf-8')
    assert 'charts-section' in content
    assert 'dashboard.png' in content


def test_custom_filename(report_generator, sample_report_data):
    """测试自定义文件名"""
    custom_name = "test_report.html"
    filepath = report_generator.generate_html_report(
        sample_report_data,
        filename=custom_name
    )

    assert filepath is not None
    assert filepath.name == custom_name


def test_minimal_report_data(report_generator):
    """测试最小化报告数据"""
    minimal_data = {
        'period': 'month',
        'start_date': '2024-01-01',
        'end_date': '2024-01-31',
        'total_records': 0,
        'generated_at': datetime.now().isoformat(),
        'summary': {
            'total_income': 0,
            'total_expense': 0,
            'net_income': 0
        },
        'by_category': {},
        'by_type': {},
        'trend': [],
        'top_expenses': [],
        'top_income': []
    }

    # HTML报告
    html_file = report_generator.generate_html_report(minimal_data)
    assert html_file is not None
    assert html_file.exists()

    # Markdown报告
    md_file = report_generator.generate_markdown_report(minimal_data)
    assert md_file is not None
    assert md_file.exists()


def test_html_table_generation(report_generator, sample_report_data):
    """测试HTML表格生成"""
    filepath = report_generator.generate_html_report(sample_report_data)
    content = filepath.read_text(encoding='utf-8')

    # 检查表格元素
    assert '<table' in content
    assert '<thead>' in content
    assert '<tbody>' in content

    # 检查数据
    assert '餐饮' in content
    assert '交通' in content
    assert '房租' in content


def test_css_styles_included(report_generator, sample_report_data):
    """测试CSS样式包含"""
    filepath = report_generator.generate_html_report(sample_report_data)
    content = filepath.read_text(encoding='utf-8')

    assert '<style>' in content
    assert 'container' in content
    assert 'summary-cards' in content
    assert 'data-table' in content


def test_period_name_mapping(report_generator):
    """测试周期名称映射"""
    for period, expected_name in [('month', '月度'), ('quarter', '季度'), ('year', '年度')]:
        data = {
            'period': period,
            'start_date': '2024-01-01',
            'end_date': '2024-01-31',
            'total_records': 0,
            'generated_at': datetime.now().isoformat(),
            'summary': {'total_income': 0, 'total_expense': 0, 'net_income': 0},
            'by_category': {},
            'by_type': {},
            'trend': [],
            'top_expenses': [],
            'top_income': []
        }

        filepath = report_generator.generate_html_report(data)
        content = filepath.read_text(encoding='utf-8')
        assert expected_name in content


def test_output_directory_creation(tmp_path):
    """测试输出目录自动创建"""
    output_dir = tmp_path / "reports" / "subfolder"
    generator = ReportGenerator(output_dir=output_dir)

    assert generator.output_dir.exists()
    assert generator.output_dir.is_dir()


def test_amount_formatting(report_generator, sample_report_data):
    """测试金额格式化"""
    filepath = report_generator.generate_html_report(sample_report_data)
    content = filepath.read_text(encoding='utf-8')

    # 检查千分位分隔符
    assert '15,000' in content or '15000' in content
    assert '12,800' in content or '12800' in content


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
