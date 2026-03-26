#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
工商银行账单解析器
支持多种格式的工商银行账单解析
"""

from typing import Optional, Dict, Any
import logging
import re
import pandas as pd
from .base import BaseBankParser

logger = logging.getLogger(__name__)


class ICBCParser(BaseBankParser):
    """工商银行账单解析器"""

    def __init__(self):
        super().__init__()
        self.bank_name = "工商银行"

    def get_bank_name(self) -> str:
        """获取银行名称"""
        return self.bank_name

    def can_parse(self, file_path: str) -> bool:
        """判断是否为工商银行账单"""
        try:
            if not file_path.endswith(('.xlsx', '.xls')):
                return False

            # 读取文件前几行
            df = self._read_excel_file(file_path, header=None)
            if df is None:
                return False

            # 检查前15行内容是否包含工商银行特征
            content = ""
            for i in range(min(15, len(df))):
                for j in range(min(20, len(df.columns))):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 工商银行强特征标识 - 必须包含至少一个
            icbc_strong_indicators = [
                "工商银行", "ICBC", "中国工商银行"
            ]

            # 工商银行特有的字段组合
            icbc_specific_fields = [
                "收入/支出金额", "储种", "对方户名", "对方账号", "历史明细"
            ]

            # 排除其他银行的强特征
            other_bank_strong_indicators = [
                # 中国农业银行特征（旧格式）
                "中国农业银⾏", "账⼾活期交易明细清单", "⼾名", "交易⽇期", "交易⾦额", "对⼿信息", "⽇志号",
                # 中国农业银行特征（新格式）
                "中国农业银行", "账户明细查询", "交易渠道", "交易类型", "交易用途",
                # 民生银行特征
                "中国民生银行", "民生银行股份有限公司", "个人账户对账单", "凭证类型", "凭证号码", "客户姓名", "客户账号"
            ]

            # 检查中国农业银行新格式的特殊组合特征
            abc_new_format_indicators = [
                "交易日期", "交易时间", "交易金额", "本次余额", "对方户名", "对方账号",
                "交易行", "交易渠道", "交易类型", "交易用途", "交易摘要"
            ]
            abc_new_format_matches = sum(1 for field in abc_new_format_indicators if field in content)
            is_abc_new_format = abc_new_format_matches >= 8  # 如果匹配8个或以上，很可能是中国农业银行新格式

            # 检查是否包含工商银行强标识
            has_icbc_strong = any(indicator in content for indicator in icbc_strong_indicators)

            # 检查工商银行特有字段
            has_icbc_fields = sum(1 for field in icbc_specific_fields if field in content) >= 2

            # 检查是否包含其他银行强标识
            has_other_strong = any(
                indicator in content for indicator in other_bank_strong_indicators
            )

            # 工商银行判定逻辑：必须有强标识或特有字段组合，且不能有其他银行强标识，也不能是中国农业银行新格式
            is_icbc = (has_icbc_strong or has_icbc_fields) and not has_other_strong and not is_abc_new_format

            if is_icbc:
                logger.info("识别为工商银行文件: %s", file_path)

            return is_icbc

        except (IOError, ValueError) as e:
            logger.debug("判断工商银行文件失败: %s, 错误: %s", file_path, e)
            return False

    def parse(self, file_path: str) -> pd.DataFrame:
        """解析工商银行账单"""
        try:
            logger.info("开始解析工商银行账单: %s", file_path)

            df = self._read_excel_file(file_path, header=None)
            if df is None:
                return pd.DataFrame()

            # 分析文件结构
            file_type = self._analyze_file_structure(df)

            if file_type == "type1":  # 个人账户对账单类型
                return self._parse_type1_format(df, file_path)
            if file_type == "type2":  # 历史明细类型
                return self._parse_type2_format(df, file_path)

            logger.warning("未识别的工商银行文件格式: %s", file_path)
            return pd.DataFrame()

        except (IOError, ValueError) as e:
            logger.error("解析工商银行账单失败: %s, 错误: %s", file_path, e)
            return pd.DataFrame()

    def _analyze_file_structure(self, df: pd.DataFrame) -> str:
        """分析文件结构类型"""
        try:
            # 检查前5行内容
            for i in range(min(5, len(df))):
                for j in range(min(10, len(df.columns))):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""

                    # Type1: 个人账户对账单格式
                    if "个人账户对账单" in cell_value or "客户姓名" in cell_value:
                        return "type1"

                    # Type2: 历史明细格式
                    if self._is_type2_format(df, i, j, cell_value):
                        return "type2"

            return "unknown"

        except (IndexError, ValueError) as e:
            logger.debug("分析文件结构失败: %s", e)
            return "unknown"

    def _is_type2_format(self, df: pd.DataFrame, i: int, j: int, cell_value: str) -> bool:
        """判断是否为Type2格式"""
        if "交易日期" not in cell_value or j > 2:
            return False

        next_cells = []
        for k in range(j+1, min(j+5, len(df.columns))):
            if not pd.isna(df.iloc[i, k]):
                next_cells.append(str(df.iloc[i, k]))

        return any("账号" in cell or "储种" in cell or "摘要" in cell for cell in next_cells)

    def _parse_type1_format(self, df: pd.DataFrame, file_path: str) -> pd.DataFrame:
        """解析Type1格式 (个人账户对账单)"""
        try:
            transactions = []

            # 寻找数据开始行 - 查找表头行
            data_start_row = -1
            for i in range(min(20, len(df))):
                row_content = ""
                for j in range(len(df.columns)):
                    cell = df.iloc[i, j]
                    if not pd.isna(cell):
                        row_content += str(cell) + " "

                if "凭证类型" in row_content or "交易时间" in row_content:
                    data_start_row = i + 1
                    break

            if data_start_row == -1:
                logger.warning("未找到工商银行Type1数据开始行: %s", file_path)
                return pd.DataFrame()

            # 解析交易记录
            i = data_start_row
            while i < len(df):
                row = df.iloc[i]

                # 检查是否为有效的交易行
                if self._is_valid_type1_transaction_row(row):
                    transaction = self._parse_type1_transaction_row(row)
                    if transaction:
                        transactions.append(transaction)

                i += 1

            logger.info("工商银行Type1格式解析完成: %d条记录", len(transactions))
            return self._standardize_output(transactions)

        except (IndexError, ValueError) as e:
            logger.error("解析Type1格式失败: %s", e)
            return pd.DataFrame()

    def _parse_type2_format(self, df: pd.DataFrame, file_path: str) -> pd.DataFrame:
        """解析Type2格式 (历史明细)"""
        try:
            transactions = []

            # 寻找数据开始行
            data_start_row = -1
            for i in range(min(10, len(df))):
                row_content = ""
                for j in range(len(df.columns)):
                    cell = df.iloc[i, j]
                    if not pd.isna(cell):
                        row_content += str(cell) + " "

                if "交易日期" in row_content and "储种" in row_content:
                    data_start_row = i + 1
                    break

            if data_start_row == -1:
                logger.warning("未找到工商银行Type2数据开始行: %s", file_path)
                return pd.DataFrame()

            # 解析交易记录
            for i in range(data_start_row, len(df)):
                row = df.iloc[i]

                # 检查是否为有效的交易行
                if self._is_valid_type2_transaction_row(row):
                    transaction = self._parse_type2_transaction_row(row)
                    if transaction:
                        transactions.append(transaction)

            logger.info("工商银行Type2格式解析完成: %d条记录", len(transactions))
            return self._standardize_output(transactions)

        except (IndexError, ValueError) as e:
            logger.error("解析Type2格式失败: %s", e)
            return pd.DataFrame()

    def _is_valid_type1_transaction_row(self, row: pd.Series) -> bool:
        """判断是否为有效的Type1交易行"""
        try:
            # 检查是否至少有3个非空列
            non_empty_count = sum(1 for val in row if not pd.isna(val) and str(val).strip() != "")
            if non_empty_count < 3:
                return False

            # 检查是否包含时间信息
            for val in row:
                if pd.isna(val):
                    continue
                cell_str = str(val).strip()
                if re.match(r'\d{4}-\d{2}-\d{2}', cell_str):
                    return True

            return False

        except (IndexError, ValueError, TypeError):
            return False

    def _is_valid_type2_transaction_row(self, row: pd.Series) -> bool:
        """判断是否为有效的Type2交易行"""
        try:
            # 检查是否至少有4个非空列
            non_empty_count = sum(1 for val in row if not pd.isna(val) and str(val).strip() != "")
            if non_empty_count < 4:
                return False

            # 检查前三列中是否有日期格式（工商银行日期通常在第2列）
            for i in range(min(3, len(row))):
                cell = row.iloc[i] if i < len(row) and not pd.isna(row.iloc[i]) else ""
                cell_str = str(cell).strip()
                if re.match(r'\d{4}-\d{2}-\d{2}', cell_str):
                    return True

            return False

        except (IndexError, ValueError, TypeError):
            return False

    def _parse_type1_transaction_row(self, row: pd.Series) -> Optional[Dict[str, Any]]:
        """解析Type1交易行"""
        try:
            transaction = {}

            # 提取日期时间
            datetime_found = self._extract_datetime_type1(row, transaction)
            if not datetime_found:
                return None

            # 提取商品说明
            self._extract_description_type1(row, transaction)

            # 提取金额
            self._extract_amount_type1(row, transaction)

            # 提取交易对方
            self._extract_counterparty_type1(transaction)

            return transaction

        except (IndexError, ValueError, TypeError) as e:
            logger.debug("解析Type1交易行失败: %s", e)
            return None

    def _extract_datetime_type1(self, row: pd.Series, transaction: dict) -> bool:
        """提取Type1的日期时间"""
        for col_idx in range(min(6, len(row))):
            if pd.isna(row.iloc[col_idx]):
                continue

            cell_str = str(row.iloc[col_idx]).strip()
            if re.match(r'\d{4}-\d{2}-\d{2}', cell_str):
                transaction['日期'] = cell_str
                return True
        return False

    def _extract_description_type1(self, row: pd.Series, transaction: dict) -> None:
        """提取Type1的商品说明"""
        for col_idx in range(min(8, len(row))):
            if pd.isna(row.iloc[col_idx]):
                continue

            cell_str = str(row.iloc[col_idx]).strip()
            if (len(cell_str) > 2 and
                not re.match(r'^\d+\.?\d*$', cell_str) and
                not re.match(r'\d{4}-\d{2}-\d{2}', cell_str)):
                transaction['商品说明'] = cell_str
                return

        transaction['商品说明'] = ""

    def _extract_amount_type1(self, row: pd.Series, transaction: dict) -> None:
        """提取Type1的金额"""
        amount = 0.0
        for col_idx in range(6, min(len(row), 10)):
            if pd.isna(row.iloc[col_idx]):
                continue

            try:
                amount_str = str(row.iloc[col_idx])
                amount = self._clean_amount_string(amount_str)
                if abs(amount) > 0.01:  # 找到有效金额
                    break
            except (ValueError, TypeError):
                continue

        # 根据交易类型或摘要判断收支类型
        description = transaction.get('商品说明', '')

        # 支出类型关键词（这些通常是支出）
        expense_keywords = ['转账', '消费', '取现', '提取', '购买', '付款', '缴费', '支付', '汇款转出']
        # 收入类型关键词
        income_keywords = ['存入', '转入', '工资', '利息', '股息', '分红', '退款', '汇款转入', '收款']

        # 根据关键词判断收支类型
        is_expense = any(keyword in description for keyword in expense_keywords)
        is_income = any(keyword in description for keyword in income_keywords)

        # 如果无法从关键词判断，则根据金额符号判断（正数为收入，负数为支出）
        if not is_expense and not is_income:
            # 默认情况下，正金额为收入，负金额为支出
            pass
        elif is_expense and amount > 0:
            # 如果是支出类型但金额为正，转为负数
            amount = -abs(amount)
        elif is_income and amount < 0:
            # 如果是收入类型但金额为负，转为正数
            amount = abs(amount)

        transaction['金额'] = amount

    def _extract_counterparty_type1(self, transaction: dict) -> None:
        """提取Type1的交易对方"""
        transaction['交易对方'] = ""

        # 从摘要中提取商户信息
        if '商品说明' in transaction:
            desc = transaction['商品说明']
            if ':' in desc:
                parts = desc.split(':', 1)
                if len(parts) > 1:
                    transaction['交易对方'] = parts[1].strip()

    def _parse_type2_transaction_row(self, row: pd.Series) -> Optional[Dict[str, Any]]:
        """解析Type2交易行"""
        try:
            transaction = {}

            # 提取日期时间 (第2列，索引1)
            date_found = False
            for col_idx in range(min(3, len(row))):
                if pd.isna(row.iloc[col_idx]):
                    continue

                cell_str = str(row.iloc[col_idx]).strip()
                if re.match(r'\d{4}-\d{2}-\d{2}', cell_str):
                    transaction['日期'] = self._parse_datetime(cell_str)
                    date_found = True
                    break

            if not date_found:
                return None

            # 提取摘要信息 (第9列开始是摘要、地区等)
            description_parts = []

            # 摘要通常在第9列 (索引8)
            if len(row) > 8 and not pd.isna(row.iloc[8]):
                desc = str(row.iloc[8]).strip()
                if desc and desc not in ['活期', '人民币', '钞']:
                    description_parts.append(desc)

            # 对方户名在第13列 (索引12)
            if len(row) > 12 and not pd.isna(row.iloc[12]):
                counterparty = str(row.iloc[12]).strip()
                if counterparty and counterparty != '（空）':
                    transaction['交易对方'] = counterparty

            transaction['商品说明'] = description_parts[0] if description_parts else "转账"

            # 提取金额 - 收入/支出金额在第11列 (索引10)
            amount = 0.0
            if len(row) > 10 and not pd.isna(row.iloc[10]):
                try:
                    amount = self._clean_amount_string(str(row.iloc[10]))
                except (ValueError, TypeError):
                    pass

            # 根据交易类型或摘要判断收支类型
            description = transaction.get('商品说明', '')

            # 支出类型关键词（这些通常是支出）
            expense_keywords = ['转账', '消费', '取现', '提取', '购买', '付款', '缴费', '支付', '汇款转出', '支取']
            # 收入类型关键词
            income_keywords = ['存入', '转入', '工资', '利息', '股息', '分红', '退款', '汇款转入', '收款', '存款']

            # 根据关键词判断收支类型
            is_expense = any(keyword in description for keyword in expense_keywords)
            is_income = any(keyword in description for keyword in income_keywords)

            # 如果无法从关键词判断，则根据金额符号判断（正数为收入，负数为支出）
            if not is_expense and not is_income:
                # 默认情况下，正金额为收入，负金额为支出
                pass
            elif is_expense and amount > 0:
                # 如果是支出类型但金额为正，转为负数
                amount = -abs(amount)
            elif is_income and amount < 0:
                # 如果是收入类型但金额为负，转为正数
                amount = abs(amount)

            transaction['金额'] = amount

            if '交易对方' not in transaction:
                transaction['交易对方'] = ""

            return transaction if amount != 0 else None

        except (IndexError, ValueError, TypeError) as e:
            logger.debug("解析Type2交易行失败: %s", e)
            return None
