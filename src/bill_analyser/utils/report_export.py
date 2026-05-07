"""Report export utilities for PDF, Excel, and HTML outputs."""

# pylint: disable=duplicate-code

from __future__ import annotations

from datetime import datetime
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
from typing import Any

import matplotlib
import matplotlib.pyplot as plt
import pandas as pd

from ..constants import OUTPUT_DIR
from .logger import get_logger, log_method, log_step

matplotlib.use("Agg")

_INVALID_REPORT_FILENAME_CHARS = re.compile(r'[<>:"/\\|?*\x00-\x1f\x7f]+')
_WINDOWS_RESERVED_REPORT_NAMES = {
    "con",
    "prn",
    "aux",
    "nul",
    *(f"com{index}" for index in range(1, 10)),
    *(f"lpt{index}" for index in range(1, 10)),
}


class ReportExporter:  # pylint: disable=too-few-public-methods
    """Export analyzer payloads to PDF, Excel, and HTML files."""

    def __init__(self, output_dir: str | Path | None = None):
        """Initialize the exporter and ensure the output directory exists."""
        self.logger = get_logger("ReportExporter")
        self.output_dir = Path(output_dir) if output_dir else OUTPUT_DIR
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self._setup_matplotlib()

    def _setup_matplotlib(self) -> None:
        """Configure matplotlib defaults used by exported figures."""
        plt.rcParams["font.sans-serif"] = ["SimHei", "Microsoft YaHei", "Arial Unicode MS"]
        plt.rcParams["axes.unicode_minus"] = False
        plt.rcParams["figure.figsize"] = (12, 8)

    @staticmethod
    def _sanitize_report_filename(filename: str) -> str:
        """Return a safe leaf filename so exports cannot escape output_dir."""
        fallback = f"report_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
        raw_filename = str(filename or "").strip()
        if not raw_filename:
            return fallback

        leaf = PurePosixPath(PureWindowsPath(raw_filename).name).name.strip()
        cleaned = _INVALID_REPORT_FILENAME_CHARS.sub("_", leaf)
        cleaned = re.sub(r"_+", "_", cleaned).strip(" ._")
        reserved_name = cleaned.split(".", maxsplit=1)[0].rstrip(" .").lower()
        if cleaned in {"", ".", ".."} or reserved_name in _WINDOWS_RESERVED_REPORT_NAMES:
            return fallback
        return cleaned

    def _resolve_export_path(self, filename: str, extension: str) -> Path:
        """Resolve an export path and keep it inside the configured output dir."""
        safe_filename = self._sanitize_report_filename(filename)
        output_dir = self.output_dir.resolve()
        output_path = (self.output_dir / f"{safe_filename}.{extension}").resolve()
        try:
            output_path.relative_to(output_dir)
        except ValueError as exc:
            raise ValueError("导出路径必须位于输出目录内") from exc
        return output_path

    @log_method
    @log_step("生成收支趋势图")
    def _generate_trend_chart(self, data: dict[str, Any], ax: Any) -> None:
        """Render the income/expense/net trend chart into the provided axes."""
        trend = data.get("trend", [])
        if not trend:
            ax.text(0.5, 0.5, "无趋势数据", ha="center", va="center")
            return

        dataframe = pd.DataFrame(trend)
        dataframe["date"] = pd.to_datetime(dataframe["date"])

        ax.plot(dataframe["date"], dataframe["income"], marker="o", label="收入", linewidth=2)
        ax.plot(dataframe["date"], dataframe["expense"], marker="s", label="支出", linewidth=2)
        ax.plot(dataframe["date"], dataframe["net"], marker="^", label="净收入", linewidth=2)

        ax.set_title("收支趋势图", fontsize=16, fontweight="bold")
        ax.set_xlabel("日期")
        ax.set_ylabel("金额（元）")
        ax.legend()
        ax.grid(True, alpha=0.3)
        plt.setp(ax.xaxis.get_majorticklabels(), rotation=45)

    @log_method
    @log_step("生成分类饼图")
    def _generate_category_pie(self, data: dict[str, Any], ax: Any) -> None:
        """Render the category pie chart into the provided axes."""
        by_category = data.get("by_category", {})
        if not by_category:
            ax.text(0.5, 0.5, "无分类数据", ha="center", va="center")
            return

        labels = list(by_category.keys())
        sizes = [category["total"] for category in by_category.values()]

        ax.pie(sizes, labels=labels, autopct="%1.1f%%", startangle=90)
        ax.set_title("支出分类占比", fontsize=16, fontweight="bold")

    @log_method
    @log_step("生成类型对比图")
    def _generate_type_bar(self, data: dict[str, Any], ax: Any) -> None:
        """Render the type comparison chart into the provided axes."""
        by_type = data.get("by_type", {})
        if not by_type:
            ax.text(0.5, 0.5, "无类型数据", ha="center", va="center")
            return

        types = list(by_type.keys())
        totals = [item["total"] for item in by_type.values()]
        counts = [item["count"] for item in by_type.values()]

        x_positions = range(len(types))
        width = 0.35

        ax.bar([index - width / 2 for index in x_positions], totals, width, label="总金额")
        secondary_axis = ax.twinx()
        secondary_axis.bar(
            [index + width / 2 for index in x_positions],
            counts,
            width,
            label="交易笔数",
            color="orange",
        )

        ax.set_title("交易类型统计", fontsize=16, fontweight="bold")
        ax.set_xlabel("交易类型")
        ax.set_ylabel("总金额（元）")
        secondary_axis.set_ylabel("交易笔数")
        ax.set_xticks(list(x_positions))
        ax.set_xticklabels(types)
        ax.legend(loc="upper left")
        secondary_axis.legend(loc="upper right")

    @log_method
    @log_step("导出报告")
    async def export_report(
        self,
        data: dict[str, Any],
        format_type: str = "pdf",
        filename: str | None = None,
    ) -> str:
        """Export analyzer data to the selected report format."""
        resolved_filename = self._sanitize_report_filename(
            filename or f"report_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
        )

        if format_type == "pdf":
            return await self._export_pdf(data, resolved_filename)
        if format_type == "excel":
            return await self._export_excel(data, resolved_filename)
        if format_type == "html":
            return await self._export_html(data, resolved_filename)

        self.logger.error("不支持的格式: %s", format_type)
        raise ValueError(f"不支持的格式: {format_type}")

    async def _export_pdf(self, data: dict[str, Any], filename: str) -> str:
        """Export the report payload as a PDF file."""
        output_path = self._resolve_export_path(filename, "pdf")

        figure, axes = plt.subplots(2, 2, figsize=(16, 12))
        figure.suptitle(f"账单分析报告 - {data.get('period', '')}", fontsize=20, fontweight="bold")

        self._generate_trend_chart(data, axes[0, 0])
        self._generate_category_pie(data, axes[0, 1])
        self._generate_type_bar(data, axes[1, 0])

        summary = data.get("summary", {})
        axes[1, 1].axis("off")
        summary_text = f"""
        统计摘要

        总收入: ¥{summary.get("total_income", 0):,.2f}
        总支出: ¥{summary.get("total_expense", 0):,.2f}
        净收入: ¥{summary.get("net_income", 0):,.2f}

        记录总数: {data.get("total_records", 0)}
        统计周期: {data.get("start_date", "")} ~ {data.get("end_date", "")}
        生成时间: {data.get("generated_at", "")}
        """
        axes[1, 1].text(0.1, 0.5, summary_text, fontsize=12, verticalalignment="center")

        plt.tight_layout()
        plt.savefig(output_path, dpi=300, bbox_inches="tight")
        plt.close()

        self.logger.info("PDF报告已生成: %s", output_path)
        return str(output_path)

    async def _export_excel(self, data: dict[str, Any], filename: str) -> str:
        """Export the report payload as an Excel workbook."""
        output_path = self._resolve_export_path(filename, "xlsx")

        with pd.ExcelWriter(output_path, engine="openpyxl") as writer:
            summary_dataframe = pd.DataFrame([data.get("summary", {})])
            summary_dataframe.to_excel(writer, sheet_name="摘要", index=False)

            if data.get("trend"):
                trend_dataframe = pd.DataFrame(data["trend"])
                trend_dataframe.to_excel(writer, sheet_name="趋势", index=False)

            if data.get("by_category"):
                category_rows = [
                    {
                        "分类": category,
                        "笔数": values["count"],
                        "总金额": values["total"],
                        "平均金额": values["average"],
                    }
                    for category, values in data["by_category"].items()
                ]
                pd.DataFrame(category_rows).to_excel(writer, sheet_name="分类统计", index=False)

        self.logger.info("Excel报告已生成: %s", output_path)
        return str(output_path)

    async def _export_html(self, data: dict[str, Any], filename: str) -> str:
        """Export the report payload as a lightweight HTML summary."""
        output_path = self._resolve_export_path(filename, "html")
        summary = data.get("summary", {})
        total_income = summary.get("total_income", 0)
        total_expense = summary.get("total_expense", 0)
        net_income = summary.get("net_income", 0)

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
                <tr><td>总收入</td><td>¥{total_income:,.2f}</td></tr>
                <tr><td>总支出</td><td>¥{total_expense:,.2f}</td></tr>
                <tr><td>净收入</td><td>¥{net_income:,.2f}</td></tr>
            </table>
            <p>生成时间: {data.get('generated_at', '')}</p>
        </body>
        </html>
        """

        with open(output_path, "w", encoding="utf-8") as file_handle:
            file_handle.write(html)

        self.logger.info("HTML报告已生成: %s", output_path)
        return str(output_path)
