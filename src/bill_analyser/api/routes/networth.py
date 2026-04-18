"""Net Worth API Routes — 净资产视图。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("NetWorthAPI")

bp = Blueprint("networth", __name__)


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


@bp.route("/snapshot", methods=["GET"])
@log_method
@require_auth
def get_net_worth_snapshot():
    """获取净资产快照：按账户分组聚合资产/负债/净值。"""
    try:
        db = _get_db()
        user_id = _get_user_id()

        accounts = _run_async(db.get_all_accounts(user_id=user_id))

        assets = []
        liabilities = []
        total_assets = 0.0
        total_liabilities = 0.0

        for acc in accounts:
            if acc.get("hidden"):
                continue
            balance = float(acc.get("balance") or 0)
            acc_type = str(acc.get("type") or "").lower()

            entry = {
                "id": acc.get("id"),
                "name": acc.get("name"),
                "type": acc_type,
                "icon": acc.get("icon"),
                "balance": round(balance, 2),
                "currency": acc.get("currency") or "CNY",
            }

            if acc_type in ("credit_card", "loan", "debt", "信用卡", "贷款"):
                liabilities.append(entry)
                total_liabilities += abs(balance)
            else:
                assets.append(entry)
                total_assets += balance

        net_worth = round(total_assets - total_liabilities, 2)

        return jsonify({
            "success": True,
            "data": {
                "assets": assets,
                "liabilities": liabilities,
                "totalAssets": round(total_assets, 2),
                "totalLiabilities": round(total_liabilities, 2),
                "netWorth": net_worth,
                "accountCount": len(assets) + len(liabilities),
            },
        })
    except Exception as exc:
        logger.error("get_net_worth_snapshot error: %s", exc)
        return jsonify({"success": False, "message": str(exc)}), 500
