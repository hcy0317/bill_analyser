"""
ABC Parser - 农业银行账单解析器

解析中国农业银行账单导出的 CSV/Excel 文件。
"""

import csv
import pandas as pd
from typing import Dict, List, Any

from .base import ParserBase
from ..utils.logger import log_method


class ABCParser(ParserBase):
    """农业银行账单解析器"""

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = ['.csv', '.xlsx', '.xls']

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
                    # CSV文件
                    for encoding in ['gbk', 'utf-8', 'gb2312']:
                        try:
                            df = pd.read_csv(file_path, encoding=encoding, nrows=15)
                            break
                        except UnicodeDecodeError:
                            continue
                    else:
                        return False
                else:
                    # Excel文件
                    df = pd.read_excel(file_path, header=None, nrows=15)
            except Exception:
                return False

            # 检查前15行内容是否包含农业银行特征
            content = ""
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 农业银行强特征标识
            abc_strong_indicators = [
                "中国农业银行股份有限公司",
                "农业银⾏",  # 特殊编码
                "账⼾活期交易明细清单"  # 特殊编码
            ]

            # 农业银行特有的列名模式（旧格式：特殊编码）
            abc_old_format_columns = [
                "⼾名",  # 特殊编码的"户"字
                "账⼾",  # 特殊编码的"户"字
                "交易⽇期",  # 特殊编码的"日"字
                "交易⾦额",  # 特殊编码的"金"字
                "对⼿信息",  # 特殊编码的"手"字
                "本次余额"
            ]

            # 农业银行特有的列名模式（新格式：正常编码）
            abc_new_format_columns = [
                ["交易日期", "交易时间", "交易金额"],
                ["对方户名", "交易用途", "本次余额"],
                ["账户明细查询", "活期明细"]
            ]

            # 排除其他银行的特征
            other_bank_indicators = [
                "支出金额",  # 民生银行
                "存入金额",  # 民生银行
                "凭证类型",  # 民生银行
                "储种",  # 工商银行
                "记账日"  # 建设银行
            ]

            # 检查强特征
            has_abc_strong = any(indicator in content for indicator in abc_strong_indicators)

            # 检查旧格式特殊编码字段（需要>=3个）
            special_field_count = sum(1 for field in abc_old_format_columns if field in content)
            has_old_format = special_field_count >= 3

            # 检查新格式列名组合（任一组合全匹配即可）
            has_new_format = any(all(col in content for col in pattern) for pattern in abc_new_format_columns)

            # 检查是否包含其他银行特征
            has_other_bank = any(indicator in content for indicator in other_bank_indicators)

            # 农业银行判定逻辑：
            # 1. 有强特征标识，或
            # 2. 有旧格式特殊编码字段（>=3个），或
            # 3. 有新格式列名组合
            # 4. 且不包含其他银行特征
            is_abc = (has_abc_strong or has_old_format or has_new_format) and not has_other_bank

            if is_abc:
                self.logger.info(f"识别为农业银行文件: {file_path}")

            return is_abc

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug(f"判断农业银行文件失败: {file_path}, 错误: {e}")
            return False

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析农业银行账单"""
        if not self.validate_file(file_path):
            return []

        bills = []

        try:
            # 读取文件
            if file_path.endswith('.csv'):
                # CSV格式
                with open(file_path, 'r', encoding='gbk') as f:
                    lines = f.readlines()

                    data_start = 0
                    for i, line in enumerate(lines):
                        if '交易日期' in line or '记账日期' in line or '交易⽇期' in line:
                            data_start = i
                            break

                    if data_start == 0:
                        self.logger.warning("未找到数据起始行")
                        return []

                    reader = csv.DictReader(lines[data_start:])

                    for row in reader:
                        try:
                            date_field = row.get('交易日期') or row.get('记账日期') or row.get('交易⽇期') or ''
                            if not date_field.strip():
                                continue

                            # 农业银行通常有收入和支出两列
                            income = row.get('收入金额') or row.get('转入金额') or '0'
                            expense = row.get('支出金额') or row.get('转出金额') or '0'

                            amount_str = '0'
                            transaction_type = '支出'

                            if float(income.replace(',', '')) > 0:
                                amount_str = income
                                transaction_type = '收入'
                            elif float(expense.replace(',', '')) > 0:
                                amount_str = expense
                                transaction_type = '支出'

                            bill = {
                                'date': date_field,
                                'type': transaction_type,
                                'counterparty': row.get('对方户名') or row.get('对方名称') or row.get('对⼿信息') or '',
                                'description': row.get('摘要') or row.get('用途') or row.get('交易用途') or '',
                                'amount': amount_str,
                                'channel': '农业银行'
                            }

                            bills.append(bill)

                        except Exception as e:  # pylint: disable=broad-except
                            self.logger.error("解析CSV行数据失败: %s", e)
            else:
                # Excel格式
                df = pd.read_excel(file_path, header=None)

                # 查找表头行（支持特殊编码）
                header_row = -1
                for i in range(min(15, len(df))):
                    row_text = ' '.join(str(df.iloc[i, j]) for j in range(len(df.columns)) if not pd.isna(df.iloc[i, j]))
                    # 检查是否包含"交易日期"或特殊编码的"交易⽇期"
                    if '交易日期' in row_text or '交易⽇期' in row_text or '记账日期' in row_text:
                        header_row = i
                        break

                if header_row == -1:
                    self.logger.warning("未找到表头行")
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

                        # 获取日期（支持多种列名）
                        date_field = (row_dict.get('交易日期') or row_dict.get('交易⽇期') or
                                    row_dict.get('记账日期') or row_dict.get('交易时间') or '')
                        if not date_field.strip() or date_field == 'nan':
                            continue

                        # 获取金额 - 农业银行可能有收入/支出分列
                        amount_str = '0'
                        transaction_type = '支出'

                        # 尝试收入/支出分列格式
                        income = row_dict.get('收入金额') or row_dict.get('转入金额') or row_dict.get('收⼊⾦额') or ''
                        expense = row_dict.get('支出金额') or row_dict.get('转出金额') or row_dict.get('⽀出⾦额') or ''

                        if income and income != 'nan' and income.strip():
                            try:
                                income_val = float(income.replace(',', ''))
                                if income_val > 0:
                                    amount_str = str(income_val)
                                    transaction_type = '收入'
                            except:
                                pass

                        if amount_str == '0' and expense and expense != 'nan' and expense.strip():
                            try:
                                expense_val = float(expense.replace(',', ''))
                                if expense_val > 0:
                                    amount_str = str(expense_val)
                                    transaction_type = '支出'
                            except:
                                pass

                        # 如果分列格式没找到，尝试单列格式
                        if amount_str == '0':
                            single_amount = row_dict.get('交易金额') or row_dict.get('交易⾦额') or row_dict.get('金额') or ''
                            if single_amount and single_amount != 'nan':
                                try:
                                    val = float(single_amount.replace(',', ''))
                                    amount_str = str(abs(val))
                                    transaction_type = '收入' if val > 0 else '支出'
                                except:
                                    pass

                        if not amount_str or amount_str == '0' or amount_str == 'nan':
                            continue

                        # 获取对方和描述
                        counterparty = (row_dict.get('对方户名') or row_dict.get('对方名称') or
                                      row_dict.get('对⼿信息') or row_dict.get('对方账号') or '')
                        description = (row_dict.get('摘要') or row_dict.get('用途') or
                                     row_dict.get('交易用途') or row_dict.get('交易附⾔') or '')

                        bill = {
                            'date': date_field.split()[0] if ' ' in date_field else date_field,
                            'type': transaction_type,
                            'counterparty': counterparty if counterparty != 'nan' else '',
                            'description': description if description != 'nan' else '',
                            'amount': amount_str,
                            'channel': '农业银行'
                        }

                        bills.append(bill)

                    except Exception as e:  # pylint: disable=broad-except
                        self.logger.error("解析Excel行数据失败: %s", e)

            self.logger.info("农业银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析农业银行账单失败: %s", e)
            return []

        return self.post_process(bills)
