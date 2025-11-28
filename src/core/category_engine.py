"""
Category Engine V2 Module - 升级版账单分类引擎

支持复杂关键词匹配: OR:|&NOT:|&AND:
Author: Bill Analyser Team
Updated: 2025-11-17
"""

from typing import Dict, List, Optional, Tuple, Any

from ..utils.logger import get_logger, log_method, log_step
from ..utils.logic import RuleEngine


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


class CategoryEngine:
    """账单分类引擎 V2"""

    def __init__(self):
        """初始化分类引擎"""
        self.logger = get_logger('CategoryEngine')
        self.rule_engine = RuleEngine()
        self.keyword_matcher = KeywordMatcher()
        self.rules: List[Dict[str, Any]] = []
        self._initialized = False
        # Lock已移除 - SQLite自带线程安全

    @property
    def is_initialized(self) -> bool:
        """是否已初始化"""
        return self._initialized

    @log_method
    @log_step("加载分类规则(DB)")
    async def load_rules_from_db(self, db):
        """
        从数据库加载分类规则

        Args:
            db: 数据库实例
        """
        try:
            # 获取所有分类
            categories = await db.get_all_categories()

            valid_rules = []
            for cat in categories:
                # 只有定义了关键词的分类才作为规则
                if cat.get('keywords'):
                    valid_rules.append({
                        'main': cat['main_category'],
                        'sub': cat['sub_category'],
                        'priority': cat.get('priority', 999),
                        'keywords': cat['keywords']
                    })

            self.rules = valid_rules
            self._initialized = True

            self.logger.info(f"从数据库加载了 {len(valid_rules)} 条分类规则")

        except Exception as e:
            self.logger.error(f"从数据库加载分类规则失败: {e}", exc_info=True)
            self.rules = []

    @log_method
    def match_category(self, bill: Dict[str, Any]) -> Tuple[Optional[str], Optional[str]]:
        """
        匹配账单分类

        Args:
            bill: 账单数据字典

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
        combined_text = f"{counterparty} {description}"

        # 遍历规则进行匹配(按priority排序, 小的优先)
        sorted_rules = sorted(
            self.rules,
            key=lambda r: r.get('priority', 999)
        )

        for rule in sorted_rules:
            keywords = rule.get('keywords')
            if not keywords:
                continue

            # 使用 KeywordMatcher 进行匹配
            if self.keyword_matcher.parse_and_match(combined_text, keywords):
                self.logger.debug(f"匹配成功: {combined_text} -> {rule['main']}-{rule['sub']}")
                return rule['main'], rule['sub']

        return None, None

    @log_method
    async def batch_match_categories(self, bills: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """
        批量匹配账单分类

        Args:
            bills: 账单列表

        Returns:
            List[Dict]: 添加了分类信息的账单列表
        """
        if not self._initialized:
            self.logger.warning("分类引擎未初始化,请先调用 load_rules()")
            return bills

        self.logger.info(f"开始批量分类 {len(bills)} 条账单")

        categorized_bills = []
        matched_count = 0

        for bill in bills:
            main_cat, sub_cat = self.match_category(bill)

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


async def get_category_engine(db=None) -> CategoryEngine:
    """获取分类引擎实例"""
    if not _category_engine_v2.is_initialized and db:
        await _category_engine_v2.load_rules_from_db(db)
    return _category_engine_v2
