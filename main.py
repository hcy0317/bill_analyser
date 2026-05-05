"""Main entry point for Bill Analyser CLI and desktop startup."""

import argparse
import asyncio
import calendar
import sys
import time
from datetime import datetime
from pathlib import Path

# 添加项目根目录到路径
sys.path.insert(0, str(Path(__file__).parent))

from bill_analyser.core.analyzer import Analyzer
from bill_analyser.core.bills import BillService
from bill_analyser.core.budgets.execution_summary import (
    build_budget_execution_summary,
    select_budget_detail_items,
)
from bill_analyser.core.sync import SyncManager
from bill_analyser.utils.logger import get_logger
from bill_analyser.utils.report_export import ReportExporter

logger = get_logger("Main")

_CLI_TO_BUDGET_PERIOD_TYPE = {
    "month": "monthly",
    "quarter": "quarterly",
    "year": "yearly",
}


def _resolve_budget_status(budget_amount: float, spent_amount: float) -> str:
    """Resolve the legacy CLI status label from budget/spent amounts."""
    if budget_amount <= 0:
        return "normal"

    ratio = spent_amount / budget_amount
    if ratio >= 1.0:
        return "exceeded"
    if ratio >= 0.9:
        return "critical"
    if ratio >= 0.8:
        return "warning"
    return "normal"


def _resolve_budget_period_window(period: str) -> tuple[str, str, str]:
    """Resolve the current budget period type and date window for CLI reporting."""
    now = datetime.now()
    if period == "year":
        return "yearly", f"{now.year}-01-01", f"{now.year}-12-31"

    if period == "quarter":
        quarter = (now.month - 1) // 3 + 1
        start_month = (quarter - 1) * 3 + 1
        end_month = start_month + 2
        end_day = calendar.monthrange(now.year, end_month)[1]
        return (
            "quarterly",
            f"{now.year}-{start_month:02d}-01",
            f"{now.year}-{end_month:02d}-{end_day:02d}",
        )

    end_day = calendar.monthrange(now.year, now.month)[1]
    return "monthly", f"{now.year}-{now.month:02d}-01", f"{now.year}-{now.month:02d}-{end_day:02d}"


def _format_budget_item_label(item: dict[str, object]) -> str:
    """Choose a stable CLI display label for a budget execution item."""
    name = str(item.get("name") or "").strip()
    if name:
        return name

    category = str(item.get("category") or "").strip()
    sub_category = str(item.get("sub_category") or "").strip()
    if category and sub_category:
        return f"{category}/{sub_category}"
    if category:
        return category
    return "未命名预算"


def _select_cli_budget_items(items: list[dict[str, object]]) -> list[dict[str, object]]:
    """Prefer leaf budgets over synchronized primary budgets within the same group."""
    return select_budget_detail_items(items)


def _ensure_unique_budget_label(
    item: dict[str, object],
    used_labels: set[str],
) -> str:
    """Keep CLI labels stable when budget names are duplicated or empty."""
    base_label = _format_budget_item_label(item)
    if base_label not in used_labels:
        return base_label

    category = str(item.get("category") or "").strip()
    sub_category = str(item.get("sub_category") or "").strip()
    path_label = "/".join(part for part in (category, sub_category) if part)
    if path_label:
        candidate = f"{base_label} [{path_label}]"
        if candidate not in used_labels:
            return candidate

    return f"{base_label} #{item.get('id', 'unknown')}"


def _build_budget_cli_report(period: str, items: list[dict[str, object]]) -> dict[str, object]:
    """Build the legacy CLI report structure from budget execution items."""
    detail_items = _select_cli_budget_items(items)
    summary = {
        **build_budget_execution_summary(items),
        "count": len(detail_items),
        "normal_count": 0,
        "warning_count": 0,
        "critical_count": 0,
        "exceeded_count": 0,
    }
    categories: dict[str, dict[str, object]] = {}
    used_labels: set[str] = set()

    for item in detail_items:
        budget_amount = float(item.get("budget_amount") or 0.0)
        spent_amount = float(item.get("spent_amount") or 0.0)
        status = _resolve_budget_status(budget_amount, spent_amount)
        label = _ensure_unique_budget_label(item, used_labels)
        used_labels.add(label)

        categories[label] = {
            "budget": budget_amount,
            "spent": spent_amount,
            "remaining": float(item.get("remaining_amount") or (budget_amount - spent_amount)),
            "usage_ratio": float(item.get("execution_rate") or 0.0),
            "status": status,
        }

        summary[f"{status}_count"] += 1

    return {
        "period": period,
        "categories": categories,
        "summary": summary,
        "generated_at": datetime.now().isoformat(),
    }


async def import_command(args):
    """导入账单命令"""
    start_time = time.time()
    logger.info("=" * 50)
    logger.info("开始导入账单: %s", args.file)

    async with BillService() as service:
        result = await service.import_bills(args.file)

        if result["success"]:
            logger.info("导入成功!")
            logger.info("总计: %d 条", result["total"])
            logger.info("有效: %d 条", result["valid"])
            logger.info("插入: %d 条", result["inserted"])
            logger.info("重复: %d 条", result["duplicates"])

            if result["categories"]:
                logger.info("分类统计:")
                for cat, count in result["categories"].items():
                    logger.info("  %s: %d 条", cat, count)
        else:
            logger.error("导入失败!")
            if result["errors"]:
                logger.error("错误信息: %s", result["errors"])

    elapsed = time.time() - start_time
    logger.info("操作耗时: %.2f 秒", elapsed)
    logger.info("=" * 50)


async def report_command(args):
    """生成报告命令"""
    start_time = time.time()
    logger.info("=" * 50)
    logger.info("开始生成报告: period=%s, format=%s", args.period, args.format)

    async with BillService() as service:
        # 生成分析报告
        analyzer = Analyzer(service.db)
        data = await analyzer.generate_report(args.period)

        logger.info("分析完成:")
        logger.info("记录数: %d", data["total_records"])
        logger.info("总收入: ¥%.2f", data["summary"]["total_income"])
        logger.info("总支出: ¥%.2f", data["summary"]["total_expense"])
        logger.info("净收入: ¥%.2f", data["summary"]["net_income"])

        # 导出报告
        generator = ReportExporter()
        output_path = await generator.export_report(data, args.format)

        logger.info("报告已生成: %s", output_path)

    elapsed = time.time() - start_time
    logger.info("操作耗时: %.2f 秒", elapsed)
    logger.info("=" * 50)


async def budget_command(args):
    """预算检查命令"""
    logger.info("=" * 50)
    logger.info("开始检查预算")

    async with BillService() as service:
        resolved_period_type, start_date, end_date = _resolve_budget_period_window(args.period)
        execution_items = await service.db.get_budget_execution_details(
            budget_type=3,
            period_type=_CLI_TO_BUDGET_PERIOD_TYPE.get(args.period, resolved_period_type),
            start_date=start_date,
            end_date=end_date,
        )
        report = _build_budget_cli_report(args.period, execution_items)

        logger.info("预算报告:")
        logger.info("总预算: ¥%.2f", report["summary"]["total_budget"])
        logger.info("已花费: ¥%.2f", report["summary"]["total_spent"])
        logger.info("正常: %d", report["summary"]["normal_count"])
        logger.info("警告: %d", report["summary"]["warning_count"])
        logger.info("严重: %d", report["summary"]["critical_count"])
        logger.info("超支: %d", report["summary"]["exceeded_count"])

        logger.info("\n分类详情:")
        for category, info in report["categories"].items():
            logger.info(
                "  %s: 预算=¥%.2f, 已用=¥%.2f, 剩余=¥%.2f, 使用率=%.1f%%, 状态=%s",
                category,
                info["budget"],
                info["spent"],
                info["remaining"],
                info["usage_ratio"],
                info["status"],
            )

    logger.info("=" * 50)


async def backup_command(args):
    """备份命令"""
    logger.info("=" * 50)
    logger.info("开始备份数据")

    manager = SyncManager()
    backup_path = await manager.backup_local()

    if backup_path:
        logger.info("备份成功: %s", backup_path)

        if args.cleanup:
            manager.cleanup_old_backups(keep_count=args.keep)
            logger.info("已清理旧备份，保留 %d 个", args.keep)
    else:
        logger.info("数据未变化，无需备份")

    logger.info("=" * 50)


def gui_command(args):
    """GUI模式命令"""
    logger.info("=" * 50)
    logger.info("启动GUI模式")

    try:
        from bill_analyser.ui.app import main as gui_main

        gui_main()
    except ImportError as e:
        logger.error("GUI模块导入失败: %s", e)
        logger.error("请确保已安装 PySide6: pip install PySide6")
        sys.exit(1)

    logger.info("=" * 50)


def main():
    """主函数"""
    parser = argparse.ArgumentParser(description="账单管理系统 - 现代化的个人账单管理与分析工具")

    subparsers = parser.add_subparsers(dest="command", help="命令")

    # 导入命令
    import_parser = subparsers.add_parser("import", help="导入账单文件")
    import_parser.add_argument("file", help="账单文件路径")
    import_parser.set_defaults(func=import_command)

    # 报告命令
    report_parser = subparsers.add_parser("report", help="生成分析报告")
    report_parser.add_argument("--period", choices=["month", "quarter", "year"], default="month", help="统计周期")
    report_parser.add_argument("--format", choices=["pdf", "excel", "html"], default="pdf", help="报告格式")
    report_parser.set_defaults(func=report_command)

    # 预算命令
    budget_parser = subparsers.add_parser("budget", help="检查预算状态")
    budget_parser.add_argument("--period", choices=["month", "quarter", "year"], default="month", help="统计周期")
    budget_parser.set_defaults(func=budget_command)

    # 备份命令
    backup_parser = subparsers.add_parser("backup", help="备份数据")
    backup_parser.add_argument("--cleanup", action="store_true", help="清理旧备份")
    backup_parser.add_argument("--keep", type=int, default=10, help="保留的备份数量")
    backup_parser.set_defaults(func=backup_command)

    # GUI命令
    gui_parser = subparsers.add_parser("gui", help="启动图形界面")
    gui_parser.set_defaults(func=gui_command)

    # 解析参数
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    # 执行命令
    if hasattr(args, "func"):
        if asyncio.iscoroutinefunction(args.func):
            asyncio.run(args.func(args))
        else:
            args.func(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        logger.info("\n程序被用户中断")
        sys.exit(0)
    except Exception as e:  # pylint: disable=broad-except
        logger.error("程序异常: %s", e, exc_info=True)
        sys.exit(1)
