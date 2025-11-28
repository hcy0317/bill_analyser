"""
CCB Parser - 建设银行账单解析器

解析中国建设银行账单导出的 Excel 文件。
"""

import pandas as pd
from typing import Dict, List, Any

from .base import ParserBase
from ..utils.logger import log_method


class CCBParser(ParserBase):
    """建设银行账单解析器"""

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = ['.xlsx', '.xls']

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为建设银行账单"""
        try:
            if not file_path.endswith(('.xlsx', '.xls')):
                return False

            # 检查文件名（优先级最低）
            if 'ccb' in file_path.lower() or '建设' in file_path:
                return True

            # 读取文件内容进行判断
            df = pd.read_excel(file_path, header=None, nrows=15)

            # 检查内容是否包含建设银行特征
            content = ""
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 建设银行强特征标识
            ccb_strong_indicators = [
                "China Construction Bank",
                "中国建设银行",
                "开户机构：",
                "账\u3000\u3000号：",  # 特殊格式的"账号："
                "币\u3000\u3000种：",   # 特殊格式的"币种："
            ]

            # 建设银行特有列名组合（核心识别依据）
            ccb_column_patterns = [
                ["记账日", "交易日期", "支出", "收入"],  # 经典格式
                ["记账日", "摘要", "账户余额"],
                ["交易日期", "支出", "收入", "账户余额"]
            ]

            # 排除其他银行特征
            other_bank_indicators = [
                "储种",  # 工商银行特有
                "对方开户行",  # 民生银行
                "凭证类型",  # 民生银行
                "⼾名", "账⼾", "对⼿信息",  # 农业银行特殊编码
                "支出金额", "存入金额"  # 民生银行（建设用"支出"、"收入"）
            ]

            # 检查强特征
            has_ccb_strong = any(indicator in content for indicator in ccb_strong_indicators)

            # 检查列名组合（任一组合全匹配即可）
            has_ccb_columns = any(all(col in content for col in pattern) for pattern in ccb_column_patterns)

            # 检查是否有其他银行特征
            has_other_bank = any(indicator in content for indicator in other_bank_indicators)

            # 建设银行判定：有强标识或列名组合，且无其他银行特征
            is_ccb = (has_ccb_strong or has_ccb_columns) and not has_other_bank

            if is_ccb:
                self.logger.info(f"识别为建设银行文件: {file_path}")

            return is_ccb

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug(f"判断建设银行文件失败: {file_path}, 错误: {e}")
            return False

            return is_ccb

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug(f"判断建设银行文件失败: {file_path}, 错误: {e}")
            return False

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析建设银行账单"""
        if not self.validate_file(file_path):
            return []

        bills = []

        try:
            # 读取Excel文件
            df = pd.read_excel(file_path, header=None)

            # 查找表头行（包含"记账日"或"交易日期"）
            header_row = -1
            for i in range(min(10, len(df))):
                row_text = ' '.join(str(df.iloc[i, j]) for j in range(len(df.columns)) if not pd.isna(df.iloc[i, j]))
                if '记账日' in row_text and '交易日期' in row_text:
                    header_row = i
                    break

            if header_row == -1:
                self.logger.warning("未找到表头行")
                return []

            # 提取列名
            headers = []
            for j in range(len(df.columns)):
                val = df.iloc[header_row, j]
                header_text = str(val).strip() if not pd.isna(val) else f'col_{j}'
                headers.append(header_text)

            # 解析数据行
            for i in range(header_row + 1, len(df)):
                try:
                    row_dict = {}
                    for j, header in enumerate(headers):
                        val = df.iloc[i, j]
                        row_dict[header] = str(val) if not pd.isna(val) else ''

                    # 获取交易日期
                    date_str = row_dict.get('交易日期') or row_dict.get('记账日') or ''
                    if not date_str or date_str == 'nan':
                        continue

                    # 格式化日期 (20211210 -> 2021-12-10)
                    if len(date_str) == 8 and date_str.isdigit():
                        date_str = f"{date_str[0:4]}-{date_str[4:6]}-{date_str[6:8]}"

                    # 获取收支金额
                    debit_str = row_dict.get('支出', '0').strip()
                    credit_str = row_dict.get('收入', '0').strip()

                    # 确定交易类型和金额
                    transaction_type = '支出'
                    amount_str = '0'

                    try:
                        debit = float(debit_str) if debit_str and debit_str != 'nan' else 0
                        credit = float(credit_str) if credit_str and credit_str != 'nan' else 0

                        if credit > 0:
                            transaction_type = '收入'
                            amount_str = str(credit)
                        elif debit > 0:
                            transaction_type = '支出'
                            amount_str = str(debit)
                        else:
                            continue  # 跳过金额为0的记录

                    except (ValueError, TypeError):
                        continue

                    # 获取摘要
                    description = row_dict.get('摘要', '').strip()
                    if not description or description == 'nan':
                        description = ''

                    # 获取对方信息（建设银行文件中可能没有单独的对方字段，使用摘要）
                    counterparty = row_dict.get('对方户名', '').strip()
                    if not counterparty or counterparty == 'nan':
                        counterparty = description  # 使用摘要作为对方

                    bill = {
                        'date': date_str,
                        'type': transaction_type,
                        'counterparty': counterparty,
                        'description': description,
                        'amount': amount_str,
                        'channel': '建设银行'
                    }

                    bills.append(bill)

                except Exception as e:  # pylint: disable=broad-except
                    self.logger.error("解析建设银行行数据失败: %s", e)
                    continue

            self.logger.info("建设银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析建设银行账单失败: %s", e)
            return []

        return self.post_process(bills)
