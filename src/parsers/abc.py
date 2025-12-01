"""
ABC Parser - 农业银行账单解析器

解析中国农业银行账单导出的 CSV/Excel 文件。

v6.59 更新:
- 将"交易摘要"列聚合进描述字段
- 使用 | 分隔符聚合多个描述字段（交易用途 + 交易摘要）
- 增强日志记录
- 重构代码降低分支复杂度

输出标准格式:
- date: 交易时间 (YYYY-MM-DD HH:MM:SS)
- amount: 金额 (支出为负, 收入为正)
- type: 类型 (收入/支出)
- description: 聚合描述 (交易用途 | 交易摘要)
- source_account_id: 'abc'
"""

import csv
from typing import Dict, List, Any, Optional, Tuple

import pandas as pd

from .base import ParserBase
from ..utils.logger import log_method


class ABCParser(ParserBase):
    """农业银行账单解析器"""

    # 解析器标识符
    PARSER_ID = "abc"
    PARSER_NAME = "农业银行"

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = ['.csv', '.xlsx', '.xls']
        self.logger.info("农业银行账单解析器已初始化 [ID=%s]", self.PARSER_ID)

    def _merge_description_fields(self, *fields) -> str:
        """
        v6.59: 聚合多个描述字段为单一描述

        使用 | 分隔符连接非空字段，并去除重复片段

        Args:
            *fields: 可变数量的描述字段

        Returns:
            str: 聚合后的描述字符串
        """
        valid_parts = []
        seen = set()

        for field in fields:
            if field and str(field).strip() and str(field).strip().lower() != 'nan':
                part = str(field).strip()
                if part.lower() not in seen:
                    seen.add(part.lower())
                    valid_parts.append(part)

        result = ' | '.join(valid_parts)
        if len(valid_parts) > 1:
            self.logger.debug("[描述聚合] 合并 %d 个字段: %s", len(valid_parts), result[:100])
        return result

    def _get_date_field(self, row_dict: Dict[str, str]) -> str:
        """获取日期字段，合并日期和时间列"""
        date_field = (row_dict.get('交易日期') or row_dict.get('交易⽇期') or
                      row_dict.get('记账日期') or '')
        if not date_field.strip() or date_field == 'nan':
            return ''

        time_field = row_dict.get('交易时间') or ''
        if time_field.strip() and time_field.strip() != 'nan':
            date_field = f"{date_field.strip()} {time_field.strip()}"

        return date_field.strip()

    def _get_amount_and_type(self, row_dict: Dict[str, str]) -> Tuple[str, str]:
        """获取金额和交易类型"""
        amount_str = '0'
        transaction_type = '支出'

        # 尝试收入/支出分列格式
        income = (row_dict.get('收入金额') or row_dict.get('转入金额') or
                  row_dict.get('收⼊⾦额') or '')
        expense = (row_dict.get('支出金额') or row_dict.get('转出金额') or
                   row_dict.get('⽀出⾦额') or '')

        if income and income != 'nan' and income.strip():
            try:
                income_val = float(income.replace(',', ''))
                if income_val > 0:
                    return str(income_val), '收入'
            except ValueError:
                pass

        if expense and expense != 'nan' and expense.strip():
            try:
                expense_val = float(expense.replace(',', ''))
                if expense_val > 0:
                    return str(expense_val), '支出'
            except ValueError:
                pass

        # 尝试单列格式
        single_amount = (row_dict.get('交易金额') or row_dict.get('交易⾦额') or
                         row_dict.get('金额') or '')
        if single_amount and single_amount != 'nan':
            try:
                val = float(single_amount.replace(',', ''))
                return str(abs(val)), '收入' if val > 0 else '支出'
            except ValueError:
                pass

        return amount_str, transaction_type

    def _get_description(self, row_dict: Dict[str, str]) -> str:
        """获取聚合描述（交易用途 + 交易摘要）"""
        transaction_purpose = (row_dict.get('交易用途') or row_dict.get('用途') or
                               row_dict.get('摘要') or row_dict.get('交易附⾔') or '')
        transaction_summary = row_dict.get('交易摘要') or ''
        return self._merge_description_fields(transaction_purpose, transaction_summary)

    def _get_counterparty(self, row_dict: Dict[str, str]) -> str:
        """获取交易对方"""
        counterparty = (row_dict.get('对方户名') or row_dict.get('对方名称') or
                        row_dict.get('对⼿信息') or row_dict.get('对方账号') or '')
        return counterparty if counterparty != 'nan' else ''

    def _parse_csv_row(self, row: Dict[str, str]) -> Optional[Dict[str, Any]]:
        """解析CSV行数据"""
        date_field = self._get_date_field(row)
        if not date_field:
            return None

        amount_str, transaction_type = self._get_amount_and_type(row)
        if not amount_str or amount_str == '0' or amount_str == 'nan':
            return None

        return {
            'date': date_field,
            'type': transaction_type,
            'counterparty': self._get_counterparty(row),
            'description': self._get_description(row),
            'amount': amount_str,
            'channel': '农业银行'
        }

    def _parse_csv(self, file_path: str) -> List[Dict[str, Any]]:
        """解析CSV格式账单"""
        bills = []

        with open(file_path, 'r', encoding='gbk') as f:
            lines = f.readlines()

        data_start = 0
        for i, line in enumerate(lines):
            if '交易日期' in line or '记账日期' in line or '交易⽇期' in line:
                data_start = i
                break

        if data_start == 0:
            self.logger.warning("未找到CSV数据起始行")
            return []

        reader = csv.DictReader(lines[data_start:])

        for row in reader:
            try:
                bill = self._parse_csv_row(row)
                if bill:
                    bills.append(bill)
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("解析CSV行数据失败: %s", e)

        return bills

    def _find_header_row(self, df: pd.DataFrame) -> int:
        """查找Excel表头行"""
        for i in range(min(15, len(df))):
            row_values = [str(df.iloc[i, j]) for j in range(len(df.columns))
                          if not pd.isna(df.iloc[i, j])]
            row_text = ' '.join(row_values)
            if '交易日期' in row_text or '交易⽇期' in row_text or '记账日期' in row_text:
                return i
        return -1

    def _parse_excel_row(self, row_dict: Dict[str, str]) -> Optional[Dict[str, Any]]:
        """解析Excel行数据"""
        date_field = self._get_date_field(row_dict)
        if not date_field:
            return None

        amount_str, transaction_type = self._get_amount_and_type(row_dict)
        if not amount_str or amount_str == '0' or amount_str == 'nan':
            return None

        return {
            'date': date_field,
            'type': transaction_type,
            'counterparty': self._get_counterparty(row_dict),
            'description': self._get_description(row_dict),
            'amount': amount_str,
            'channel': '农业银行'
        }

    def _parse_excel(self, file_path: str) -> List[Dict[str, Any]]:
        """解析Excel格式账单"""
        bills = []
        df = pd.read_excel(file_path, header=None)

        header_row = self._find_header_row(df)
        if header_row == -1:
            self.logger.warning("未找到Excel表头行")
            return []

        # 提取列名
        headers = []
        for j in range(len(df.columns)):
            val = df.iloc[header_row, j]
            headers.append(str(val).strip() if not pd.isna(val) else f'col_{j}')

        # 解析数据行
        for i in range(header_row + 1, len(df)):
            try:
                row_dict = {}
                for j, header in enumerate(headers):
                    val = df.iloc[i, j]
                    row_dict[header] = str(val) if not pd.isna(val) else ''

                bill = self._parse_excel_row(row_dict)
                if bill:
                    bills.append(bill)
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("解析Excel行数据失败: %s", e)

        return bills

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为农业银行账单"""
        try:
            if not file_path.endswith(('.xlsx', '.xls', '.csv')):
                return False

            # 检查文件名（优先级最低）
            if 'abc' in file_path.lower() or '农业' in file_path:
                return True

            # 尝试读取文件内容
            try:
                if file_path.endswith('.csv'):
                    for encoding in ['gbk', 'utf-8', 'gb2312']:
                        try:
                            df = pd.read_csv(file_path, encoding=encoding, nrows=15)
                            break
                        except UnicodeDecodeError:
                            continue
                    else:
                        return False
                else:
                    df = pd.read_excel(file_path, header=None, nrows=15)
            except Exception:  # pylint: disable=broad-except
                return False

            # 检查前15行内容是否包含农业银行特征
            content = ""
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            return self._check_abc_content(content)

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug("判断农业银行文件失败: %s, 错误: %s", file_path, e)
            return False

    def _check_abc_content(self, content: str) -> bool:
        """检查内容是否符合农业银行特征"""
        # 农业银行强特征标识
        abc_strong_indicators = [
            "中国农业银行股份有限公司",
            "农业银⾏",
            "账⼾活期交易明细清单"
        ]

        # 农业银行特有的列名模式（旧格式：特殊编码）
        abc_old_format_columns = [
            "⼾名", "账⼾", "交易⽇期", "交易⾦额", "对⼿信息", "本次余额"
        ]

        # 农业银行特有的列名模式（新格式：正常编码）
        abc_new_format_columns = [
            ["交易日期", "交易时间", "交易金额"],
            ["对方户名", "交易用途", "本次余额"],
            ["账户明细查询", "活期明细"]
        ]

        # 排除其他银行的特征
        other_bank_indicators = [
            "支出金额", "存入金额", "凭证类型", "储种", "记账日"
        ]

        # 检查特征
        has_abc_strong = any(ind in content for ind in abc_strong_indicators)
        special_field_count = sum(1 for f in abc_old_format_columns if f in content)
        has_old_format = special_field_count >= 3
        has_new_format = any(all(c in content for c in p) for p in abc_new_format_columns)
        has_other_bank = any(ind in content for ind in other_bank_indicators)

        is_abc = (has_abc_strong or has_old_format or has_new_format) and not has_other_bank

        if is_abc:
            self.logger.info("内容符合农业银行特征")

        return is_abc

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析农业银行账单"""
        if not self.validate_file(file_path):
            return []

        self.logger.info("[ABC解析器] 开始解析文件: %s", file_path)

        try:
            if file_path.endswith('.csv'):
                bills = self._parse_csv(file_path)
            else:
                bills = self._parse_excel(file_path)

            self.logger.info("[ABC解析器] 农业银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("[ABC解析器] 解析农业银行账单失败: %s", e)
            return []

        return self.post_process(bills)
