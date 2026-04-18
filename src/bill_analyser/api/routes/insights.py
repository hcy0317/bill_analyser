"""Insights API Routes — 异常洞察。"""

import asyncio
from collections import defaultdict
from datetime import datetime, timedelta
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("InsightsAPI")

bp = Blueprint("insights", __name__)


def _get_db() -> Any:
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _get_user_id() -> int:
    return int(getattr(request, "user_id", 0) or 0)


def _run_async(coroutine: Any) -> Any:
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


@bp.route("/anomalies", methods=["GET"])
@log_method
@require_auth
def get_anomalies():
    """检测异常模式：月度分类偏离、重复扣款嫌疑、大额交易。

    Query params:
        months: number of months to analyze (default: 6)
    """
    try:
        db = _get_db()
        user_id = _get_user_id()
        months = min(int(request.args.get("months") or 6), 24)

        # Fetch recent bills
        end_date = datetime.now().strftime("%Y-%m-%d")
        start_date = (datetime.now() - timedelta(days=months * 30)).strftime("%Y-%m-%d")
        bills = _run_async(db.get_bills(
            filters={"start_date": start_date, "end_date": end_date},
            limit=50000, offset=0, user_id=user_id,
        ))

        anomalies = []

        # 1. Large transactions (> 3x average for same category)
        category_amounts: dict[str, list[float]] = defaultdict(list)
        for bill in bills:
            cat = str(bill.get("main_category") or "未分类")
            amount = abs(float(bill.get("amount") or 0))
            bill_type = str(bill.get("type") or "")
            if bill_type in ("expense", "支出") and amount > 0:
                category_amounts[cat].append(amount)

        for bill in bills:
            cat = str(bill.get("main_category") or "未分类")
            amount = abs(float(bill.get("amount") or 0))
            bill_type = str(bill.get("type") or "")
            if bill_type not in ("expense", "支出") or amount <= 0:
                continue
            avg = sum(category_amounts[cat]) / len(category_amounts[cat]) if category_amounts[cat] else 0
            if avg > 0 and amount > avg * 3 and amount > 100:
                anomalies.append({
                    "type": "large_transaction",
                    "severity": "warning",
                    "billId": bill.get("id"),
                    "date": str(bill.get("date", ""))[:10],
                    "amount": round(amount, 2),
                    "category": cat,
                    "average": round(avg, 2),
                    "ratio": round(amount / avg, 1),
                    "description": bill.get("description") or bill.get("counterparty") or "",
                    "message": f"此笔交易金额 ¥{amount:.2f} 是 {cat} 分类平均值 ¥{avg:.2f} 的 {amount/avg:.1f} 倍",
                })

        # 2. Duplicate charges (same amount, same counterparty, within 3 days)
        expense_bills = [
            b for b in bills
            if str(b.get("type") or "") in ("expense", "支出")
        ]
        expense_bills.sort(key=lambda b: str(b.get("date", "")))

        seen_pairs: set[str] = set()
        for i, bill_a in enumerate(expense_bills):
            for j in range(i + 1, min(i + 20, len(expense_bills))):
                bill_b = expense_bills[j]
                date_a = str(bill_a.get("date", ""))[:10]
                date_b = str(bill_b.get("date", ""))[:10]
                try:
                    dt_a = datetime.strptime(date_a, "%Y-%m-%d")
                    dt_b = datetime.strptime(date_b, "%Y-%m-%d")
                    gap = abs((dt_b - dt_a).days)
                except (ValueError, TypeError):
                    continue

                if gap > 3:
                    break

                amt_a = round(abs(float(bill_a.get("amount") or 0)), 2)
                amt_b = round(abs(float(bill_b.get("amount") or 0)), 2)
                cp_a = str(bill_a.get("counterparty") or "").strip().lower()
                cp_b = str(bill_b.get("counterparty") or "").strip().lower()

                if amt_a == amt_b and amt_a > 0 and cp_a and cp_a == cp_b:
                    pair_key = f"{bill_a.get('id')}_{bill_b.get('id')}"
                    if pair_key not in seen_pairs:
                        seen_pairs.add(pair_key)
                        anomalies.append({
                            "type": "duplicate_charge",
                            "severity": "info",
                            "billIds": [bill_a.get("id"), bill_b.get("id")],
                            "dates": [date_a, date_b],
                            "amount": amt_a,
                            "counterparty": bill_a.get("counterparty"),
                            "message": f"疑似重复扣款: {bill_a.get('counterparty')} ¥{amt_a:.2f} ({date_a} & {date_b})",
                        })

        # 3. Monthly category deviation
        monthly_cat: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))
        for bill in bills:
            bill_type = str(bill.get("type") or "")
            if bill_type not in ("expense", "支出"):
                continue
            month = str(bill.get("date", ""))[:7]
            cat = str(bill.get("main_category") or "未分类")
            monthly_cat[month][cat] += abs(float(bill.get("amount") or 0))

        if len(monthly_cat) >= 3:
            months_sorted = sorted(monthly_cat.keys())
            latest_month = months_sorted[-1]
            prev_months = months_sorted[:-1]

            for cat, latest_total in monthly_cat[latest_month].items():
                prev_totals = [monthly_cat[m].get(cat, 0) for m in prev_months]
                avg_prev = sum(prev_totals) / len(prev_totals) if prev_totals else 0
                if avg_prev > 50 and latest_total > avg_prev * 2:
                    anomalies.append({
                        "type": "category_spike",
                        "severity": "warning",
                        "month": latest_month,
                        "category": cat,
                        "currentAmount": round(latest_total, 2),
                        "averageAmount": round(avg_prev, 2),
                        "ratio": round(latest_total / avg_prev, 1),
                        "message": f"{latest_month} 的 {cat} 支出 ¥{latest_total:.2f} 是历史平均 ¥{avg_prev:.2f} 的 {latest_total/avg_prev:.1f} 倍",
                    })

        # Sort by severity then date
        severity_order = {"error": 0, "warning": 1, "info": 2}
        anomalies.sort(key=lambda a: (severity_order.get(a.get("severity", "info"), 9), a.get("date", "")))

        return jsonify({
            "success": True,
            "data": {
                "anomalies": anomalies[:100],  # Cap at 100
                "totalCount": len(anomalies),
                "analyzedBills": len(bills),
                "analyzedMonths": months,
                "startDate": start_date,
                "endDate": end_date,
            },
        })
    except Exception as exc:
        logger.error("get_anomalies error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500
