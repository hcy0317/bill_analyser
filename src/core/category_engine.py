"""
Category Engine V2 Module - 升级版账单分类引擎

支持复杂关键词匹配: OR:|&NOT:|&AND:
支持按金额正负匹配对应类型分类（正=收入, 负=支出）
v6.53: 添加类型过滤支持，允许选择性加载和匹配特定类型的分类规则
Author: Bill Analyser Team
Updated: 2025-12-01
"""

from typing import Dict, List, Optional, Tuple, Any

from ..utils.logger import get_logger, log_method, log_step
from ..utils.logic import RuleEngine
from ..utils.constants import TransactionType


class KeywordMatcher:
    """复杂关键词匹配器

    支持格式:
    - 简单: "滴滴出行"
    - OR: "OR:滴滴|快的|优步"
    - 组合: "OR:滴滴|快的&NOT:退款|取消&AND:打车"
    """

    def __init__(self):
        self.logger = get_logger('KeywordMatcher')

    @log_method
    def parse_and_match(self, text: str, rule: str) -> bool:
        """
        解析并匹配复杂规则

        Args:
            text: 要匹配的文本
            rule: 匹配规则

        Returns:
            bool: 是否匹配
        """
        if not rule or not text:
            return False

        text_lower = text.lower()

        # 解析规则
        or_patterns = []
        not_patterns = []
        and_patterns = []

        # 分割规则部分
        parts = rule.split('&')

        for part in parts:
            part = part.strip()

            if part.startswith('OR:'):
                # OR逻辑: 包含任意一个
                keywords = part[3:].split('|')
                or_patterns.extend([k.strip().lower() for k in keywords if k.strip()])

            elif part.startswith('NOT:'):
                # NOT逻辑: 不能包含
                keywords = part[4:].split('|')
                not_patterns.extend([k.strip().lower() for k in keywords if k.strip()])

            elif part.startswith('AND:'):
                # AND逻辑: 必须全部包含
                keywords = part[4:].split('|')
                and_patterns.extend([k.strip().lower() for k in keywords if k.strip()])
            else:
                # 简单匹配,作为OR处理
                if part:
                    or_patterns.append(part.lower())

        # 执行匹配逻辑
        # 1. 检查NOT条件(排除)
        for pattern in not_patterns:
            if pattern in text_lower:
                self.logger.debug(f"NOT匹配失败: 文本包含排除词 '{pattern}'")
                return False

        # 2. 检查AND条件(必须)
        for pattern in and_patterns:
            if pattern not in text_lower:
                self.logger.debug(f"AND匹配失败: 文本不包含必需词 '{pattern}'")
                return False

        # 3. 检查OR条件(任意)
        if or_patterns:
            for pattern in or_patterns:
                if pattern in text_lower:
                    self.logger.debug(f"OR匹配成功: 文本包含 '{pattern}'")
                    return True
            self.logger.debug("OR匹配失败: 文本不包含任何关键词")
            return False

        # 如果只有AND/NOT条件,且都通过了,则匹配成功
        if and_patterns or not_patterns:
            return True

        # 空规则不匹配
        return False

    def extract_positive_keywords(self, rule: str) -> List[str]:
        """从规则中提取正向关键词（OR和AND，不包含NOT）

        用于配对识别时提取可用于匹配的关键词列表。

        Args:
            rule: 关键词规则字符串

        Returns:
            List[str]: 关键词列表（小写）
        """
        if not rule:
            return []

        keywords = []
        parts = rule.split('&')

        for part in parts:
            part = part.strip()

            if part.startswith('OR:'):
                # OR逻辑: 任意一个
                kws = part[3:].split('|')
                keywords.extend([k.strip().lower() for k in kws if k.strip()])

            elif part.startswith('AND:'):
                # AND逻辑: 必须全部包含
                kws = part[4:].split('|')
                keywords.extend([k.strip().lower() for k in kws if k.strip()])

            elif part.startswith('NOT:'):
                # NOT逻辑: 跳过（排除词不用于正向匹配）
                pass

            else:
                # 简单关键词
                if part:
                    keywords.append(part.lower())

        return keywords


class CategoryEngine:
    """账单分类引擎 V2"""

    def __init__(self):
        """初始化分类引擎"""
        self.logger = get_logger('CategoryEngine')
        self.rule_engine = RuleEngine()
        self.keyword_matcher = KeywordMatcher()
        self.rules: List[Dict[str, Any]] = []
        self._initialized = False
        self._current_user_id: int = 1  # 当前加载规则的用户ID
        # Lock已移除 - SQLite自带线程安全

    @property
    def is_initialized(self) -> bool:
        """是否已初始化"""
        return self._initialized

    @property
    def current_user_id(self) -> int:
        """当前加载规则的用户ID"""
        return self._current_user_id

    @log_method
    @log_step("加载分类规则(DB)")
    async def load_rules_from_db(
        self,
        db,
        user_id: int = 1,
        types: Optional[List[int]] = None
    ):
        """
        从数据库加载分类规则

        Args:
            db: 数据库实例
            user_id: 用户ID (默认1, 用于多用户数据隔离)
            types: 可选的类型过滤列表，只加载指定类型的规则
                   例如: [TransactionType.INVESTMENT] 只加载投资类型规则
                   None表示加载所有类型
        """
        try:
            # 获取指定用户的所有分类
            types_str = str(types) if types else 'all'
            self.logger.info(f"从数据库加载分类规则 (user_id={user_id}, types={types_str})")
            categories = await db.get_all_categories(user_id=user_id)

            # v6.53: 将types转换为set便于快速查找
            type_filter: set = set()
            if types:
                type_filter = set(types)

            valid_rules = []
            for cat in categories:
                # 只有定义了关键词的分类才作为规则
                keywords = cat.get('keywords')
                if keywords:
                    rule_type = cat.get('type', TransactionType.EXPENSE)

                    # v6.53: 如果指定了类型过滤，只加载匹配的类型
                    if types and rule_type not in type_filter:
                        continue

                    valid_rules.append({
                        'main': cat['main_category'],
                        'sub': cat['sub_category'],
                        'priority': cat.get('priority', 999),
                        'keywords': keywords,
                        'type': rule_type  # 分类类型
                    })

            self.rules = valid_rules
            self._initialized = True
            self._current_user_id = user_id  # 记录当前加载的用户ID

            self.logger.info(f"从数据库加载了 {len(valid_rules)} 条分类规则 (user_id={user_id})")
            if valid_rules:
                # 记录前3条规则用于调试
                for i, rule in enumerate(valid_rules[:3]):
                    self.logger.debug(
                        f"规则#{i+1}: {rule['main']}/{rule['sub']} -> '{rule['keywords'][:50]}...'"
                    )

        except Exception as e:
            self.logger.error(f"从数据库加载分类规则失败: {e}", exc_info=True)
            self.rules = []

    @log_method
    def match_category(
        self,
        bill: Dict[str, Any],
        types: Optional[List[int]] = None
    ) -> Tuple[Optional[str], Optional[str]]:
        """
        匹配账单分类

        v6.44改进：优先通过关键词匹配所有类型的分类规则，
        如果匹配到投资类分类，会同时更新账单的type字段为'投资'。
        
        v6.53改进：添加types参数，支持只在指定类型的分类中匹配。
        
        匹配逻辑：
        1. 首先在所有分类规则中按关键词匹配（或只在指定类型中匹配）
        2. 如果匹配到投资/转账类分类，更新账单type字段
        3. 如果没有匹配到，根据金额正负判断默认类型

        Args:
            bill: 账单数据字典，必须包含amount字段
            types: 可选的类型过滤列表，只在指定类型的规则中匹配
                   例如: [TransactionType.INVESTMENT] 只匹配投资类型规则
                   None表示在所有类型中匹配

        Returns:
            Tuple[Optional[str], Optional[str]]: (主分类, 子分类)
        """
        if not self._initialized:
            self.logger.warning("分类引擎未初始化")
            return None, None

        if not bill:
            return None, None

        # 获取账单字段用于匹配
        counterparty = str(bill.get('counterparty', ''))
        description = str(bill.get('description', ''))
        original_category = str(bill.get('original_category', ''))
        combined_text = f"{counterparty} {description} {original_category}"

        # 获取金额和原始类型
        amount = float(bill.get('amount', 0))
        original_type = str(bill.get('type', '')).strip()

        self.logger.debug(
            "[分类匹配] counterparty='%s', description='%s', amount=%.2f, type='%s'",
            counterparty[:30], description[:30], amount, original_type
        )

        # v6.53: 如果指定了类型过滤，只在指定类型的规则中匹配
        type_filter: set = set()
        if types:
            type_filter = set(types)
            self.logger.debug("[分类匹配] 类型过滤: %s", types)

        # ===== 第一步：在所有分类规则中按关键词匹配（优先级排序）=====
        # 按优先级排序所有规则（小的优先）
        # v6.53: 如果有类型过滤，先过滤规则
        rules_to_match = self.rules
        if types:
            rules_to_match = [r for r in self.rules if r.get('type') in type_filter]
            self.logger.debug("[分类匹配] 过滤后规则数: %d/%d", len(rules_to_match), len(self.rules))

        sorted_rules = sorted(
            rules_to_match,
            key=lambda r: r.get('priority', 999)
        )

        for rule in sorted_rules:
            keywords = rule.get('keywords')
            if not keywords:
                continue

            # 使用 KeywordMatcher 进行匹配
            if self.keyword_matcher.parse_and_match(combined_text, keywords):
                rule_type = rule.get('type')
                main_cat = rule['main']
                sub_cat = rule['sub']

                # v6.63: 单条匹配日志改为 DEBUG，减少批量导入时的冗余输出
                self.logger.debug(
                    "[分类匹配] '%s' -> %s/%s (type=%s)",
                    combined_text[:30], main_cat, sub_cat, rule_type
                )

                # v6.52: 修复分类匹配逻辑
                # 只有 dedup_type='transfer' 的账单才能匹配转账类型分类
                # 转账类型的判断应该在去重阶段通过转账配对机制完成
                # 关键词匹配不应该改变账单的type为转账

                # 获取账单的去重类型
                dedup_type = str(bill.get('_dedup_type', '')).lower()

                # 如果匹配到投资类分类，更新账单type为'投资'
                if rule_type == TransactionType.INVESTMENT:
                    bill['type'] = '投资'
                # 如果匹配到转账类分类，只有 dedup_type='transfer' 时才更新type
                elif rule_type == TransactionType.TRANSFER:
                    if dedup_type == 'transfer':
                        bill['type'] = '转账'
                    else:
                        # 不更新type，跳过此规则继续查找其他规则
                        self.logger.debug(
                            "[分类匹配] 跳过转账规则(dedup_type='%s')", dedup_type
                        )
                        continue

                return main_cat, sub_cat

        # ===== 第二步：如果没有匹配到任何规则，根据金额正负确定默认类型搜索 =====
        # 这部分保持原有逻辑，用于没有关键词匹配的账单
        self.logger.debug("[分类匹配] 关键词未匹配，使用默认类型逻辑")

        # 根据金额正负和原始类型确定要匹配的分类类型
        if original_type in ['转账', '转出', '转入']:
            target_types = [TransactionType.TRANSFER]
        elif original_type in ['投资', '投资理财', '理财']:
            target_types = [TransactionType.INVESTMENT]
        elif amount > 0:
            target_types = [TransactionType.INCOME]
        elif amount < 0:
            target_types = [TransactionType.EXPENSE]
        else:
            target_types = [TransactionType.EXPENSE, TransactionType.INCOME,
                           TransactionType.TRANSFER, TransactionType.INVESTMENT]

        # 过滤目标类型的规则（此时可能有无关键词的默认分类）
        filtered_rules = [
            rule for rule in self.rules
            if rule.get('type') in target_types
        ]

        self.logger.debug(
            "[分类匹配] 默认类型过滤: target_types=%s, 规则数=%d/%d",
            target_types, len(filtered_rules), len(self.rules)
        )

        return None, None

    @log_method
    async def batch_match_categories(
        self,
        bills: List[Dict[str, Any]],
        types: Optional[List[int]] = None
    ) -> List[Dict[str, Any]]:
        """
        批量匹配账单分类

        Args:
            bills: 账单列表
            types: 可选的类型过滤列表，只在指定类型的规则中匹配
                   例如: [TransactionType.INVESTMENT] 只匹配投资类型规则
                   None表示在所有类型中匹配

        Returns:
            List[Dict]: 添加了分类信息的账单列表
        """
        if not self._initialized:
            self.logger.warning("分类引擎未初始化,请先调用 load_rules()")
            return bills

        types_str = str(types) if types else 'all'
        self.logger.info(f"开始批量分类 {len(bills)} 条账单 (types={types_str})")

        categorized_bills = []
        matched_count = 0

        for bill in bills:
            # v6.53: 传递types参数给match_category
            main_cat, sub_cat = self.match_category(bill, types=types)

            # 添加分类信息
            categorized_bill = bill.copy()
            categorized_bill['main_category'] = main_cat
            categorized_bill['sub_category'] = sub_cat

            categorized_bills.append(categorized_bill)

            if main_cat:
                matched_count += 1

        match_rate = (matched_count / len(bills) * 100) if bills else 0
        self.logger.info(
            f"批量分类完成: 成功匹配 {matched_count}/{len(bills)} 条 "
            f"({match_rate:.1f}%)"
        )

        return categorized_bills

    @log_method
    def get_categories_tree(self) -> Dict[str, Dict[str, List[str]]]:
        """
        获取分类树结构(用于UI显示)
        按支出/收入/转账/投资分组

        Returns:
            Dict[str, Dict[str, List[str]]]:
            {
                "支出": {主分类: [子分类列表]},
                "收入": {主分类: [子分类列表]},
                "转账": {主分类: [子分类列表]},
                "投资": {主分类: [子分类列表]}
            }
        """
        tree = {
            "支出": {},
            "收入": {},
            "转账": {},
            "投资": {}
        }

        for rule in self.rules:
            main = rule.get('main')
            sub = rule.get('sub')

            if not main or not sub:
                continue

            # 根据主分类名称判断类型
            if main == "收入":
                bill_type = "收入"
            elif main == "转账":
                bill_type = "转账"
            elif main == "投资":
                bill_type = "投资"
            else:
                bill_type = "支出"

            if main not in tree[bill_type]:
                tree[bill_type][main] = []

            if sub not in tree[bill_type][main]:
                tree[bill_type][main].append(sub)

        self.logger.debug(
            f"生成分类树: 支出 {len(tree['支出'])} 个, "
            f"收入 {len(tree['收入'])} 个, "
            f"转账 {len(tree['转账'])} 个, "
            f"投资 {len(tree['投资'])} 个"
        )
        return tree


# 全局分类引擎实例
_category_engine_v2 = CategoryEngine()


async def get_category_engine(
    db=None,
    user_id: int = 1,
    types: Optional[List[int]] = None
) -> CategoryEngine:
    """获取分类引擎实例
    
    Args:
        db: 数据库实例
        user_id: 用户ID，用于加载用户特定的分类规则
        types: 可选的类型过滤列表，只加载指定类型的规则
               例如: [TransactionType.INVESTMENT] 只加载投资类型规则
               None表示加载所有类型
    
    Returns:
        CategoryEngine: 分类引擎实例
    """
    if not _category_engine_v2.is_initialized and db:
        await _category_engine_v2.load_rules_from_db(db, user_id=user_id, types=types)
    elif _category_engine_v2.is_initialized and _category_engine_v2.current_user_id != user_id and db:
        # 如果用户ID变化，重新加载规则
        await _category_engine_v2.load_rules_from_db(db, user_id=user_id, types=types)
    return _category_engine_v2
