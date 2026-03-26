"""Smart Deduplication Engine - 智能去重引擎

处理多来源账单的智能去重，包括：
1. 支付宝/微信与银行账单的重复检测（优先保留支付宝/微信）
2. 账户间转账/投资识别（时间接近+金额条件）
3. 总账单与分账单识别（时间接近+金额相加）
4. 与数据库已导入账单的重复检测（v6.32新增）
5. 基于相似度的通用去重（v6.40新增）

去重策略优先级：
- 支付平台（wechat, alipay）> 银行（icbc, cmbc, abc, ccb）

v6.36 重大重构 (2025-11-29):
- 转账配对逻辑统一：
  1. 解析阶段所有账单type只能是收入或支出，不参考原始type
  2. 转账配对条件：时间30秒内 + 金额绝对值相等且正负相反 + 来源不同
  3. 金额为负的是转出账户(source_account)，为正的是转入账户(destination_account)

v6.37 动态关键词 (2025-11-29):
- 投资和转账关键词从数据库分类系统动态获取：
  1. 从categories表中type=4(转账)和type=5(投资)的分类获取keywords字段
  2. 支持复杂关键词规则(OR:|&NOT:|&AND:语法)
  3. 关键词缓存机制，避免每次配对都查询数据库
  4. 降级策略：数据库无关键词时使用默认关键词列表

v6.39 投资配对逻辑重构 (2025-11-29):
- 投资配对与转账配对分离处理：
  1. 投资配对条件：时间30秒内 + 金额相同（符号相同，都是收入或都是支出）+ 包含投资关键词 + 来源不同
  2. 投资账户方向规则：
     - 金额为正（收入）: 第一个账户是投资账户，第二个是来源账户
     - 金额为负（支出）: 第一个账户是来源账户，第二个是投资账户

v6.40 通用相似度去重 (2025-11-29):
- 新增基于相似度的通用去重方法，适用于同一来源的重复账单：
  1. 去重条件：时间差≤30秒 + 金额完全相同 + 类型相同（都是收入或支出）+ 交易对方相似度≥50%
  2. 合并策略：优先保留支付宝/微信的时间、类型、金额
  3. 字段合并：counterparty、payment_method、description取两者不同部分合并
  4. 处理顺序：完全重复 → 相似度去重 → 转账/投资配对 → 平台-银行重复 → 分账单 → 数据库重复
"""

from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional, Tuple, Set
from enum import Enum
import hashlib
from difflib import SequenceMatcher

from ..utils.logger import get_logger, log_method
from ..utils.constants import TransactionType
from .category_engine import KeywordMatcher


class DeduplicationType(Enum):
    """去重类型"""

    EXACT = "exact"  # 完全重复
    PLATFORM_BANK = "platform_bank"  # 支付平台与银行重复
    SIMILAR = "similar"  # 相似度去重（v6.40新增）
    TRANSFER = "transfer"  # 转账配对
    INVESTMENT = "investment"  # 投资配对
    SPLIT_MERGE = "split_merge"  # 总账单与分账单
    DATABASE_DUPLICATE = "database_duplicate"  # 与数据库已有账单重复


@dataclass
class DuplicateGroup:
    """重复账单组"""

    type: DeduplicationType
    bills: List[Dict[str, Any]]
    keep_bill: Dict[str, Any]  # 保留的账单
    remove_bills: List[Dict[str, Any]]  # 移除的账单
    reason: str  # 去重原因说明


@dataclass
class DeduplicationResult:
    """去重结果"""

    original_count: int  # 原始账单数
    kept_bills: List[Dict[str, Any]]  # 保留的账单
    removed_count: int  # 移除的账单数
    duplicate_groups: List[DuplicateGroup]  # 重复组详情
    transfer_pairs: List[Tuple[Dict, Dict]]  # 识别的转账对
    investment_pairs: List[Tuple[Dict, Dict]]  # 识别的投资对
    split_groups: List[Dict]  # 识别的分账单组


class SmartDeduplicationEngine:
    """智能去重引擎

    实现多来源账单的智能去重：
    1. 支付平台账单优先保留（信息更丰富）
    2. 识别账户间转账（一收一支，时间接近，金额相等）
    3. 识别投资交易（类似转账但可能涉及投资账户）
    4. 识别分账单（多个小额加起来等于一个大额）
    """

    # 支付平台优先级（数值越小优先级越高）
    SOURCE_PRIORITY = {
        "wechat": 1,
        "alipay": 2,
        "icbc": 10,
        "cmbc": 10,
        "abc": 10,
        "ccb": 10,
    }

    # 时间容差（秒）
    TIME_TOLERANCE_TRANSFER = 30  # 转账/投资配对
    TIME_TOLERANCE_SPLIT = 30  # 分账单配对
    TIME_TOLERANCE_PLATFORM_BANK = 60  # 支付平台与银行配对（稍微宽松一些）

    # 金额容差（元）
    AMOUNT_TOLERANCE = 0.01

    # 默认关键词（当数据库无法获取时使用）
    DEFAULT_INVESTMENT_KEYWORDS = [
        "基金",
        "理财",
        "投资",
        "股票",
        "债券",
        "定期",
        "余额宝",
        "零钱通",
        "黄金",
        "保险",
        "fund",
        "invest",
        "蚂蚁财富",
        "蚂蚁基金",
        "博时黄金",
        "国泰黄金",
        "天弘",
        "etf",
        "lof",
        "指数",
        "货币基金",
        "定投",
    ]

    DEFAULT_TRANSFER_KEYWORDS = [
        "转账",
        "转入",
        "转出",
        "汇款",
        "transfer",
        "银行转入",
        "银行卡转",
        "跨行转账",
        "同行转账",
    ]

    def __init__(self):
        """初始化去重引擎"""
        self.logger = get_logger("SmartDedup")
        # v6.37: 关键词缓存
        self._investment_keywords: Optional[List[str]] = None
        self._transfer_keywords: Optional[List[str]] = None
        self._keywords_loaded: bool = False
        self._keywords_user_id: Optional[int] = None
        # v6.37: 使用CategoryEngine中的KeywordMatcher解析关键词规则
        self._keyword_matcher = KeywordMatcher()
        self.logger.info("[智能去重引擎] 初始化完成")

    @log_method
    async def load_keywords_from_db(self, db, user_id: int = 1) -> None:
        """从数据库分类系统加载转账和投资关键词（v6.37新增）

        从categories表中获取type=4(转账)和type=5(投资)分类的keywords字段，
        解析并缓存用于后续配对判断。

        关键词规则语法支持：
        - 简单关键词: "转账"
        - OR逻辑: "OR:转账|汇款|转入"
        - 组合逻辑: "OR:转账|汇款&NOT:退款"

        Args:
            db: 数据库实例
            user_id: 用户ID（用于多用户数据隔离）
        """
        self.logger.info("[加载关键词] 从数据库加载分类关键词 (user_id=%d)", user_id)

        try:
            # 获取所有分类
            categories = await db.get_all_categories(user_id=user_id)

            investment_keywords_set: Set[str] = set()
            transfer_keywords_set: Set[str] = set()

            for cat in categories:
                cat_type = cat.get("type", 0)
                keywords_str = cat.get("keywords", "")

                if not keywords_str:
                    continue

                # 使用KeywordMatcher解析关键词字符串，提取所有可用于匹配的关键词
                extracted_keywords = self._keyword_matcher.extract_positive_keywords(keywords_str)

                if cat_type == TransactionType.TRANSFER:
                    transfer_keywords_set.update(extracted_keywords)
                    self.logger.debug(
                        "[加载关键词] 转账分类 '%s/%s' 提取关键词: %s",
                        cat.get("main_category"),
                        cat.get("sub_category"),
                        extracted_keywords[:5],  # 只显示前5个
                    )
                elif cat_type == TransactionType.INVESTMENT:
                    investment_keywords_set.update(extracted_keywords)
                    self.logger.debug(
                        "[加载关键词] 投资分类 '%s/%s' 提取关键词: %s",
                        cat.get("main_category"),
                        cat.get("sub_category"),
                        extracted_keywords[:5],
                    )

            # 转换为列表并缓存
            self._transfer_keywords = list(transfer_keywords_set)
            self._investment_keywords = list(investment_keywords_set)
            self._keywords_loaded = True
            self._keywords_user_id = user_id

            self.logger.info(
                "[加载关键词] 成功加载 投资关键词=%d个, 转账关键词=%d个",
                len(self._investment_keywords),
                len(self._transfer_keywords),
            )

            # 如果没有从数据库获取到关键词，使用默认值
            if not self._investment_keywords:
                self._investment_keywords = self.DEFAULT_INVESTMENT_KEYWORDS.copy()
                self.logger.warning(
                    "[加载关键词] 数据库无投资分类关键词，使用默认列表 (%d个)", len(self._investment_keywords)
                )

            if not self._transfer_keywords:
                self._transfer_keywords = self.DEFAULT_TRANSFER_KEYWORDS.copy()
                self.logger.warning(
                    "[加载关键词] 数据库无转账分类关键词，使用默认列表 (%d个)", len(self._transfer_keywords)
                )

        except Exception as e:
            self.logger.error("[加载关键词] 从数据库加载失败: %s，使用默认关键词", e)
            self._investment_keywords = self.DEFAULT_INVESTMENT_KEYWORDS.copy()
            self._transfer_keywords = self.DEFAULT_TRANSFER_KEYWORDS.copy()
            self._keywords_loaded = True

    def _get_investment_keywords(self) -> List[str]:
        """获取投资关键词列表（v6.37新增）

        优先返回从数据库加载的关键词，未加载则返回默认列表。

        Returns:
            List[str]: 投资关键词列表
        """
        if self._investment_keywords is not None:
            return self._investment_keywords
        return self.DEFAULT_INVESTMENT_KEYWORDS.copy()

    def _get_transfer_keywords(self) -> List[str]:
        """获取转账关键词列表（v6.37新增）

        优先返回从数据库加载的关键词，未加载则返回默认列表。

        Returns:
            List[str]: 转账关键词列表
        """
        if self._transfer_keywords is not None:
            return self._transfer_keywords
        return self.DEFAULT_TRANSFER_KEYWORDS.copy()

    def clear_keywords_cache(self) -> None:
        """清除关键词缓存（v6.37新增）

        当分类系统的关键词发生变化时调用。
        """
        self._investment_keywords = None
        self._transfer_keywords = None
        self._keywords_loaded = False
        self._keywords_user_id = None
        self.logger.info("[关键词缓存] 已清除")

    @log_method
    def process(self, bills: List[Dict[str, Any]]) -> DeduplicationResult:
        """
        处理账单去重（不包含数据库对比）

        处理流程：
        1. 首先检测并标记完全重复的账单
        2. 检测转账配对（一正一负）
        3. 检测投资配对（同方向+投资关键词）
        4. 检测支付平台与银行的重复（v6.39: 移到配对后，避免误判投资配对）
        5. 检测分账单

        v6.39变更：将转账/投资配对移到平台-银行重复检测之前
        原因：投资配对需要同金额同方向，与平台-银行重复条件相同
        如果先做平台-银行检测，投资配对会被错误标记为重复

        Args:
            bills: 标准格式账单列表

        Returns:
            DeduplicationResult: 去重结果
        """
        self.logger.info("[去重开始] 处理 %d 条账单", len(bills))

        original_count = len(bills)
        duplicate_groups: List[DuplicateGroup] = []
        transfer_pairs: List[Tuple[Dict, Dict]] = []
        investment_pairs: List[Tuple[Dict, Dict]] = []
        split_groups: List[Dict] = []

        # 为每条账单生成唯一标识
        for bill in bills:
            bill["_dedup_id"] = self._generate_bill_hash(bill)
            bill["_removed"] = False
            bill["_merged_from"] = []  # 追踪合并来源

        # 1. 检测完全重复
        exact_groups = self._find_exact_duplicates(bills)
        duplicate_groups.extend(exact_groups)
        self.logger.info("[完全重复] 发现 %d 组", len(exact_groups))

        # 2. v6.40: 相似度去重（30秒+金额相同+类型相同+50%交易对方相似度）
        similar_groups = self._find_similar_duplicates(bills)
        duplicate_groups.extend(similar_groups)
        self.logger.info("[相似度去重] 发现 %d 组", len(similar_groups))

        # 3. v6.39: 先做转账/投资配对（避免被平台-银行重复误判）
        all_pairs = self._find_paired_transactions(bills)
        # 按类型分离转账和投资配对
        transfer_pairs = [p for p in all_pairs if p[0].get("type") == "转账"]
        investment_pairs = [p for p in all_pairs if p[0].get("type") == "投资"]
        self.logger.info("[转账配对] 发现 %d 对", len(transfer_pairs))
        self.logger.info("[投资配对] 发现 %d 对", len(investment_pairs))

        # 4. 检测支付平台与银行重复（配对后的账单已标记_removed，不会再被检测）
        platform_bank_groups = self._find_platform_bank_duplicates(bills)
        duplicate_groups.extend(platform_bank_groups)
        self.logger.info("[平台-银行重复] 发现 %d 组", len(platform_bank_groups))

        # 5. 检测分账单
        split_groups = self._find_split_bills(bills)
        self.logger.info("[分账单] 发现 %d 组", len(split_groups))

        # 收集保留的账单
        kept_bills = [b for b in bills if not b.get("_removed", False)]

        # 清理临时字段
        for bill in kept_bills:
            bill.pop("_dedup_id", None)
            bill.pop("_removed", None)
            bill.pop("_merged_from", None)

        result = DeduplicationResult(
            original_count=original_count,
            kept_bills=kept_bills,
            removed_count=original_count - len(kept_bills),
            duplicate_groups=duplicate_groups,
            transfer_pairs=transfer_pairs,
            investment_pairs=investment_pairs,
            split_groups=split_groups,
        )

        self.logger.info(
            "[去重完成] 原始: %d, 保留: %d, 移除: %d", original_count, len(kept_bills), result.removed_count
        )

        return result

    @log_method
    async def process_with_db(self, bills: List[Dict[str, Any]], db, user_id: int = 1) -> DeduplicationResult:
        """
        处理账单去重（包含数据库对比）

        处理流程：
        1. 首先检测并标记完全重复的账单
        2. 检测转账配对（一正一负）
        3. 检测投资配对（同方向+投资关键词）
        4. 检测支付平台与银行的重复（v6.39: 移到配对后，避免误判投资配对）
        5. 检测分账单
        6. 与数据库已导入账单对比去重

        v6.39变更：将转账/投资配对移到平台-银行重复检测之前
        原因：投资配对需要同金额同方向，与平台-银行重复条件相同

        Args:
            bills: 标准格式账单列表
            db: 数据库实例
            user_id: 用户ID

        Returns:
            DeduplicationResult: 去重结果
        """
        self.logger.info("[去重开始(含数据库)] 处理 %d 条账单", len(bills))

        # v6.37: 先加载关键词（用于转账/投资配对）
        await self.load_keywords_from_db(db, user_id)

        original_count = len(bills)
        duplicate_groups: List[DuplicateGroup] = []
        transfer_pairs: List[Tuple[Dict, Dict]] = []
        investment_pairs: List[Tuple[Dict, Dict]] = []
        split_groups: List[Dict] = []

        # 为每条账单生成唯一标识
        for bill in bills:
            bill["_dedup_id"] = self._generate_bill_hash(bill)
            bill["_removed"] = False
            bill["_merged_from"] = []

        # 1. 检测完全重复
        exact_groups = self._find_exact_duplicates(bills)
        duplicate_groups.extend(exact_groups)
        self.logger.info("[完全重复] 发现 %d 组", len(exact_groups))

        # 2. v6.40: 相似度去重（30秒+金额相同+类型相同+50%交易对方相似度）
        similar_groups = self._find_similar_duplicates(bills)
        duplicate_groups.extend(similar_groups)
        self.logger.info("[相似度去重] 发现 %d 组", len(similar_groups))

        # 3. v6.39: 先做转账/投资配对（避免被平台-银行重复误判）
        all_pairs = self._find_paired_transactions(bills)
        # 按类型分离转账和投资配对
        transfer_pairs = [p for p in all_pairs if p[0].get("type") == "转账"]
        investment_pairs = [p for p in all_pairs if p[0].get("type") == "投资"]
        self.logger.info("[转账配对] 发现 %d 对", len(transfer_pairs))
        self.logger.info("[投资配对] 发现 %d 对", len(investment_pairs))

        # 4. 检测支付平台与银行重复（配对后的账单已标记_removed，不会再被检测）
        platform_bank_groups = self._find_platform_bank_duplicates(bills)
        duplicate_groups.extend(platform_bank_groups)
        self.logger.info("[平台-银行重复] 发现 %d 组", len(platform_bank_groups))

        # 5. 检测分账单
        split_groups = self._find_split_bills(bills)
        self.logger.info("[分账单] 发现 %d 组", len(split_groups))

        # 6. 与数据库已有账单对比
        db_duplicate_groups = await self.find_database_duplicates(bills, db, user_id)
        duplicate_groups.extend(db_duplicate_groups)
        self.logger.info("[数据库重复] 发现 %d 组", len(db_duplicate_groups))

        # 收集保留的账单
        kept_bills = [b for b in bills if not b.get("_removed", False)]

        # 清理临时字段
        for bill in kept_bills:
            bill.pop("_dedup_id", None)
            bill.pop("_removed", None)
            bill.pop("_merged_from", None)
            bill.pop("_duplicate_of_db_id", None)

        result = DeduplicationResult(
            original_count=original_count,
            kept_bills=kept_bills,
            removed_count=original_count - len(kept_bills),
            duplicate_groups=duplicate_groups,
            transfer_pairs=transfer_pairs,
            investment_pairs=investment_pairs,
            split_groups=split_groups,
        )

        self.logger.info(
            "[去重完成(含数据库)] 原始: %d, 保留: %d, 移除: %d", original_count, len(kept_bills), result.removed_count
        )

        return result

    def _generate_bill_hash(self, bill: Dict[str, Any]) -> str:
        """生成账单的唯一哈希值"""
        key_fields = [
            str(bill.get("date", "")),
            str(bill.get("amount", 0)),
            str(bill.get("counterparty", "")),
            str(bill.get("description", "")),
        ]
        key_str = "|".join(key_fields)
        return hashlib.md5(key_str.encode("utf-8")).hexdigest()[:12]

    def _parse_datetime(self, date_str: str) -> Optional[datetime]:
        """解析日期时间字符串"""
        formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d",
        ]
        for fmt in formats:
            try:
                return datetime.strptime(str(date_str).strip(), fmt)
            except ValueError:
                continue
        return None

    def _time_close(self, dt1: datetime, dt2: datetime, tolerance_seconds: int) -> bool:
        """判断两个时间是否接近"""
        diff = abs((dt1 - dt2).total_seconds())
        return diff <= tolerance_seconds

    def _amount_equal(self, amt1: float, amt2: float) -> bool:
        """判断两个金额是否相等（考虑容差）"""
        return abs(abs(amt1) - abs(amt2)) <= self.AMOUNT_TOLERANCE

    def _amount_opposite(self, amt1: float, amt2: float) -> bool:
        """判断两个金额是否相反（一正一负，绝对值相等）

        用于转账配对：一个账户转出（负），另一个账户转入（正）
        """
        return (amt1 * amt2 < 0) and self._amount_equal(amt1, -amt2)

    def _amount_same_direction(self, amt1: float, amt2: float) -> bool:
        """判断两个金额是否方向相同且绝对值相等（v6.39新增）

        用于投资配对：两个账单都是收入或都是支出
        例如：蚂蚁基金买入 -50.00 与 银行卡扣款 -50.00
        """
        return (amt1 * amt2 > 0) and self._amount_equal(amt1, amt2)

    def _counterparty_similarity(self, cp1: str, cp2: str) -> float:
        """计算两个交易对方字符串的相似度（v6.40新增）

        使用SequenceMatcher计算基于最长公共子序列的相似度。
        会对字符串进行标准化处理：去除空白、转小写。

        Args:
            cp1: 第一个交易对方字符串
            cp2: 第二个交易对方字符串

        Returns:
            float: 相似度 0.0-1.0
        """
        # 标准化：去除空白、转小写
        s1 = str(cp1 or "").strip().lower()
        s2 = str(cp2 or "").strip().lower()

        # 如果都为空，认为相似
        if not s1 and not s2:
            return 1.0
        # 如果只有一个为空，相似度为0
        if not s1 or not s2:
            return 0.0

        # 使用SequenceMatcher计算相似度
        return SequenceMatcher(None, s1, s2).ratio()

    def _merge_bill_fields(self, primary_bill: Dict[str, Any], secondary_bill: Dict[str, Any]) -> Dict[str, Any]:
        """合并两个账单的字段（v6.40新增）

        优先保留primary_bill的时间、类型、金额，
        合并两者不同的counterparty、payment_method、description。

        合并策略：
        - date, amount, type: 使用primary_bill的值
        - counterparty, payment_method, description:
          如果不同则用 "primary内容 / secondary内容" 格式合并

        Args:
            primary_bill: 主账单（通常是支付宝/微信账单）
            secondary_bill: 次账单（通常是银行账单）

        Returns:
            Dict: 合并后的账单
        """
        merged = primary_bill.copy()

        # 需要合并的字段
        merge_fields = ["counterparty", "payment_method", "description"]

        for field in merge_fields:
            primary_val = str(primary_bill.get(field, "") or "").strip()
            secondary_val = str(secondary_bill.get(field, "") or "").strip()

            # 如果两者都有值且不同，则合并
            if primary_val and secondary_val:
                # 检查是否一个包含另一个
                if primary_val in secondary_val:
                    merged[field] = secondary_val
                elif secondary_val in primary_val:
                    merged[field] = primary_val
                elif primary_val != secondary_val:
                    # 两者不同，用分隔符合并
                    merged[field] = f"{primary_val} / {secondary_val}"
            elif secondary_val and not primary_val:
                # 主账单没有，使用次账单的值
                merged[field] = secondary_val

        # 记录合并来源
        merged["_merged_from"].append(
            {
                "source": secondary_bill.get("source_account_id"),
                "date": secondary_bill.get("date"),
                "amount": secondary_bill.get("amount"),
            }
        )

        self.logger.debug(
            "[字段合并] 主账单=%s, 次账单=%s, 合并字段=%s",
            primary_bill.get("source_account_id"),
            secondary_bill.get("source_account_id"),
            merge_fields,
        )

        return merged

    def _get_source_priority(self, source_id: str) -> int:
        """获取来源优先级"""
        return self.SOURCE_PRIORITY.get(source_id, 100)

    def _find_exact_duplicates(self, bills: List[Dict[str, Any]]) -> List[DuplicateGroup]:
        """查找完全重复的账单"""
        groups = []
        hash_map: Dict[str, List[Dict[str, Any]]] = {}

        for bill in bills:
            if bill.get("_removed"):
                continue
            hash_val = bill["_dedup_id"]
            if hash_val not in hash_map:
                hash_map[hash_val] = []
            hash_map[hash_val].append(bill)

        for hash_val, dup_bills in hash_map.items():
            if len(dup_bills) > 1:
                # 按来源优先级排序，保留优先级最高的
                dup_bills.sort(key=lambda b: self._get_source_priority(b.get("source_account_id", "")))
                keep_bill = dup_bills[0]
                remove_bills = dup_bills[1:]

                # 标记移除
                for bill in remove_bills:
                    bill["_removed"] = True

                groups.append(
                    DuplicateGroup(
                        type=DeduplicationType.EXACT,
                        bills=dup_bills,
                        keep_bill=keep_bill,
                        remove_bills=remove_bills,
                        reason=f"完全重复，保留 {keep_bill.get('source_account_id')} 来源",
                    )
                )

        return groups

    def _find_platform_bank_duplicates(self, bills: List[Dict[str, Any]]) -> List[DuplicateGroup]:
        """查找支付平台与银行的重复账单

        场景：用户在微信/支付宝通过银行卡支付，会同时产生：
        1. 微信/支付宝账单
        2. 银行账单

        识别条件：
        - 时间接近（60秒内）
        - 金额相等（绝对值相等且符号相同，都是支出或都是收入）
        - 一个来自支付平台，一个来自银行

        v6.35 修复：添加金额方向检查，避免将转账误识别为重复
        - 转账场景：平台收入(+100) vs 银行支出(-100) → 不是重复，是转账
        - 重复场景：平台支出(-100) vs 银行支出(-100) → 是重复
        """
        groups = []
        platform_sources = {"wechat", "alipay"}
        bank_sources = {"icbc", "cmbc", "abc", "ccb"}

        # 分离平台账单和银行账单
        platform_bills = [b for b in bills if b.get("source_account_id") in platform_sources and not b.get("_removed")]
        bank_bills = [b for b in bills if b.get("source_account_id") in bank_sources and not b.get("_removed")]

        matched_bank_indices = set()

        for p_bill in platform_bills:
            p_dt = self._parse_datetime(p_bill.get("date", ""))
            p_amt = float(p_bill.get("amount", 0))

            if not p_dt:
                continue

            for i, b_bill in enumerate(bank_bills):
                if i in matched_bank_indices:
                    continue

                b_dt = self._parse_datetime(b_bill.get("date", ""))
                b_amt = float(b_bill.get("amount", 0))

                if not b_dt:
                    continue

                # v6.35: 检查时间接近、金额相等（绝对值）、且符号相同（方向相同）
                # 符号相同表示都是支出或都是收入，这才是真正的重复
                # 符号相反表示一个是支出一个是收入，这是转账而不是重复
                same_direction = p_amt * b_amt > 0  # 同号为True
                if (
                    self._time_close(p_dt, b_dt, self.TIME_TOLERANCE_PLATFORM_BANK)
                    and self._amount_equal(p_amt, b_amt)
                    and same_direction
                ):
                    matched_bank_indices.add(i)
                    b_bill["_removed"] = True

                    groups.append(
                        DuplicateGroup(
                            type=DeduplicationType.PLATFORM_BANK,
                            bills=[p_bill, b_bill],
                            keep_bill=p_bill,
                            remove_bills=[b_bill],
                            reason=(
                                f"支付平台({p_bill.get('source_account_id')})"
                                f"与银行({b_bill.get('source_account_id')})重复，"
                                f"保留支付平台账单（信息更丰富）"
                            ),
                        )
                    )
                    break

        return groups

    def _find_similar_duplicates(self, bills: List[Dict[str, Any]]) -> List[DuplicateGroup]:
        """基于相似度查找重复账单（v6.40新增，v6.41增强）

        通用去重方法，不依赖特定的账单来源。适用于：
        - 同一来源的重复账单（如两条ABC银行账单描述不同但实际是同一笔）
        - 不同来源但非平台-银行的重复

        识别条件：

        **规则1：同源精确匹配（v6.41新增）**
        针对同一来源的重复账单，条件更宽松：
        1. 来源相同（source_account_id相同）
        2. 时间接近（2秒内）
        3. 金额完全相同（包括符号）
        4. 类型相同（都是收入或都是支出）
        → 无需满足counterparty相似度条件

        **规则2：跨源相似度匹配（v6.40）**
        针对不同来源的账单：
        1. 时间接近（30秒内）
        2. 金额完全相同（包括符号）
        3. 类型相同（都是收入或都是支出）
        4. 交易对方(counterparty)相似度≥50%
        5. 排除可能是投资配对的场景（不同账户+投资关键词）

        处理策略：
        - 优先保留支付宝/微信账单的时间、类型、金额
        - 合并两者的counterparty、payment_method、description字段
        """
        groups: List[DuplicateGroup] = []
        matched: Set[int] = set()

        # 支付平台优先
        platform_sources = {"wechat", "alipay"}

        # 获取投资关键词用于排除投资配对场景
        investment_keywords = self._get_investment_keywords()

        # 只处理未被移除的账单
        active_bills = [(i, b) for i, b in enumerate(bills) if not b.get("_removed")]

        # 按时间排序以提高匹配效率
        active_bills.sort(key=lambda x: x[1].get("date", ""))

        n = len(active_bills)
        self.logger.debug("[相似度去重] 活跃账单数: %d", n)

        for i, (idx1, bill1) in enumerate(active_bills):
            if idx1 in matched:
                continue

            dt1 = self._parse_datetime(bill1.get("date", ""))
            amt1 = float(bill1.get("amount", 0))
            cp1 = str(bill1.get("counterparty", ""))
            source1 = str(bill1.get("source_account_id", ""))
            desc1 = str(bill1.get("description", "")).lower()

            if not dt1:
                continue

            for j in range(i + 1, n):
                idx2, bill2 = active_bills[j]

                if idx2 in matched:
                    continue

                dt2 = self._parse_datetime(bill2.get("date", ""))
                amt2 = float(bill2.get("amount", 0))
                source2 = str(bill2.get("source_account_id", ""))

                if not dt2:
                    continue

                # 如果时间差超过30秒，后面的账单时间差只会更大，可以break
                time_diff = abs((dt2 - dt1).total_seconds())
                if time_diff > self.TIME_TOLERANCE_TRANSFER:
                    # 因为是按时间排序的，后面的时间差只会更大
                    # 但要注意处理时间相同但秒数不同的情况
                    if (dt2 - dt1).total_seconds() > self.TIME_TOLERANCE_TRANSFER:
                        break
                    continue

                # 条件1: 金额完全相同（包括符号）
                if abs(amt1 - amt2) > self.AMOUNT_TOLERANCE:
                    continue

                # 条件2: 类型相同（都是收入或都是支出，即符号相同）
                if amt1 * amt2 <= 0:  # 符号不同
                    continue

                # v6.41: 同源宽松规则
                # 如果是同一来源的账单，时间差≤2秒，金额完全相同，无需满足counterparty相似度
                # 这种情况通常是银行对同一笔交易记录了两条不同描述的账单
                is_same_source = source1 == source2 and source1  # 确保来源非空
                is_very_close_time = time_diff <= 2.0  # 2秒以内

                # 计算counterparty相似度
                cp2 = str(bill2.get("counterparty", ""))
                similarity = self._counterparty_similarity(cp1, cp2)

                # 判断是否为同源精确匹配（无需相似度）
                is_same_source_exact_match = is_same_source and is_very_close_time
                if is_same_source_exact_match:
                    self.logger.debug(
                        "[相似度去重] 同源精确匹配: source=%s, 时间差=%.1f秒, "
                        "counterparty='%s' vs '%s' (相似度=%.2f，忽略)",
                        source1,
                        time_diff,
                        cp1[:30],
                        cp2[:30],
                        similarity,
                    )
                    # 不检查相似度，直接进入匹配逻辑
                else:
                    # 非同源：排除投资配对场景
                    if source1 != source2:
                        desc2 = str(bill2.get("description", "")).lower()
                        combined_text = f"{desc1} {desc2}"
                        is_investment = any(kw in combined_text for kw in investment_keywords)
                        if is_investment:
                            self.logger.debug(
                                "[相似度去重] 跳过投资配对场景: %s vs %s, 描述='%s'/'%s'",
                                source1,
                                source2,
                                desc1[:30],
                                desc2[:30],
                            )
                            continue

                    # 非同源精确匹配：需要满足counterparty相似度≥50%
                    if similarity < 0.5:
                        self.logger.debug(
                            "[相似度去重] 不匹配: %s vs %s, 相似度=%.2f < 0.5", cp1[:30], cp2[:30], similarity
                        )
                        continue

                # 找到重复！
                matched.add(idx1)
                matched.add(idx2)

                # 确定主账单和次账单
                # 优先级：支付平台 > 其他
                source2 = str(bill2.get("source_account_id", ""))
                if source1 in platform_sources and source2 not in platform_sources:
                    primary_bill, secondary_bill = bill1, bill2
                elif source2 in platform_sources and source1 not in platform_sources:
                    primary_bill, secondary_bill = bill2, bill1
                else:
                    # 都是平台或都是银行，按来源优先级
                    if self._get_source_priority(source1) <= self._get_source_priority(source2):
                        primary_bill, secondary_bill = bill1, bill2
                    else:
                        primary_bill, secondary_bill = bill2, bill1

                # 合并字段
                merged_bill = self._merge_bill_fields(primary_bill, secondary_bill)

                # 更新主账单的字段
                for key, value in merged_bill.items():
                    if not key.startswith("_"):
                        primary_bill[key] = value

                # 标记次账单为已移除
                secondary_bill["_removed"] = True

                groups.append(
                    DuplicateGroup(
                        type=DeduplicationType.SIMILAR,
                        bills=[primary_bill, secondary_bill],
                        keep_bill=primary_bill,
                        remove_bills=[secondary_bill],
                        reason=(
                            f"相似度去重: 时间差{time_diff:.0f}秒, "
                            f"counterparty相似度{similarity:.0%}, "
                            f"保留{primary_bill.get('source_account_id')}账单"
                        ),
                    )
                )

                self.logger.info(
                    "[相似度去重] 匹配成功: 时间差=%.0f秒, 金额=%.2f, 相似度=%.2f, 保留=%s, 移除=%s",
                    time_diff,
                    amt1,
                    similarity,
                    source1,
                    source2 if primary_bill is bill1 else source1,
                )

                break  # 找到一个匹配就停止

        self.logger.info("[相似度去重] 发现 %d 组重复", len(groups))
        return groups

    def _find_transfer_pairs(self, bills: List[Dict[str, Any]]) -> List[Tuple[Dict, Dict]]:
        """识别账户间转账（v6.36重构）

        v6.36新逻辑：
        1. 先通过统一条件(时间30秒+金额相反+来源不同)找到所有配对
        2. 配对后通过分类关键词区分投资和转账
        3. 金额为负的是转出账户(source_account)，为正的是转入账户(destination_account)

        配对条件（与投资配对完全一致）：
        - 时间接近（30秒内）
        - 金额绝对值相等，符号相反（一正一负）
        - 来自不同账户
        - 不参考原始type，只通过分类关键词判断

        Returns:
            转账配对列表，每对包含(转出账单, 转入账单)
        """
        # 调用统一的配对方法
        all_pairs = self._find_paired_transactions(bills)

        # 过滤出转账类型的配对
        transfer_pairs = [pair for pair in all_pairs if pair[0].get("type") == "转账"]

        return transfer_pairs

    def _find_investment_pairs(self, bills: List[Dict[str, Any]]) -> List[Tuple[Dict, Dict]]:
        """识别投资交易（v6.36重构）

        v6.36新逻辑：
        1. 先通过统一条件(时间30秒+金额相反+来源不同)找到所有配对
        2. 配对后通过分类关键词区分投资和转账
        3. 金额为负的是转出账户(source_account)，为正的是转入账户(destination_account)

        配对条件（与转账配对完全一致）：
        - 时间接近（30秒内）
        - 金额绝对值相等，符号相反（一正一负）
        - 来自不同账户
        - 不参考原始type，只通过分类关键词判断

        Returns:
            投资配对列表，每对包含(转出账单, 转入账单)
        """
        # 调用统一的配对方法
        all_pairs = self._find_paired_transactions(bills)

        # 过滤出投资类型的配对
        investment_pairs = [pair for pair in all_pairs if pair[0].get("type") == "投资"]

        return investment_pairs

    def _find_paired_transactions(self, bills: List[Dict[str, Any]]) -> List[Tuple[Dict, Dict]]:
        """统一的账单配对方法（v6.36新增，v6.39重构）

        v6.39核心逻辑变更：
        1. 转账配对条件：时间30秒内 + 金额绝对值相等且正负相反 + 来源不同
        2. 投资配对条件：时间30秒内 + 金额相同（符号相同）+ 来源不同 + 包含投资关键词
        3. 投资配对时金额符号决定账户角色：
           - 金额为正（收入）: bill1是投资账户, bill2是来源账户
           - 金额为负（支出）: bill1是来源账户, bill2是投资账户

        Args:
            bills: 账单列表

        Returns:
            配对列表，每对为(第一个账单, 第二个账单)元组
        """
        pairs: List[Tuple[Dict, Dict]] = []
        matched: Set[int] = set()

        # v6.37: 使用从数据库加载的动态关键词（如未加载则使用默认值）
        investment_keywords = self._get_investment_keywords()
        transfer_keywords = self._get_transfer_keywords()

        self.logger.debug(
            "[配对] 使用关键词 - 投资(%d个): %s, 转账(%d个): %s",
            len(investment_keywords),
            investment_keywords[:5] if len(investment_keywords) > 5 else investment_keywords,
            len(transfer_keywords),
            transfer_keywords[:5] if len(transfer_keywords) > 5 else transfer_keywords,
        )

        # 过滤活跃账单（未被移除且金额不为0）
        active_bills = [
            (i, b) for i, b in enumerate(bills) if not b.get("_removed") and abs(float(b.get("amount", 0))) > 0.001
        ]

        self.logger.debug("[配对] 活跃账单数: %d", len(active_bills))

        for i, (idx1, bill1) in enumerate(active_bills):
            if idx1 in matched:
                continue

            dt1 = self._parse_datetime(bill1.get("date", ""))
            amt1 = float(bill1.get("amount", 0))
            source1 = str(bill1.get("source_account_id", ""))

            if not dt1:
                continue

            for j in range(i + 1, len(active_bills)):
                idx2, bill2 = active_bills[j]

                if idx2 in matched:
                    continue

                dt2 = self._parse_datetime(bill2.get("date", ""))
                amt2 = float(bill2.get("amount", 0))
                source2 = str(bill2.get("source_account_id", ""))

                if not dt2:
                    continue

                # 基础条件：时间接近 + 来源不同
                if not (self._time_close(dt1, dt2, self.TIME_TOLERANCE_TRANSFER) and source1 != source2):
                    continue

                # 合并描述和原始分类用于关键词匹配
                desc1 = str(bill1.get("description", "")).lower()
                desc2 = str(bill2.get("description", "")).lower()
                cat1 = str(bill1.get("original_category", "")).lower()
                cat2 = str(bill2.get("original_category", "")).lower()
                counterparty1 = str(bill1.get("counterparty", "")).lower()
                counterparty2 = str(bill2.get("counterparty", "")).lower()

                # 合并所有文本用于关键词匹配
                combined_text = f"{desc1} {desc2} {cat1} {cat2} {counterparty1} {counterparty2}"

                # 判断是否包含投资关键词
                is_investment = any(kw in combined_text for kw in investment_keywords)
                # 判断是否包含转账关键词
                is_transfer = any(kw in combined_text for kw in transfer_keywords)

                pair_type = None

                # v6.39: 投资配对条件 - 金额相同（符号相同）+ 投资关键词
                if is_investment and self._amount_same_direction(amt1, amt2):
                    pair_type = "投资"
                    matched.add(idx1)
                    matched.add(idx2)

                    # 设置账单类型
                    bill1["type"] = pair_type
                    bill2["type"] = pair_type

                    # v6.39投资账户方向规则:
                    # 金额为正（收入）: bill1是投资账户, bill2是来源账户
                    # 金额为负（支出）: bill1是来源账户, bill2是投资账户
                    if amt1 > 0:
                        # 正金额（收入）: bill1是投资账户（destination），bill2是来源账户（source）
                        investment_bill = bill1  # 投资账户
                        funding_bill = bill2  # 来源账户
                    else:
                        # 负金额（支出）: bill1是来源账户（source），bill2是投资账户（destination）
                        investment_bill = bill2  # 投资账户
                        funding_bill = bill1  # 来源账户

                    # 设置双账户信息
                    # 来源账单的destination指向投资账户
                    funding_bill["destination_account_id"] = investment_bill.get("source_account_id")
                    # 投资账单的destination保持自己，source指向来源账户
                    investment_bill["destination_account_id"] = investment_bill.get("source_account_id")
                    investment_bill["source_account_id"] = funding_bill.get("source_account_id")

                    self.logger.info(
                        "[%s配对] 金额=%.2f, 来源账户=%s -> 投资账户=%s, 描述='%s'/'%s'",
                        pair_type,
                        abs(amt1),
                        funding_bill.get("source_account_id"),
                        investment_bill.get("destination_account_id"),
                        desc1[:30],
                        desc2[:30],
                    )

                    pairs.append((bill1, bill2))
                    break

                # 转账配对条件 - 金额一正一负（符号相反）
                # 注：is_transfer变量用于日志记录，转账是默认行为不需要关键词匹配
                elif self._amount_opposite(amt1, amt2):
                    # 如果有转账关键词则明确为转账，否则默认也是转账
                    pair_type = "转账"
                    matched.add(idx1)
                    matched.add(idx2)

                    # 日志中记录是否匹配到转账关键词
                    keyword_source = "转账关键词" if is_transfer else "默认规则"

                    # 设置账单类型
                    bill1["type"] = pair_type
                    bill2["type"] = pair_type

                    # 转账账户方向规则:
                    # 金额为负的是转出方(source_account)
                    # 金额为正的是转入方(destination_account)
                    if amt1 < 0:
                        outgoing_bill = bill1  # 转出
                        incoming_bill = bill2  # 转入
                    else:
                        outgoing_bill = bill2  # 转出
                        incoming_bill = bill1  # 转入

                    # 设置双账户信息
                    # 转出账单：source是自己，destination是对方
                    outgoing_bill["destination_account_id"] = incoming_bill.get("source_account_id")
                    # 转入账单：source是对方，destination是自己
                    incoming_bill["destination_account_id"] = incoming_bill.get("source_account_id")
                    incoming_bill["source_account_id"] = outgoing_bill.get("source_account_id")

                    self.logger.info(
                        "[%s配对] 金额=%.2f, 转出账户=%s -> 转入账户=%s, 匹配来源=%s, 描述='%s'/'%s'",
                        pair_type,
                        abs(amt1),
                        outgoing_bill.get("source_account_id"),
                        outgoing_bill.get("destination_account_id"),
                        keyword_source,
                        desc1[:30],
                        desc2[:30],
                    )

                    pairs.append((outgoing_bill, incoming_bill))
                    break

        self.logger.info("[统一配对] 共发现 %d 对配对账单", len(pairs))
        return pairs

    def _find_split_bills(self, bills: List[Dict[str, Any]]) -> List[Dict]:
        """识别分账单

        场景：一笔总消费被拆分为多笔小额：
        - 时间接近（30秒内）
        - 方向相同（都是支出或都是收入）
        - 分账单金额之和等于总账单

        处理方式：保留分账单，移除总账单（分账单信息更详细）
        """
        groups = []
        matched = set()

        active_bills = [
            (i, b) for i, b in enumerate(bills) if not b.get("_removed") and abs(float(b.get("amount", 0))) > 0
        ]

        # 按时间排序
        active_bills.sort(key=lambda x: x[1].get("date", ""))

        n = len(active_bills)

        for i, (idx1, bill1) in enumerate(active_bills):
            if idx1 in matched:
                continue

            dt1 = self._parse_datetime(bill1.get("date", ""))
            amt1 = float(bill1.get("amount", 0))

            if not dt1 or abs(amt1) < 10:  # 忽略小额账单
                continue

            # 寻找时间接近的同方向账单
            candidate_bills = []
            for j in range(i + 1, min(i + 20, n)):  # 最多检查后续20条
                idx2, bill2 = active_bills[j]

                if idx2 in matched:
                    continue

                dt2 = self._parse_datetime(bill2.get("date", ""))
                amt2 = float(bill2.get("amount", 0))

                if not dt2:
                    continue

                # 时间差太大则停止
                if abs((dt2 - dt1).total_seconds()) > self.TIME_TOLERANCE_SPLIT:
                    break

                # 方向相同（符号相同）
                if amt1 * amt2 > 0:
                    candidate_bills.append((idx2, bill2, amt2))

            # 检查候选账单之和是否等于当前账单
            if len(candidate_bills) >= 2:
                total = sum(b[2] for b in candidate_bills)
                if self._amount_equal(total, amt1):
                    # 找到分账单组
                    matched.add(idx1)
                    for idx2, _, _ in candidate_bills:
                        matched.add(idx2)

                    # 移除总账单，保留分账单
                    bill1["_removed"] = True

                    groups.append(
                        {
                            "total_bill": bill1,
                            "split_bills": [b[1] for b in candidate_bills],
                            "total_amount": amt1,
                            "reason": f"总账单({amt1})拆分为{len(candidate_bills)}笔分账单",
                        }
                    )

                    self.logger.debug("[分账单] 总额 %s -> %d 笔", amt1, len(candidate_bills))

        return groups

    @log_method
    async def find_database_duplicates(
        self, bills: List[Dict[str, Any]], db, user_id: int = 1, time_tolerance_seconds: int = 300
    ) -> List[DuplicateGroup]:
        """与数据库已有账单对比，查找重复

        用于导入时检测新账单是否与已导入的账单重复。

        Args:
            bills: 待导入的账单列表
            db: 数据库实例
            user_id: 用户ID
            time_tolerance_seconds: 时间容差（秒），默认5分钟

        Returns:
            List[DuplicateGroup]: 重复组列表
        """
        groups: List[DuplicateGroup] = []

        if not bills:
            return groups

        # 获取待导入账单的日期范围
        dates = []
        for bill in bills:
            dt = self._parse_datetime(bill.get("date", ""))
            if dt:
                dates.append(dt)

        if not dates:
            return groups

        min_date = min(dates)
        max_date = max(dates)

        # 扩展日期范围（前后各1天）
        start_date = (min_date - timedelta(days=1)).strftime("%Y-%m-%d")
        end_date = (max_date + timedelta(days=1)).strftime("%Y-%m-%d")

        self.logger.info("[数据库去重] 查询日期范围: %s ~ %s", start_date, end_date)

        # 从数据库获取范围内的账单
        try:
            existing_bills = await db.get_bills_by_date_range(start_date, end_date, user_id=user_id)
            self.logger.info("[数据库去重] 已有账单数: %d", len(existing_bills))
        except Exception as e:
            self.logger.error("[数据库去重] 查询失败: %s", e)
            return groups

        if not existing_bills:
            return groups

        # 构建已有账单的索引（按日期+金额分组）
        existing_index: Dict[str, List[Dict]] = {}
        for eb in existing_bills:
            key = f"{eb.get('date', '')[:10]}_{float(eb.get('amount', 0)):.2f}"
            if key not in existing_index:
                existing_index[key] = []
            existing_index[key].append(eb)

        # 检测重复
        for bill in bills:
            if bill.get("_removed"):
                continue

            bill_date = bill.get("date", "")
            bill_amt = float(bill.get("amount", 0))
            key = f"{bill_date[:10]}_{bill_amt:.2f}"

            candidates = existing_index.get(key, [])
            if not candidates:
                continue

            bill_dt = self._parse_datetime(bill_date)
            if not bill_dt:
                continue

            bill_desc = str(bill.get("description", "")).lower()
            bill_counterparty = str(bill.get("counterparty", "")).lower()

            for eb in candidates:
                eb_dt = self._parse_datetime(eb.get("date", ""))
                if not eb_dt:
                    continue

                # 检查时间是否接近
                if not self._time_close(bill_dt, eb_dt, time_tolerance_seconds):
                    continue

                # 检查金额是否相等
                if not self._amount_equal(bill_amt, float(eb.get("amount", 0))):
                    continue

                # 检查描述或交易对手是否相似
                eb_desc = str(eb.get("description", "")).lower()
                eb_counterparty = str(eb.get("counterparty", "")).lower()

                desc_similar = bill_desc and eb_desc and (bill_desc in eb_desc or eb_desc in bill_desc)
                counterparty_similar = (
                    bill_counterparty
                    and eb_counterparty
                    and (bill_counterparty in eb_counterparty or eb_counterparty in bill_counterparty)
                )

                if desc_similar or counterparty_similar or (not bill_desc and not eb_desc):
                    # 找到重复，标记移除
                    bill["_removed"] = True
                    bill["_duplicate_of_db_id"] = eb.get("id")

                    groups.append(
                        DuplicateGroup(
                            type=DeduplicationType.DATABASE_DUPLICATE,
                            bills=[bill, eb],
                            keep_bill=eb,
                            remove_bills=[bill],
                            reason=(
                                f"与数据库已有账单重复 (ID={eb.get('id')}, "
                                f"日期={eb.get('date')}, 金额={eb.get('amount')})"
                            ),
                        )
                    )
                    self.logger.debug(
                        "[数据库重复] 新账单 %s/%s 与已有账单 ID=%s 重复", bill_date, bill_amt, eb.get("id")
                    )
                    break

        return groups

    def extract_investment_target(self, bill: Dict[str, Any]) -> Optional[str]:
        """从账单描述中提取投资目标账户

        用于投资类型交易的双账户识别。

        Args:
            bill: 账单数据

        Returns:
            Optional[str]: 目标账户名称（如"余额宝"、"零钱通"）
        """
        description = str(bill.get("description", ""))
        counterparty = str(bill.get("counterparty", ""))
        combined = f"{description} {counterparty}"

        # 投资目标账户关键词
        investment_targets = {
            "余额宝": ["余额宝", "天弘余额宝"],
            "零钱通": ["零钱通", "微信零钱通"],
            "理财通": ["理财通", "腾讯理财通"],
            "招财宝": ["招财宝"],
            "基金": ["基金", "定投"],
            "股票": ["股票", "证券"],
            "黄金": ["黄金", "存金宝"],
            "定期": ["定期", "定存"],
        }

        for target_name, keywords in investment_targets.items():
            for keyword in keywords:
                if keyword in combined:
                    self.logger.debug("[投资目标] 从 '%s' 提取到目标: %s", combined[:50], target_name)
                    return target_name

        return None


# 便捷函数
_engine: Optional[SmartDeduplicationEngine] = None


def get_dedup_engine() -> SmartDeduplicationEngine:
    """获取去重引擎单例"""
    global _engine  # pylint: disable=global-statement
    if _engine is None:
        _engine = SmartDeduplicationEngine()
    return _engine


def smart_deduplicate(bills: List[Dict[str, Any]]) -> DeduplicationResult:
    """智能去重"""
    return get_dedup_engine().process(bills)
