from __future__ import annotations

from pathlib import Path
from typing import Any

import matplotlib.pyplot as plt
import pandas as pd
import pytest

from bill_analyser.core import report as report_module
from bill_analyser.core.report import ReportGenerator


@pytest.fixture
def sample_report_data() -> dict[str, Any]:
    """Shared analysis payload for report export tests."""
    return {
        "period": "2025-01",
        "trend": [
            {"date": "2025-01-01", "income": 200.0, "expense": 100.0, "net": 100.0},
            {"date": "2025-01-02", "income": 100.0, "expense": 60.0, "net": 40.0},
        ],
        "by_category": {
            "餐饮": {"count": 2, "total": 80.0, "average": 40.0},
            "交通": {"count": 1, "total": 20.0, "average": 20.0},
        },
        "by_type": {
            "收入": {"count": 2, "total": 300.0},
            "支出": {"count": 3, "total": 120.0},
        },
        "summary": {
            "total_income": 300.0,
            "total_expense": 120.0,
            "net_income": 180.0,
        },
        "total_records": 5,
        "start_date": "2025-01-01",
        "end_date": "2025-01-02",
        "generated_at": "2025-01-03T12:00:00",
    }



def test_report_generator_initializes_output_directory_and_matplotlib(tmp_path: Path) -> None:
    """初始化应创建输出目录并设置 matplotlib 参数。"""
    generator = ReportGenerator(output_dir=str(tmp_path / "reports"))

    assert generator.output_dir == tmp_path / "reports"
    assert generator.output_dir.exists() is True
    assert plt.rcParams["axes.unicode_minus"] is False
    assert tuple(plt.rcParams["figure.figsize"]) == (12.0, 8.0)
    assert "SimHei" in plt.rcParams["font.sans-serif"]


def test_report_generator_uses_default_output_dir_when_not_provided(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """未传 output_dir 时应回退到模块默认 OUTPUT_DIR。"""
    monkeypatch.setattr(report_module, "OUTPUT_DIR", tmp_path / "default_reports")

    generator = ReportGenerator()

    assert generator.output_dir == tmp_path / "default_reports"
    assert generator.output_dir.exists() is True



def test_chart_helpers_handle_empty_and_populated_data(
    tmp_path: Path,
    sample_report_data: dict[str, Any],
) -> None:
    """图表 helper 应在空数据和正常数据下都稳定工作。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    empty_fig, empty_axes = plt.subplots(1, 3)
    generator._generate_trend_chart({}, empty_axes[0])
    generator._generate_category_pie({}, empty_axes[1])
    generator._generate_type_bar({}, empty_axes[2])
    assert empty_axes[0].texts[0].get_text() == "无趋势数据"
    assert empty_axes[1].texts[0].get_text() == "无分类数据"
    assert empty_axes[2].texts[0].get_text() == "无类型数据"
    plt.close(empty_fig)

    full_fig, full_axes = plt.subplots(1, 3)
    generator._generate_trend_chart(sample_report_data, full_axes[0])
    generator._generate_category_pie(sample_report_data, full_axes[1])
    generator._generate_type_bar(sample_report_data, full_axes[2])
    assert full_axes[0].get_title() == "收支趋势图"
    assert full_axes[1].get_title() == "支出分类占比"
    assert full_axes[2].get_title() == "交易类型统计"
    plt.close(full_fig)


@pytest.mark.asyncio
async def test_export_report_dispatches_by_format_and_rejects_unknown_types(
    tmp_path: Path,
    sample_report_data: dict[str, Any],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """导出入口应分派到对应实现，并拒绝未知格式。"""
    generator = ReportGenerator(output_dir=str(tmp_path))
    called_formats: list[tuple[str, str]] = []

    async def fake_export_pdf(data: dict[str, Any], filename: str) -> str:
        called_formats.append(("pdf", filename))
        return str(tmp_path / f"{filename}.pdf")

    async def fake_export_excel(data: dict[str, Any], filename: str) -> str:
        called_formats.append(("excel", filename))
        return str(tmp_path / f"{filename}.xlsx")

    async def fake_export_html(data: dict[str, Any], filename: str) -> str:
        called_formats.append(("html", filename))
        return str(tmp_path / f"{filename}.html")

    monkeypatch.setattr(generator, "_export_pdf", fake_export_pdf)
    monkeypatch.setattr(generator, "_export_excel", fake_export_excel)
    monkeypatch.setattr(generator, "_export_html", fake_export_html)

    assert await generator.export_report(sample_report_data, format_type="pdf", filename="demo") == str(tmp_path / "demo.pdf")
    assert await generator.export_report(sample_report_data, format_type="excel", filename="demo") == str(tmp_path / "demo.xlsx")
    assert await generator.export_report(sample_report_data, format_type="html", filename="demo") == str(tmp_path / "demo.html")
    with pytest.raises(ValueError, match="不支持的格式"):
        await generator.export_report(sample_report_data, format_type="csv", filename="demo")

    assert called_formats == [("pdf", "demo"), ("excel", "demo"), ("html", "demo")]


@pytest.mark.asyncio
async def test_export_report_generates_default_filename_when_omitted(
    tmp_path: Path,
    sample_report_data: dict[str, Any],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """未传文件名时应自动生成 report_ 时间戳前缀。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    async def fake_export_pdf(_data: dict[str, Any], filename: str) -> str:
        return filename

    monkeypatch.setattr(generator, "_export_pdf", fake_export_pdf)

    generated_filename = await generator.export_report(sample_report_data, format_type="pdf")

    assert generated_filename.startswith("report_")


@pytest.mark.asyncio
async def test_export_report_sanitizes_user_supplied_filename(
    tmp_path: Path,
    sample_report_data: dict[str, Any],
) -> None:
    """导出报告应将用户传入的文件名限制在输出目录内。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    html_path = await generator.export_report(sample_report_data, format_type="html", filename="../escape")

    exported_path = Path(html_path)
    assert exported_path == tmp_path / "escape.html"
    assert exported_path.exists() is True
    assert not (tmp_path.parent / "escape.html").exists()


def test_report_filename_sanitizer_falls_back_for_empty_leaf_names() -> None:
    """报告文件名清理应拒绝空文件名和目录回退符。"""
    assert ReportGenerator._sanitize_report_filename("").startswith("report_")
    assert ReportGenerator._sanitize_report_filename("..").startswith("report_")


def test_report_filename_sanitizer_replaces_windows_unsafe_leaf_names() -> None:
    """报告文件名清理应替换 Windows ADS、通配符和控制字符。"""
    assert ReportGenerator._sanitize_report_filename("evil:ads") == "evil_ads"
    assert ReportGenerator._sanitize_report_filename("report?.html") == "report_.html"
    assert ReportGenerator._sanitize_report_filename("report\u0007name") == "report_name"
    assert ReportGenerator._sanitize_report_filename("CON").startswith("report_")
    assert ReportGenerator._sanitize_report_filename("LPT1.txt").startswith("report_")


@pytest.mark.asyncio
async def test_export_pdf_writes_report_file(tmp_path: Path, sample_report_data: dict[str, Any]) -> None:
    """PDF 导出应产出实际文件。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    pdf_path = await generator._export_pdf(sample_report_data, "sample_report")

    assert pdf_path.endswith("sample_report.pdf")
    assert Path(pdf_path).exists() is True


@pytest.mark.asyncio
async def test_export_excel_writes_expected_sheets(tmp_path: Path, sample_report_data: dict[str, Any]) -> None:
    """Excel 导出应包含摘要、趋势和分类统计工作表。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    excel_path = await generator._export_excel(sample_report_data, "sample_report")

    assert excel_path.endswith("sample_report.xlsx")
    assert Path(excel_path).exists() is True
    workbook = pd.ExcelFile(excel_path)
    assert set(workbook.sheet_names) == {"摘要", "趋势", "分类统计"}


@pytest.mark.asyncio
async def test_export_html_writes_summary_content(tmp_path: Path, sample_report_data: dict[str, Any]) -> None:
    """HTML 导出应写出报告标题和关键汇总字段。"""
    generator = ReportGenerator(output_dir=str(tmp_path))

    html_path = await generator._export_html(sample_report_data, "sample_report")
    html_text = Path(html_path).read_text(encoding="utf-8")

    assert html_path.endswith("sample_report.html")
    assert "账单分析报告 - 2025-01" in html_text
    assert "¥300.00" in html_text
    assert "2025-01-03T12:00:00" in html_text
