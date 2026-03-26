#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
民生银行账单解析器
重构版本 - 提供精确的民生银行账单解析功能
"""

import logging
import re
from datetime import datetime
from typing import Dict, Any, List, Optional

import pandas as pd

from .base import BaseBankParser

logger = logging.getLogger(__name__)


class CMBCParser(BaseBankParser):
    """民生银行账单解析器"""

    def __init__(self):
        super().__init__()
        self.bank_name = "民生银行"
        self.logger = logging.getLogger(f"{__name__}.{self.__class__.__name__}")

    def can_parse(self, file_path: str) -> bool:
        """判断是否为民生银行账单"""
        try:
            if not file_path.endswith((".xlsx", ".xls", ".csv")):
                return False

            # 尝试读取文件内容进行判断
            try:
                if file_path.endswith(".csv"):
                    # CSV文件，尝试多种编码
                    for encoding in ["gbk", "utf-8", "gb2312"]:
                        try:
                            df = pd.read_csv(file_path, encoding=encoding, nrows=15)
                            break
                        except UnicodeDecodeError:
                            continue
                    else:
                        return False
                else:
                    # Excel文件，也要检查HTML格式
                    df = None

                    # 先尝试作为HTML表格读取（新格式）
                    try:
                        tables = pd.read_html(file_path, encoding="utf-8")
                        if tables and len(tables) > 0:
                            df = tables[0]  # 取第一个表格
                    except Exception:
                        pass

                    # 如果HTML读取失败，尝试Excel读取（旧格式）
                    if df is None or df.empty:
                        df = pd.read_excel(file_path, nrows=15, header=None)

            except Exception:
                return False

            # 检查内容是否包含民生银行特征
            content = ""
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 民生银行强特征标识 - 必须包含至少一个
            cmbc_strong_indicators = ["中国民生银行", "民生银行股份有限公司", "个人账户对账单"]

            # 民生银行特有的字段组合（包括新格式字段）
            cmbc_specific_fields = [
                "凭证类型",
                "凭证号码",
                "客户姓名",
                "客户账号",
                "账户账号",
                "支出金额",
                "存入金额",
                "对方账号",
                "对方名称",
                "对方开户行",
            ]

            # 排除其他银行的强特征 - 只检查前3行避免交易记录中的银行名干扰
            other_bank_title_indicators = [
                "账⼾活期交易明细清单"  # 中国农业银行特有标题
            ]

            # 检查前3行是否包含其他银行的标题性特征
            title_content = ""
            for i in range(min(3, len(df))):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if pd.notna(df.iloc[i, j]) else ""
                    title_content += cell_value + " "

            # 检查是否包含民生银行强标识
            has_cmbc_strong = any(indicator in content for indicator in cmbc_strong_indicators)

            # 检查民生银行特有字段（新格式需要更少的匹配数）
            field_matches = sum(1 for field in cmbc_specific_fields if field in content)
            has_cmbc_fields = field_matches >= 3

            # 新格式特殊检查：同时包含"支出金额"和"存入金额"
            has_new_format = "支出金额" in content and "存入金额" in content and "交易时间" in content

            # 检查标题区域是否包含其他银行强标识
            has_other_strong = any(indicator in title_content for indicator in other_bank_title_indicators)

            # 民生银行判定逻辑：必须有强标识或特有字段组合或新格式特征，且不能有其他银行强标识
            is_cmbc = (has_cmbc_strong or has_cmbc_fields or has_new_format) and not has_other_strong

            if is_cmbc:
                self.logger.info(f"识别为民生银行文件: {file_path}")

            return is_cmbc

        except Exception as e:
            self.logger.debug(f"判断民生银行文件失败: {file_path}, 错误: {e}")
            return False

    def get_bank_name(self) -> str:
        """获取银行名称"""
        return self.bank_name

    def parse(self, file_path: str) -> pd.DataFrame:
        """解析民生银行账单"""
        try:
            self.logger.info(f"开始解析民生银行账单: {file_path}")

            # 读取文件
            df = self._read_file(file_path)
            if df is None or df.empty:
                self.logger.warning(f"无法读取文件或文件为空: {file_path}")
                return pd.DataFrame()

            # 寻找数据开始行
            header_keywords = [
                ["交易日期", "记账日期", "交易时间", "摘要", "交易金额", "余额"],
                ["日期", "时间", "摘要", "金额", "余额"],
                ["交易日期", "摘要", "支出", "收入", "余额"],
                ["日期", "摘要", "借方发生额", "贷方发生额", "余额"],
                ["交易时间", "交易类型", "交易金额", "账户余额", "交易摘要"],
                ["交易时间", "支出金额", "存入金额", "账户余额", "摘要"],  # 新格式特征
                ["交易时间", "支出金额", "存入金额", "对方账号", "对方名称"],  # 新格式特征
            ]

            data_start_row = -1
            for row_idx in range(len(df)):
                row_content = ""
                for col_idx in range(len(df.columns)):
                    cell_value = str(df.iloc[row_idx, col_idx]) if not pd.isna(df.iloc[row_idx, col_idx]) else ""
                    row_content += cell_value + " "

                # 检查是否包含表头关键字
                for keywords in header_keywords:
                    matches = sum(1 for keyword in keywords if keyword in row_content)
                    if matches >= 3:  # 至少包含3个关键字
                        self.logger.debug(f"找到数据开始行 {row_idx}: {row_content[:100]}...")
                        data_start_row = row_idx
                        break

                if data_start_row != -1:
                    break
            if data_start_row == -1:
                self.logger.warning(f"未找到民生银行数据开始行: {file_path}")
                return pd.DataFrame()

            # 提取列名
            headers = self._extract_headers(df, data_start_row)
            if not headers:
                self.logger.warning(f"无法提取有效列名: {file_path}")
                return pd.DataFrame()

            # 解析交易记录
            transactions = self._parse_transactions(df, data_start_row + 1, headers)

            if not transactions:
                self.logger.warning(f"未找到有效交易记录: {file_path}")
                return pd.DataFrame()

            # 转换为DataFrame
            result_df = pd.DataFrame(transactions)

            # 标准化输出格式
            transactions_list = transactions  # transactions已经是列表格式
            result_df = self._standardize_output(transactions_list)

            self.logger.info(f"民生银行账单解析完成: {len(result_df)}条记录")
            return result_df

        except Exception as e:
            self.logger.error(f"解析民生银行账单失败: {file_path}, 错误: {e}")
            return pd.DataFrame()

    def _read_file(self, file_path: str) -> Optional[pd.DataFrame]:
        """读取文件"""
        try:
            if file_path.endswith(".csv"):
                # CSV文件，尝试多种编码
                for encoding in ["gbk", "utf-8", "gb2312", "utf-8-sig"]:
                    try:
                        df = pd.read_csv(file_path, encoding=encoding, header=None)
                        self.logger.debug(f"成功使用{encoding}编码读取CSV文件")
                        return df
                    except UnicodeDecodeError:
                        continue
                self.logger.error(f"无法读取CSV文件，尝试了多种编码: {file_path}")
                return None
            else:
                # Excel文件，首先尝试HTML格式（新格式）
                try:
                    tables = pd.read_html(file_path, encoding="utf-8")
                    if tables and len(tables) > 0:
                        self.logger.debug(f"成功以HTML格式读取文件: {file_path}")
                        return tables[0]  # 返回第一个表格
                except Exception as e:
                    self.logger.debug(f"HTML格式读取失败，尝试Excel格式: {e}")

                # 尝试Excel格式读取（旧格式）
                try:
                    df = pd.read_excel(file_path, header=None)
                    self.logger.debug(f"成功以Excel格式读取文件: {file_path}")
                    return df
                except Exception as e:
                    # 尝试指定引擎
                    try:
                        df = pd.read_excel(file_path, header=None, engine="openpyxl")
                        self.logger.debug(f"成功使用openpyxl引擎读取文件: {file_path}")
                        return df
                    except Exception:
                        self.logger.error(f"无法读取Excel文件: {file_path}, 错误: {e}")
                        return None

        except Exception as e:
            self.logger.error(f"读取文件失败: {file_path}, 错误: {e}")
            return None

    def _extract_headers(self, df: pd.DataFrame, header_row: int) -> List[str]:
        """提取列名"""
        headers = []
        if header_row < len(df):
            for col_idx in range(len(df.columns)):
                cell_value = str(df.iloc[header_row, col_idx]) if not pd.isna(df.iloc[header_row, col_idx]) else ""
                headers.append(cell_value.strip())
        return headers

    def _parse_transactions(self, df: pd.DataFrame, start_row: int, headers: List[str]) -> List[Dict[str, Any]]:
        """解析交易记录"""
        transactions = []

        for row_idx in range(start_row, len(df)):
            row = df.iloc[row_idx]

            # 检查是否为有效的交易行
            if not self._is_valid_transaction_row(row):
                continue

            transaction = self._parse_transaction_row(row, headers)
            if transaction:
                transactions.append(transaction)

        return transactions

    def _is_valid_transaction_row(self, row: pd.Series) -> bool:
        """判断是否为有效的交易行"""
        try:
            # 检查是否至少有3个非空列
            non_empty_count = sum(1 for val in row if not pd.isna(val) and str(val).strip() != "")
            if non_empty_count < 3:
                return False

            # 检查是否包含日期信息（包括新格式的制表符分割时间）
            date_found = False
            for val in row:
                if pd.isna(val):
                    continue
                cell_str = str(val).strip()
                # 匹配各种日期格式，包括新格式的制表符时间
                if (
                    re.match(r"\d{4}[-/]\d{2}[-/]\d{2}", cell_str)
                    or re.match(r"\d{8}", cell_str)
                    or re.match(r"\d{4}\.\d{2}\.\d{2}", cell_str)
                    or re.match(r"\d{8}\t\d{2}:\d{2}:\d{2}", cell_str)
                ):  # 新格式时间
                    date_found = True
                    break

            # 检查是否包含数字金额
            amount_found = False
            for val in row:
                if pd.isna(val):
                    continue
                try:
                    float_val = float(val)
                    if abs(float_val) > 0.01:  # 金额大于1分钱
                        amount_found = True
                        break
                except ValueError, TypeError:
                    # 尝试清理字符串后转换
                    try:
                        cell_str = str(val).strip()
                        # 先排除日期时间格式，避免误判
                        if (
                            re.match(r"\d{4}[-/]\d{2}[-/]\d{2}", cell_str)
                            or re.match(r"\d{8}$", cell_str)
                            or re.match(r"\d{4}\.\d{2}\.\d{2}", cell_str)
                            or re.match(r"\d{8}\s+\d{2}:\d{2}:\d{2}", cell_str)
                            or re.match(r"\d{8}\t\d{2}:\d{2}:\d{2}", cell_str)
                        ):
                            continue  # 跳过日期时间格式

                        cleaned = re.sub(r"[^\d.-]", "", cell_str)
                        if cleaned and len(cleaned) <= 10:  # 避免过长的数字（如日期时间）
                            float_val = float(cleaned)
                            if abs(float_val) > 0.01:
                                amount_found = True
                                break
                    except ValueError, TypeError:
                        continue

            return date_found and amount_found

        except Exception:
            return False

    def _parse_transaction_row(self, row: pd.Series, headers: List[str]) -> Optional[Dict[str, Any]]:
        """解析交易行"""
        try:
            transaction = {"日期": "", "商品说明": "", "交易对方": "", "金额": 0.0}

            # 解析日期
            date_str = self._extract_date(row)
            if date_str:
                # 使用基类的时间解析方法，确保时间格式正确
                transaction["日期"] = self._parse_datetime(date_str)
            else:
                return None  # 没有日期的记录无效

            # 解析商品说明/摘要
            description = self._extract_description(row, headers)
            transaction["商品说明"] = description if description else "未知交易"

            # 解析金额
            amount = self._extract_amount(row, headers)
            transaction["金额"] = amount

            # 解析交易对方（如果有的话）
            counterparty = self._extract_counterparty(row, headers)
            transaction["交易对方"] = counterparty if counterparty else ""

            return transaction

        except Exception as e:
            self.logger.debug(f"解析民生银行交易行失败: {e}")
            return None

    def _extract_date(self, row: pd.Series) -> Optional[str]:
        """提取日期"""
        for val in row:
            if pd.isna(val):
                continue
            cell_str = str(val).strip()

            # 新格式：处理包含制表符的时间格式 "YYYYMMDD\tHH:MM:SS"
            if "\t" in cell_str and re.match(r"\d{8}\t\d{2}:\d{2}:\d{2}", cell_str):
                parts = cell_str.split("\t", maxsplit=1)
                date_part = parts[0]  # 日期部分
                time_part = parts[1] if len(parts) > 1 else ""  # 时间部分
                try:
                    year = date_part[:4]
                    month = date_part[4:6]
                    day = date_part[6:8]
                    # 验证日期有效性
                    datetime.strptime(f"{year}-{month}-{day}", "%Y-%m-%d")
                    # 返回完整的日期时间格式
                    return f"{year}-{month}-{day} {time_part}" if time_part else f"{year}-{month}-{day} 00:00:00"
                except ValueError:
                    continue

            # 新格式：处理包含空格的时间格式 "YYYYMMDD HH:MM:SS"
            if " " in cell_str and re.match(r"\d{8}\s+\d{2}:\d{2}:\d{2}", cell_str):
                parts = cell_str.split(maxsplit=1)
                date_part = parts[0]  # 日期部分
                time_part = parts[1] if len(parts) > 1 else ""  # 时间部分
                try:
                    year = date_part[:4]
                    month = date_part[4:6]
                    day = date_part[6:8]
                    # 验证日期有效性
                    datetime.strptime(f"{year}-{month}-{day}", "%Y-%m-%d")
                    # 返回完整的日期时间格式
                    return f"{year}-{month}-{day} {time_part}" if time_part else f"{year}-{month}-{day} 00:00:00"
                except ValueError:
                    continue

            # 标准日期格式 YYYY-MM-DD 或 YYYY/MM/DD
            if re.match(r"\d{4}[-/]\d{2}[-/]\d{2}", cell_str):
                # 检查是否已经包含时间
                if " " in cell_str:
                    return cell_str.replace("/", "-")
                else:
                    return cell_str.replace("/", "-") + " 00:00:00"

            # 紧凑格式 YYYYMMDD
            if re.match(r"\d{8}", cell_str) and len(cell_str) == 8:
                try:
                    year = cell_str[:4]
                    month = cell_str[4:6]
                    day = cell_str[6:8]
                    # 验证日期有效性
                    datetime.strptime(f"{year}-{month}-{day}", "%Y-%m-%d")
                    return f"{year}-{month}-{day} 00:00:00"
                except ValueError:
                    continue

            # 点分格式 YYYY.MM.DD
            if re.match(r"\d{4}\.\d{2}\.\d{2}", cell_str):
                # 检查是否已经包含时间
                if " " in cell_str:
                    return cell_str.replace(".", "-")
                else:
                    return cell_str.replace(".", "-") + " 00:00:00"

        return None

    def _extract_description(self, row: pd.Series, headers: List[str]) -> Optional[str]:
        """提取商品说明/摘要"""
        # 新格式优先查找摘要列（可能有两个摘要列，取第一个非空的）
        summary_keywords = ["摘要", "交易摘要", "商品说明", "说明", "用途", "备注", "交易方式"]

        descriptions = []
        for i, header in enumerate(headers):
            if any(keyword in header for keyword in summary_keywords):
                if i < len(row) and not pd.isna(row.iloc[i]):
                    desc = str(row.iloc[i]).strip()
                    if len(desc) > 1 and not re.match(r"^\d+[-/.]?\d*$", desc):
                        descriptions.append(desc)

        # 如果找到多个描述，选择最长且非重复的
        if descriptions:
            # 去重并按长度排序
            unique_descriptions = list(set(descriptions))
            unique_descriptions.sort(key=len, reverse=True)
            return unique_descriptions[0]

        # 如果没找到特定列，寻找最可能的描述文本
        for val in row:
            if pd.isna(val):
                continue
            cell_str = str(val).strip()

            # 跳过日期、纯数字、制表符分割的时间等
            if (
                len(cell_str) > 2
                and not re.match(r"^\d+[-/.]?\d*$", cell_str)
                and not re.match(r"\d{4}[-/.]\d{2}[-/.]\d{2}", cell_str)
                and not re.match(r"^\d{8}$", cell_str)
                and not re.match(r"\d{8}\t\d{2}:\d{2}:\d{2}", cell_str)
            ):  # 排除新格式时间
                # 排除一些常见的非描述内容
                if cell_str not in ["收入", "支出", "借", "贷", "成功", "失败"]:
                    return cell_str

        return None

    def _extract_amount(self, row: pd.Series, headers: List[str]) -> float:
        """提取金额"""
        # 新格式：优先处理支出金额和存入金额分离的情况
        debit_amount = 0.0  # 支出金额（负数）
        credit_amount = 0.0  # 存入金额（正数）

        # 查找支出金额和存入金额列
        for i, header in enumerate(headers):
            if i >= len(row):
                continue

            cell_value = row.iloc[i] if not pd.isna(row.iloc[i]) else ""

            if "支出金额" in header or "支出" in header:
                try:
                    if str(cell_value).strip():  # 非空值
                        debit_amount = -abs(self._clean_amount_string(str(cell_value)))
                except ValueError, TypeError:
                    pass
            elif "存入金额" in header or "收入金额" in header or "存入" in header:
                try:
                    if str(cell_value).strip():  # 非空值
                        credit_amount = abs(self._clean_amount_string(str(cell_value)))
                except ValueError, TypeError:
                    pass

        # 如果找到了分离的支出/存入金额，返回非零的那个
        if debit_amount != 0.0:
            return debit_amount
        if credit_amount != 0.0:
            return credit_amount

        # 原有逻辑：查找其他金额相关列
        amount_keywords = ["金额", "交易金额", "发生额", "借方", "贷方"]

        amounts = []

        for i, header in enumerate(headers):
            if any(keyword in header for keyword in amount_keywords):
                if i < len(row) and not pd.isna(row.iloc[i]):
                    try:
                        amount = self._clean_amount_string(str(row.iloc[i]))
                        if abs(amount) > 0:
                            # 根据列名判断正负
                            if "支出" in header or "借方" in header:
                                amounts.append(-abs(amount))
                            elif "收入" in header or "贷方" in header:
                                amounts.append(abs(amount))
                            else:
                                amounts.append(amount)
                    except ValueError, TypeError:
                        continue

        # 如果找到了特定列的金额，返回绝对值最大的
        if amounts:
            return max(amounts, key=abs)

        # 否则寻找行中的数字金额
        for val in row:
            if pd.isna(val):
                continue
            try:
                amount = self._clean_amount_string(str(val))
                if abs(amount) > 0.01:  # 大于1分钱
                    amounts.append(amount)
            except ValueError, TypeError:
                continue

        # 返回绝对值最大的金额
        return max(amounts, key=abs) if amounts else 0.0

    def _extract_counterparty(self, row: pd.Series, headers: List[str]) -> Optional[str]:
        """提取交易对方"""
        # 新格式：优先查找对方名称列，避免匹配到对方账号列
        # 按优先级排序，精确匹配优先
        counterparty_priority_keywords = [
            "对方名称",  # 最优先：对方名称
            "对方户名",  # 次优先：对方户名
            "交易对方",  # 第三：交易对方
            "收款方",  # 第四：收款方
            "付款方",  # 第五：付款方
        ]

        # 首先尝试精确匹配优先级关键词
        for keyword in counterparty_priority_keywords:
            for i, header in enumerate(headers):
                if keyword == header.strip():  # 精确匹配
                    if i < len(row) and not pd.isna(row.iloc[i]):
                        counterparty = str(row.iloc[i]).strip()
                        if len(counterparty) > 1:
                            return counterparty

        # 如果精确匹配失败，尝试包含匹配，但排除账号相关列
        for keyword in counterparty_priority_keywords:
            for i, header in enumerate(headers):
                header_clean = header.strip()
                if (
                    keyword in header_clean and "账号" not in header_clean and "账户" not in header_clean
                ):  # 排除账号/账户列
                    if i < len(row) and not pd.isna(row.iloc[i]):
                        counterparty = str(row.iloc[i]).strip()
                        if len(counterparty) > 1:
                            return counterparty

        return None
