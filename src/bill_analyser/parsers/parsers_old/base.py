#!/usr/bin/env python3

"""
银行账单解析器基类
"""

import logging
import re
from abc import ABC, abstractmethod
from datetime import datetime
from typing import Any

import pandas as pd

logger = logging.getLogger(__name__)


class BaseBankParser(ABC):
    """银行账单解析器基类"""

    def __init__(self):
        self.bank_name = ""
        self.supported_formats = [".xlsx", ".xls", ".csv"]

    @abstractmethod
    def can_parse(self, file_path: str) -> bool:
        """判断是否可以解析此文件"""
        raise NotImplementedError

    @abstractmethod
    def parse(self, file_path: str) -> pd.DataFrame:
        """解析银行账单文件"""
        raise NotImplementedError

    @abstractmethod
    def get_bank_name(self) -> str:
        """获取银行名称"""
        raise NotImplementedError

    def get_supported_formats(self) -> list[str]:
        """获取支持的文件格式"""
        return self.supported_formats

    def _read_excel_file(self, file_path: str, header=None) -> pd.DataFrame | None:
        """读取Excel文件"""
        try:
            return pd.read_excel(file_path, header=header)
        except (FileNotFoundError, PermissionError, ValueError) as e:
            logger.error("读取Excel文件失败: %s, 错误: %s", file_path, e)
            return None

    def _find_data_start_row(self, df: pd.DataFrame, indicators: list[str]) -> int:
        """查找数据开始行"""
        for i in range(len(df)):
            for j in range(len(df.columns)):
                cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                for indicator in indicators:
                    if indicator in cell_value:
                        return i + 1  # 数据从下一行开始
        return -1

    def _clean_amount_string(self, amount_str: str) -> float:
        """清理并转换金额字符串"""
        if pd.isna(amount_str) or amount_str == "":
            return 0.0

        # 转换为字符串并清理
        amount_str = str(amount_str).strip()

        # 移除逗号、空格等
        amount_str = re.sub(r"[,\s]", "", amount_str)

        # 处理负号
        is_negative = False
        if amount_str.startswith("-") or amount_str.startswith("−"):
            is_negative = True
            amount_str = amount_str[1:]

        # 提取数字
        match = re.search(r"\d+\.?\d*", amount_str)
        if match:
            try:
                amount = float(match.group())
                return -amount if is_negative else amount
            except ValueError:
                pass

        return 0.0

    def _parse_datetime(self, date_str: str, time_str: str = "") -> str:
        """解析日期时间"""
        try:
            date_str = str(date_str).strip()
            time_str = str(time_str).strip() if time_str else ""

            # 修复：处理包含换行符的日期时间字符串，如 "2024-01-17\n17:30:36"
            if "\n" in date_str:
                parts = date_str.split("\n")
                if len(parts) >= 2:
                    date_part = parts[0].strip()
                    time_part = parts[1].strip()
                    # 递归调用处理分离后的日期和时间
                    return self._parse_datetime(date_part, time_part)

            # 处理不同的日期格式
            if len(date_str) == 8 and date_str.isdigit():  # 20240101
                date_obj = datetime.strptime(date_str, "%Y%m%d")
            elif len(date_str) >= 10:  # 2024-01-01 或 2024-01-01 21:17:21
                if " " in date_str:
                    return date_str  # 已包含时间
                date_obj = datetime.strptime(date_str[:10], "%Y-%m-%d")
            else:
                return date_str  # 返回原始字符串

            # 添加时间部分
            if time_str and ":" in time_str:
                try:
                    if len(time_str) == 6 and time_str.isdigit():  # 151732
                        time_obj = datetime.strptime(time_str, "%H%M%S")
                        return f"{date_obj.strftime('%Y-%m-%d')} {time_obj.strftime('%H:%M:%S')}"
                    if ":" in time_str:
                        return f"{date_obj.strftime('%Y-%m-%d')} {time_str}"
                except ValueError:
                    pass

            # 修复：对于只有日期没有时间的情况，使用00:00:00而不是当前时间
            return date_obj.strftime("%Y-%m-%d") + " 00:00:00"

        except (ValueError, TypeError) as e:
            logger.debug("解析日期时间失败: %s, %s, 错误: %s", date_str, time_str, e)
            return str(date_str)

    def _standardize_output(self, transactions: list[dict[str, Any]]) -> pd.DataFrame:
        """标准化输出格式"""
        if not transactions:
            return pd.DataFrame()

        # 标准列名
        standard_columns = ["日期", "商品说明", "交易对方", "收支", "金额", "大类", "小类", "交易状态"]

        # 确保所有交易都有标准列
        standardized_transactions = []
        for transaction in transactions:
            # 创建新的标准化交易记录
            std_transaction = {}

            # 处理每个标准列
            for col in standard_columns:
                if col == "金额":
                    # 确保金额是数值类型，避免DataFrame多列赋值错误
                    amount_value = transaction.get("金额", 0)
                    if isinstance(amount_value, (list, tuple, pd.Series)):
                        # 如果是列表或Series，取第一个有效值
                        if len(amount_value) > 0:
                            amount_value = amount_value[0] if hasattr(amount_value, "__getitem__") else 0
                        else:
                            amount_value = 0
                    try:
                        std_transaction[col] = float(amount_value) if amount_value is not None else 0.0
                    except ValueError, TypeError:
                        std_transaction[col] = 0.0
                elif col == "大类":
                    std_transaction[col] = transaction.get(col, "其他")
                elif col == "小类":
                    std_transaction[col] = transaction.get(col, "其他")
                elif col == "收支":
                    if col in transaction:
                        std_transaction[col] = transaction[col]
                    else:
                        amount = std_transaction.get("金额", 0)
                        std_transaction[col] = "收入" if amount > 0 else "支出"
                elif col == "交易状态":
                    std_transaction[col] = transaction.get(col, "成功")  # 银行数据默认状态为成功
                else:
                    std_transaction[col] = transaction.get(col, "")

            standardized_transactions.append(std_transaction)

        # 创建DataFrame
        df = pd.DataFrame(standardized_transactions)

        # 再次确保金额列是数值型，并处理异常值
        if "金额" in df.columns and not df.empty:
            # 使用 .loc 避免 SettingWithCopyWarning
            df = df.copy()
            df.loc[:, "金额"] = pd.to_numeric(df["金额"], errors="coerce").fillna(0.0)

        # 过滤掉无效记录
        if not df.empty:
            df = df[df["金额"] != 0]

        return df[standard_columns] if not df.empty else pd.DataFrame(columns=standard_columns)
