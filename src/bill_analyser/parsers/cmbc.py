"""
CMBC Parser - 民生银行账单解析器

解析中国民生银行账单导出的 CSV/Excel 文件。

输出标准格式:
- date: 交易时间 (YYYY-MM-DD HH:MM:SS)
- amount: 金额 (支出为负, 收入为正)
- type: 类型 (收入/支出)
- description: 聚合描述
- source_account_id: 'cmbc'
"""

import csv
from typing import Dict, List, Any

import pandas as pd

from .base import ParserBase
from ..utils.logger import log_method


class CMBCParser(ParserBase):
    """民生银行账单解析器"""

    # 解析器标识符
    PARSER_ID = "cmbc"
    PARSER_NAME = "民生银行"

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = [".csv", ".xlsx", ".xls"]
        self.logger.info("民生银行账单解析器已初始化 [ID=%s]", self.PARSER_ID)

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为民生银行账单"""
        try:
            if not file_path.endswith((".xlsx", ".xls", ".csv")):
                return False

            # 检查文件名（优先级最低）
            if "cmbc" in file_path.lower() or "民生" in file_path:
                return True

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
                    # Excel文件，先尝试HTML格式
                    try:
                        tables = pd.read_html(file_path, encoding="utf-8")
                        if tables and len(tables) > 0:
                            df = tables[0].head(15)
                        else:
                            df = pd.read_excel(file_path, nrows=15, header=None)
                    except Exception:  # pylint: disable=broad-except
                        df = pd.read_excel(file_path, nrows=15, header=None)

            except Exception:  # pylint: disable=broad-except
                return False

            # 检查内容是否包含民生银行特征
            content = ""
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content += cell_value + " "

            # 民生银行强特征标识
            cmbc_strong_indicators = ["中国民生银行", "民生银行股份有限公司", "个人账户对账单"]

            # 民生银行特有的列名组合（最关键的识别依据）
            cmbc_column_patterns = [
                # 旧格式
                ["凭证类型", "凭证号码"],
                ["客户姓名", "客户账号"],
                ["对方开户行", "凭证号码"],
                # 新格式（HTML导出）
                ["交易时间", "支出金额", "存入金额", "账户余额"],
                ["对方账号", "对方名称", "对方开户行"],
            ]

            # 排除其他银行的特征
            other_bank_indicators = [
                "账⼾活期交易明细清单",  # 农业银行
                "储种",  # 工商银行
                "账户明细查询",  # 可能是农业银行
            ]

            # 检查是否包含民生银行强标识
            has_cmbc_strong = any(indicator in content for indicator in cmbc_strong_indicators)

            # 检查是否包含民生银行特有的列名组合（任一组合全匹配即可）
            has_cmbc_columns = False
            for pattern in cmbc_column_patterns:
                if all(col in content for col in pattern):
                    has_cmbc_columns = True
                    break

            # 检查是否包含其他银行特征
            has_other_bank = any(indicator in content for indicator in other_bank_indicators)

            # 民生银行判定逻辑：
            # 1. 有强标识 或 有特有列名组合
            # 2. 且不包含其他银行特征
            is_cmbc = (has_cmbc_strong or has_cmbc_columns) and not has_other_bank

            if is_cmbc:
                self.logger.info(f"识别为民生银行文件: {file_path}")

            return is_cmbc

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug(f"判断民生银行文件失败: {file_path}, 错误: {e}")
            return False

    def _is_html_file(self, file_path: str) -> bool:
        """检查文件是否为HTML格式（某些银行导出的.xls实际是HTML）"""
        try:
            with open(file_path, "rb") as f:
                first_bytes = f.read(100)
                # 检查是否以 <html 或 <!DOCTYPE html 开头
                first_str = first_bytes.decode("utf-8", errors="ignore").lower().strip()
                return first_str.startswith("<html") or first_str.startswith("<!doctype html")
        except Exception:  # pylint: disable=broad-except
            return False

    def _parse_html_xls(self, file_path: str) -> List[Dict[str, Any]]:
        """解析HTML格式伪装的.xls文件（民生银行网银导出格式）"""
        bills = []

        try:
            # 使用pandas的read_html读取HTML表格
            tables = pd.read_html(file_path, encoding="utf-8")

            if not tables:
                self.logger.warning("HTML文件中未找到表格")
                return []

            # 民生银行通常只有一个表格
            df = tables[0]
            self.logger.info(f"[民生银行HTML] 读取到 {len(df)} 行数据")

            # 查找表头行（包含"交易时间"的行）
            header_row_idx = None
            for idx in range(min(10, len(df))):
                row_values = [str(v) for v in df.iloc[idx].tolist() if pd.notna(v)]
                row_text = " ".join(row_values)
                if "交易时间" in row_text:
                    header_row_idx = idx
                    break

            if header_row_idx is None:
                self.logger.warning("未找到民生银行表头行")
                return []

            # 提取列名
            headers = []
            for val in df.iloc[header_row_idx].tolist():
                if pd.isna(val):
                    headers.append("")
                else:
                    headers.append(str(val).strip())

            self.logger.info(f"[民生银行HTML] 列名: {headers}")

            # 从表头下一行开始解析数据
            for idx in range(header_row_idx + 1, len(df)):
                try:
                    row = df.iloc[idx]
                    row_dict = {}
                    for col_idx, header in enumerate(headers):
                        if header and col_idx < len(row):
                            val = row.iloc[col_idx]
                            row_dict[header] = str(val).strip() if pd.notna(val) else ""

                    # 提取交易时间（格式: 20170101\t19:36:56）
                    date_str = row_dict.get("交易时间", "")
                    if not date_str or date_str == "nan" or "交易时间" in date_str:
                        continue

                    # 处理日期格式：20170101\t19:36:56 -> 2017-01-01 19:36:56
                    date_str = date_str.replace("\\t", " ").replace("\t", " ")
                    if len(date_str) >= 8 and date_str[:8].isdigit():
                        # 格式化日期：20170101 -> 2017-01-01
                        date_part = date_str[:8]
                        time_part = date_str[8:].strip() if len(date_str) > 8 else ""
                        formatted_date = f"{date_part[:4]}-{date_part[4:6]}-{date_part[6:8]}"
                        if time_part:
                            formatted_date += f" {time_part}"
                        date_str = formatted_date

                    # 提取金额 - 民生银行有"支出金额"和"存入金额"两列
                    debit_str = row_dict.get("支出金额", "").strip()
                    credit_str = row_dict.get("存入金额", "").strip()

                    amount_value = 0.0
                    transaction_type = "支出"

                    if credit_str and credit_str not in ["", "nan", "NaN"]:
                        try:
                            amount_value = abs(float(credit_str.replace(",", "")))
                            transaction_type = "收入"
                        except ValueError, TypeError:
                            pass
                    elif debit_str and debit_str not in ["", "nan", "NaN"]:
                        try:
                            amount_value = abs(float(debit_str.replace(",", "")))
                            transaction_type = "支出"
                        except ValueError, TypeError:
                            pass

                    if amount_value == 0:
                        continue

                    # 构建账单
                    bill = {
                        "date": date_str,
                        "type": transaction_type,
                        "counterparty": row_dict.get("对方名称", "") or row_dict.get("对方户名", ""),
                        "description": row_dict.get("摘要", "") or row_dict.get("交易方式", ""),
                        "amount": str(amount_value),
                        "channel": "民生银行",
                        "opponent_account": row_dict.get("对方账号", ""),
                        "payment_method": row_dict.get("交易方式", ""),
                    }

                    bills.append(bill)

                except Exception as e:  # pylint: disable=broad-except
                    self.logger.debug(f"解析民生银行HTML行失败: {e}")
                    continue

            self.logger.info(f"[民生银行HTML] 解析完成: {len(bills)} 条账单")

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error(f"解析民生银行HTML账单失败: {e}")
            return []

        return bills

    @log_method
    def parse(self, file_path: str) -> List[Dict[str, Any]]:
        """解析民生银行账单"""
        if not self.validate_file(file_path):
            return []

        bills = []

        try:
            # 读取文件
            if file_path.endswith(".csv"):
                # CSV格式
                with open(file_path, "r", encoding="gbk") as f:
                    lines = f.readlines()

                    data_start = 0
                    for i, line in enumerate(lines):
                        if "交易日期" in line or "记账日期" in line or "交易时间" in line:
                            data_start = i
                            break

                    if data_start == 0:
                        self.logger.warning("未找到数据起始行")
                        return []

                    reader = csv.DictReader(lines[data_start:])

                    for row in reader:
                        try:
                            date_field = row.get("交易日期") or row.get("记账日期") or row.get("交易时间") or ""
                            if not date_field.strip():
                                continue

                            amount_str = row.get("交易金额") or row.get("金额") or "0"

                            # 判断收支
                            transaction_type = "支出"
                            if row.get("收/支") == "收入" or float(amount_str.replace(",", "")) > 0:
                                transaction_type = "收入"

                            bill = {
                                "date": date_field,
                                "type": transaction_type,
                                "counterparty": row.get("交易对手") or row.get("对方户名") or row.get("对方名称") or "",
                                "description": row.get("交易说明") or row.get("摘要") or "",
                                "amount": amount_str.replace("-", ""),
                                "channel": "民生银行",
                            }

                            bills.append(bill)

                        except Exception as e:  # pylint: disable=broad-except
                            self.logger.error("解析CSV行数据失败: %s", e)
            else:
                # Excel格式 - 先检查是否为HTML伪装的.xls
                if self._is_html_file(file_path):
                    self.logger.info(f"检测到HTML格式的.xls文件: {file_path}")
                    bills = self._parse_html_xls(file_path)
                    return self.post_process(bills)

                # 真正的Excel格式
                df = pd.read_excel(file_path, header=None)

                # 查找表头行
                header_row = -1
                for i in range(min(10, len(df))):
                    cols = df.columns
                    row_text = " ".join(str(df.iloc[i, j]) for j in range(len(cols)) if not pd.isna(df.iloc[i, j]))
                    if "交易日期" in row_text or "交易时间" in row_text:
                        header_row = i
                        break

                if header_row == -1:
                    self.logger.warning("未找到表头行")
                    return []

                # 提取列名
                headers = []
                for j in range(len(df.columns)):
                    val = df.iloc[header_row, j]
                    headers.append(str(val) if not pd.isna(val) else f"col_{j}")

                # 解析数据行
                for i in range(header_row + 1, len(df)):
                    try:
                        row_dict = {}
                        for j, header in enumerate(headers):
                            val = df.iloc[i, j]
                            row_dict[header] = str(val) if not pd.isna(val) else ""

                        # 获取日期
                        date_field = (
                            row_dict.get("交易日期") or row_dict.get("交易时间") or row_dict.get("记账日期") or ""
                        )
                        if not date_field.strip() or date_field == "nan":
                            continue

                        # 获取金额 - 新格式可能有支出金额和存入金额分列
                        amount_str = "0"
                        transaction_type = "支出"

                        if "支出金额" in row_dict and "存入金额" in row_dict:
                            # 新格式：分列显示
                            debit = row_dict.get("支出金额", "").strip()
                            credit = row_dict.get("存入金额", "").strip()

                            if credit and credit != "nan" and credit != "":
                                try:
                                    amount_str = str(abs(float(credit.replace(",", ""))))
                                    transaction_type = "收入"
                                except ValueError, TypeError:
                                    pass
                            elif debit and debit != "nan" and debit != "":
                                try:
                                    amount_str = str(abs(float(debit.replace(",", ""))))
                                    transaction_type = "支出"
                                except ValueError, TypeError:
                                    pass
                        else:
                            # 旧格式：单列显示
                            amount_str = row_dict.get("交易金额") or row_dict.get("金额") or "0"
                            try:
                                if float(amount_str.replace(",", "")) > 0:
                                    transaction_type = "收入"
                            except ValueError, TypeError:
                                pass

                        if not amount_str or amount_str == "0" or amount_str == "nan":
                            continue

                        bill = {
                            "date": date_field.split()[0] if " " in date_field else date_field,  # 去除可能的时间部分
                            "type": transaction_type,
                            "counterparty": row_dict.get("对方名称")
                            or row_dict.get("对方户名")
                            or row_dict.get("交易对手")
                            or "",
                            "description": row_dict.get("摘要") or row_dict.get("交易说明") or "",
                            "amount": str(abs(float(amount_str.replace(",", "")))),
                            "channel": "民生银行",
                        }

                        bills.append(bill)

                    except Exception as e:  # pylint: disable=broad-except
                        self.logger.error("解析Excel行数据失败: %s", e)

            self.logger.info("民生银行账单解析完成: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析民生银行账单失败: %s", e)
            return []

        return self.post_process(bills)
