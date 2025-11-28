"""
ICBC Parser - 工商银行账单解析器

解析中国工商银行账单导出的 CSV/Excel 文件。
支持格式：
1. CSV格式（编码gbk）
2. XLS/XLSX格式（第2行为列名，第3行开始为数据）
"""

import csv
from pathlib import Path
from typing import Dict, List, Any
import openpyxl
import pandas as pd

from .base import ParserBase
from ..utils.logger import log_method


class ICBCParser(ParserBase):
    """工商银行账单解析器"""

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = ['.csv', '.xlsx', '.xls']
        self.logger.info("工商银行账单解析器已初始化")

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为工商银行账单"""
        file_ext = Path(file_path).suffix.lower()

        # 检查文件名（优先级最低）
        if 'icbc' in file_path.lower() or '工商' in file_path:
            return True

        if file_ext in ['.xlsx', '.xls']:
            return self._can_parse_excel(file_path)
        elif file_ext == '.csv':
            return self._can_parse_csv(file_path)

        return False

    def _can_parse_csv(self, file_path: str) -> bool:
        """判断CSV是否为工商银行账单"""
        try:
            with open(file_path, 'r', encoding='gbk') as f:
                first_lines = ''.join([f.readline() for _ in range(10)])

            # 工商银行强特征
            icbc_indicators = ['工商银行', 'ICBC', '中国工商银行']
            has_icbc = any(indicator in first_lines for indicator in icbc_indicators)

            # 排除其他银行
            other_bank = ['民生银行', '农业银行', '建设银行']
            has_other = any(bank in first_lines for bank in other_bank)

            return has_icbc and not has_other
        except Exception:  # pylint: disable=broad-except
            return False

    def _can_parse_excel(self, file_path: str) -> bool:
        """判断Excel是否为工商银行账单"""
        try:
            content = ""

            # 先检查是否是HTML格式伪装的xls
            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    first_line = f.readline()
                    if first_line.strip().startswith('<html'):
                        # 读取更多内容
                        content = first_line + ''.join([f.readline() for _ in range(20)])
            except:
                pass

            # 如果不是HTML或读取失败，尝试用pandas读取Excel
            if not content:
                try:
                    df = pd.read_excel(file_path, header=None, nrows=15)
                    for i in range(len(df)):
                        for j in range(len(df.columns)):
                            cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                            content += cell_value + " "
                except:
                    return False

            # 工商银行强特征标识
            icbc_strong_indicators = [
                "中国工商银行", "工商银行", "ICBC"
            ]

            # 工商银行特有列名组合（核心识别依据）
            icbc_column_patterns = [
                ["储种", "账号", "交易日期"],  # 经典格式
                ["账号", "交易日期", "交易时间", "对方账号"],  # 新格式
                ["交易日期", "交易附言", "对方账号名称"]
            ]

            # 排除其他银行特征
            other_bank_indicators = [
                "支出金额",  # 民生银行特有（与"存入金额"配对）
                "存入金额",  # 民生银行特有
                "凭证类型",  # 民生银行
                "⼾名", "账⼾", "对⼿信息",  # 农业银行特殊编码
                "记账日",  # 建设银行（工商用"交易日期"）
                "开户机构：",  # 建设银行
                "账户明细查询",  # 农业银行
                "交易用途"  # 农业银行
            ]

            # 检查强特征
            has_icbc_strong = any(indicator in content for indicator in icbc_strong_indicators)

            # 检查列名组合（任一组合全匹配即可）
            has_icbc_columns = any(all(col in content for col in pattern) for pattern in icbc_column_patterns)

            # 检查是否有其他银行特征
            has_other_bank = any(indicator in content for indicator in other_bank_indicators)

            # 工商银行判定：有强标识或列名组合，且无其他银行特征
            is_icbc = (has_icbc_strong or has_icbc_columns) and not has_other_bank

            if is_icbc:
                self.logger.info(f"识别为工商银行文件: {file_path}")

            return is_icbc

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug(f"判断工商银行Excel失败: {e}")
            return False

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析工商银行账单"""
        if not self.validate_file(file_path):
            return []

        file_ext = Path(file_path).suffix.lower()

        if file_ext == '.csv':
            return self._parse_csv(file_path)
        elif file_ext in ['.xlsx', '.xls']:
            # 检查是否是HTML格式伪装的xls
            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    first_line = f.readline()
                    if first_line.strip().startswith('<html'):
                        return self._parse_html_xls(file_path)
            except Exception:  # pylint: disable=broad-except
                pass

            return self._parse_excel(file_path)
        else:
            self.logger.error("不支持的文件格式: %s", file_ext)
            return []

    @log_method
    def _parse_html_xls(self, file_path: str) -> List[Dict[str, Any]]:
        """解析HTML格式伪装的xls文件(工商银行导出的格式)"""
        bills = []

        try:
            import pandas as pd

            # 使用pandas读取HTML表格
            dfs = pd.read_html(file_path, encoding='utf-8')

            if not dfs:
                self.logger.warning("未找到HTML表格")
                return []

            # 通常第一个表格是账单数据
            df = dfs[0]

            self.logger.info(f"读取到 {len(df)} 行数据，列名: {df.columns.tolist()}")

            # 查找列名(可能在不同行)
            column_row_idx = None
            for idx in range(min(10, len(df))):
                row_values = df.iloc[idx].tolist()
                if any('交易日期' in str(v) for v in row_values):
                    column_row_idx = idx
                    break

            if column_row_idx is not None:
                # 使用找到的行作为列名
                df.columns = df.iloc[column_row_idx].tolist()
                df = df.iloc[column_row_idx + 1:]  # 从下一行开始是数据

            # 解析每一行
            for _, row in df.iterrows():
                try:
                    # 提取交易日期
                    date_str = str(row.get('交易日期', ''))
                    if not date_str or date_str == 'nan':
                        continue

                    # 提取金额
                    amount_str = str(row.get('收入/支出金额', '') or row.get('金额', ''))
                    if not amount_str or amount_str == 'nan' or amount_str == '0':
                        continue

                    try:
                        amount_value = float(amount_str.replace(',', ''))
                    except (ValueError, TypeError):
                        continue

                    if amount_value == 0:
                        continue

                    # 判断收支类型
                    transaction_type = '收入' if amount_value > 0 else '支出'

                    bill = {
                        'date': date_str,
                        'type': transaction_type,
                        'counterparty': str(row.get('对方户名', '') or row.get('交易对方', '')),
                        'description': str(row.get('摘要', '') or row.get('用途', '')),
                        'amount': str(abs(amount_value)),
                        'channel': '工商银行'
                    }
                    bills.append(bill)

                except Exception as e:  # pylint: disable=broad-except
                    self.logger.debug(f"解析HTML行失败: {e}")
                    continue

            self.logger.info(f"HTML工商银行账单解析完成: {len(bills)} 条")

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error(f"解析HTML工商银行账单失败: {e}")
            return []

        return self.post_process(bills)

    @log_method
    def _parse_csv(self, file_path: str) -> List[Dict[str, Any]]:
        """解析CSV格式的工商银行账单"""
        bills = []

        try:
            with open(file_path, 'r', encoding='gbk') as f:
                lines = f.readlines()

                # 查找数据起始行
                data_start = 0
                for i, line in enumerate(lines):
                    if '交易日期' in line or '记账日期' in line:
                        data_start = i
                        break

                if data_start == 0:
                    self.logger.warning("未找到数据起始行")
                    return []

                reader = csv.DictReader(lines[data_start:])

                for row in reader:
                    bill = self._extract_bill_from_csv_row(row)
                    if bill:
                        bills.append(bill)

            self.logger.info("CSV工商银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析CSV工商银行账单失败: %s", e)
            return []

        return self.post_process(bills)

    @log_method
    def _parse_excel(self, file_path: str) -> List[Dict[str, Any]]:
        """解析Excel格式的工商银行账单"""
        bills = []

        try:
            wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
            ws = wb.active

            # 第2行是列名
            header_row = next(ws.iter_rows(min_row=2, max_row=2, values_only=True), None)
            if not header_row:
                self.logger.error("未找到列名行")
                wb.close()
                return []

            # 创建列名到索引的映射（跳过空列）
            column_map = {}
            for i, col in enumerate(header_row):
                if col:
                    column_map[col] = i

            self.logger.info("列名映射: %s", column_map)

            # 从第3行开始解析数据
            for row in ws.iter_rows(min_row=3, values_only=True):
                try:
                    # 跳过空行
                    if not row or not row[1]:  # 第2列是交易日期
                        continue

                    bill = self._extract_bill_from_excel_row(row, column_map)
                    if bill:
                        bills.append(bill)

                except Exception as e:  # pylint: disable=broad-except
                    self.logger.error("解析Excel行数据失败: %s", e)

            wb.close()
            self.logger.info("Excel工商银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析Excel工商银行账单失败: %s", e)
            return []

        return self.post_process(bills)

    def _extract_bill_from_csv_row(self, row: Dict[str, str]) -> Dict[str, Any]:
        """从CSV行数据提取账单信息"""
        try:
            # 获取日期字段
            date_field = row.get('交易日期') or row.get('记账日期') or ''
            if not date_field or len(date_field.strip()) == 0:
                return None

            # 获取金额和类型
            amount_str = row.get('交易金额') or row.get('金额') or '0'
            transaction_type = '支出'

            # 判断收支类型
            if row.get('收/支'):
                transaction_type = row.get('收/支')
            elif row.get('借贷标志') == '贷' or 'income' in str(row).lower():
                transaction_type = '收入'

            return {
                'date': date_field,
                'type': transaction_type,
                'counterparty': row.get('对方户名') or row.get('交易对方') or '',
                'description': row.get('摘要') or row.get('用途') or '',
                'amount': str(amount_str).replace('+', '').replace('-', ''),
                'channel': '工商银行'
            }

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("提取CSV账单信息失败: %s", e)
            return None

    def _extract_bill_from_excel_row(self, row: tuple, column_map: Dict[str, int]) -> Dict[str, Any]:
        """从Excel行数据提取账单信息"""
        try:
            def get_cell(col_name: str) -> str:
                idx = column_map.get(col_name)
                if idx is not None and idx < len(row):
                    value = row[idx]
                    return str(value) if value is not None else ''
                return ''

            # 提取交易日期（可能带换行符）
            date_str = get_cell('交易日期')
            if not date_str:
                return None

            # 处理换行符（如 "2016-08-11\n20:15:27"）
            date_str = date_str.replace('\n', ' ')

            # 提取金额
            amount_str = get_cell('收入/支出金额')
            if not amount_str or amount_str == '0':
                return None

            # 跳过非数字金额(如表头重复行)
            try:
                amount_value = float(amount_str)
            except (ValueError, TypeError):
                return None

            # 跳过零金额
            if amount_value == 0:
                return None

            # 判断收支类型（正数为收入，负数为支出）
            transaction_type = '收入' if amount_value > 0 else '支出'
            amount_str = str(abs(amount_value))

            return {
                'date': date_str,
                'type': transaction_type,
                'counterparty': get_cell('对方户名'),
                'description': get_cell('摘要'),
                'amount': amount_str,
                'channel': '工商银行'
            }

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("提取Excel账单信息失败: %s", e)
            return None

