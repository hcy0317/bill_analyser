#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
中国农业银行账单解析器
"""

import logging
import re
from typing import Dict, Any, List, Optional

import pandas as pd

from .base import BaseBankParser

logger = logging.getLogger(__name__)


class ABCParser(BaseBankParser):
    """中国农业银行账单解析器"""

    def __init__(self):
        super().__init__()
        self.bank_name = "中国农业银行"

    def can_parse(self, file_path: str) -> bool:
        """判断是否为中国农业银行账单"""
        try:
            if not file_path.endswith(('.xlsx', '.xls')):
                return False

            # 读取文件前几行
            df = self._read_excel_file(file_path, header=None)
            if df is None:
                return False

            # 检查前10行内容是否包含中国农业银行特征
            content = ""
            for i in range(min(10, len(df))):
                for j in range(min(15, len(df.columns))):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 中国农业银行强特征标识（必须包含至少一个）
            abc_strong_indicators = [
                "中国农业银⾏", "账⼾活期交易明细清单", "账户明细查询"  # 新格式标识
            ]

            # 中国农业银行特有的字段组合（包括新格式字段）
            abc_specific_fields = [
                "⼾名", "账⼾", "交易⽇期", "交易⾦额", "对⼿信息", "⽇志号", "交易附⾔",
                # 新格式字段
                "交易日期", "交易时间", "交易金额", "本次余额", "对方户名", "对方账号",
                "交易行", "交易渠道", "交易类型", "交易用途", "交易摘要"
            ]

            # 排除其他银行的强特征
            other_bank_strong_indicators = [
                "工商银行", "ICBC", "中国民生银行", "民生银行股份有限公司", "个人账户对账单", "凭证类型"
            ]

            # 检查是否包含中国农业银行强标识
            has_abc_strong = any(indicator in content for indicator in abc_strong_indicators)

            # 检查中国农业银行特有字段（至少要有4个）
            has_abc_fields = sum(1 for field in abc_specific_fields if field in content) >= 4

            # 新格式特殊检查：同时包含新格式的核心字段组合
            has_new_format = all(field in content for field in ["交易日期", "对方户名", "交易用途"])

            # 检查是否包含其他银行强标识
            has_other_strong = any(indicator in content for indicator in other_bank_strong_indicators)

            # 中国农业银行判定逻辑：必须有强标识或特有字段组合或新格式特征，且不能有其他银行强标识
            is_abc = (has_abc_strong or has_abc_fields or has_new_format) and not has_other_strong

            if is_abc:
                logger.info("识别为中国农业银行文件: %s", file_path)

            return is_abc

        except Exception as e:
            logger.debug("判断中国农业银行文件失败: %s, 错误: %s", file_path, e)
            return False

    def get_bank_name(self) -> str:
        """获取银行名称"""
        return self.bank_name

    def parse(self, file_path: str) -> pd.DataFrame:
        """解析中国农业银行账单"""
        try:
            logger.info("开始解析中国农业银行账单: %s", file_path)

            df = self._read_excel_file(file_path, header=None)
            if df is None:
                return pd.DataFrame()

            # 寻找数据开始行
            data_start_row = self._find_abc_data_start_row(df)
            if data_start_row == -1:
                logger.warning("未找到中国农业银行数据开始行: %s", file_path)
                return pd.DataFrame()

            # 解析交易记录
            transactions = self._parse_transactions(df, data_start_row)

            logger.info("中国农业银行账单解析完成: %d条记录", len(transactions))
            return self._standardize_output(transactions)

        except Exception as e:
            logger.error("解析中国农业银行账单失败: %s, 错误: %s", file_path, e)
            return pd.DataFrame()

    def _find_abc_data_start_row(self, df: pd.DataFrame) -> int:
        """寻找数据开始行"""
        # 寻找表头行（支持旧格式和新格式）
        header_indicators = [
            # 旧格式标识符（特殊编码中文）
            "交易⽇期", "交易⾦额", "本次余额",
            # 新格式标识符（正常中文）
            "交易日期", "交易时间", "交易金额", "对方户名", "交易用途", "交易摘要"
        ]

        for i in range(len(df)):
            row_content = ""
            for j in range(len(df.columns)):
                cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                row_content += cell_value + " "

            # 检查是否包含表头指示器（至少包含3个）
            matches = sum(1 for indicator in header_indicators if indicator in row_content)
            if matches >= 3:
                logger.debug("找到数据表头行 %d: %s...", i, row_content[:100])
                return i + 1  # 数据从下一行开始

        return -1

    def _parse_transactions(self, df: pd.DataFrame, start_row: int) -> List[Dict[str, Any]]:
        """解析交易记录"""
        transactions = []

        for i in range(start_row, len(df)):
            row = df.iloc[i]

            # 检查是否为有效的交易行
            if self._is_valid_transaction_row(row):
                transaction = self._parse_transaction_row(row)
                if transaction:
                    transactions.append(transaction)

        return transactions

    def _is_valid_transaction_row(self, row: pd.Series) -> bool:
        """判断是否为有效的交易行（支持新旧格式）"""
        try:
            # 第一列应该是日期
            first_col = str(row.iloc[0]) if not pd.isna(row.iloc[0]) else ""

            # 检查旧格式：8位数字日期格式 (20240802)
            if len(first_col) == 8 and first_col.isdigit():
                return True

            # 检查新格式：可能是日期字符串格式，如"2024-08-25"或"20240825"
            if first_col and (
                len(first_col) >= 8 and
                ('20' in first_col[:4] or first_col[:8].isdigit())
            ):
                return True

            # 排除包含警告或结束信息的行
            if any(keyword in first_col for keyword in ["该交易明细", "数据缺失", "明细内", "本次查询"]):
                return False

            return False

        except Exception:
            return False

    def _clean_transaction_purpose(self, purpose: str) -> str:
        """清理交易用途，移除前面的英文数字前缀"""
        if not purpose or purpose.strip() == "" or purpose == "nan":
            return ""

        try:
            # 清理多种前缀模式
            purpose = purpose.strip()

            # 模式1: 纯数字开头 (如: 51070025188900180850玉米低氮胁迫机制解析项目绩效)
            pattern1 = r'^\d+'
            cleaned = re.sub(pattern1, '', purpose).strip()

            # 模式2: NA开头的复杂编码 (如: NA2025072959653965920531090310207蚂蚁（杭州）基金销售有限公司)
            pattern2 = r'^NA\d+'
            cleaned = re.sub(pattern2, '', cleaned).strip()

            # 模式3: UA开头的编码 (如: UA0723a26285553192支付宝-理财-蚂蚁（杭州）基金销售有限公司)
            pattern3 = r'^UA[0-9a-zA-Z]+'
            cleaned = re.sub(pattern3, '', cleaned).strip()

            # 模式4: 其他英文字母数字组合开头
            pattern4 = r'^[A-Z]+[0-9a-zA-Z]*'
            cleaned = re.sub(pattern4, '', cleaned).strip()

            # 如果清理后为空或太短，返回原始内容
            if not cleaned or len(cleaned) < 2:
                return purpose

            return cleaned

        except Exception as e:
            logger.debug("清理交易用途失败: %s, 错误: %s", purpose, e)
            return purpose

    def _parse_transaction_row(self, row: pd.Series) -> Optional[Dict[str, Any]]:
        """解析交易行（支持新旧格式）"""
        try:
            # 检测格式类型
            is_new_format = len(row) >= 11

            if is_new_format:
                return self._parse_new_format_transaction(row)
            return self._parse_old_format_transaction(row)

        except Exception as e:
            logger.debug("解析中国农业银行交易行失败: %s", e)
            return None

    def _parse_new_format_transaction(self, row: pd.Series) -> Optional[Dict[str, Any]]:
        """解析新格式交易行"""
        try:
            transaction = {}

            # 新格式列映射 (0-based index)
            # ['交易日期', '交易时间', '交易金额', '本次余额', '对方户名', '对方账号', '交易行', '交易渠道', '交易类型', '交易用途', '交易摘要']

            # 交易日期 (第1列)
            if len(row) > 0 and not pd.isna(row.iloc[0]):
                date_str = str(row.iloc[0]).strip()
                # 交易时间 (第2列)
                time_str = ""
                if len(row) > 1 and not pd.isna(row.iloc[1]):
                    time_str = str(row.iloc[1]).strip()
                transaction['日期'] = self._parse_datetime(date_str, time_str)

            # 交易金额 (第3列)
            amount = 0.0
            if len(row) > 2 and not pd.isna(row.iloc[2]):
                amount_str = str(row.iloc[2])
                amount = self._clean_amount_string(amount_str)
            transaction['金额'] = amount

            # 交易对方 (第5列 - 对方户名)
            if len(row) > 4 and not pd.isna(row.iloc[4]):
                counterparty = str(row.iloc[4]).strip()
                if counterparty and counterparty != "--":
                    transaction['交易对方'] = counterparty
                else:
                    # 如果对方户名为空，使用对方账号 (第6列)
                    if len(row) > 5 and not pd.isna(row.iloc[5]):
                        counterparty_account = str(row.iloc[5]).strip()
                        transaction['交易对方'] = counterparty_account if counterparty_account != "--" else ""
                    else:
                        transaction['交易对方'] = ""
            else:
                transaction['交易对方'] = ""

            # 商品说明 (第10列 - 交易用途，需要清理前缀)
            if len(row) > 9 and not pd.isna(row.iloc[9]):
                raw_purpose = str(row.iloc[9]).strip()
                cleaned_purpose = self._clean_transaction_purpose(raw_purpose)
                transaction['商品说明'] = cleaned_purpose if cleaned_purpose else "未知交易"
            elif len(row) > 10 and not pd.isna(row.iloc[10]):
                # 如果交易用途为空，使用交易摘要 (第11列)
                transaction['商品说明'] = str(row.iloc[10]).strip()
            else:
                transaction['商品说明'] = "未知交易"

            return transaction

        except Exception as e:
            logger.debug("解析新格式中国农业银行交易行失败: %s", e)
            return None

    def _parse_old_format_transaction(self, row: pd.Series) -> Optional[Dict[str, Any]]:
        """解析旧格式交易行"""
        try:
            transaction = {}

            # 交易日期 (第1列, 格式: 20240802)
            if len(row) > 0 and not pd.isna(row.iloc[0]):
                date_str = str(row.iloc[0]).strip()
                if len(date_str) == 8 and date_str.isdigit():
                    # 交易时间 (第2列, 格式: 151732)
                    time_str = ""
                    if len(row) > 1 and not pd.isna(row.iloc[1]):
                        time_str = str(row.iloc[1]).strip()

                    transaction['日期'] = self._parse_datetime(date_str, time_str)

            # 交易摘要/商品说明 (第3列)
            if len(row) > 2 and not pd.isna(row.iloc[2]):
                transaction['商品说明'] = str(row.iloc[2]).strip()
            else:
                transaction['商品说明'] = "未知交易"

            # 交易金额 (第4列)
            amount = 0.0
            if len(row) > 3 and not pd.isna(row.iloc[3]):
                amount_str = str(row.iloc[3])
                amount = self._clean_amount_string(amount_str)

            transaction['金额'] = amount

            # 对手信息/交易对方 (第6列)
            if len(row) > 5 and not pd.isna(row.iloc[5]):
                counterpart = str(row.iloc[5]).strip()
                if counterpart and counterpart != "--":
                    transaction['交易对方'] = counterpart
                else:
                    transaction['交易对方'] = ""
            else:
                transaction['交易对方'] = ""

            # 交易附言 (第9列)
            if len(row) > 8 and not pd.isna(row.iloc[8]):
                remark = str(row.iloc[8]).strip()
                if remark and remark != transaction['商品说明']:
                    # 如果附言与摘要不同，将附言添加到商品说明中
                    if transaction['商品说明'] != "未知交易":
                        transaction['商品说明'] += f" - {remark}"
                    else:
                        transaction['商品说明'] = remark

            return transaction

        except Exception as e:
            logger.debug("解析旧格式中国农业银行交易行失败: %s", e)
            return None
