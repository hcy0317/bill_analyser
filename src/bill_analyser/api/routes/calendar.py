"""Calendar API Routes — 日历/现金流视图。"""

import asyncio
from collections import defaultdict
from datetime import datetime, timedelta
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("CalendarAPI")

bp = Blueprint("calendar", __name__)


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


@bp.route("/events", methods=["GET"])
@log_method
@require_auth
def get_calendar_events():
    """获取日历事件数据：按日聚合已入账交易 + 周期预测。

    Query params:
        start_date: YYYY-MM-DD (required)
        end_date: YYYY-MM-DD (required)
    """
    try:
        db = _get_db()
        user_id = _get_user_id()
        start_date = request.args.get("start_date", "")
        end_date = request.args.get("end_date", "")

        if not start_date or not end_date:
            return jsonify({"success": False, "message": "start_date and end_date are required"}), 400

        # 1. Fetch actual bills in date range
        bills = _run_async(db.get_bills_by_date_range(start_date, end_date, user_id=user_id))

        # 2. Aggregate by day
        daily: dict[str, dict[str, Any]] = defaultdict(lambda: {
            "date": "",
            "income": 0.0,
            "expense": 0.0,
            "transferIn": 0.0,
            "transferOut": 0.0,
            "count": 0,
            "bills": [],
        })

        for bill in bills:
            bill_date = str(bill.get("date", ""))[:10]
            if not bill_date:
                continue
            day = daily[bill_date]
            day["date"] = bill_date
            day["count"] += 1

            amount = abs(float(bill.get("amount") or 0))
            bill_type = str(bill.get("type") or "").lower()

            if bill_type in ("expense", "支出"):
                day["expense"] += amount
            elif bill_type in ("income", "收入"):
                day["income"] += amount
            elif bill_type in ("transfer", "转账"):
                day["transferOut"] += amount

            day["bills"].append({
                "id": bill.get("id"),
                "amount": float(bill.get("amount") or 0),
                "type": bill.get("type"),
                "counterparty": bill.get("counterparty"),
                "description": bill.get("description"),
                "mainCategory": bill.get("main_category"),
                "subCategory": bill.get("sub_category"),
            })

        # 3. Generate recurring projections
        recurring_events = []
        try:
            recurring_rules = _run_async(db.get_enabled_recurring_templates(user_id=user_id))
            start_dt = datetime.strptime(start_date, "%Y-%m-%d")
            end_dt = datetime.strptime(end_date, "%Y-%m-%d")

            freq_days = {
                "weekly": 7, "biweekly": 14, "monthly": 30,
                "bimonthly": 60, "quarterly": 90,
                "semiannual": 180, "annual": 365,
            }

            for rule in recurring_rules:
                next_date_str = rule.get("next_date")
                if not next_date_str:
                    continue
                try:
                    next_dt = datetime.strptime(str(next_date_str)[:10], "%Y-%m-%d")
                except (ValueError, TypeError):
                    continue

                freq = str(rule.get("frequency") or "monthly")
                interval = freq_days.get(freq, 30)

                # Generate up to 50 occurrences within range
                current = next_dt
                for _ in range(50):
                    if current > end_dt:
                        break
                    if current >= start_dt:
                        recurring_events.append({
                            "date": current.strftime("%Y-%m-%d"),
                            "type": "recurring_projection",
                            "name": rule.get("name"),
                            "amount": float(rule.get("amount") or 0),
                            "billType": rule.get("type"),
                            "frequency": freq,
                            "recurringId": rule.get("id"),
                        })
                    current += timedelta(days=interval)

        except Exception as exc:
            logger.warning("Failed to generate recurring projections: %s", exc)

        # 4. Round amounts
        events = []
        for date_str in sorted(daily.keys()):
            d = daily[date_str]
            events.append({
                "date": d["date"],
                "income": round(d["income"], 2),
                "expense": round(d["expense"], 2),
                "transferIn": round(d["transferIn"], 2),
                "transferOut": round(d["transferOut"], 2),
                "net": round(d["income"] - d["expense"], 2),
                "count": d["count"],
                "bills": d["bills"],
            })

        return jsonify({
            "success": True,
            "data": {
                "events": events,
                "recurringProjections": recurring_events,
                "startDate": start_date,
                "endDate": end_date,
            },
        })
    except Exception as exc:
        logger.error("get_calendar_events error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500
