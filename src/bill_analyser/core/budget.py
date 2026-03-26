"""
Budget Module - 预算管理模块

预算监控、预警和报告生成。
"""

from collections.abc import Callable
from datetime import datetime
from enum import Enum
from typing import Any

from ..utils.config import get_config
from ..utils.logger import get_logger, log_method, log_step
from .db import Database


class BudgetStatus(Enum):
    """预算状态枚举"""

    NORMAL = "normal"  # 正常
    WARNING = "warning"  # 警告（超过80%）
    CRITICAL = "critical"  # 严重（超过90%）
    EXCEEDED = "exceeded"  # 超支


class BudgetManager:
    """预算管理器"""

    def __init__(self, db: Database | None = None):
        """初始化"""
        self.logger = get_logger("BudgetManager")
        self.db = db or Database()
        self.budgets: dict[str, float] = {}
        self.alert_callbacks: list[Callable] = []

    @log_method
    @log_step("加载预算配置")
    async def load_budgets(self, config_file: str = "budget.json"):
        """加载预算配置"""
        budgets = get_config(config_file)
        self.budgets = budgets
        self.logger.info("已加载 %d 个预算项", len(budgets))

    @log_method
    def check_budget_status(self, category: str, spent: float) -> BudgetStatus:
        """
        检查预算状态

        Args:
            category: 分类
            spent: 已花费金额

        Returns:
            BudgetStatus: 预算状态
        """
        budget = self.budgets.get(category)
        if not budget:
            return BudgetStatus.NORMAL

        ratio = spent / budget

        if ratio >= 1.0:
            status = BudgetStatus.EXCEEDED
        elif ratio >= 0.9:
            status = BudgetStatus.CRITICAL
        elif ratio >= 0.8:
            status = BudgetStatus.WARNING
        else:
            status = BudgetStatus.NORMAL

        # 触发警报
        if status in [BudgetStatus.WARNING, BudgetStatus.CRITICAL, BudgetStatus.EXCEEDED]:
            self._trigger_alert(category, budget, spent, status)

        return status

    def _trigger_alert(self, category: str, budget: float, spent: float, status: BudgetStatus):
        """触发警报"""
        self.logger.warning(
            "预算警报 | 分类=%s, 预算=%.2f, 已花费=%.2f, 状态=%s", category, budget, spent, status.value
        )

        # 调用注册的回调函数
        for callback in self.alert_callbacks:
            try:
                callback(category, budget, spent, status)
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("警报回调执行失败: %s", e)

    def register_alert_callback(self, callback: Callable):
        """注册警报回调函数"""
        self.alert_callbacks.append(callback)
        self.logger.info("已注册警报回调函数")

    @log_method
    @log_step("生成预算报告")
    async def get_budget_report(self, period: str = "month") -> dict[str, Any]:
        """
        生成预算报告

        Args:
            period: 时间周期

        Returns:
            Dict: 预算报告
        """
        from .analyzer import Analyzer  # 避免循环导入

        analyzer = Analyzer(self.db)
        analysis = await analyzer.generate_report(period)

        by_category = analysis.get("by_category", {})

        report = {
            "period": period,
            "categories": {},
            "summary": {
                "total_budget": sum(self.budgets.values()),
                "total_spent": 0,
                "normal_count": 0,
                "warning_count": 0,
                "critical_count": 0,
                "exceeded_count": 0,
            },
            "generated_at": datetime.now().isoformat(),
        }

        for category, budget in self.budgets.items():
            spent = by_category.get(category, {}).get("total", 0.0)
            status = self.check_budget_status(category, spent)

            report["categories"][category] = {
                "budget": budget,
                "spent": spent,
                "remaining": budget - spent,
                "usage_ratio": (spent / budget * 100) if budget > 0 else 0,
                "status": status.value,
            }

            report["summary"]["total_spent"] += spent

            if status == BudgetStatus.NORMAL:
                report["summary"]["normal_count"] += 1
            elif status == BudgetStatus.WARNING:
                report["summary"]["warning_count"] += 1
            elif status == BudgetStatus.CRITICAL:
                report["summary"]["critical_count"] += 1
            elif status == BudgetStatus.EXCEEDED:
                report["summary"]["exceeded_count"] += 1

        self.logger.info("预算报告生成完成")
        return report
