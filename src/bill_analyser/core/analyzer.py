"""
Analyzer Module - 数据分析模块

使用 pandas 进行账单数据分析，生成统计报告。
"""

from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional
import pandas as pd

from .db import Database
from ..utils.logger import get_logger, log_method, log_step
from ..utils.charts import ChartGenerator, generate_all_charts


class Analyzer:
    """账单数据分析器"""

    def __init__(self, db: Optional[Database] = None, output_dir: Optional[Path] = None):
        """
        初始化分析器

        Args:
            db: 数据库实例
            output_dir: 图表输出目录
        """
        self.logger = get_logger("Analyzer")
        self.db = db or Database()
        self._cache: Dict[str, Any] = {}
        self._cache_timeout = 300  # 缓存5分钟
        self._cache_timestamps: Dict[str, float] = {}
        self.chart_generator = ChartGenerator(output_dir=output_dir)

    def _get_cache_key(self, period: str, filters: Optional[Dict] = None) -> str:
        """生成缓存键"""
        filter_str = str(sorted((filters or {}).items()))
        return f"{period}_{filter_str}"

    def _is_cache_valid(self, key: str) -> bool:
        """检查缓存是否有效"""
        if key not in self._cache:
            return False

        timestamp = self._cache_timestamps.get(key, 0)
        return (datetime.now().timestamp() - timestamp) < self._cache_timeout

    @log_method
    def _get_period_dates(self, period: str = "month") -> tuple:
        """
        获取时间段的起止日期

        Args:
            period: 时间段 ('month', 'quarter', 'year')

        Returns:
            tuple: (start_date, end_date)
        """
        now = datetime.now()

        if period == "month":
            start_date = now.replace(day=1, hour=0, minute=0, second=0, microsecond=0)
            # 下个月第一天
            if now.month == 12:
                end_date = now.replace(year=now.year + 1, month=1, day=1)
            else:
                end_date = now.replace(month=now.month + 1, day=1)

        elif period == "quarter":
            quarter = (now.month - 1) // 3 + 1
            start_month = (quarter - 1) * 3 + 1
            start_date = now.replace(month=start_month, day=1, hour=0, minute=0, second=0)

            end_month = start_month + 3
            if end_month > 12:
                end_date = now.replace(year=now.year + 1, month=end_month - 12, day=1)
            else:
                end_date = now.replace(month=end_month, day=1)

        elif period == "year":
            start_date = now.replace(month=1, day=1, hour=0, minute=0, second=0)
            end_date = now.replace(year=now.year + 1, month=1, day=1)

        else:
            # 默认最近30天
            start_date = now - timedelta(days=30)
            end_date = now

        return start_date.strftime("%Y-%m-%d"), end_date.strftime("%Y-%m-%d")

    @log_method
    @log_step("生成数据分析报告")
    async def generate_report(self, period: str = "month", filters: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """
        生成分析报告

        Args:
            period: 时间段 ('month', 'quarter', 'year')
            filters: 额外的过滤条件

        Returns:
            Dict: 分析报告数据
        """
        # 检查缓存
        cache_key = self._get_cache_key(period, filters)
        if self._is_cache_valid(cache_key):
            self.logger.info("使用缓存的分析结果")
            return self._cache[cache_key]

        # 获取时间范围
        start_date, end_date = self._get_period_dates(period)

        # 构建查询过滤条件
        query_filters = filters.copy() if filters else {}
        query_filters["date_from"] = start_date
        query_filters["date_to"] = end_date

        # 获取账单数据
        bills = await self.db.get_bills(query_filters)

        if not bills:
            self.logger.warning("指定时间段内没有账单数据")
            return self._empty_report()

        # 转换为 DataFrame
        df = pd.DataFrame(bills)
        df["date"] = pd.to_datetime(df["date"])
        df["amount"] = pd.to_numeric(df["amount"], errors="coerce")

        # 生成报告
        report = {
            "period": period,
            "start_date": start_date,
            "end_date": end_date,
            "total_records": len(df),
            "summary": self._calculate_summary(df),
            "by_category": self._calculate_by_category(df),
            "by_type": self._calculate_by_type(df),
            "trend": self._calculate_trend(df, period),
            "top_expenses": self._get_top_expenses(df, limit=10),
            "top_income": self._get_top_income(df, limit=10),
            "generated_at": datetime.now().isoformat(),
        }

        # 缓存结果
        self._cache[cache_key] = report
        self._cache_timestamps[cache_key] = datetime.now().timestamp()

        self.logger.info("分析报告生成完成")
        return report

    def _empty_report(self) -> Dict[str, Any]:
        """返回空报告"""
        return {
            "period": "",
            "start_date": "",
            "end_date": "",
            "total_records": 0,
            "summary": {"total_income": 0, "total_expense": 0, "net_income": 0},
            "by_category": {},
            "by_type": {},
            "trend": [],
            "top_expenses": [],
            "top_income": [],
            "generated_at": datetime.now().isoformat(),
        }

    def _calculate_summary(self, df: pd.DataFrame) -> Dict[str, float]:
        """计算总体汇总"""
        summary = {"total_income": 0.0, "total_expense": 0.0, "net_income": 0.0}

        if "type" in df.columns and "amount" in df.columns:
            summary["total_income"] = float(df[df["type"] == "收入"]["amount"].sum())
            summary["total_expense"] = float(df[df["type"] == "支出"]["amount"].sum())
            summary["net_income"] = summary["total_income"] - summary["total_expense"]

        return summary

    def _calculate_by_category(self, df: pd.DataFrame) -> Dict[str, Dict[str, Any]]:
        """按分类统计"""
        if "main_category" not in df.columns:
            return {}

        # 过滤掉未分类的
        df_categorized = df[df["main_category"].notna()]

        if df_categorized.empty:
            return {}

        result = {}
        for category in df_categorized["main_category"].unique():
            category_df = df_categorized[df_categorized["main_category"] == category]
            result[category] = {
                "count": len(category_df),
                "total": float(category_df["amount"].sum()),
                "average": float(category_df["amount"].mean()),
                "sub_categories": {},
            }

            # 子分类统计
            if "sub_category" in category_df.columns:
                for sub_cat in category_df["sub_category"].unique():
                    if pd.notna(sub_cat):
                        sub_df = category_df[category_df["sub_category"] == sub_cat]
                        result[category]["sub_categories"][sub_cat] = {
                            "count": len(sub_df),
                            "total": float(sub_df["amount"].sum()),
                        }

        return result

    def _calculate_by_type(self, df: pd.DataFrame) -> Dict[str, Dict[str, Any]]:
        """按交易类型统计"""
        if "type" not in df.columns:
            return {}

        result = {}
        for trans_type in df["type"].unique():
            type_df = df[df["type"] == trans_type]
            result[trans_type] = {
                "count": len(type_df),
                "total": float(type_df["amount"].sum()),
                "average": float(type_df["amount"].mean()),
            }

        return result

    def _calculate_trend(self, df: pd.DataFrame, period: str) -> List[Dict[str, Any]]:
        """计算趋势数据"""
        if "date" not in df.columns or "type" not in df.columns:
            return []

        # 根据周期确定分组频率
        freq_map = {
            "month": "D",  # 按天
            "quarter": "W",  # 按周
            "year": "M",  # 按月
        }
        freq = freq_map.get(period, "D")

        # 按日期和类型分组
        df_grouped = df.groupby([pd.Grouper(key="date", freq=freq), "type"])["amount"].sum().reset_index()

        # 转换为趋势数据
        trend = []
        for date_val in df_grouped["date"].unique():
            date_data = df_grouped[df_grouped["date"] == date_val]

            income = float(date_data[date_data["type"] == "收入"]["amount"].sum())
            expense = float(date_data[date_data["type"] == "支出"]["amount"].sum())

            trend.append(
                {"date": date_val.strftime("%Y-%m-%d"), "income": income, "expense": expense, "net": income - expense}
            )

        return sorted(trend, key=lambda x: x["date"])

    def _get_top_expenses(self, df: pd.DataFrame, limit: int = 10) -> List[Dict[str, Any]]:
        """获取最大支出TOP N"""
        if "type" not in df.columns:
            return []

        expenses = df[df["type"] == "支出"].nlargest(limit, "amount")

        return [
            {
                "date": row["date"].strftime("%Y-%m-%d"),
                "amount": float(row["amount"]),
                "counterparty": row.get("counterparty", ""),
                "description": row.get("description", ""),
                "category": row.get("main_category", ""),
            }
            for _, row in expenses.iterrows()
        ]

    def _get_top_income(self, df: pd.DataFrame, limit: int = 10) -> List[Dict[str, Any]]:
        """获取最大收入TOP N"""
        if "type" not in df.columns:
            return []

        income = df[df["type"] == "收入"].nlargest(limit, "amount")

        return [
            {
                "date": row["date"].strftime("%Y-%m-%d"),
                "amount": float(row["amount"]),
                "counterparty": row.get("counterparty", ""),
                "description": row.get("description", ""),
            }
            for _, row in income.iterrows()
        ]

    @log_method
    def clear_cache(self):
        """清除缓存"""
        count = len(self._cache)
        self._cache.clear()
        self._cache_timestamps.clear()
        self.logger.info("已清除 %d 个分析缓存", count)

    @log_method
    async def generate_report_with_charts(
        self, period: str = "month", filters: Optional[Dict[str, Any]] = None, generate_charts: bool = True
    ) -> Dict[str, Any]:
        """
        生成分析报告（含图表）

        Args:
            period: 时间段 ('month', 'quarter', 'year')
            filters: 额外的过滤条件
            generate_charts: 是否生成图表

        Returns:
            Dict: 分析报告数据（含图表文件路径）
        """
        # 生成基础报告
        report = await self.generate_report(period, filters)

        # 生成图表
        if generate_charts and report.get("total_records", 0) > 0:
            self.logger.info("开始生成可视化图表")
            chart_files = generate_all_charts(report, self.chart_generator.output_dir)
            report["charts"] = {k: str(v) for k, v in chart_files.items() if v}
            self.logger.info("已生成 %d 个图表", len(report["charts"]))

        return report

    @log_method
    def generate_trend_chart(self, report_data: Dict[str, Any], filename: Optional[str] = None) -> Optional[Path]:
        """
        生成收支趋势图

        Args:
            report_data: 报告数据
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        trend_data = report_data.get("trend", [])
        if not trend_data:
            return None

        period = report_data.get("period", "month")
        period_name = {"month": "月度", "quarter": "季度", "year": "年度"}.get(period, "")
        title = f"{period_name}收支趋势"

        return self.chart_generator.generate_trend_chart(trend_data, title, filename)

    @log_method
    def generate_category_pie(
        self, report_data: Dict[str, Any], chart_type: str = "expense", filename: Optional[str] = None
    ) -> Optional[Path]:
        """
        生成分类饼图

        Args:
            report_data: 报告数据
            chart_type: 类型 ('expense' 或 'income')
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        category_data = report_data.get("by_category", {})
        if not category_data:
            return None

        return self.chart_generator.generate_category_pie_chart(category_data, chart_type, filename=filename)

    @log_method
    def generate_top_expenses_chart(
        self, report_data: Dict[str, Any], filename: Optional[str] = None
    ) -> Optional[Path]:
        """
        生成Top支出排行榜

        Args:
            report_data: 报告数据
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        top_data = report_data.get("top_expenses", [])
        if not top_data:
            return None

        return self.chart_generator.generate_top_merchants_chart(top_data, filename=filename)

    @log_method
    def generate_comparison_chart(self, report_data: Dict[str, Any], filename: Optional[str] = None) -> Optional[Path]:
        """
        生成收支对比图

        Args:
            report_data: 报告数据
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        summary_data = report_data.get("summary", {})
        if not summary_data:
            return None

        period = report_data.get("period", "month")
        period_name = {"month": "月度", "quarter": "季度", "year": "年度"}.get(period, "")
        title = f"{period_name}收支对比"

        return self.chart_generator.generate_comparison_bar_chart(summary_data, title, filename)

    @log_method
    async def generate_heatmap(
        self, period: str = "month", filters: Optional[Dict[str, Any]] = None, filename: Optional[str] = None
    ) -> Optional[Path]:
        """
        生成消费热力图

        Args:
            period: 时间段
            filters: 过滤条件
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        # 获取时间范围
        start_date, end_date = self._get_period_dates(period)

        # 构建查询过滤条件
        query_filters = filters.copy() if filters else {}
        query_filters["date_from"] = start_date
        query_filters["date_to"] = end_date

        # 获取账单数据
        bills = await self.db.get_bills(query_filters)

        if not bills:
            self.logger.warning("没有账单数据")
            return None

        return self.chart_generator.generate_heatmap(bills, filename=filename)

    @log_method
    def generate_dashboard(self, report_data: Dict[str, Any], filename: Optional[str] = None) -> Optional[Path]:
        """
        生成综合仪表盘

        Args:
            report_data: 报告数据
            filename: 输出文件名

        Returns:
            Path: 图片文件路径
        """
        return self.chart_generator.generate_comprehensive_dashboard(report_data, filename)

    # ==================== UI Backend API所需的额外方法 ====================

    @log_method
    async def get_trends(self, period: str = "month", category: Optional[str] = None) -> Dict[str, Any]:
        """
        获取趋势数据

        Args:
            period: 时间周期
            category: 分类过滤

        Returns:
            Dict: 趋势数据
        """
        # 获取最近12个周期的数据
        trends = []
        now = datetime.now()

        for i in range(12, 0, -1):
            if period == "month":
                target_date = now - timedelta(days=30 * i)
                start_date = target_date.replace(day=1)
                if target_date.month == 12:
                    end_date = target_date.replace(year=target_date.year + 1, month=1, day=1) - timedelta(days=1)
                else:
                    end_date = target_date.replace(month=target_date.month + 1, day=1) - timedelta(days=1)
            elif period == "year":
                target_year = now.year - i
                start_date = datetime(target_year, 1, 1)
                end_date = datetime(target_year, 12, 31)
            else:
                continue

            filters = {"date_from": start_date.strftime("%Y-%m-%d"), "date_to": end_date.strftime("%Y-%m-%d")}

            if category:
                filters["main_category"] = category

            bills = await self.db.get_bills(filters)

            income = sum(b["amount"] for b in bills if b["type"] == "收入")
            expense = sum(b["amount"] for b in bills if b["type"] == "支出")

            trends.append(
                {
                    "period": start_date.strftime("%Y-%m" if period == "month" else "%Y"),
                    "income": round(income, 2),
                    "expense": round(expense, 2),
                    "net": round(income - expense, 2),
                }
            )

        return {"trends": trends, "period": period, "category": category}

    @log_method
    async def get_comparison(self, period: str = "month", compare_type: str = "category") -> Dict[str, Any]:
        """
        获取对比数据

        Args:
            period: 时间周期
            compare_type: 对比类型(category/month/year)

        Returns:
            Dict: 对比数据
        """
        start_date, end_date = self._get_period_dates(period)

        filters = {"date_from": start_date, "date_to": end_date}

        bills = await self.db.get_bills(filters)

        if compare_type == "category":
            # 按分类对比
            category_data = {}
            for bill in bills:
                cat = bill.get("main_category", "未分类")
                if cat not in category_data:
                    category_data[cat] = {"income": 0, "expense": 0, "count": 0}

                if bill["type"] == "收入":
                    category_data[cat]["income"] += bill["amount"]
                elif bill["type"] == "支出":
                    category_data[cat]["expense"] += bill["amount"]
                category_data[cat]["count"] += 1

            comparison = [
                {
                    "name": cat,
                    "income": round(data["income"], 2),
                    "expense": round(data["expense"], 2),
                    "count": data["count"],
                    "net": round(data["income"] - data["expense"], 2),
                }
                for cat, data in category_data.items()
            ]
            comparison.sort(key=lambda x: x["expense"], reverse=True)

        else:
            comparison = []

        return {"comparison": comparison, "period": period, "compare_type": compare_type}

    @log_method
    async def analyze_category(self, period: str = "month", main_category: Optional[str] = None) -> Dict[str, Any]:
        """
        分析指定分类

        Args:
            period: 时间周期
            main_category: 主分类

        Returns:
            Dict: 分类分析数据
        """
        start_date, end_date = self._get_period_dates(period)

        filters = {"date_from": start_date, "date_to": end_date}

        if main_category:
            filters["main_category"] = main_category

        bills = await self.db.get_bills(filters)

        # 子分类统计
        sub_categories = {}
        total_amount = 0

        for bill in bills:
            if bill["type"] != "支出":
                continue

            sub_cat = bill.get("sub_category", "其他")
            if sub_cat not in sub_categories:
                sub_categories[sub_cat] = {"amount": 0, "count": 0, "items": []}

            sub_categories[sub_cat]["amount"] += bill["amount"]
            sub_categories[sub_cat]["count"] += 1
            sub_categories[sub_cat]["items"].append(bill)
            total_amount += bill["amount"]

        # 计算占比
        analysis = []
        for sub_cat, data in sub_categories.items():
            percentage = (data["amount"] / total_amount * 100) if total_amount > 0 else 0
            analysis.append(
                {
                    "sub_category": sub_cat,
                    "amount": round(data["amount"], 2),
                    "count": data["count"],
                    "percentage": round(percentage, 2),
                    "avg_amount": round(data["amount"] / data["count"], 2) if data["count"] > 0 else 0,
                }
            )

        analysis.sort(key=lambda x: x["amount"], reverse=True)

        return {
            "main_category": main_category,
            "period": period,
            "total_amount": round(total_amount, 2),
            "sub_categories": analysis,
        }
