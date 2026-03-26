#!/usr/bin/env python
"""重建预算历史快照。

用法示例：
    .venv\\Scripts\\python scripts\rebuild_budget_history.py --user-id 1 --start 2025-01-01 --end 2026-03-08
    .venv\\Scripts\\python scripts\rebuild_budget_history.py --user-id 1 --start 2025-01-01 --end 2026-03-08 --dry-run

说明：
- 支持 monthly / quarterly / yearly 三种周期。
- 支持支出(3) / 投资(5) 两种预算类型批量重建。
- 默认重建 monthly,quarterly,yearly + 3,5 全组合。
"""

from __future__ import annotations

import argparse
import asyncio
import calendar
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import date, datetime
from pathlib import Path

# 允许直接从项目根目录运行脚本
PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from bill_analyser.core.db import Database


@dataclass(frozen=True)
class SnapshotPeriod:
    """预算快照周期。"""

    period_type: str
    start_date: str
    end_date: str


def parse_args() -> argparse.Namespace:
    """解析命令行参数。"""
    parser = argparse.ArgumentParser(description="重建 budget_history 历史快照")
    parser.add_argument("--user-id", type=int, default=1, help="用户ID，默认 1")
    parser.add_argument("--start", required=True, help="开始日期，格式 YYYY-MM-DD")
    parser.add_argument("--end", required=True, help="结束日期，格式 YYYY-MM-DD")
    parser.add_argument(
        "--period-types", default="monthly,quarterly,yearly", help="周期类型，逗号分隔，默认 monthly,quarterly,yearly"
    )
    parser.add_argument("--budget-types", default="3,5", help="预算类型，逗号分隔，默认 3,5（支出/投资）")
    parser.add_argument("--db-path", default="", help="可选数据库路径，默认 data/bills.db")
    parser.add_argument("--dry-run", action="store_true", help="仅输出将要重建的周期，不写入数据库")
    return parser.parse_args()


def parse_iso_date(value: str) -> date:
    """解析 ISO 日期。"""
    return datetime.strptime(value, "%Y-%m-%d").date()


def month_range(year: int, month: int) -> tuple[str, str]:
    """返回某月起止日期。"""
    last_day = calendar.monthrange(year, month)[1]
    return (f"{year}-{month:02d}-01", f"{year}-{month:02d}-{last_day:02d}")


def quarter_range(year: int, quarter: int) -> tuple[str, str]:
    """返回某季度起止日期。"""
    start_month = (quarter - 1) * 3 + 1
    end_month = start_month + 2
    end_day = calendar.monthrange(year, end_month)[1]
    return (f"{year}-{start_month:02d}-01", f"{year}-{end_month:02d}-{end_day:02d}")


def year_range(year: int) -> tuple[str, str]:
    """返回某年起止日期。"""
    return f"{year}-01-01", f"{year}-12-31"


def iter_month_periods(start: date, end: date) -> Iterable[SnapshotPeriod]:
    """按月生成周期。"""
    year = start.year
    month = start.month

    while (year, month) <= (end.year, end.month):
        period_start, period_end = month_range(year, month)
        yield SnapshotPeriod("monthly", period_start, period_end)
        month += 1
        if month > 12:
            month = 1
            year += 1


def iter_quarter_periods(start: date, end: date) -> Iterable[SnapshotPeriod]:
    """按季度生成周期。"""
    year = start.year
    quarter = ((start.month - 1) // 3) + 1

    while True:
        period_start, period_end = quarter_range(year, quarter)
        if parse_iso_date(period_start) > end:
            break
        yield SnapshotPeriod("quarterly", period_start, period_end)
        quarter += 1
        if quarter > 4:
            quarter = 1
            year += 1


def iter_year_periods(start: date, end: date) -> Iterable[SnapshotPeriod]:
    """按年生成周期。"""
    for year in range(start.year, end.year + 1):
        period_start, period_end = year_range(year)
        yield SnapshotPeriod("yearly", period_start, period_end)


def build_periods(start: date, end: date, period_types: list[str]) -> list[SnapshotPeriod]:
    """根据周期类型构建所有待重建周期。"""
    all_periods: list[SnapshotPeriod] = []

    if "monthly" in period_types:
        all_periods.extend(iter_month_periods(start, end))
    if "quarterly" in period_types:
        all_periods.extend(iter_quarter_periods(start, end))
    if "yearly" in period_types:
        all_periods.extend(iter_year_periods(start, end))

    return all_periods


async def rebuild_snapshots(args: argparse.Namespace) -> int:
    """执行预算历史快照重建。"""
    start = parse_iso_date(args.start)
    end = parse_iso_date(args.end)

    if start > end:
        raise ValueError("开始日期不能晚于结束日期")

    period_types = [item.strip() for item in args.period_types.split(",") if item.strip()]
    budget_types = [int(item.strip()) for item in args.budget_types.split(",") if item.strip()]
    periods = build_periods(start, end, period_types)

    print(f"用户ID: {args.user_id}")
    print(f"周期类型: {period_types}")
    print(f"预算类型: {budget_types}")
    print(f"时间范围: {args.start} ~ {args.end}")
    print(f"待处理周期数: {len(periods)}")

    if args.dry_run:
        for period in periods:
            print(f"[DRY-RUN] {period.period_type}: {period.start_date} ~ {period.end_date}")
        return 0

    db = Database(args.db_path or None)
    await db.init_db()

    total_created = 0
    total_runs = 0
    try:
        for budget_type in budget_types:
            for period in periods:
                result = await db.create_budget_execution_snapshots(
                    budget_type=budget_type,
                    period_type=period.period_type,
                    start_date=period.start_date,
                    end_date=period.end_date,
                    user_id=args.user_id,
                )
                created_count = int(result.get("created_count", 0))
                total_created += created_count
                total_runs += 1
                print(
                    f"[OK] type={budget_type} period={period.period_type} "
                    f"range={period.start_date}~{period.end_date} created={created_count}"
                )
    finally:
        await db.close()

    print(f"完成: 执行 {total_runs} 次快照重建，累计写入/覆盖 {total_created} 条历史记录")
    return 0


def main() -> int:
    """脚本入口。"""
    args = parse_args()
    return asyncio.run(rebuild_snapshots(args))


if __name__ == "__main__":
    raise SystemExit(main())
