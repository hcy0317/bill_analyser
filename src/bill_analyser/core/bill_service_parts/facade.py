"""Public BillService class composed from functional-domain mixins."""
from __future__ import annotations

# pylint: disable=too-many-ancestors,too-many-public-methods,too-many-instance-attributes,line-too-long

from .common import (
    Any,
    BillValidator,
    CategoryEngine,
    Database,
    ParserFactory,
    SmartDeduplicationEngine,
    get_logger,
    log_method,
    log_step,
    warnings,
)
from .account_matching import AccountMatchingMixin
from .investment_helpers import InvestmentHelpersMixin
from .import_learning_rules import ImportLearningRulesMixin
from .import_legacy import ImportLegacyMixin
from .category_maintenance import CategoryMaintenanceMixin
from .import_v2_pipeline import ImportV2PipelineMixin
from .import_preview_projection import ImportPreviewProjectionMixin
from .import_preview_paging import ImportPreviewPagingMixin
from .matching_reads import MatchingReadsMixin
from .matching_preview_accept import MatchingPreviewAcceptMixin
from .matching_preview_reject_clear import MatchingPreviewRejectClearMixin
from .matching_actions import MatchingActionsMixin
from .preview_pairing_decisions import PreviewPairingDecisionsMixin
from .preview_learning_decisions import PreviewLearningDecisionsMixin
from .import_learning_signals import ImportLearningSignalsMixin
from .import_reclassify import ImportReclassifyMixin


class BillService(
    AccountMatchingMixin,
    InvestmentHelpersMixin,
    ImportLearningRulesMixin,
    ImportLegacyMixin,
    CategoryMaintenanceMixin,
    ImportV2PipelineMixin,
    ImportPreviewProjectionMixin,
    ImportPreviewPagingMixin,
    MatchingReadsMixin,
    MatchingPreviewAcceptMixin,
    MatchingPreviewRejectClearMixin,
    MatchingActionsMixin,
    PreviewPairingDecisionsMixin,
    PreviewLearningDecisionsMixin,
    ImportLearningSignalsMixin,
    ImportReclassifyMixin,
):
    """账单导入服务 public facade，功能实现由各领域 mixin 组合提供。"""

    _PARSER_SOURCE_LABELS = {
        "wechat": "微信",
        "alipay": "支付宝",
        "abc": "农业银行",
        "ccb": "建设银行",
        "cmbc": "民生银行",
        "icbc": "工商银行",
        "generic": "通用来源",
    }

    def __init__(
        self,
        db: Database | None = None,
        deduplication_mode: Any | None = None,
        use_smart_dedup: bool = True,
    ):
        """
        初始化服务

        参数：
            db: 数据库实例，如果为 None 则创建新实例
            deduplication_mode: 已弃用，仅为兼容旧调用签名保留
            use_smart_dedup: 已弃用，仅为兼容旧调用签名保留
        """
        self.logger = get_logger("BillService")
        self.db = db or Database()
        self.category_engine = CategoryEngine()
        self.parser_factory = ParserFactory()
        self.validator = BillValidator()

        if deduplication_mode is not None:
            warnings.warn(
                "BillService(deduplication_mode=...) 已弃用；运行时仅保留 SmartDeduplicationEngine。",
                DeprecationWarning,
                stacklevel=2,
            )
        if not use_smart_dedup:
            warnings.warn(
                "BillService(use_smart_dedup=False) 已弃用；运行时会继续使用 SmartDeduplicationEngine。",
                DeprecationWarning,
                stacklevel=2,
            )

        # 智能去重引擎是唯一运行时去重实现。
        self.smart_dedup_engine = SmartDeduplicationEngine()
        self._initialized = False

    @log_method
    @log_step("初始化账单服务")
    async def initialize(self):
        """初始化服务"""
        if self._initialized:
            return

        # 初始化数据库
        await self.db.init_db()

        # 加载分类规则
        await self.category_engine.load_rules_from_db(self.db)

        self._initialized = True
        self.logger.info("账单服务初始化完成")

    async def _reload_category_rules_for_import(self, user_id: int = 1) -> None:
        """Reload canonical category_rules before import classification.

        Category rules can be edited by another request while a long-lived
        BillService instance keeps an initialized CategoryEngine.  Import
        preview/confirmation must therefore read the canonical category_rules
        source at the start of each classification pass instead of trusting an
        already-initialized in-memory rule set.
        """
        await self.category_engine.load_rules_from_db(self.db, user_id=user_id)

        if not self.category_engine.rules:
            migrate_keywords_to_rules = getattr(self.db, "migrate_keywords_to_rules", None)
            if callable(migrate_keywords_to_rules):
                try:
                    migration_result = await migrate_keywords_to_rules(user_id=user_id)
                except Exception as exc:  # pylint: disable=broad-exception-caught
                    self.logger.warning(
                        "导入分类规则为空，自动迁移旧关键词失败: user_id=%d, error=%s",
                        user_id,
                        exc,
                    )
                else:
                    migrated_count = 0
                    if isinstance(migration_result, dict):
                        migrated_count = int(migration_result.get("migrated") or 0)
                    if migrated_count > 0:
                        self.logger.info(
                            "导入分类规则为空，已从旧关键词迁移 %d 条 canonical 规则: user_id=%d",
                            migrated_count,
                            user_id,
                        )
                        await self.category_engine.load_rules_from_db(self.db, user_id=user_id)

        self.logger.debug(
            "分类规则已刷新: user_id=%d, 规则数=%d",
            user_id,
            len(self.category_engine.rules),
        )

    async def close(self):
        """关闭服务"""
        await self.db.close()
        self.logger.info("账单服务已关闭")

    async def __aenter__(self):
        """异步上下文管理器入口"""
        await self.initialize()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """异步上下文管理器退出"""
        await self.close()
