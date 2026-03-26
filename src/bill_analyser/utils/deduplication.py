"""
Deduplication Engine - 账单去重引擎

实现两种去重机制：
1. 支付宝/微信与银行账单去重：识别支付宝/微信中来自银行的交易
2. 账户间转账去重：识别不同账户间的转账记录

去重模式：
- simple: 仅删除完全重复
- advanced: 模糊匹配+时间窗口
- aggressive: 最激进（容易误删）
"""

from datetime import datetime
from typing import List, Dict, Any, Tuple, Optional
from enum import Enum
import difflib
from collections import defaultdict

from .logger import get_logger, log_method, log_step


class DeduplicationMode(Enum):
    """去重模式"""
    SIMPLE = "simple"           # 仅完全匹配
    ADVANCED = "advanced"       # 模糊匹配+时间窗口
    AGGRESSIVE = "aggressive"   # 最激进模式


class DeduplicationEngine:
    """账单去重引擎"""

    def __init__(self, mode: DeduplicationMode = DeduplicationMode.ADVANCED):
        """
        初始化去重引擎

        Args:
            mode: 去重模式
        """
        self.mode = mode
        self.logger = get_logger(self.__class__.__name__)
        self.logger.info(f"去重引擎已初始化 | 模式: {mode.value}")

        # 配置参数
        self.amount_tolerance = 0.01  # 金额容差（元）
        self.similarity_threshold = 0.90  # 相似度阈值
        self.time_window_hours = 24  # 时间窗口（小时）

        # aggressive模式的宽松参数
        if mode == DeduplicationMode.AGGRESSIVE:
            self.amount_tolerance = 0.05
            self.similarity_threshold = 0.80
            self.time_window_hours = 48

    @log_method
    def deduplicate_all(
        self,
        bills: List[Dict[str, Any]]
    ) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
        """
        执行完整去重流程

        Args:
            bills: 账单列表

        Returns:
            (去重后的账单列表, 统计信息)
        """
        self.logger.info(f"开始去重 | 原始账单数: {len(bills)}")

        # 统计信息
        stats = {
            'original_count': len(bills),
            'payment_bank_duplicates': 0,
            'transfer_duplicates': 0,
            'final_count': 0,
            'removed_bills': []
        }

        # 第一步：支付宝/微信与银行账单去重
        bills, payment_bank_removed = self._deduplicate_payment_bank(bills)
        stats['payment_bank_duplicates'] = len(payment_bank_removed)
        stats['removed_bills'].extend(payment_bank_removed)

        # 第二步：账户间转账去重
        bills, transfer_removed = self._deduplicate_transfers(bills)
        stats['transfer_duplicates'] = len(transfer_removed)
        stats['removed_bills'].extend(transfer_removed)

        stats['final_count'] = len(bills)

        self.logger.info(
            f"去重完成 | 原始: {stats['original_count']}, "
            f"支付工具重复: {stats['payment_bank_duplicates']}, "
            f"转账重复: {stats['transfer_duplicates']}, "
            f"最终: {stats['final_count']}"
        )

        return bills, stats

    @log_step("支付宝/微信与银行账单去重")
    def _deduplicate_payment_bank(
        self,
        bills: List[Dict[str, Any]]
    ) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
        """
        去除支付宝/微信中来自银行的交易

        支付宝/微信的资金来源于银行卡，这些交易在银行账单中已经记录，
        需要去重避免重复计算。

        Args:
            bills: 账单列表

        Returns:
            (去重后的账单列表, 被移除的账单列表)
        """
        # 分类账单
        payment_bills = []  # 支付宝/微信账单
        bank_bills = []     # 银行账单
        other_bills = []    # 其他账单

        for bill in bills:
            source = bill.get('source', '').lower()
            if source in ['alipay', 'wechat']:
                payment_bills.append(bill)
            elif source in ['icbc', 'cmbc', 'abc', 'ccb']:
                bank_bills.append(bill)
            else:
                other_bills.append(bill)

        self.logger.info(
            f"账单分类 | 支付工具: {len(payment_bills)}, "
            f"银行: {len(bank_bills)}, 其他: {len(other_bills)}"
        )

        # 如果没有支付宝/微信账单或没有银行账单，则无需去重
        if not payment_bills or not bank_bills:
            self.logger.info("无需支付工具-银行去重")
            return bills, []

        # 去重
        removed_bills = []
        kept_bank_bills = []
        matched_bank_bill_ids = set()

        for payment_bill in payment_bills:
            # 检查是否与任何银行账单重复
            for bank_bill in bank_bills:
                # 使用对象ID作为唯一标识，因为此时账单可能还没有数据库ID
                if id(bank_bill) in matched_bank_bill_ids:
                    continue

                if self._is_duplicate_payment_bank(payment_bill, bank_bill):
                    self.logger.debug(
                        f"发现重复 | 支付工具账单: {payment_bill.get('date')} "
                        f"{payment_bill.get('amount')} {payment_bill.get('description', '')[:20]} "
                        f"<-> 银行账单(将被合并): {bank_bill.get('date')} "
                        f"{bank_bill.get('amount')} {bank_bill.get('description', '')[:20]}"
                    )
                    
                    # v6.32: 合并银行账单信息到支付工具账单
                    # 保留支付工具的时间和描述，合并counterparty和payment_method
                    self._merge_bank_info_to_payment(payment_bill, bank_bill)
                    
                    removed_bills.append(bank_bill)
                    matched_bank_bill_ids.add(id(bank_bill))
                    # 找到对应银行账单后，停止查找（假设一对一）
                    break

        # 保留未匹配的银行账单
        kept_bank_bills = [b for b in bank_bills if id(b) not in matched_bank_bill_ids]

        # 合并结果: 保留的银行账单 + 所有支付工具账单(已合并银行信息) + 其他账单
        result = kept_bank_bills + payment_bills + other_bills

        self.logger.info(
            f"支付工具-银行去重完成 | 合并银行账单: {len(removed_bills)}, 保留总数: {len(result)}"
        )

        return result, removed_bills

    def _merge_bank_info_to_payment(
        self,
        payment_bill: Dict[str, Any],
        bank_bill: Dict[str, Any]
    ) -> None:
        """
        将银行账单信息合并到支付工具账单 (v6.32新增)
        
        合并策略：
        - 保留支付工具的时间(date)和描述(description)
        - 合并counterparty: 如果支付工具没有，使用银行的
        - 合并payment_method: 附加银行渠道信息
        
        Args:
            payment_bill: 支付工具账单 (将被修改)
            bank_bill: 银行账单
        """
        # 1. 合并counterparty - 如果支付工具没有或为空，使用银行的
        payment_counterparty = payment_bill.get('counterparty', '').strip()
        bank_counterparty = bank_bill.get('counterparty', '').strip()
        
        if not payment_counterparty and bank_counterparty:
            payment_bill['counterparty'] = bank_counterparty
            self.logger.debug(f"合并counterparty: 使用银行账单的 '{bank_counterparty}'")
        elif payment_counterparty and bank_counterparty and payment_counterparty != bank_counterparty:
            # 两者都有且不同，追加银行的（用括号标注来源）
            payment_bill['counterparty'] = f"{payment_counterparty} ({bank_counterparty})"
            self.logger.debug(f"合并counterparty: '{payment_counterparty}' + 银行 '({bank_counterparty})'")
        
        # 2. 合并payment_method - 附加银行渠道信息
        payment_method = payment_bill.get('payment_method', '').strip()
        bank_source = bank_bill.get('source_account_id', '') or bank_bill.get('channel', '')
        
        if bank_source:
            if payment_method:
                payment_bill['payment_method'] = f"{payment_method} → {bank_source}"
            else:
                payment_bill['payment_method'] = str(bank_source)
            self.logger.debug(f"合并payment_method: '{payment_bill['payment_method']}'")
        
        self.logger.info(
            f"账单合并完成 | 支付工具: {payment_bill.get('date')} ¥{payment_bill.get('amount')} "
            f"counterparty='{payment_bill.get('counterparty', '')}' "
            f"payment_method='{payment_bill.get('payment_method', '')}'"
        )

    @log_method
    def _is_duplicate_payment_bank(
        self,
        payment_bill: Dict[str, Any],
        bank_bill: Dict[str, Any]
    ) -> bool:
        """
        判断支付宝/微信账单是否与银行账单重复

        Args:
            payment_bill: 支付宝/微信账单
            bank_bill: 银行账单

        Returns:
            是否重复
        """
        # 1. 完全匹配：交易号相同
        if self.mode in [DeduplicationMode.SIMPLE, DeduplicationMode.ADVANCED, DeduplicationMode.AGGRESSIVE]:
            payment_trans_no = payment_bill.get('transaction_no', '')
            bank_trans_no = bank_bill.get('transaction_no', '')
            if payment_trans_no and bank_trans_no and payment_trans_no == bank_trans_no:
                return True

        # Simple模式只检查完全匹配
        if self.mode == DeduplicationMode.SIMPLE:
            return False

        # 2. 金额匹配（支付宝/微信是支出，银行也是支出）
        payment_amount = abs(float(payment_bill.get('amount', 0)))
        bank_amount = abs(float(bank_bill.get('amount', 0)))

        if abs(payment_amount - bank_amount) > self.amount_tolerance:
            return False

        # 3. 时间窗口匹配
        payment_date = self._parse_date(payment_bill.get('date'))
        bank_date = self._parse_date(bank_bill.get('date'))

        if not payment_date or not bank_date:
            return False

        time_diff = abs((payment_date - bank_date).total_seconds() / 3600)
        if time_diff > self.time_window_hours:
            return False

        # 4. 描述相似度匹配（Advanced和Aggressive模式）
        if self.mode in [DeduplicationMode.ADVANCED, DeduplicationMode.AGGRESSIVE]:
            payment_desc = str(payment_bill.get('description', ''))
            bank_desc = str(bank_bill.get('description', ''))

            # 检查描述中是否包含对方
            if '支付宝' in bank_desc or 'alipay' in bank_desc.lower():
                return True
            if '微信' in bank_desc or 'wechat' in bank_desc.lower() or '财付通' in bank_desc:
                return True

            # 计算相似度
            similarity = self._calculate_similarity(payment_desc, bank_desc)
            if similarity >= self.similarity_threshold:
                return True

        return False

    @log_step("账户间转账去重")
    def _deduplicate_transfers(
        self,
        bills: List[Dict[str, Any]]
    ) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
        """
        去除账户间转账记录

        识别不同账户间的转账（如：工商卡 -> 支付宝 -> 微信），
        这类转账不是真实消费，需要去除。

        Args:
            bills: 账单列表

        Returns:
            (去重后的账单列表, 被移除的账单列表)
        """
        # 按日期和金额分组
        groups = defaultdict(list)

        for bill in bills:
            date = self._parse_date(bill.get('date'))
            if not date:
                continue

            amount = abs(float(bill.get('amount', 0)))
            # 使用日期和金额作为键（金额保留两位小数）
            key = (date.date(), round(amount, 2))
            groups[key].append(bill)

        removed_bills = []
        kept_bills = []

        # 检查每个分组
        for (date, amount), group_bills in groups.items():
            if len(group_bills) < 2:
                # 单条记录，不可能是转账对
                kept_bills.extend(group_bills)
                continue

            # 检查是否为转账对
            transfer_pairs = self._find_transfer_pairs(group_bills)

            if transfer_pairs:
                self.logger.debug(
                    f"发现转账对 | 日期: {date}, 金额: {amount}, "
                    f"转账对数: {len(transfer_pairs)}"
                )

                # 标记为转账的账单
                transfer_bill_ids = set()
                for pair in transfer_pairs:
                    transfer_bill_ids.add(id(pair[0]))
                    transfer_bill_ids.add(id(pair[1]))
                    
                    # 更新类型为转账
                    pair[0]['type'] = '转账'
                    pair[1]['type'] = '转账'
                    
                    # 更新分类为转账
                    pair[0]['main_category'] = '转账'
                    pair[0]['sub_category'] = '转账'
                    pair[1]['main_category'] = '转账'
                    pair[1]['sub_category'] = '转账'
                    
                    # 添加到移除列表
                    removed_bills.append(pair[0])
                    removed_bills.append(pair[1])

                # 只保留非转账账单
                for bill in group_bills:
                    if id(bill) not in transfer_bill_ids:
                        kept_bills.append(bill)
            else:
                # 没有转账对，全部保留
                kept_bills.extend(group_bills)

        self.logger.info(
            f"转账去重完成 | 移除: {len(removed_bills)}, 保留: {len(kept_bills)}"
        )

        return kept_bills, removed_bills

    @log_method
    def _find_transfer_pairs(
        self,
        bills: List[Dict[str, Any]]
    ) -> List[Tuple[Dict[str, Any], Dict[str, Any]]]:
        """
        在一组账单中查找转账对

        转账特征：
        1. 金额相同或接近
        2. 时间接近（同一天或相邻天）
        3. 一方是支出，另一方是收入
        4. 描述包含"转账"、"充值"、"提现"等关键词

        Args:
            bills: 账单列表

        Returns:
            转账对列表
        """
        transfer_pairs = []
        checked = set()

        # 转账关键词
        transfer_keywords = [
            '转账', '充值', '提现', '转入', '转出',
            '存入', '取出', '划转', '归还',
            'transfer', 'deposit', 'withdraw'
        ]

        for i, bill1 in enumerate(bills):
            if i in checked:
                continue

            bill1_desc = str(bill1.get('description', '')).lower()
            bill1_amount = float(bill1.get('amount', 0))

            # 检查是否包含转账关键词
            has_transfer_keyword = any(
                keyword in bill1_desc or keyword in bill1_desc
                for keyword in transfer_keywords
            )

            for j, bill2 in enumerate(bills[i+1:], start=i+1):
                if j in checked:
                    continue

                bill2_desc = str(bill2.get('description', '')).lower()
                bill2_amount = float(bill2.get('amount', 0))

                # 检查金额是否相同或接近
                if abs(abs(bill1_amount) - abs(bill2_amount)) > self.amount_tolerance:
                    continue

                # 检查是否一方支出一方收入
                if (bill1_amount > 0 and bill2_amount > 0) or \
                   (bill1_amount < 0 and bill2_amount < 0):
                    continue

                # 检查时间
                date1 = self._parse_date(bill1.get('date'))
                date2 = self._parse_date(bill2.get('date'))
                if date1 and date2:
                    time_diff_hours = abs((date1 - date2).total_seconds() / 3600)
                    if time_diff_hours > self.time_window_hours:
                        continue

                # 检查描述相似度或包含转账关键词
                has_bill2_keyword = any(
                    keyword in bill2_desc or keyword in bill2_desc
                    for keyword in transfer_keywords
                )

                if has_transfer_keyword or has_bill2_keyword:
                    transfer_pairs.append((bill1, bill2))
                    checked.add(i)
                    checked.add(j)
                    break

                if self.mode == DeduplicationMode.AGGRESSIVE:
                    # Aggressive模式下，即使没有关键词也可能识别为转账
                    similarity = self._calculate_similarity(bill1_desc, bill2_desc)
                    if similarity >= self.similarity_threshold:
                        transfer_pairs.append((bill1, bill2))
                        checked.add(i)
                        checked.add(j)
                        break

        return transfer_pairs

    @staticmethod
    def _parse_date(date_str: Any) -> Optional[datetime]:
        """
        解析日期字符串

        Args:
            date_str: 日期字符串或datetime对象

        Returns:
            datetime对象或None
        """
        if isinstance(date_str, datetime):
            return date_str

        if not date_str:
            return None

        date_str = str(date_str)

        # 尝试多种日期格式
        formats = [
            '%Y-%m-%d %H:%M:%S',
            '%Y-%m-%d',
            '%Y/%m/%d %H:%M:%S',
            '%Y/%m/%d',
            '%Y%m%d',
            '%Y-%m-%d %H:%M',
        ]

        for fmt in formats:
            try:
                return datetime.strptime(date_str, fmt)
            except ValueError:
                continue

        return None

    @staticmethod
    def _calculate_similarity(text1: str, text2: str) -> float:
        """
        计算两个文本的相似度

        Args:
            text1: 文本1
            text2: 文本2

        Returns:
            相似度（0-1）
        """
        if not text1 or not text2:
            return 0.0

        # 使用SequenceMatcher计算相似度
        return difflib.SequenceMatcher(None, text1, text2).ratio()

    @log_method
    def get_deduplication_report(
        self,
        stats: Dict[str, Any]
    ) -> str:
        """
        生成去重报告

        Args:
            stats: 统计信息

        Returns:
            报告文本
        """
        report = [
            "=" * 60,
            "账单去重报告",
            "=" * 60,
            f"去重模式: {self.mode.value}",
            f"原始账单数: {stats['original_count']}",
            f"支付工具-银行重复: {stats['payment_bank_duplicates']}",
            f"账户间转账重复: {stats['transfer_duplicates']}",
            f"总移除: {stats['payment_bank_duplicates'] + stats['transfer_duplicates']}",
            f"最终账单数: {stats['final_count']}",
            "=" * 60,
        ]

        return "\n".join(report)
