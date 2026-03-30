from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from pathlib import Path
from typing import Any, cast

import pandas as pd
import pytest

from bill_analyser.core import analyzer as analyzer_module
from bill_analyser.core.analyzer import Analyzer


SAMPLE_BILLS: list[dict[str, Any]] = [
    {
        "date": "2025-01-03",
        "type": "支出",
        "amount": 120.0,
        "counterparty": "餐厅A",
        "description": "午餐",
        "main_category": "餐饮",
        "sub_category": "午餐",
    },
    {
        "date": "2025-01-05",
        "type": "支出",
        "amount": 80.0,
        "counterparty": "超市B",
        "description": "买菜",
        "main_category": "餐饮",
        "sub_category": "食材",
    },
    {
        "date": "2025-01-08",
        "type": "收入",
        "amount": 5000.0,
        "counterparty": "公司",
        "description": "工资",
        "main_category": None,
        "sub_category": None,
    },
]


class FakeAnalyzerDB:
    """Minimal async DB stub for Analyzer tests."""

    def __init__(self, responses: list[list[dict[str, Any]]] | None = None, default_response: list[dict[str, Any]] | None = None) -> None:
        self.responses = list(responses or [])
        self.default_response = list(default_response or [])
        self.calls: list[dict[str, Any] | None] = []

    async def get_bills(self, filters: dict[str, Any] | None = None, *_args: object, **_kwargs: object) -> list[dict[str, Any]]:
        self.calls.append(filters.copy() if filters else None)
        if self.responses:
            return self.responses.pop(0)
        return list(self.default_response)


class FakeChartGenerator:
    """Fake chart generator returning deterministic paths."""

    def __init__(self, output_dir: Path) -> None:
        self.output_dir = output_dir
        self.calls: list[tuple[str, Any]] = []

    def generate_trend_chart(self, trend_data: list[dict[str, Any]], title: str, filename: str | None = None) -> Path:
        self.calls.append(("trend", title))
        return self.output_dir / (filename or "trend.png")

    def generate_category_pie_chart(self, category_data: dict[str, Any], chart_type: str, filename: str | None = None) -> Path:
        self.calls.append(("pie", chart_type))
        return self.output_dir / (filename or f"{chart_type}.png")

    def generate_top_merchants_chart(self, top_data: list[dict[str, Any]], filename: str | None = None) -> Path:
        self.calls.append(("top", len(top_data)))
        return self.output_dir / (filename or "top.png")

    def generate_comparison_bar_chart(self, summary_data: dict[str, Any], title: str, filename: str | None = None) -> Path:
        self.calls.append(("comparison", title))
        return self.output_dir / (filename or "comparison.png")

    def generate_heatmap(self, bills: list[dict[str, Any]], filename: str | None = None) -> Path:
        self.calls.append(("heatmap", len(bills)))
        return self.output_dir / (filename or "heatmap.png")

    def generate_comprehensive_dashboard(self, report_data: dict[str, Any], filename: str | None = None) -> Path:
        self.calls.append(("dashboard", report_data.get("period")))
        return self.output_dir / (filename or "dashboard.png")


@pytest.mark.asyncio
async def test_generate_report_uses_cache_and_returns_empty_report_when_no_bills(tmp_path: Path) -> None:
    """generate_report 应覆盖缓存命中和空报告路径。"""
    db = FakeAnalyzerDB(responses=[SAMPLE_BILLS])
    analyzer = Analyzer(db=cast(Any, db), output_dir=tmp_path)

    cache_key = analyzer._get_cache_key("month", {"channel": "bank"})
    assert cache_key.startswith("month_")
    assert analyzer._is_cache_valid("missing") is False

    report = await analyzer.generate_report("month", {"channel": "bank"})
    assert report["period"] == "month"
    assert report["total_records"] == 3
    assert report["summary"]["total_income"] == 5000.0
    assert report["summary"]["total_expense"] == 200.0
    assert report["summary"]["net_income"] == 4800.0
    assert report["by_category"]["餐饮"]["count"] == 2
    assert report["by_type"]["支出"]["count"] == 2
    assert report["top_expenses"][0]["amount"] == 120.0
    assert report["top_income"][0]["amount"] == 5000.0
    assert analyzer._is_cache_valid(analyzer._get_cache_key("month", {"channel": "bank"})) is True
    assert len(db.calls) == 1

    cached_report = await analyzer.generate_report("month", {"channel": "bank"})
    assert cached_report is report
    assert len(db.calls) == 1

    analyzer.clear_cache()
    assert analyzer._cache == {}

    empty_analyzer = Analyzer(db=cast(Any, FakeAnalyzerDB(default_response=[])), output_dir=tmp_path)
    empty_report = await empty_analyzer.generate_report("unknown")
    assert empty_report["total_records"] == 0
    assert empty_report["summary"] == {"total_income": 0, "total_expense": 0, "net_income": 0}
    assert empty_report["trend"] == []



def test_internal_calculation_helpers_cover_summary_categories_type_trend_and_top_lists(tmp_path: Path) -> None:
    """内部统计 helper 应覆盖正常分支与缺字段回退。"""
    analyzer = Analyzer(db=cast(Any, FakeAnalyzerDB(default_response=[])), output_dir=tmp_path)
    df = pd.DataFrame(SAMPLE_BILLS)
    df["date"] = pd.to_datetime(df["date"])
    df["amount"] = pd.to_numeric(df["amount"])

    summary = analyzer._calculate_summary(df)
    assert summary == {"total_income": 5000.0, "total_expense": 200.0, "net_income": 4800.0}

    by_category = analyzer._calculate_by_category(df)
    assert by_category["餐饮"]["sub_categories"]["午餐"]["total"] == 120.0
    assert by_category["餐饮"]["sub_categories"]["食材"]["count"] == 1

    by_type = analyzer._calculate_by_type(df)
    assert by_type["收入"]["average"] == 5000.0
    assert by_type["支出"]["total"] == 200.0

    month_trend = analyzer._calculate_trend(df, "month")
    assert month_trend[0]["date"] == "2025-01-03"
    assert month_trend[-1]["net"] == 5000.0

    year_trend = analyzer._calculate_trend(df, "year")
    assert year_trend == [{"date": "2025-01-31", "income": 5000.0, "expense": 200.0, "net": 4800.0}]

    top_expenses = analyzer._get_top_expenses(df, limit=2)
    assert [item["counterparty"] for item in top_expenses] == ["餐厅A", "超市B"]
    top_income = analyzer._get_top_income(df, limit=1)
    assert top_income == [{"date": "2025-01-08", "amount": 5000.0, "counterparty": "公司", "description": "工资"}]

    assert analyzer._calculate_by_category(pd.DataFrame([{"amount": 10.0}])) == {}
    assert analyzer._calculate_by_type(pd.DataFrame([{"amount": 10.0}])) == {}
    assert analyzer._calculate_trend(pd.DataFrame([{"amount": 10.0}]), "month") == []
    assert analyzer._get_top_expenses(pd.DataFrame([{"amount": 10.0}])) == []
    assert analyzer._get_top_income(pd.DataFrame([{"amount": 10.0}])) == []


def test_period_dates_and_category_aggregation_cover_tail_branches(tmp_path: Path) -> None:
    """日期范围 helper 和无子分类分类聚合应覆盖剩余尾分支。"""
    analyzer = Analyzer(db=cast(Any, FakeAnalyzerDB(default_response=[])), output_dir=tmp_path)

    quarter_start, quarter_end = analyzer._get_period_dates("quarter")
    year_start, year_end = analyzer._get_period_dates("year")
    fallback_start, fallback_end = analyzer._get_period_dates("unsupported")

    assert quarter_start.endswith("-01")
    assert quarter_end.endswith("-01")
    assert year_start.endswith("-01-01")
    assert year_end.endswith("-01-01")
    assert fallback_start <= fallback_end

    categorized_df = pd.DataFrame(
        [
            {"main_category": "餐饮", "sub_category": None, "amount": 88.0},
            {"main_category": "餐饮", "sub_category": "午餐", "amount": 12.0},
        ]
    )
    categorized = analyzer._calculate_by_category(categorized_df)

    assert categorized["餐饮"]["count"] == 2
    assert categorized["餐饮"]["sub_categories"] == {"午餐": {"count": 1, "total": 12.0}}


@pytest.mark.asyncio
async def test_chart_entrypoints_cover_report_with_charts_heatmap_and_dashboard(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """图表相关入口应覆盖有数据/无数据和 wrapper 行为。"""
    db = FakeAnalyzerDB(responses=[SAMPLE_BILLS, SAMPLE_BILLS, []])
    analyzer = Analyzer(db=cast(Any, db), output_dir=tmp_path)
    fake_chart_generator = FakeChartGenerator(tmp_path)
    analyzer.chart_generator = cast(Any, fake_chart_generator)

    monkeypatch.setattr(
        analyzer_module,
        "generate_all_charts",
        lambda report, output_dir: {"trend": output_dir / "trend.png", "pie": output_dir / "pie.png"},
    )

    report_with_charts = await analyzer.generate_report_with_charts("month", generate_charts=True)
    assert report_with_charts["charts"] == {
        "trend": str(tmp_path / "trend.png"),
        "pie": str(tmp_path / "pie.png"),
    }

    assert analyzer.generate_trend_chart({"trend": []}) is None
    trend_path = analyzer.generate_trend_chart(report_with_charts, filename="monthly-trend.png")
    assert trend_path == tmp_path / "monthly-trend.png"

    assert analyzer.generate_category_pie({"by_category": {}}) is None
    pie_path = analyzer.generate_category_pie(report_with_charts, chart_type="expense", filename="pie.png")
    assert pie_path == tmp_path / "pie.png"

    assert analyzer.generate_top_expenses_chart({"top_expenses": []}) is None
    top_path = analyzer.generate_top_expenses_chart(report_with_charts, filename="top.png")
    assert top_path == tmp_path / "top.png"

    assert analyzer.generate_comparison_chart({"summary": {}}) is None
    comparison_path = analyzer.generate_comparison_chart(report_with_charts, filename="comparison.png")
    assert comparison_path == tmp_path / "comparison.png"

    heatmap_path = await analyzer.generate_heatmap("month", filename="heatmap.png")
    assert heatmap_path == tmp_path / "heatmap.png"
    assert await analyzer.generate_heatmap("month", filename="empty-heatmap.png") is None

    dashboard_path = analyzer.generate_dashboard(report_with_charts, filename="dashboard.png")
    assert dashboard_path == tmp_path / "dashboard.png"


@pytest.mark.asyncio
async def test_api_helpers_cover_trends_comparison_and_category_analysis(tmp_path: Path) -> None:
    """面向 UI API 的辅助方法应覆盖月份/年份趋势、分类对比和分类分析。"""
    repeated_bills = [
        {"type": "收入", "amount": 1000.0, "main_category": "工资", "sub_category": None},
        {"type": "支出", "amount": 200.0, "main_category": "餐饮", "sub_category": "午餐"},
        {"type": "支出", "amount": 50.0, "main_category": "餐饮", "sub_category": "早餐"},
    ]
    db = FakeAnalyzerDB(default_response=repeated_bills)
    analyzer = Analyzer(db=cast(Any, db), output_dir=tmp_path)

    monthly_trends = await analyzer.get_trends(period="month", category="餐饮")
    assert monthly_trends["period"] == "month"
    assert monthly_trends["category"] == "餐饮"
    assert len(monthly_trends["trends"]) == 12
    assert monthly_trends["trends"][0]["income"] == 1000.0
    assert monthly_trends["trends"][0]["expense"] == 250.0

    yearly_trends = await analyzer.get_trends(period="year")
    assert len(yearly_trends["trends"]) == 12
    assert yearly_trends["trends"][0]["period"].isdigit()

    skipped_period = await analyzer.get_trends(period="week")
    assert skipped_period["trends"] == []

    comparison = await analyzer.get_comparison(period="month", compare_type="category")
    assert comparison["comparison"][0]["name"] == "餐饮"
    assert comparison["comparison"][0]["expense"] == 250.0
    assert comparison["comparison"][0]["count"] == 2

    empty_comparison = await analyzer.get_comparison(period="month", compare_type="year")
    assert empty_comparison["comparison"] == []

    category_analysis = await analyzer.analyze_category(period="month", main_category="餐饮")
    assert category_analysis["main_category"] == "餐饮"
    assert category_analysis["total_amount"] == 250.0
    assert category_analysis["sub_categories"][0]["sub_category"] == "午餐"
    assert category_analysis["sub_categories"][0]["percentage"] == 80.0
