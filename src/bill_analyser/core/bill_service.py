"""
Bill Service Module - 账单导入服务

异步账单导入、识别、分类和写入数据库的服务模块。
支持：
- 多格式账单解析（微信、支付宝、各银行）
- 智能去重（支付平台vs银行、转账识别、分账单识别）
- 关键词匹配自动分类
- 预览模式（先预览后确认导入）
"""

import asyncio
import re
from datetime import datetime
from typing import Dict, List, Any, Optional, Tuple

from .db import Database
from .category_engine import CategoryEngine
from .investment_settings import (
    DEFAULT_INVESTMENT_NAMED_PRODUCT_PATTERNS,
    DEFAULT_INVESTMENT_PLATFORM_ALIASES,
    DEFAULT_INVESTMENT_PRODUCT_PATTERNS,
    build_user_investment_keyword_settings,
)
from .smart_dedup import SmartDeduplicationEngine, DeduplicationType
from ..parsers.factory import ParserFactory
from ..utils.logger import get_logger, log_method, log_step
from ..utils.validator import BillValidator
from ..utils.deduplication import DeduplicationEngine, DeduplicationMode
# v6.72: 移除 TransactionType 导入，分类类型过滤改为由 match_category() 内部自动处理


class BillService:
    """账单导入服务

    支持功能：
    - 自动识别账单格式并选择解析器
    - 智能去重：支付平台vs银行、转账识别、分账单合并
    - 关键词匹配自动分类
    - 预览模式：解析后先预览，确认后再导入
    - 批量文件导入
    """

    def __init__(
        self,
        db: Optional[Database] = None,
        deduplication_mode: Optional[DeduplicationMode] = None,
        use_smart_dedup: bool = True
    ):
        """
        初始化服务

        Args:
            db: 数据库实例，如果为 None 则创建新实例
            deduplication_mode: 去重模式（用于传统去重引擎）
            use_smart_dedup: 是否使用智能去重引擎（推荐True）
        """
        self.logger = get_logger('BillService')
        self.db = db or Database()
        self.category_engine = CategoryEngine()
        self.parser_factory = ParserFactory()
        self.validator = BillValidator()

        # 智能去重引擎（优先使用）
        self.use_smart_dedup = use_smart_dedup
        self.smart_dedup_engine = SmartDeduplicationEngine() if use_smart_dedup else None

        # 传统去重引擎（备用）
        self.deduplication_engine = DeduplicationEngine(
            deduplication_mode or DeduplicationMode.ADVANCED
        )
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

    @log_method
    @log_step("导入账单文件")
    async def import_bills(
        self,
        file_path: str,
        parser_type: str = 'auto',
        preview_only: bool = False,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        导入账单文件

        Args:
            file_path: 账单文件路径
            parser_type: 解析器类型（auto/wechat/alipay/icbc/cmbc/abc/ccb）
            preview_only: 是否仅预览，不实际写入数据库
            user_id: 用户ID（多用户隔离）

        Returns:
            Dict: 导入结果统计，包含：
                - success: 是否成功
                - total: 原始解析数量
                - valid: 验证通过数量
                - inserted: 插入数量
                - duplicates: 数据库重复数量
                - dedup_stats: 智能去重统计
                - uncategorized: 未分类账单列表（供人工分类）
                - preview: 预览数据（仅preview_only=True时）
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'file': file_path,
            'parser_type': parser_type,
            'preview_only': preview_only,
            'total': 0,
            'valid': 0,
            'invalid': 0,
            'inserted': 0,
            'duplicates': 0,
            'dedup_stats': None,
            'categories': {},
            'uncategorized': [],  # 未分类账单，需人工处理
            'errors': [],
            'preview': []  # 预览数据
        }

        try:
            # 1. 解析文件
            self.logger.info("步骤 1/5: 解析文件 (parser_type=%s)", parser_type)
            loop = asyncio.get_event_loop()

            # 检测实际解析器类型
            if parser_type == 'auto':
                parser_info = self.parser_factory.detect_parser(file_path)
                detected_type = parser_info['id'] if parser_info else None
                self.logger.info("自动检测解析器类型: %s", detected_type)
                result['detected_parser'] = detected_type
            else:
                detected_type = parser_type

            # 使用检测到的解析器解析
            bills = await loop.run_in_executor(
                None,
                self.parser_factory.parse,
                file_path,
                detected_type if detected_type else None
            )

            result['total'] = len(bills)
            result['parser_type'] = detected_type or 'unknown'

            if not bills:
                result['errors'].append("文件解析失败或无有效数据")
                return result

            self.logger.info("解析完成: %d 条账单 (使用 %s 解析器)",
                             len(bills), result['parser_type'])

            # 2. 验证账单
            self.logger.info("步骤 2/5: 验证数据")
            valid_bills, invalid_bills = await loop.run_in_executor(
                None,
                self.validator.validate_bills,
                bills
            )

            result['valid'] = len(valid_bills)
            result['invalid'] = len(invalid_bills)

            if invalid_bills:
                for invalid_bill in invalid_bills[:10]:  # 只记录前10个错误
                    result['errors'].append({
                        'index': invalid_bill.get('_index'),
                        'errors': invalid_bill.get('_validation_errors')
                    })

            if not valid_bills:
                result['errors'].append("没有有效的账单数据")
                return result

            # 3. 智能去重（包含数据库对比，跨文件去重）
            self.logger.info("步骤 3/5: 智能去重（含数据库对比）")
            if self.use_smart_dedup and self.smart_dedup_engine:
                # v6.45: 使用 process_with_db() 替代 process()
                # process_with_db() 会额外与数据库已有账单对比，实现跨文件去重
                dedup_result = await self.smart_dedup_engine.process_with_db(
                    valid_bills, self.db, user_id
                )
                deduplicated_bills = dedup_result.kept_bills

                # 统计数据库重复数量
                db_dup_count = sum(
                    1 for g in dedup_result.duplicate_groups
                    if g.type == DeduplicationType.DATABASE_DUPLICATE
                )

                result['dedup_stats'] = {
                    'original_count': dedup_result.original_count,
                    'removed_count': dedup_result.removed_count,
                    'transfer_pairs': len(dedup_result.transfer_pairs),
                    'split_groups': len(dedup_result.split_groups),
                    'duplicate_groups': len(dedup_result.duplicate_groups),
                    'database_duplicates': db_dup_count
                }
                self.logger.info(
                    "智能去重完成: 移除 %d, 转账对 %d, 分账组 %d, 数据库重复 %d",
                    dedup_result.removed_count,
                    len(dedup_result.transfer_pairs),
                    len(dedup_result.split_groups),
                    db_dup_count
                )
            else:
                # 使用传统去重引擎
                deduplicated_bills, dedup_stats = await loop.run_in_executor(
                    None,
                    self.deduplication_engine.deduplicate_all,
                    valid_bills
                )
                result['dedup_stats'] = dedup_stats

            # 4. 分类账单（预览和正式导入都需要）
            self.logger.info("步骤 4/5: 自动分类 (user_id=%d)", user_id)

            # 检查是否需要重新加载分类规则（当user_id变化时）
            engine_initialized = self.category_engine.is_initialized
            engine_user_id = self.category_engine.current_user_id
            self.logger.debug(
                "[分类规则检查] initialized=%s, engine_user=%d, request_user=%d",
                engine_initialized, engine_user_id, user_id
            )

            if not engine_initialized or engine_user_id != user_id:
                await self.category_engine.load_rules_from_db(self.db, user_id=user_id)
                self.logger.debug("分类规则加载: user_id=%d, 规则数=%d",
                                  user_id, len(self.category_engine.rules))

            learned_seed_count = await self._apply_import_learning_rules(
                deduplicated_bills,
                user_id=user_id,
                type_only=True,
                record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[长期学习] 分类前类型预填充 %d 条", learned_seed_count)

            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            # 不再需要在调用层分离账单，由 match_category() 内部根据金额自动判断

            self.logger.info("[分类匹配] 共 %d 条账单待分类", len(deduplicated_bills))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(
                deduplicated_bills, types=None
            )

            # 4.2 投资账单专门识别链路（平台/产品关键词）
            self.logger.info("步骤 4.2/5: 投资候选识别")
            categorized_bills = await self._detect_investment_candidates(
                categorized_bills, user_id=user_id
            )

            # 5. 账户匹配（基于账户名称）
            self.logger.info("步骤 4.5/5: 自动匹配账户")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            # 6. v6.77: 存取转账检测（用户指定的分类自动转换为转账类型）
            self.logger.info("步骤 4.6/5: 存取转账检测")
            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learned_replay_count = await self._apply_import_learning_rules(
                matched_bills,
                user_id=user_id,
                type_only=False,
                record_usage=True
            )
            if learned_replay_count > 0:
                self.logger.info("[长期学习] 匹配后回放 %d 条", learned_replay_count)

            # 如果是预览模式，返回分类和匹配后的数据供前端确认
            if preview_only:
                # 分离已匹配和未匹配的账单
                matched_list = []
                unmatched_list = []

                for bill in matched_bills:
                    preview_bill = self._bill_to_preview(bill)

                    # 判断是否有有效分类（非空字符串）
                    has_category = bool(bill.get('main_category'))

                    # 判断是否有有效账户ID（必须是数字类型，不能是解析器标识符）
                    source_account_id = bill.get('source_account_id')
                    has_account = source_account_id is not None and (
                        isinstance(source_account_id, int) or
                        (isinstance(source_account_id, str) and source_account_id.isdigit())
                    )

                    preview_bill['is_matched'] = has_category and has_account
                    preview_bill['has_category'] = has_category
                    preview_bill['has_account'] = has_account

                    # 添加调试信息
                    self.logger.debug(
                        "预览账单: date=%s, main_category=%s, source_account_id=%s, "
                        "has_category=%s, has_account=%s, is_matched=%s",
                        bill.get('date', '')[:10],
                        bill.get('main_category'),
                        source_account_id,
                        has_category,
                        has_account,
                        preview_bill['is_matched']
                    )

                    if preview_bill['is_matched']:
                        matched_list.append(preview_bill)
                    else:
                        unmatched_list.append(preview_bill)

                result['preview'] = matched_list + unmatched_list  # 已匹配在前，未匹配在后
                result['matched_count'] = len(matched_list)
                result['unmatched_count'] = len(unmatched_list)
                result['success'] = True
                self.logger.info(
                    "预览模式统计: 已匹配 %d 条 (分类+账户), 未匹配 %d 条",
                    len(matched_list), len(unmatched_list)
                )
                return result

            # 统计分类结果，收集未分类账单
            for bill in matched_bills:
                main_cat = bill.get('main_category')
                if main_cat:
                    result['categories'][main_cat] = result['categories'].get(main_cat, 0) + 1
                else:
                    # 收集未分类账单供人工分类
                    result['uncategorized'].append(self._bill_to_preview(bill))

            self.logger.info(
                "分类完成: 已分类 %d 条, 未分类 %d 条",
                len(matched_bills) - len(result['uncategorized']),
                len(result['uncategorized'])
            )

            # 5. 写入数据库
            self.logger.info("步骤 5/5: 写入数据库")
            batch_id = datetime.now().strftime('%Y%m%d%H%M%S')
            inserted = await self.db.insert_bills(matched_bills, batch_id, user_id=user_id)

            result['inserted'] = inserted
            result['duplicates'] = len(deduplicated_bills) - inserted
            result['success'] = True

            self.logger.info(
                "导入完成: 总计 %d 条, 有效 %d 条, 去重后 %d 条, 插入 %d 条, "
                "数据库重复 %d 条, 未分类 %d 条",
                result['total'],
                result['valid'],
                len(deduplicated_bills),
                result['inserted'],
                result['duplicates'],
                len(result['uncategorized'])
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("导入账单失败: %s", e, exc_info=True)
            result['errors'].append(str(e))

        return result

    def _prepare_preview_data(self, bills: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """准备预览数据，转换datetime为字符串，添加前端期望的字段"""
        preview_bills = []
        for bill in bills:
            preview_bills.append(self._bill_to_preview(bill))

        self.logger.info("[预览数据] 准备了 %d 条预览数据", len(preview_bills))
        if preview_bills:
            sample = preview_bills[0]
            self.logger.info("[预览数据] 示例字段: %s", list(sample.keys()))

        return preview_bills

    def _bill_to_preview(self, bill: Dict[str, Any]) -> Dict[str, Any]:
        """将账单转换为预览格式，添加前端期望的字段"""
        bill_copy = bill.copy()

        # 处理datetime类型
        for k, v in bill_copy.items():
            if isinstance(v, datetime):
                bill_copy[k] = v.strftime('%Y-%m-%d %H:%M:%S')

        # 前端期望字段: time, type, amount, description, counterparty, paymentMethod,
        #              main_category, sub_category, account
        # 后端实际字段: date, type, amount, description, counterparty, payment_method,
        #              main_category, sub_category, source_account_id

        # 添加time字段（前端期望）
        if 'time' not in bill_copy and 'date' in bill_copy:
            bill_copy['time'] = bill_copy['date']

        # 添加account字段（前端期望）
        if 'account' not in bill_copy:
            bill_copy['account'] = bill_copy.get('source_account_id', '')

        # v6.32: 添加paymentMethod字段（驼峰命名，前端期望）
        if 'paymentMethod' not in bill_copy:
            bill_copy['paymentMethod'] = bill_copy.get('payment_method', '')

        self.logger.debug(f"[预览数据] time={bill_copy.get('time', '')}, "
                         f"counterparty={bill_copy.get('counterparty', '')[:20]}, "
                         f"paymentMethod={bill_copy.get('paymentMethod', '')[:20]}")
        return bill_copy

    @log_method
    async def _match_accounts(
        self,
        bills: List[Dict[str, Any]],
        user_id: int = 1
    ) -> List[Dict[str, Any]]:
        """
        根据账单描述自动匹配账户

        v6.39重构：简化账户匹配逻辑，只使用两级匹配

        匹配规则（按优先级）：
        1. 检查是否已是有效的数据库账户ID
        2. 使用 payment_method 与账户名称/别名匹配（优先级最高）
        3. 从 description 中提取关键词与账户名称/别名匹配（次优先级）

        注意：转账/投资类型的账户已在 SmartDeduplicationEngine 配对时设置，
        此方法主要处理收入/支出类型的源账户匹配。

        Args:
            bills: 账单列表
            user_id: 用户ID

        Returns:
            List[Dict]: 添加了账户信息的账单列表
        """
        # 获取用户的账户别名映射
        alias_mapping = await self.db.get_account_alias_mapping(user_id)
        if not alias_mapping:
            self.logger.warning("用户没有任何账户，跳过账户匹配")
            return bills

        # 获取用户的所有账户（用于通过ID查找账户名称）
        accounts = await self.db.get_all_accounts(user_id=user_id)
        account_by_id = {str(acc.get('id')): acc for acc in accounts}

        matched_count = 0

        # v6.63: 简化日志，只输出别名数量
        self.logger.debug("账户别名映射: %d条", len(alias_mapping))

        # 预先按别名长度排序（长的优先匹配，更精确）
        sorted_aliases = sorted(alias_mapping.keys(), key=len, reverse=True)

        for bill in bills:
            # 获取当前的 source_account_id
            current_account_id = bill.get('source_account_id')

            # 检查是否已经是有效的数据库账户ID（数字类型）
            is_valid_account_id = isinstance(current_account_id, int) or (
                isinstance(current_account_id, str) and current_account_id.isdigit()
            )

            if is_valid_account_id and str(current_account_id) in account_by_id:
                matched_count += 1
                bill['account_name'] = account_by_id[str(current_account_id)].get('name')
                self.logger.debug("账单已有有效账户ID: %s", current_account_id)
            else:
                matched_account_id = None
                match_source = None

                # v6.39 优先级1: 使用 payment_method 匹配账户
                payment_method = str(bill.get('payment_method', '')).strip()
                if payment_method:
                    payment_method_lower = payment_method.lower()
                    for alias in sorted_aliases:
                        if alias.lower() in payment_method_lower:
                            matched_account_id = alias_mapping[alias]
                            match_source = f"payment_method:{alias}"
                            break

                # v6.66: 优先级2: 从 description/counterparty 中匹配账户别名
                # 将此优先级提前到 _parser_id 匹配之前
                if not matched_account_id:
                    description = str(bill.get('description', ''))
                    counterparty = str(bill.get('counterparty', ''))
                    combined_text = f"{description} {counterparty}"
                    combined_text_lower = combined_text.lower()

                    for alias in sorted_aliases:
                        if alias.lower() in combined_text_lower:
                            matched_account_id = alias_mapping[alias]
                            match_source = f"description:{alias}"
                            break

                # M2-1: 历史账单记忆匹配（优先于 parser_id 回退）
                if not matched_account_id:
                    history_match = await self.db.get_historical_source_account_suggestion(
                        user_id=user_id,
                        payment_method=str(bill.get('payment_method', '')),
                        counterparty=str(bill.get('counterparty', '')),
                        description=str(bill.get('description', '')),
                        bill_type=str(bill.get('type', '')),
                    )
                    if history_match:
                        matched_account_id = history_match['account_id']
                        match_source = (
                            'history:' + ','.join(history_match.get('reasons', []))
                        )

                # v6.66: 优先级3（最低）- 使用 _parser_id 匹配账户（回退机制）
                # _parser_id 是解析器标识，如 'alipay', 'wechat', 'abc', 'icbc' 等
                # 仅当 payment_method 和 description/counterparty 都无法匹配时才使用
                if not matched_account_id:
                    parser_id = str(bill.get('_parser_id', '')).strip()
                    if parser_id:
                        parser_id_lower = parser_id.lower()
                        for alias in sorted_aliases:
                            if alias.lower() == parser_id_lower or \
                               alias.lower() in parser_id_lower or \
                               parser_id_lower in alias.lower():
                                matched_account_id = alias_mapping[alias]
                                match_source = f"parser_id:{alias}"
                                self.logger.debug(
                                    "[源账户-parser_id匹配] parser_id='%s' 匹配别名='%s'",
                                    parser_id, alias
                                )
                                break

                # 如果匹配到账户，更新账单
                if matched_account_id:
                    matched_account = account_by_id.get(str(matched_account_id))
                    if matched_account:
                        bill['source_account_id'] = matched_account_id
                        bill['account_name'] = matched_account.get('name')
                        matched_count += 1
                        self.logger.debug(
                            "源账户匹配成功: 来源=%s -> 账户=%s (ID=%s)",
                            match_source,
                            matched_account.get('name'), matched_account_id
                        )
                else:
                    # 未匹配到账户，清空（避免混淆）
                    bill['source_account_id'] = None
                    self.logger.debug(
                        "源账户匹配失败: payment_method='%s', parser_id='%s', description='%s'",
                        payment_method[:20] if payment_method else '',
                        str(bill.get('_parser_id', ''))[:20],
                        str(bill.get('description', ''))[:30]
                    )

            # v6.42.1: 投资类型账单的目标账户匹配
            # 如果账单类型是投资，且没有destination_account_id，尝试从counterparty/description匹配
            bill_type = str(bill.get('type', '')).lower()
            is_investment = bill_type in ['投资', 'investment', '5']

            if is_investment:
                current_dest_id = bill.get('destination_account_id')
                # 检查目标账户是否已有效
                is_valid_dest = isinstance(current_dest_id, int) or (
                    isinstance(current_dest_id, str) and current_dest_id.isdigit()
                )

                if not is_valid_dest or str(current_dest_id) not in account_by_id:
                    # v6.65: 优先使用 counterparty 和 description 匹配目标账户
                    counterparty = str(bill.get('counterparty', ''))
                    description = str(bill.get('description', ''))
                    parser_id = str(bill.get('_parser_id', ''))
                    investment_hint = str(bill.get('_investment_hint', ''))
                    # 将 parser_id 也加入匹配文本，当 counterparty/description 无法匹配时提供回退
                    combined_text = (
                        f"{counterparty} {description} {parser_id} {investment_hint}"
                    )
                    combined_text_lower = combined_text.lower()

                    dest_account_id = None
                    dest_match_source = None

                    for alias in sorted_aliases:
                        if alias.lower() in combined_text_lower:
                            potential_id = alias_mapping[alias]
                            # 确保目标账户不同于源账户
                            if str(potential_id) != str(bill.get('source_account_id')):
                                dest_account_id = potential_id
                                dest_match_source = alias
                                break

                    # v6.65: 如果上面匹配失败，单独尝试使用 parser_id 精确匹配
                    if not dest_account_id and parser_id:
                        parser_id_lower = parser_id.lower()
                        for alias in sorted_aliases:
                            if alias.lower() == parser_id_lower or \
                               alias.lower() in parser_id_lower or \
                               parser_id_lower in alias.lower():
                                potential_id = alias_mapping[alias]
                                if str(potential_id) != str(bill.get('source_account_id')):
                                    dest_account_id = potential_id
                                    dest_match_source = f"parser_id:{alias}"
                                    break

                    if not dest_account_id:
                        history_dest = await self.db.get_historical_destination_account_suggestion(
                            user_id=user_id,
                            payment_method=str(bill.get('payment_method', '')),
                            counterparty=counterparty,
                            description=description,
                            bill_type=str(bill.get('type', '')),
                            source_account_id=(
                                int(bill.get('source_account_id'))
                                if isinstance(bill.get('source_account_id'), int) or
                                (isinstance(bill.get('source_account_id'), str) and
                                 str(bill.get('source_account_id')).isdigit())
                                else None
                            )
                        )
                        if history_dest:
                            dest_account_id = history_dest['account_id']
                            dest_match_source = (
                                'history:' + ','.join(history_dest.get('reasons', []))
                            )

                    if dest_account_id:
                        dest_account = account_by_id.get(str(dest_account_id))
                        if dest_account:
                            bill['destination_account_id'] = dest_account_id
                            self.logger.debug(
                                "[投资目标账户] 匹配成功: 别名='%s' -> 账户=%s",
                                dest_match_source,
                                dest_account.get('name')
                            )
                    else:
                        self.logger.debug(
                            "[投资目标账户] 匹配失败: counterparty='%s', parser_id='%s'",
                            counterparty[:30] if counterparty else '',
                            parser_id[:20] if parser_id else ''
                        )

            # v6.62: 转账类型账单的目标账户匹配
            # 使用转账配对时记录的 _destination_parser_id 和 _destination_payment_method
            is_transfer = bill_type in ['转账', 'transfer', '4']

            if is_transfer:
                current_dest_id = bill.get('destination_account_id')
                # 检查目标账户是否已有效
                is_valid_dest = isinstance(current_dest_id, int) or (
                    isinstance(current_dest_id, str) and current_dest_id.isdigit()
                )

                if not is_valid_dest or str(current_dest_id) not in account_by_id:
                    # 使用转账配对时记录的转入账单信息来匹配目标账户
                    dest_parser_id = str(bill.get('_destination_parser_id', ''))
                    dest_payment_method = str(bill.get('_destination_payment_method', ''))
                    dest_counterparty = str(bill.get('_destination_counterparty', ''))
                    combined_text = f"{dest_parser_id} {dest_payment_method} {dest_counterparty}"
                    combined_text_lower = combined_text.lower()

                    dest_account_id = None
                    dest_match_source = None

                    for alias in sorted_aliases:
                        if alias.lower() in combined_text_lower:
                            potential_id = alias_mapping[alias]
                            # 确保目标账户不同于源账户
                            if str(potential_id) != str(bill.get('source_account_id')):
                                dest_account_id = potential_id
                                dest_match_source = alias
                                break

                    if not dest_account_id:
                        history_dest = await self.db.get_historical_destination_account_suggestion(
                            user_id=user_id,
                            payment_method=dest_payment_method,
                            counterparty=dest_counterparty,
                            description='',
                            bill_type=str(bill.get('type', '')),
                            source_account_id=(
                                int(bill.get('source_account_id'))
                                if isinstance(bill.get('source_account_id'), int) or
                                (isinstance(bill.get('source_account_id'), str) and
                                 str(bill.get('source_account_id')).isdigit())
                                else None
                            )
                        )
                        if history_dest:
                            dest_account_id = history_dest['account_id']
                            dest_match_source = (
                                'history:' + ','.join(history_dest.get('reasons', []))
                            )

                    if dest_account_id:
                        dest_account = account_by_id.get(str(dest_account_id))
                        if dest_account:
                            bill['destination_account_id'] = dest_account_id
                            self.logger.debug(
                                "[转账目标账户] 匹配成功: parser_id='%s', "
                                "payment_method='%s', 匹配别名='%s' -> 账户=%s (ID=%s)",
                                dest_parser_id,
                                dest_payment_method[:20] if dest_payment_method else '',
                                dest_match_source,
                                dest_account.get('name'), dest_account_id
                            )
                    else:
                        self.logger.debug(
                            "[转账目标账户] 匹配失败: parser_id='%s', payment_method='%s'",
                            dest_parser_id,
                            dest_payment_method[:20] if dest_payment_method else ''
                        )

        self.logger.info(
            "账户匹配完成: 源账户匹配 %d/%d 条",
            matched_count, len(bills)
        )

        return bills

    async def _detect_investment_candidates(
        self,
        bills: List[Dict[str, Any]],
        user_id: int = 1
    ) -> List[Dict[str, Any]]:
        """基于平台/产品关键词识别投资账单。

        这是 M2 的轻量增强切片：
        - 不依赖用户预先配置投资关键词规则
        - 优先识别常见投资平台、基金/理财产品与投资动作词
        - 命中后仅提升账单类型为“投资”，再交由现有双账户匹配链路处理资金流向
        """
        _ = user_id
        if not bills:
            return bills

        keyword_config = await self._get_investment_keyword_config(user_id)
        detected_count = 0

        for bill in bills:
            candidate = self._score_investment_candidate(
                bill,
                keyword_config=keyword_config
            )
            if not candidate:
                continue

            bill['type'] = '投资'
            bill['_investment_hint'] = candidate.get('hint_text', '')
            bill['_investment_candidate_score'] = candidate.get('score', 0.0)
            bill['_investment_candidate_reason'] = candidate.get('reason', '')
            bill['_investment_platform'] = candidate.get('platform', '')
            bill['_investment_product'] = candidate.get('product', '')
            detected_count += 1

            self.logger.debug(
                "[投资候选识别] 命中: date=%s, score=%.2f, reason=%s, hint=%s",
                str(bill.get('date', ''))[:19],
                candidate.get('score', 0.0),
                candidate.get('reason', ''),
                candidate.get('hint_text', '')
            )

        self.logger.info(
            "[投资候选识别] 完成: 命中 %d/%d 条",
            detected_count, len(bills)
        )
        return bills

    async def _get_investment_keyword_config(
        self,
        user_id: int = 1
    ) -> Dict[str, List[str]]:
        """获取用户有效的投资识别关键词配置。"""
        user = await self.db.get_user_by_id(user_id)
        return build_user_investment_keyword_settings(user)

    def _score_investment_candidate(
        self,
        bill: Dict[str, Any],
        allow_existing_investment: bool = False,
        keyword_config: Optional[Dict[str, List[str]]] = None
    ) -> Optional[Dict[str, Any]]:
        """为单条账单计算投资候选分数。"""
        current_type = str(bill.get('type', '') or '').strip().lower()
        if current_type in ['转账', 'transfer', '4']:
            return None
        if not allow_existing_investment and current_type in ['投资', 'investment', '5']:
            return None

        text_parts = [
            str(bill.get('counterparty', '') or '').strip(),
            str(bill.get('payment_method', '') or '').strip(),
            str(bill.get('description', '') or '').strip(),
            str(bill.get('main_category', '') or '').strip(),
            str(bill.get('sub_category', '') or '').strip(),
            str(bill.get('original_category', '') or '').strip(),
        ]
        text_blob = ' '.join(part for part in text_parts if part)
        if not text_blob:
            return None

        text_lower = text_blob.lower()
        effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
        investment_profile = self._extract_investment_profile(
            text_blob,
            keyword_config=effective_keyword_config
        )
        normalized_platform = investment_profile.get('platform', '')
        normalized_product = investment_profile.get('product', '')

        platform_keywords = effective_keyword_config['platform_keywords']
        product_keywords = effective_keyword_config['product_keywords']
        exclude_keywords = effective_keyword_config['exclude_keywords']

        matched_platforms = []
        if normalized_platform:
            matched_platforms.append(normalized_platform)
        matched_platforms.extend([
            kw for kw in platform_keywords
            if kw not in matched_platforms and kw.lower() in text_lower
        ])
        matched_products = [kw for kw in product_keywords if kw.lower() in text_lower]
        if normalized_product and normalized_product not in matched_products:
            matched_products.insert(0, normalized_product)
        matched_excludes = [kw for kw in exclude_keywords if kw.lower() in text_lower]

        score = 0.0
        if matched_platforms:
            score += min(0.65, 0.38 * len(matched_platforms[:2]))
        if matched_products:
            score += min(0.42, 0.18 * len(matched_products[:3]))
        if matched_excludes:
            score -= min(0.48, 0.28 * len(matched_excludes[:2]))

        # 若没有平台词，则至少需要两个投资相关产品/动作词，降低误判。
        if not matched_platforms and len(matched_products) < 2:
            return None

        if score < 0.55:
            return None

        reason_parts: List[str] = []
        if matched_platforms:
            reason_parts.append('platform:' + '/'.join(matched_platforms[:2]))
        if matched_products:
            reason_parts.append('product:' + '/'.join(matched_products[:3]))
        if matched_excludes:
            reason_parts.append('exclude:' + '/'.join(matched_excludes[:2]))

        hint_tokens = matched_platforms[:1] + matched_products[:2]
        hint_text = ' '.join(hint_tokens)

        return {
            'score': round(min(score, 1.0), 2),
            'reason': ', '.join(reason_parts),
            'hint_text': hint_text,
            'platform': normalized_platform,
            'product': normalized_product
        }

    def _extract_investment_profile(
        self,
        text: str,
        keyword_config: Optional[Dict[str, List[str]]] = None
    ) -> Dict[str, str]:
        """提取投资平台与产品归一信息。"""
        raw_text = str(text or '').strip()
        if not raw_text:
            return {'platform': '', 'product': ''}

        text_lower = raw_text.lower()
        effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)

        generic_platforms = {'基金销售平台', '证券账户'}

        platform = ''
        best_platform_score = (-1, -1)
        for canonical, aliases in DEFAULT_INVESTMENT_PLATFORM_ALIASES:
            alias_candidates = [canonical] + aliases
            matched_aliases = [alias for alias in alias_candidates if alias.lower() in text_lower]
            if not matched_aliases:
                continue

            best_alias = max(matched_aliases, key=len)
            platform_score = (0 if canonical in generic_platforms else 1, len(best_alias))
            if platform_score > best_platform_score:
                best_platform_score = platform_score
                platform = canonical

        if not platform:
            for keyword in sorted(
                effective_keyword_config['platform_keywords'],
                key=len,
                reverse=True
            ):
                if keyword.lower() in text_lower:
                    platform = keyword
                    break

        product = ''
        named_product = ''

        matched_config_products = [
            keyword for keyword in sorted(
                effective_keyword_config['product_keywords'],
                key=len,
                reverse=True
            )
            if keyword.lower() in text_lower
        ]
        if matched_config_products:
            product = matched_config_products[0]

        for pattern in DEFAULT_INVESTMENT_NAMED_PRODUCT_PATTERNS:
            match = re.search(pattern, raw_text, re.IGNORECASE)
            if match:
                candidate = self._clean_investment_product_name(
                    match.group(1).strip(),
                    platform=platform
                )
                if candidate:
                    named_product = candidate
                    break

        if named_product and (not product or len(named_product) > len(product)):
            product = named_product

        if not product:
            best_product_score = -1
            for canonical, aliases in DEFAULT_INVESTMENT_PRODUCT_PATTERNS:
                alias_candidates = [canonical] + aliases
                matched_aliases = [alias for alias in alias_candidates if alias.lower() in text_lower]
                if not matched_aliases:
                    continue

                best_alias = max(matched_aliases, key=len)
                if len(best_alias) > best_product_score:
                    best_product_score = len(best_alias)
                    product = best_alias

        if product:
            product = self._clean_investment_product_name(product, platform=platform)

        return {
            'platform': platform,
            'product': product
        }

    def _clean_investment_product_name(self, product: str, platform: str = '') -> str:
        """清理提取出的投资产品名，移除平台前缀与交易动作后缀。"""
        cleaned = str(product or '').strip().strip('|｜,， ')
        if not cleaned:
            return ''

        cleaned = re.sub(r'^(买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出)[-－:：\s]*', '', cleaned, flags=re.IGNORECASE)
        cleaned = re.sub(r'[-－:：\s]*(买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出|确认份额|分红再投资)$', '', cleaned, flags=re.IGNORECASE)

        for canonical, aliases in DEFAULT_INVESTMENT_PLATFORM_ALIASES:
            alias_candidates = [canonical] + aliases
            for alias in alias_candidates:
                cleaned = re.sub(
                    rf'^{re.escape(alias)}[-－:：\s]*',
                    '',
                    cleaned,
                    flags=re.IGNORECASE
                )

        if platform:
            cleaned = re.sub(
                rf'^{re.escape(platform)}[-－:：\s]*',
                '',
                cleaned,
                flags=re.IGNORECASE
            )

        if '|' in cleaned or '｜' in cleaned:
            candidates = [segment.strip().strip('|｜,， ') for segment in re.split(r'[|｜]', cleaned)]
            preferred = [
                segment for segment in candidates
                if re.search(r'(基金|ETF|LOF|REITs|REIT|理财|计划|组合|债券|股票|黄金|A类|C类|联接)', segment, re.IGNORECASE)
            ]
            if preferred:
                cleaned = max(preferred, key=len)

        cleaned = re.sub(r'\s+', ' ', cleaned).strip().strip('-－:：|｜,， ')
        return cleaned[:80]

    @log_method
    async def _detect_cash_transfers(
        self,
        bills: List[Dict[str, Any]],
        user_id: int = 1
    ) -> List[Dict[str, Any]]:
        """
        v6.77: 检测存取转账（用户指定的分类自动转换为转账类型）

        当账单的分类匹配用户设置的"存取分类"时，将其转换为转账类型：
        - 原收入账单：来源账户=现金账户，目标账户=原匹配账户
        - 原支出账单：来源账户=原匹配账户，目标账户=现金账户

        Args:
            bills: 账单列表
            user_id: 用户ID

        Returns:
            List[Dict]: 处理后的账单列表
        """
        # 1. 获取用户设置
        user = await self.db.get_user_by_id(user_id)
        if not user:
            self.logger.warning("[存取转账检测] 用户不存在: user_id=%d", user_id)
            return bills

        cash_account_id = user.get('cash_account_id')
        cash_transfer_category_id = user.get('cash_transfer_category_id')

        # 2. 如果没有配置，直接返回
        if not cash_account_id or not cash_transfer_category_id:
            self.logger.debug(
                "[存取转账检测] 用户未配置存取设置: cash_account_id=%s, "
                "cash_transfer_category_id=%s",
                cash_account_id, cash_transfer_category_id
            )
            return bills

        # 3. 获取目标分类信息
        category = await self.db.get_category_by_id(cash_transfer_category_id, user_id)
        if not category:
            self.logger.warning(
                "[存取转账检测] 分类不存在: category_id=%d",
                cash_transfer_category_id
            )
            return bills

        target_main_category = category.get('main_category', '')
        target_sub_category = category.get('sub_category', '')

        self.logger.info(
            "[存取转账检测] 开始检测, 目标分类='%s/%s', 现金账户ID=%d",
            target_main_category, target_sub_category, cash_account_id
        )

        # 4. 遍历账单检测并转换
        converted_count = 0
        for bill in bills:
            bill_main_category = bill.get('main_category', '')
            bill_sub_category = bill.get('sub_category', '')

            # 检查分类是否匹配
            if (bill_main_category == target_main_category and
                    bill_sub_category == target_sub_category):
                # 获取原类型和金额
                original_type = str(bill.get('type', '')).lower()
                amount = float(bill.get('amount', 0))
                matched_account_id = bill.get('source_account_id')

                # 确定账户方向
                # 收入（amount > 0 或 type 包含收入）：现金 -> 匹配账户
                # 支出（amount < 0 或 type 包含支出）：匹配账户 -> 现金
                is_income = (
                    amount > 0 or
                    original_type in ['收入', 'income', '2']
                )

                if is_income:
                    # 收入：从现金账户转入匹配账户
                    bill['source_account_id'] = cash_account_id
                    bill['destination_account_id'] = matched_account_id
                    direction = "现金->账户"
                else:
                    # 支出：从匹配账户转出到现金账户
                    # source_account_id 保持不变（已是匹配账户）
                    bill['destination_account_id'] = cash_account_id
                    direction = "账户->现金"

                # 设置类型为转账
                bill['type'] = '转账'

                # 设置 destination_amount（与 amount 绝对值相等）
                bill['destination_amount'] = abs(amount)

                converted_count += 1
                self.logger.debug(
                    "[存取转账检测] 转换: date=%s, amount=%.2f, %s, "
                    "source=%s, dest=%s",
                    bill.get('date', '')[:10], amount, direction,
                    bill.get('source_account_id'),
                    bill.get('destination_account_id')
                )

        if converted_count > 0:
            self.logger.info(
                "[存取转账检测] 完成, 转换 %d 条账单为转账类型",
                converted_count
            )

        return bills

    @staticmethod
    def _normalize_learning_text(raw_value: Any) -> str:
        """标准化长期学习匹配文本。"""
        if raw_value is None:
            return ''

        text = str(raw_value).strip().lower()
        if not text:
            return ''

        parts = [part.strip() for part in text.split('|') if part.strip()]
        if parts:
            text = ' | '.join(parts)

        return ' '.join(text.split())

    async def _apply_import_learning_rules(
        self,
        bills: List[Dict[str, Any]],
        user_id: int = 1,
        type_only: bool = False,
        record_usage: bool = False
    ) -> int:
        """应用长期导入学习规则。"""
        if not bills:
            return 0

        user = await self.db.get_user_by_id(user_id)
        if not user or not bool(user.get('import_learning_enabled', 1)):
            self.logger.debug("[长期学习] 已关闭或用户不存在: user_id=%d", user_id)
            return 0

        rules = await self.db.get_import_learning_rules(
            user_id=user_id,
            enabled_only=True,
            limit=1000
        )
        if not rules:
            return 0

        rule_lookup = {
            (rule.get('match_type', ''), rule.get('normalized_match_value', '')): rule
            for rule in rules
            if rule.get('match_type') and rule.get('normalized_match_value')
        }
        category_cache: Dict[int, Optional[Dict[str, Any]]] = {}
        matched_rule_ids: List[int] = []
        applied_count = 0

        for bill in bills:
            matched_rule = None
            for match_type, raw_value in [
                ('counterparty', bill.get('counterparty', '')),
                ('description', bill.get('description', '')),
                ('payment_method', bill.get('payment_method', '')),
            ]:
                normalized_value = self._normalize_learning_text(raw_value)
                if not normalized_value:
                    continue

                matched_rule = rule_lookup.get((match_type, normalized_value))
                if matched_rule:
                    bill['_import_learning_match_type'] = match_type
                    bill['_import_learning_rule_id'] = matched_rule.get('id')
                    break

            if not matched_rule:
                continue

            learned_type = matched_rule.get('learned_type')
            if learned_type:
                bill['type'] = learned_type

            if not type_only:
                learned_category_id = matched_rule.get('learned_category_id')
                if learned_category_id:
                    cat_id = int(learned_category_id)
                    if cat_id not in category_cache:
                        category_cache[cat_id] = await self.db.get_category_by_id(cat_id, user_id)
                    category = category_cache.get(cat_id)
                    if category:
                        bill['main_category'] = category.get('main_category', '')
                        bill['sub_category'] = category.get('sub_category', '')

                if matched_rule.get('learned_source_account_id'):
                    bill['source_account_id'] = matched_rule.get('learned_source_account_id')
                if matched_rule.get('learned_destination_account_id'):
                    bill['destination_account_id'] = matched_rule.get('learned_destination_account_id')

            applied_count += 1
            rule_id = matched_rule.get('id')
            if record_usage and rule_id:
                matched_rule_ids.append(int(rule_id))

        if record_usage and matched_rule_ids:
            await self.db.increment_import_learning_rule_usage(matched_rule_ids, user_id=user_id)

        if applied_count > 0:
            self.logger.info(
                "[长期学习] 应用完成: user_id=%d, bills=%d, applied=%d, type_only=%s",
                user_id, len(bills), applied_count, type_only
            )

        return applied_count

    async def _build_session_annotation_rule_lookup(
        self,
        previews: List[Dict[str, Any]],
        annotation_samples: List[Dict[str, Any]],
        user_id: int = 1
    ) -> Dict[Tuple[str, str], Dict[str, Any]]:
        """基于当前会话人工标注构建临时学习规则查找表。"""
        if not previews or not annotation_samples:
            return {}

        preview_map = {
            int(preview['id']): preview for preview in previews if preview.get('id')
        }
        rule_lookup: Dict[Tuple[str, str], Dict[str, Any]] = {}
        category_cache: Dict[int, Optional[Dict[str, Any]]] = {}

        for sample in annotation_samples:
            preview_id = int(sample.get('preview_id', 0) or 0)
            source_preview = preview_map.get(preview_id)
            if not source_preview:
                continue

            learned_category_id = sample.get('annotated_category_id')
            learned_main_category = ''
            learned_sub_category = ''
            if learned_category_id:
                category_id = int(learned_category_id)
                if category_id not in category_cache:
                    category_cache[category_id] = await self.db.get_category_by_id(
                        category_id, user_id=user_id
                    )
                category = category_cache.get(category_id) or {}
                learned_main_category = category.get('main_category', '')
                learned_sub_category = category.get('sub_category', '')

            rule_payload = {
                'preview_id': preview_id,
                'annotated_type': sample.get('annotated_type', ''),
                'annotated_category_id': learned_category_id,
                'annotated_main_category': learned_main_category,
                'annotated_sub_category': learned_sub_category,
                'annotated_source_account_id': sample.get('annotated_source_account_id'),
                'annotated_destination_account_id': sample.get('annotated_destination_account_id'),
            }

            for match_type, raw_value in [
                ('counterparty', source_preview.get('preview_counterparty', '')),
                ('description', source_preview.get('preview_description', '')),
                ('payment_method', source_preview.get('preview_payment_method', '')),
            ]:
                normalized_value = self._normalize_learning_text(raw_value)
                if not normalized_value:
                    continue
                rule_lookup[(match_type, normalized_value)] = rule_payload

        if rule_lookup:
            self.logger.info(
                "[会话学习] 构建临时规则 %d 条", len(rule_lookup)
            )

        return rule_lookup

    @staticmethod
    def _apply_session_annotation_learning_rules(
        bills: List[Dict[str, Any]],
        rule_lookup: Dict[Tuple[str, str], Dict[str, Any]],
        annotation_map: Dict[int, Dict[str, Any]],
        type_only: bool = False
    ) -> int:
        """将当前会话人工标注以临时学习规则形式回放到相似账单。"""
        if not bills or not rule_lookup:
            return 0

        applied_count = 0

        for bill in bills:
            bill_id = int(bill.get('id', 0) or 0)
            if bill_id and bill_id in annotation_map:
                continue

            matched_rule = None
            for match_type, raw_value in [
                ('counterparty', bill.get('counterparty', '')),
                ('description', bill.get('description', '')),
                ('payment_method', bill.get('payment_method', '')),
            ]:
                normalized_value = BillService._normalize_learning_text(raw_value)
                if not normalized_value:
                    continue

                matched_rule = rule_lookup.get((match_type, normalized_value))
                if matched_rule:
                    bill['_session_annotation_match_type'] = match_type
                    bill['_session_annotation_source_preview_id'] = matched_rule.get('preview_id')
                    break

            if not matched_rule:
                continue

            if matched_rule.get('annotated_type'):
                bill['type'] = matched_rule.get('annotated_type')

            if not type_only:
                if matched_rule.get('annotated_main_category'):
                    bill['main_category'] = matched_rule.get('annotated_main_category', '')
                    bill['sub_category'] = matched_rule.get('annotated_sub_category', '')

                if matched_rule.get('annotated_source_account_id'):
                    bill['source_account_id'] = matched_rule.get('annotated_source_account_id')

                if matched_rule.get('annotated_destination_account_id'):
                    bill['destination_account_id'] = matched_rule.get('annotated_destination_account_id')

            applied_count += 1

        return applied_count

    @log_method
    async def promote_session_annotations_to_learning(
        self,
        session_id: str,
        preview_updates: Optional[List[Dict[str, Any]]] = None,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """将当前会话人工标注提升为长期学习规则。"""
        preview_ids: List[int] = []
        if preview_updates:
            await self.db.save_import_annotation_samples(session_id, preview_updates, user_id=user_id)
            preview_ids = [
                int(item['id']) for item in preview_updates
                if item.get('id')
            ]

        promote_result = await self.db.promote_import_annotation_samples_to_learning(
            session_id,
            preview_ids=preview_ids or None,
            user_id=user_id
        )
        return {
            'success': True,
            'session_id': session_id,
            **promote_result,
        }

    @log_method
    async def import_multiple_files(
        self,
        file_paths: List[str],
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        批量导入多个文件

        Args:
            file_paths: 文件路径列表
            user_id: 用户ID

        Returns:
            Dict: 汇总结果
        """
        summary = {
            'total_files': len(file_paths),
            'success_files': 0,
            'failed_files': 0,
            'total_bills': 0,
            'inserted_bills': 0,
            'uncategorized_bills': [],
            'results': []
        }

        self.logger.info("开始批量导入 %d 个文件", len(file_paths))

        for file_path in file_paths:
            result = await self.import_bills(file_path, user_id=user_id)
            summary['results'].append(result)

            if result['success']:
                summary['success_files'] += 1
                summary['total_bills'] += result['total']
                summary['inserted_bills'] += result['inserted']
                summary['uncategorized_bills'].extend(result.get('uncategorized', []))
            else:
                summary['failed_files'] += 1

        self.logger.info(
            "批量导入完成: 成功 %d 个文件，失败 %d 个文件，共插入 %d 条账单，"
            "未分类 %d 条",
            summary['success_files'],
            summary['failed_files'],
            summary['inserted_bills'],
            len(summary['uncategorized_bills'])
        )

        return summary

    @log_method
    async def import_preview_confirmed(
        self,
        preview_bills: List[Dict[str, Any]],
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        确认导入预览的账单

        用户在预览后确认，可能修改了分类，然后调用此方法实际导入

        Args:
            preview_bills: 经用户确认（可能修改）的预览账单列表
            user_id: 用户ID

        Returns:
            Dict: 导入结果
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'total': len(preview_bills),
            'inserted': 0,
            'duplicates': 0,
            'errors': []
        }

        try:
            # 写入数据库
            batch_id = datetime.now().strftime('%Y%m%d%H%M%S')
            inserted = await self.db.insert_bills(preview_bills, batch_id, user_id=user_id)

            result['inserted'] = inserted
            result['duplicates'] = len(preview_bills) - inserted
            result['success'] = True

            self.logger.info(
                "预览确认导入完成: 总计 %d 条, 插入 %d 条, 重复 %d 条",
                result['total'],
                result['inserted'],
                result['duplicates']
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("预览确认导入失败: %s", e, exc_info=True)
            result['errors'].append(str(e))

        return result

    @log_method
    async def batch_update_category(
        self,
        bill_ids: List[int],
        main_category: str,
        sub_category: Optional[str] = None,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        批量更新账单分类

        Args:
            bill_ids: 账单ID列表
            main_category: 主分类
            sub_category: 子分类（可选）
            user_id: 用户ID

        Returns:
            Dict: 更新结果
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'total': len(bill_ids),
            'updated': 0,
            'errors': []
        }

        updated = 0
        for bill_id in bill_ids:
            try:
                success = await self.db.update_bill(
                    bill_id,
                    {
                        'main_category': main_category,
                        'sub_category': sub_category
                    },
                    user_id=user_id
                )
                if success:
                    updated += 1
            except Exception as e:  # pylint: disable=broad-except
                result['errors'].append(f"Bill {bill_id}: {str(e)}")

        result['updated'] = updated
        result['success'] = True

        self.logger.info(
            "批量分类更新完成: 总计 %d 条, 更新 %d 条",
            result['total'],
            result['updated']
        )

        return result

    @log_method
    async def add_category_keyword(
        self,
        main_category: str,
        sub_category: Optional[str],
        keyword: str,
        user_id: int = 1
    ) -> bool:
        """
        为分类添加关键词

        Args:
            main_category: 主分类
            sub_category: 子分类
            keyword: 要添加的关键词
            user_id: 用户ID

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        # 获取现有分类规则
        category = await self.db.get_category_by_name(
            main_category,
            sub_category or '',
            user_id=user_id
        )

        if not category:
            self.logger.warning("分类不存在: %s/%s", main_category, sub_category)
            return False

        # 添加关键词
        current_keywords = category.get('keywords', '') or ''
        keyword_list = [k.strip() for k in current_keywords.split(',') if k.strip()]

        if keyword not in keyword_list:
            keyword_list.append(keyword)
            new_keywords = ','.join(keyword_list)

            success = await self.db.update_category(
                category['id'],
                {'keywords': new_keywords},
                user_id=user_id
            )

            if success:
                # 重新加载分类规则（使用当前用户ID）
                await self.category_engine.load_rules_from_db(self.db, user_id=user_id)
                self.logger.info(
                    "添加关键词成功: %s/%s <- '%s' (user_id=%d)",
                    main_category, sub_category, keyword, user_id
                )
                return True

        return False

    @log_method
    async def refresh_category_for_bills(
        self,
        bill_ids: Optional[List[int]] = None,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        刷新账单分类（使用最新的分类规则重新匹配）

        Args:
            bill_ids: 要刷新的账单ID列表，如果为None则刷新所有未分类账单
            user_id: 用户ID

        Returns:
            Dict: 刷新结果
        """
        if not self._initialized:
            await self.initialize()

        # 重新加载分类规则（使用当前用户ID）
        self.logger.info("刷新分类规则 (user_id=%d)", user_id)
        await self.category_engine.load_rules_from_db(self.db, user_id=user_id)

        result = {
            'success': False,
            'total': 0,
            'categorized': 0,
            'still_uncategorized': 0
        }

        # 获取需要分类的账单
        if bill_ids:
            bills = []
            for bid in bill_ids:
                bill = await self.db.get_bill_by_id(bid, user_id=user_id)
                if bill:
                    bills.append(bill)
        else:
            # 获取所有未分类账单
            bills = await self.db.get_bills(
                filters={'main_category': None},
                user_id=user_id
            )

        result['total'] = len(bills)

        # 重新分类
        for bill in bills:
            main_cat, sub_cat = self.category_engine.match_category(bill)

            if main_cat:
                await self.db.update_bill(
                    bill['id'],
                    {'main_category': main_cat, 'sub_category': sub_cat},
                    user_id=user_id
                )
                result['categorized'] += 1
            else:
                result['still_uncategorized'] += 1

        result['success'] = True

        self.logger.info(
            "分类刷新完成: 总计 %d 条, 成功分类 %d 条, 仍未分类 %d 条",
            result['total'],
            result['categorized'],
            result['still_uncategorized']
        )

        return result

    @log_method
    @log_step("清理无效账单")
    async def clean_invalid(self) -> int:
        """
        清理无效的账单数据

        Returns:
            int: 清理的数量
        """
        if not self._initialized:
            await self.initialize()

        # 获取所有账单
        bills = await self.db.get_bills()

        invalid_ids = []
        for bill in bills:
            is_valid, _ = self.validator.validate_bill(bill)
            if not is_valid:
                invalid_ids.append(bill['id'])

        # 删除无效账单
        deleted = 0
        for bill_id in invalid_ids:
            if await self.db.delete_bill(bill_id):
                deleted += 1

        self.logger.info("清理完成: 删除了 %d 条无效账单", deleted)
        return deleted

    @log_method
    @log_step("更新账单分类")
    async def update_categories(self, force: bool = False) -> Dict[str, int]:
        """
        重新分类所有账单

        Args:
            force: 是否强制重新分类（包括已有分类的账单）

        Returns:
            Dict: {'total': 总数, 'updated': 更新数}
        """
        if not self._initialized:
            await self.initialize()

        # 获取需要分类的账单
        if force:
            bills = await self.db.get_bills()
        else:
            bills = await self.db.get_bills({'main_category': None})

        self.logger.info("开始重新分类 %d 条账单", len(bills))

        updated = 0
        for bill in bills:
            main_cat, sub_cat = self.category_engine.match_category(bill)

            if main_cat:
                success = await self.db.update_bill(
                    bill['id'],
                    {
                        'main_category': main_cat,
                        'sub_category': sub_cat
                    }
                )

                if success:
                    updated += 1

        self.logger.info("分类更新完成: 总计 %d 条，更新 %d 条", len(bills), updated)

        return {
            'total': len(bills),
            'updated': updated
        }

    @log_method
    async def get_bills(self, filters: Optional[Dict[str, Any]] = None,
                       limit: Optional[int] = None,
                       offset: int = 0) -> List[Dict[str, Any]]:
        """
        查询账单

        Args:
            filters: 过滤条件
            limit: 限制数量
            offset: 偏移量

        Returns:
            List[Dict]: 账单列表
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_bills(filters, limit, offset)

    @log_method
    async def delete_bill(self, bill_id: int) -> bool:
        """
        删除账单

        Args:
            bill_id: 账单ID

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.delete_bill(bill_id)

    @log_method
    async def update_bill(self, bill_id: int, updates: Dict[str, Any]) -> bool:
        """
        更新账单

        Args:
            bill_id: 账单ID
            updates: 更新内容

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.update_bill(bill_id, updates)

    @log_method
    async def deduplicate(self) -> int:
        """
        去除重复账单

        Returns:
            int: 删除的数量
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.deduplicate()

    @log_method
    async def get_statistics(self) -> Dict[str, Any]:
        """
        获取账单统计信息

        Returns:
            Dict: 统计信息
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_statistics()

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

    # ==================== v6.47: 三阶段账单导入系统 ====================

    @log_method
    @log_step("阶段1: 多文件并行解析")
    async def import_stage1_parse(
        self,
        file_paths: List[str],
        session_id: str,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        阶段1：多文件并行解析

        使用多线程并行解析多个账单文件，将解析结果写入 bills_parser_template 表。
        每个文件使用独立线程通过 run_in_executor 执行解析。

        Args:
            file_paths: 账单文件路径列表
            session_id: 导入会话ID（UUID格式）
            user_id: 用户ID

        Returns:
            Dict: 解析结果统计
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'session_id': session_id,
            'file_count': len(file_paths),
            'total_parsed': 0,
            'failed_files': [],
            'file_results': [],
            'errors': []
        }

        self.logger.info("[阶段1] 开始解析 %d 个文件, session_id=%s",
                         len(file_paths), session_id)

        # 创建导入会话
        await self.db.create_import_session(session_id, user_id, len(file_paths))

        # 定义单文件解析函数（在线程池中执行）
        def parse_single_file(file_path: str) -> Dict[str, Any]:
            """解析单个文件（同步，用于线程池）"""
            file_result = {
                'file': file_path,
                'success': False,
                'parser_type': None,
                'count': 0,
                'bills': [],
                'error': None
            }

            try:
                # 检测解析器类型
                parser_info = self.parser_factory.detect_parser(file_path)
                if not parser_info:
                    file_result['error'] = f"无法识别文件格式: {file_path}"
                    return file_result

                parser_type = parser_info['id']
                file_result['parser_type'] = parser_type

                # 解析文件
                bills = self.parser_factory.parse(file_path, parser_type)
                file_result['bills'] = bills
                file_result['count'] = len(bills)
                file_result['success'] = True

                self.logger.info("[阶段1] 文件解析完成: %s, parser=%s, count=%d",
                                 file_path, parser_type, len(bills))

            except Exception as e:
                file_result['error'] = str(e)
                self.logger.error("[阶段1] 文件解析失败: %s, error=%s",
                                  file_path, e, exc_info=True)

            return file_result

        # 使用 asyncio.gather + run_in_executor 并行解析
        loop = asyncio.get_event_loop()
        parse_tasks = [
            loop.run_in_executor(None, parse_single_file, file_path)
            for file_path in file_paths
        ]

        # 等待所有解析任务完成
        file_results = await asyncio.gather(*parse_tasks, return_exceptions=True)

        # 处理解析结果
        for i, file_result in enumerate(file_results):
            if isinstance(file_result, Exception):
                result['failed_files'].append(file_paths[i])
                result['errors'].append(f"{file_paths[i]}: {str(file_result)}")
                continue

            result['file_results'].append({
                'file': file_result['file'],
                'success': file_result['success'],
                'parser_type': file_result['parser_type'],
                'count': file_result['count'],
                'error': file_result['error']
            })

            if file_result['success'] and file_result['bills']:
                # 验证账单
                valid_bills, invalid_bills = self.validator.validate_bills(
                    file_result['bills']
                )

                if valid_bills:
                    # 写入 bills_parser_template 表
                    inserted = await self.db.insert_parser_templates(
                        session_id,
                        valid_bills,
                        file_result['parser_type'],
                        user_id
                    )
                    result['total_parsed'] += inserted
                    self.logger.info("[阶段1] 写入解析模板: %d 条", inserted)

                if invalid_bills:
                    self.logger.warning("[阶段1] 无效账单: %d 条", len(invalid_bills))
            else:
                result['failed_files'].append(file_result['file'])
                if file_result['error']:
                    result['errors'].append(f"{file_result['file']}: {file_result['error']}")

        # 更新会话状态
        await self.db.update_import_session_status(
            session_id,
            'deduping' if result['total_parsed'] > 0 else 'failed',
            total_parsed=result['total_parsed']
        )

        result['success'] = result['total_parsed'] > 0
        self.logger.info("[阶段1完成] session=%s, 解析成功=%d条, 失败文件=%d个",
                         session_id, result['total_parsed'], len(result['failed_files']))

        return result

    @log_method
    @log_step("阶段2: 去重匹配预览")
    async def import_stage2_dedup(
        self,
        session_id: str,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        阶段2：去重、账户匹配、分类匹配，生成预览数据

        从 bills_parser_template 读取解析数据，执行以下处理：
        1. 四种去重机制（transfer, platform_bank, similar, split_merge）
        2. 数据库重复检测
        3. 账户别名匹配
        4. 分类关键词匹配
        5. 写入 bills_preview 表

        Args:
            session_id: 导入会话ID
            user_id: 用户ID

        Returns:
            Dict: 去重和匹配结果统计
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'session_id': session_id,
            'template_count': 0,
            'preview_count': 0,
            'dedup_stats': {},
            'match_stats': {},
            'errors': []
        }

        self.logger.info("[阶段2] 开始去重和匹配, session_id=%s", session_id)

        try:
            # 1. 获取未处理的解析模板
            templates = await self.db.get_unprocessed_templates_for_dedup(session_id)
            result['template_count'] = len(templates)

            if not templates:
                result['errors'].append("没有待处理的解析数据")
                await self.db.update_import_session_status(session_id, 'failed')
                return result

            self.logger.info("[阶段2] 获取解析模板: %d 条", len(templates))

            # 2. 转换为标准账单格式（用于去重引擎）
            bills = []
            template_id_map = {}  # bill_index -> template_id

            for idx, template in enumerate(templates):
                # v6.74: 获取 payment_method，如果为空则使用 parser_id 作为回退
                raw_payment_method = template.get('parser_payment_method', '')
                parser_id = template.get('parser_id', '')
                if raw_payment_method and str(raw_payment_method).strip():
                    payment_method = str(raw_payment_method).strip()
                    payment_method_source = 'parser_payment_method'
                elif parser_id and str(parser_id).strip():
                    # 使用 parser_id（如 'alipay', 'wechat', 'abc'）作为回退
                    payment_method = str(parser_id).strip()
                    payment_method_source = 'parser_id_fallback'
                else:
                    payment_method = ''
                    payment_method_source = 'empty'

                self.logger.debug(
                    "[模板转账单] idx=%d, payment_method='%s' (来源=%s), parser_id='%s'",
                    idx, payment_method, payment_method_source, parser_id
                )

                bill = {
                    'date': template.get('parser_date', ''),
                    'amount': float(template.get('parser_amount', 0)),
                    'type': template.get('parser_type', ''),
                    'description': template.get('parser_description', ''),
                    'counterparty': template.get('parser_counterparty', ''),
                    'payment_method': payment_method,
                    'original_type': template.get('parser_original_type', ''),
                    'original_category': template.get('parser_original_category', ''),
                    'source_account_id': template.get('parser_account_id'),
                    '_template_id': template.get('id'),  # 保存模板ID用于回溯
                    '_parser_id': parser_id,
                    '_payment_method_source': payment_method_source,  # 记录来源便于调试
                }
                bills.append(bill)
                template_id_map[idx] = template.get('id')

            # 3. 执行智能去重（包含数据库对比）
            if self.use_smart_dedup and self.smart_dedup_engine:
                dedup_result = await self.smart_dedup_engine.process_with_db(
                    bills, self.db, user_id 
                )

                result['dedup_stats'] = {
                    'original_count': dedup_result.original_count,
                    'removed_count': dedup_result.removed_count,
                    'transfer_pairs': len(dedup_result.transfer_pairs),
                    'split_groups': len(dedup_result.split_groups),
                    'duplicate_groups': len(dedup_result.duplicate_groups)
                }

                kept_bills = dedup_result.kept_bills
            else:
                kept_bills = bills
                result['dedup_stats'] = {'original_count': len(bills), 'removed_count': 0}

            # 4. 加载分类规则
            if not self.category_engine.is_initialized or \
               self.category_engine.current_user_id != user_id:
                await self.category_engine.load_rules_from_db(self.db, user_id=user_id)

            learned_seed_count = await self._apply_import_learning_rules(
                kept_bills,
                user_id=user_id,
                type_only=True,
                record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[阶段2] 长期学习类型预填充 %d 条", learned_seed_count)

            # 5. 分类匹配
            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            self.logger.info("[阶段2] 分类匹配: 共 %d 条账单", len(kept_bills))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(
                kept_bills, types=None
            )

            # 5.5 投资账单专门识别链路（平台/产品关键词）
            self.logger.info("[阶段2] 投资候选识别...")
            categorized_bills = await self._detect_investment_candidates(
                categorized_bills, user_id=user_id
            )

            # 6. 账户匹配
            self.logger.info("[阶段2] 执行账户匹配...")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            # 7. v6.77: 存取转账检测（用户指定的分类自动转换为转账类型）
            self.logger.info("[阶段2] 存取转账检测...")
            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learning_replay_count = await self._apply_import_learning_rules(
                matched_bills,
                user_id=user_id,
                type_only=False,
                record_usage=True
            )
            if learning_replay_count > 0:
                self.logger.info("[阶段2] 长期学习结果回放 %d 条", learning_replay_count)

            # 8. 生成预览数据并写入 bills_preview 表
            self.logger.info("[阶段2] 生成预览数据...")
            preview_list = []
            matched_category_count = 0
            matched_account_count = 0
            recurring_templates = await self.db.get_enabled_recurring_templates(user_id=user_id)

            for bill in matched_bills:
                # 统计匹配情况
                has_category = bool(bill.get('main_category'))
                has_account = bill.get('source_account_id') is not None and (
                    isinstance(bill.get('source_account_id'), int) or
                    (isinstance(bill.get('source_account_id'), str) and
                     bill.get('source_account_id', '').isdigit())
                )

                if has_category:
                    matched_category_count += 1
                if has_account:
                    matched_account_count += 1

                # v6.50: 确定去重类型 - 仅使用 _dedup_type 字段
                # 只有通过转账配对功能配对的账单才会有 _dedup_type='transfer'
                # 不再根据 type 字段回退判断，避免 platform_bank 被错误标记为 transfer
                dedup_type = bill.get('_dedup_type', 'remaining')

                # 收集来源模板ID（确保类型安全）
                source_ids: list = []
                template_id = bill.get('_template_id')
                if template_id is not None:
                    source_ids.append(template_id)
                merged_ids = bill.get('_merged_template_ids')
                if merged_ids and isinstance(merged_ids, list):
                    source_ids.extend(merged_ids)

                # v6.50: 投资类型账单特殊处理
                # 投资类型的 destination_amount 应等于 amount（投资金额转入投资账户）
                bill_type = str(bill.get('type', '')).lower()
                is_investment = bill_type in ['投资', 'investment', '5']
                amount = float(bill.get('amount', 0))

                if is_investment:
                    # 投资类型：destination_amount = |amount| (正数)
                    destination_amount = abs(amount)
                else:
                    # 其他类型：使用原有值
                    destination_amount = float(bill.get('destination_amount', 0))

                # v6.74: 获取 preview_payment_method，如果为空则使用 _parser_id 作为回退
                raw_payment_method = bill.get('payment_method', '')
                parser_id = bill.get('_parser_id', '')
                if raw_payment_method and str(raw_payment_method).strip():
                    preview_payment_method = str(raw_payment_method).strip()
                elif parser_id and str(parser_id).strip():
                    # 使用 _parser_id（如 'alipay', 'wechat', 'abc'）作为回退
                    preview_payment_method = str(parser_id).strip()
                    self.logger.debug(
                        "[账单转预览] 使用 parser_id 回退: payment_method 为空, "
                        "使用 parser_id='%s' 作为 preview_payment_method",
                        parser_id
                    )
                else:
                    preview_payment_method = ''

                recurring_candidates = self.db.build_recurring_candidates_for_bill_data(
                    bill,
                    recurring_templates,
                    linked_recurring_id=bill.get('created_from_recurring'),
                    tolerance_days=3
                )
                top_recurring_candidate = recurring_candidates[0] if recurring_candidates else None

                # 构建预览数据
                # v6.54: preview_amount 和 preview_destination_amount 使用绝对值
                preview_data = {
                    'preview_date': bill.get('date', ''),
                    'preview_type': bill.get('type', ''),
                    'preview_amount': abs(amount),
                    'preview_destination_amount': abs(destination_amount),
                    'preview_main_category': bill.get('main_category', ''),
                    'preview_sub_category': bill.get('sub_category', ''),
                    'preview_source_account_id': bill.get('source_account_id'),
                    'preview_destination_account_id': bill.get('destination_account_id'),
                    'preview_counterparty': bill.get('counterparty', ''),
                    'preview_payment_method': preview_payment_method,
                    'preview_description': bill.get('description', ''),
                    'preview_recurring_id': top_recurring_candidate.get('id') if top_recurring_candidate else None,
                    'preview_recurring_name': top_recurring_candidate.get('name', '') if top_recurring_candidate else '',
                    'preview_recurring_candidate_count': len(recurring_candidates),
                    'preview_recurring_match_score': top_recurring_candidate.get('matchScore', 0) if top_recurring_candidate else 0,
                    'preview_recurring_match_reasons': '|'.join(top_recurring_candidate.get('matchReasons', [])) if top_recurring_candidate else '',
                    'preview_recurring_matched_date': top_recurring_candidate.get('matchedOccurrenceDate', '') if top_recurring_candidate else '',
                    'dedup_type': dedup_type,
                    'dedup_source_ids': source_ids
                }
                preview_list.append(preview_data)

            # 批量写入预览表
            preview_count = await self.db.insert_preview_bills_batch(
                session_id, preview_list, user_id
            )

            result['preview_count'] = preview_count
            result['match_stats'] = {
                'category_matched': matched_category_count,
                'account_matched': matched_account_count,
                'total': len(matched_bills)
            }

            # 标记模板为已处理
            template_ids: List[int] = []
            for t in templates:
                t_id = t.get('id')
                if t_id is not None:
                    template_ids.append(int(t_id))
            await self.db.update_parser_template_status(template_ids, processed=True)

            # 更新会话状态
            await self.db.update_import_session_status(
                session_id,
                'previewing',
                total_preview=preview_count
            )

            result['success'] = True
            self.logger.info("[阶段2完成] session=%s, 预览=%d条, 分类匹配=%d, 账户匹配=%d",
                             session_id, preview_count,
                             matched_category_count, matched_account_count)

        except Exception as e:
            self.logger.error("[阶段2] 处理失败: %s", e, exc_info=True)
            result['errors'].append(str(e))
            await self.db.update_import_session_status(session_id, 'failed')

        return result

    @log_method
    @log_step("阶段3: 确认导入")
    async def import_stage3_confirm(
        self,
        session_id: str,
        user_id: int = 1,
        selected_ids: Optional[List[int]] = None  # v6.56: 改为接收选中的ID列表（可选）
    ) -> Dict[str, Any]:
        """
        阶段3：确认导入

        将用户选中的预览账单写入正式账单表 bills。
        注意：预览表的更新（用户编辑、选中状态）由 API 层调用 db.update_preview_bills_batch 完成。
        本方法只负责将选中的预览账单写入正式表。

        Args:
            session_id: 导入会话ID
            user_id: 用户ID
            selected_ids: 可选，用户选中的预览账单ID列表（用于日志记录，实际过滤在 db 层）

        Returns:
            Dict: 确认结果
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'session_id': session_id,
            'confirmed_count': 0,
            'skipped_count': 0,
            'duplicate_count': 0,
            'errors': []
        }

        self.logger.info("[阶段3] 开始确认导入, session_id=%s, selected_ids=%s",
                         session_id, len(selected_ids) if selected_ids else 'all')

        try:
            # v6.56: 预览表更新已在 API 层完成，这里直接确认写入正式表
            # confirm_preview_to_bills 会自动只处理 preview_is_selected=1 的账单
            confirm_result = await self.db.confirm_preview_to_bills(session_id, user_id)

            result['imported_count'] = confirm_result.get('confirmed_count', 0)
            result['duplicate_count'] = confirm_result.get('duplicate_count', 0)
            result['errors'].extend(confirm_result.get('errors', []))

            # 清理临时数据
            clear_result = await self.db.clear_session_data(session_id)
            self.logger.info("[阶段3] 清理临时数据: parser=%d, preview=%d",
                             clear_result.get('parser_count', 0),
                             clear_result.get('preview_count', 0))

            result['success'] = True
            self.logger.info("[阶段3完成] session=%s, 导入=%d条, 重复跳过=%d条",
                             session_id, result['imported_count'], result['duplicate_count'])

        except Exception as e:
            self.logger.error("[阶段3] 确认失败: %s", e, exc_info=True)
            result['errors'].append(str(e))
            await self.db.update_import_session_status(session_id, 'failed')

        return result

    @log_method
    async def get_import_preview(
        self,
        session_id: str,
        selected_only: bool = False
    ) -> List[Dict[str, Any]]:
        """
        获取导入预览数据

        Args:
            session_id: 导入会话ID
            selected_only: 是否只获取选中的

        Returns:
            List[Dict]: 预览账单列表
        """
        previews = await self.db.get_preview_by_session(session_id, selected_only)
        preview_user_id = previews[0].get('user_id', 1) if previews else 1
        keyword_config = await self._get_investment_keyword_config(int(preview_user_id or 1))

        # 转换为前端期望的格式 (v6.51: 保持preview_前缀与前端字段名匹配)
        result = []
        for preview in previews:
            transfer_suggestion = self._build_transfer_suggestion_from_preview(preview)
            investment_signal = self._build_investment_signal_from_preview(
                preview,
                keyword_config=keyword_config
            )
            item = {
                'id': preview.get('id'),
                # v6.51: 前端 convertPreviewToImportTransaction 期望 preview_date/preview_amount 等字段
                'preview_date': preview.get('preview_date', ''),
                'preview_type': preview.get('preview_type', ''),
                'preview_amount': preview.get('preview_amount', 0),
                'preview_destination_amount': preview.get('preview_destination_amount', 0),
                'preview_main_category': preview.get('preview_main_category', ''),
                'preview_sub_category': preview.get('preview_sub_category', ''),
                'preview_source_account_id': preview.get('preview_source_account_id'),
                'preview_destination_account_id': preview.get('preview_destination_account_id'),
                'preview_counterparty': preview.get('preview_counterparty', ''),
                'preview_payment_method': preview.get('preview_payment_method', ''),
                'preview_description': preview.get('preview_description', ''),
                'preview_recurring_id': preview.get('preview_recurring_id'),
                'preview_recurring_name': preview.get('preview_recurring_name', ''),
                'preview_recurring_candidate_count': preview.get('preview_recurring_candidate_count', 0),
                'preview_recurring_match_score': preview.get('preview_recurring_match_score', 0),
                'preview_recurring_match_reasons': preview.get('preview_recurring_match_reasons', ''),
                'preview_recurring_matched_date': preview.get('preview_recurring_matched_date', ''),
                'preview_selected': bool(preview.get('preview_selected', 1)),
                'dedup_type': preview.get('dedup_type', ''),
                'dedup_source_ids': preview.get('dedup_source_ids', ''),
                'suggested_preview_type': transfer_suggestion.get('suggested_preview_type', ''),
                'transfer_suggestion_score': transfer_suggestion.get('score', 0.0),
                'transfer_suggestion_level': transfer_suggestion.get('level', ''),
                'transfer_suggestion_reason': transfer_suggestion.get('reason', ''),
                'investment_signal_score': investment_signal.get('score', 0.0),
                'investment_signal_level': investment_signal.get('level', ''),
                'investment_signal_reason': investment_signal.get('reason', ''),
                'investment_platform': investment_signal.get('platform', ''),
                'investment_product': investment_signal.get('product', ''),
            }
            result.append(item)

        return result

    def _build_transfer_suggestion_from_preview(
        self,
        preview: Dict[str, Any]
    ) -> Dict[str, Any]:
        """根据预览账单生成疑似转账推荐。

        该推荐是一个轻量级 M2 切片：
        - 不修改数据库结构
        - 仅在读取预览时基于现有字段计算推荐分数
        - 主要用于提醒用户将可疑的收支账单快速切换为转账类型
        """
        preview_type = str(preview.get('preview_type', '') or '').strip().lower()
        if preview_type in ['转账', 'transfer', '4', '投资', 'investment', '5']:
            return {}

        score = 0.0
        reasons: List[str] = []

        source_account_id = preview.get('preview_source_account_id')
        destination_account_id = preview.get('preview_destination_account_id')
        destination_amount = float(preview.get('preview_destination_amount', 0) or 0)
        dedup_type = str(preview.get('dedup_type', '') or '').strip().lower()

        category_text = ' '.join(filter(None, [
            str(preview.get('preview_main_category', '') or '').strip(),
            str(preview.get('preview_sub_category', '') or '').strip(),
        ])).lower()
        text_blob = ' '.join(filter(None, [
            str(preview.get('preview_counterparty', '') or '').strip(),
            str(preview.get('preview_payment_method', '') or '').strip(),
            str(preview.get('preview_description', '') or '').strip(),
            category_text,
        ])).lower()

        transfer_keywords = [
            '转账', '转入', '转出', '还款', '充值', '提现', '存取', '存款', '取款', '划转'
        ]

        if dedup_type == 'transfer':
            score += 0.75
            reasons.append('dedup_pair')

        if source_account_id and destination_account_id and str(source_account_id) != str(destination_account_id):
            score += 0.35
            reasons.append('dual_account')

        if destination_amount > 0:
            score += 0.20
            reasons.append('destination_amount')

        if any(keyword in category_text for keyword in ['转账', '存取', '还款']):
            score += 0.20
            reasons.append('transfer_category')

        matched_keywords = [keyword for keyword in transfer_keywords if keyword in text_blob]
        if matched_keywords:
            score += min(0.30, 0.12 * len(matched_keywords))
            reasons.append('keyword:' + '/'.join(matched_keywords[:3]))

        score = min(score, 1.0)
        if score < 0.55:
            return {}

        if score >= 0.8:
            level = 'high'
        elif score >= 0.65:
            level = 'medium'
        else:
            level = 'low'

        reason_text = ', '.join(reasons)
        self.logger.debug(
            '[预览转账推荐] preview_id=%s, score=%.2f, level=%s, reasons=%s',
            preview.get('id'), score, level, reason_text
        )
        return {
            'suggested_preview_type': '转账',
            'score': round(score, 2),
            'level': level,
            'reason': reason_text
        }

    def _build_investment_signal_from_preview(
        self,
        preview: Dict[str, Any],
        keyword_config: Optional[Dict[str, List[str]]] = None
    ) -> Dict[str, Any]:
        """为投资类型预览账单生成可解释信号。"""
        preview_type = str(preview.get('preview_type', '') or '').strip().lower()
        if preview_type not in ['投资', 'investment', '5']:
            return {}

        candidate = self._score_investment_candidate(
            {
                'type': preview.get('preview_type', ''),
                'counterparty': preview.get('preview_counterparty', ''),
                'payment_method': preview.get('preview_payment_method', ''),
                'description': preview.get('preview_description', ''),
                'main_category': preview.get('preview_main_category', ''),
                'sub_category': preview.get('preview_sub_category', ''),
                'original_category': (
                    preview.get('preview_sub_category', '') or
                    preview.get('preview_main_category', '')
                ),
            },
            allow_existing_investment=True,
            keyword_config=keyword_config
        )
        if not candidate:
            return {}

        score = float(candidate.get('score', 0.0) or 0.0)
        if score >= 0.8:
            level = 'high'
        elif score >= 0.65:
            level = 'medium'
        else:
            level = 'low'

        return {
            'score': score,
            'level': level,
            'reason': candidate.get('reason', ''),
            'platform': candidate.get('platform', ''),
            'product': candidate.get('product', '')
        }

    @log_method
    async def update_preview_selections(
        self,
        session_id: str,
        selections: Dict[int, bool]
    ) -> int:
        """
        更新预览选中状态

        Args:
            session_id: 导入会话ID
            selections: {preview_id: selected, ...}

        Returns:
            int: 更新的记录数
        """
        self.logger.debug("[更新预览选中] session=%s, 选择数=%d", session_id, len(selections))

        select_ids = [pid for pid, selected in selections.items() if selected]
        deselect_ids = [pid for pid, selected in selections.items() if not selected]

        updated = 0
        if select_ids:
            updated += await self.db.update_preview_selection(select_ids, True)
        if deselect_ids:
            updated += await self.db.update_preview_selection(deselect_ids, False)

        return updated

    @log_method
    async def cancel_import_session(self, session_id: str) -> Dict[str, Any]:
        """
        取消导入会话，清理临时数据

        Args:
            session_id: 导入会话ID

        Returns:
            Dict: 清理结果
        """
        self.logger.info("[取消导入] session_id=%s", session_id)

        # 清理临时数据
        clear_result = await self.db.clear_session_data(session_id)

        # 更新会话状态
        await self.db.update_import_session_status(session_id, 'cancelled')

        return {
            'success': True,
            'session_id': session_id,
            'cleared': clear_result
        }

    @log_method
    async def reclassify_preview_bills(
        self,
        session_id: str,
        preview_updates: Optional[List[Dict[str, Any]]] = None,
        user_id: int = 1
    ) -> Dict[str, Any]:
        """
        v6.55: 重新分类预览账单

        功能：
        1. 强制刷新分类规则
        2. 从 bills_preview 表读取所有账单
        3. 根据 dedup_type 使用不同类型的分类规则：
           - transfer: 使用 TRANSFER 类型规则
           - 其他: 使用 EXPENSE/INCOME/INVESTMENT 规则
        4. 重新执行账户匹配
        5. 更新 bills_preview 表

        Args:
            session_id: 导入会话ID
            user_id: 用户ID

        Returns:
            Dict: 重新分类结果
        """
        self.logger.info("[重新分类] 开始 session=%s, user_id=%d", session_id, user_id)

        result = {
            'success': False,
            'session_id': session_id,
            'total': 0,
            'categorized': 0,
            'account_matched': 0,
            'session_samples_saved': 0,
            'session_suggestion_applied': 0,
            'annotation_applied': 0,
            'errors': []
        }

        try:
            if preview_updates:
                self.logger.info("[重新分类] 步骤0: 保存当前会话人工标注样本 (%d 条)",
                                 len(preview_updates))
                result['session_samples_saved'] = await self.db.save_import_annotation_samples(
                    session_id, preview_updates, user_id=user_id
                )

            # 1. 强制刷新分类规则
            self.logger.info("[重新分类] 步骤1: 刷新分类规则")
            await self.category_engine.load_rules_from_db(self.db, user_id=user_id)
            self.logger.info("[重新分类] 分类规则加载完成，规则数量: %d",
                             len(self.category_engine.rules))

            # 2. 获取所有预览账单
            self.logger.info("[重新分类] 步骤2: 读取预览账单")
            previews = await self.db.get_preview_by_session(session_id)
            result['total'] = len(previews)
            self.logger.info("[重新分类] 读取到 %d 条预览账单", len(previews))

            if not previews:
                result['success'] = True
                return result

            # 3. 将预览数据转换为账单格式，用于分类匹配
            bills_for_category = []
            for preview in previews:
                bill = {
                    'id': preview.get('id'),
                    'date': preview.get('preview_date', ''),
                    'type': preview.get('preview_type', ''),
                    'amount': float(preview.get('preview_amount', 0)),
                    'counterparty': preview.get('preview_counterparty', ''),
                    'payment_method': preview.get('preview_payment_method', ''),
                    'description': preview.get('preview_description', ''),
                    '_dedup_type': preview.get('dedup_type', 'remaining'),
                    # 保留原有账户ID用于后续更新
                    'source_account_id': preview.get('preview_source_account_id'),
                    'destination_account_id': preview.get('preview_destination_account_id'),
                }
                bills_for_category.append(bill)

            annotation_samples = await self.db.get_import_annotation_samples(
                session_id, user_id=user_id
            )
            annotation_map = {
                int(sample['preview_id']): sample for sample in annotation_samples
                if sample.get('preview_id')
            }
            session_rule_lookup = await self._build_session_annotation_rule_lookup(
                previews,
                annotation_samples,
                user_id=user_id
            )

            for bill in bills_for_category:
                sample = annotation_map.get(int(bill.get('id', 0))) if bill.get('id') else None
                if sample and sample.get('annotated_type'):
                    bill['type'] = sample.get('annotated_type')

            result['session_suggestion_applied'] = self._apply_session_annotation_learning_rules(
                bills_for_category,
                session_rule_lookup,
                annotation_map,
                type_only=True
            )
            if result['session_suggestion_applied'] > 0:
                self.logger.info(
                    "[重新分类] 会话临时学习类型预填充 %d 条",
                    result['session_suggestion_applied']
                )

            learned_seed_count = await self._apply_import_learning_rules(
                bills_for_category,
                user_id=user_id,
                type_only=True,
                record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[重新分类] 长期学习类型预填充 %d 条", learned_seed_count)

            for bill in bills_for_category:
                sample = annotation_map.get(int(bill.get('id', 0))) if bill.get('id') else None
                if sample and sample.get('annotated_type'):
                    bill['type'] = sample.get('annotated_type')

            # 4. 分类匹配
            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            self.logger.info("[重新分类] 步骤3: 分类匹配 (%d 条账单)", len(bills_for_category))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(
                bills_for_category, types=None
            )

            # 4.5 投资账单专门识别链路（平台/产品关键词）
            self.logger.info("[重新分类] 步骤3.5: 投资候选识别")
            categorized_bills = await self._detect_investment_candidates(
                categorized_bills, user_id=user_id
            )

            # 5. 账户匹配
            self.logger.info("[重新分类] 步骤4: 账户匹配")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learned_replay_count = await self._apply_import_learning_rules(
                matched_bills,
                user_id=user_id,
                type_only=False,
                record_usage=True
            )
            if learned_replay_count > 0:
                self.logger.info("[重新分类] 长期学习结果回放 %d 条", learned_replay_count)

            result['session_suggestion_applied'] += self._apply_session_annotation_learning_rules(
                matched_bills,
                session_rule_lookup,
                annotation_map,
                type_only=False
            )
            if result['session_suggestion_applied'] > 0:
                self.logger.info(
                    "[重新分类] 会话临时学习结果回放累计 %d 条",
                    result['session_suggestion_applied']
                )

            result['annotation_applied'] = await self._apply_session_annotation_samples(
                matched_bills,
                annotation_map,
                user_id=user_id
            )

            # 6. 统计并更新预览表
            self.logger.info("[重新分类] 步骤5: 更新预览表")
            updates = []
            categorized_count = 0
            account_matched_count = 0

            for bill in matched_bills:
                preview_id = bill.get('id')
                if not preview_id:
                    continue

                update_data = {
                    'id': preview_id,
                    'preview_type': bill.get('type', ''),
                    'preview_main_category': bill.get('main_category', ''),
                    'preview_sub_category': bill.get('sub_category', ''),
                    'preview_source_account_id': bill.get('source_account_id'),
                    'preview_destination_account_id': bill.get('destination_account_id'),
                }
                updates.append(update_data)

                if bill.get('main_category'):
                    categorized_count += 1
                if bill.get('source_account_id') and (
                    isinstance(bill.get('source_account_id'), int) or
                    (isinstance(bill.get('source_account_id'), str) and
                     str(bill.get('source_account_id', '')).isdigit())
                ):
                    account_matched_count += 1

            # 批量更新
            if updates:
                updated = await self.db.batch_update_preview_classification(updates)
                self.logger.info("[重新分类] 更新了 %d 条预览记录", updated)

            result['success'] = True
            result['categorized'] = categorized_count
            result['account_matched'] = account_matched_count

            self.logger.info(
                "[重新分类] 完成: 总计 %d, 分类匹配 %d, 账户匹配 %d, 会话样本 %d, 临时学习 %d, 精确回放 %d",
                result['total'], result['categorized'], result['account_matched'],
                result['session_samples_saved'], result['session_suggestion_applied'],
                result['annotation_applied']
            )

        except Exception as e:
            self.logger.error("[重新分类] 失败: %s", e, exc_info=True)
            result['errors'].append(str(e))

        return result

    @log_method
    async def _apply_session_annotation_samples(
        self,
        bills: List[Dict[str, Any]],
        annotation_map: Dict[int, Dict[str, Any]],
        user_id: int = 1
    ) -> int:
        """将当前会话中的人工标注样本回放到重新分类结果。"""
        if not bills or not annotation_map:
            return 0

        applied_count = 0
        category_cache: Dict[int, Dict[str, Any]] = {}

        for bill in bills:
            bill_id = bill.get('id')
            if not bill_id:
                continue

            sample = annotation_map.get(int(bill_id))
            if not sample:
                continue

            if sample.get('annotated_type'):
                bill['type'] = sample.get('annotated_type')

            category_id = sample.get('annotated_category_id')
            if category_id:
                category_id = int(category_id)
                if category_id not in category_cache:
                    category_cache[category_id] = await self.db.get_category_by_id(
                        category_id, user_id=user_id
                    ) or {}
                category = category_cache.get(category_id, {})
                bill['main_category'] = category.get('main_category', '')
                bill['sub_category'] = category.get('sub_category', '')

            if sample.get('annotated_source_account_id'):
                bill['source_account_id'] = sample.get('annotated_source_account_id')

            if sample.get('annotated_destination_account_id'):
                bill['destination_account_id'] = sample.get('annotated_destination_account_id')

            applied_count += 1

        self.logger.info("[重新分类] 回放会话标注样本 %d 条", applied_count)
        return applied_count
