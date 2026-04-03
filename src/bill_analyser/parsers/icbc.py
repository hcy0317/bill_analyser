"""
工商银行账单解析器

解析中国工商银行账单导出的 CSV/Excel 文件。
支持格式：
1. CSV格式（编码gbk）
2. XLS/XLSX格式（第2行为列名，第3行开始为数据）

输出标准格式：
- date: 交易时间 (YYYY-MM-DD HH:MM:SS)
- amount: 金额 (支出为负, 收入为正)
- type: 类型 (收入/支出/转账)
- description: 聚合描述
- source_account_id: 'icbc'
"""

import csv
from pathlib import Path
from typing import Any

import openpyxl
import pandas as pd

from ..utils.logger import log_method
from .base import ParserBase


class ICBCParser(ParserBase):
    """工商银行账单解析器"""

    # 解析器标识符
    PARSER_ID = "icbc"
    PARSER_NAME = "工商银行"

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = [".csv", ".xlsx", ".xls"]
        self.logger.info("工商银行账单解析器已初始化 [ID=%s]", self.PARSER_ID)

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为工商银行账单"""
        file_ext = Path(file_path).suffix.lower()

        # 检查文件名（优先级最低）
        if "icbc" in file_path.lower() or "工商" in file_path:
            return True

        if file_ext in [".xlsx", ".xls"]:
            return self._can_parse_excel(file_path)
        if file_ext == ".csv":
            return self._can_parse_csv(file_path)

        return False

    def _can_parse_csv(self, file_path: str) -> bool:
        """判断CSV是否为工商银行账单"""
        try:
            text, _encoding = self.read_text_with_fallback(file_path)
            first_lines = "\n".join(text.splitlines()[:10])

            # 工商银行强特征
            icbc_indicators = ["工商银行", "ICBC", "中国工商银行"]
            has_icbc = any(indicator in first_lines for indicator in icbc_indicators)

            # 排除其他银行
            other_bank = ["民生银行", "农业银行", "建设银行"]
            has_other = any(bank in first_lines for bank in other_bank)

            csv_header_patterns = [
                ["交易日期", "交易金额", "对方户名", "对方账号", "交易流水号"],
                ["记账日期", "金额", "对方户名", "对方账号", "交易流水号"],
            ]
            has_icbc_header = any(
                all(column in first_lines for column in pattern)
                for pattern in csv_header_patterns
            )

            return (has_icbc or has_icbc_header) and not has_other
        except Exception:  # pylint: disable=broad-except
            return False

    def _detect_file_format(self, file_path: str) -> tuple:
        """检测Excel文件格式

        返回：
            tuple: (is_html_format: bool, error: Optional[str])
        """
        try:
            with open(file_path, "rb") as f:
                header_bytes = f.read(8)

            # 检查文件魔数（magic number）
            # HTML格式伪装的xls: 以<html开头
            if header_bytes.startswith(b"<htm") or header_bytes.startswith(b"<HTM"):
                self.logger.debug("检测到HTML伪装XLS格式: %s", file_path)
                return True, None

            # 其他格式（XLSX/OLE2/未知）都使用pandas读取
            self.logger.debug("检测到标准Excel格式: %s", file_path)
            return False, None

        except OSError as e:
            return False, str(e)

    def _read_html_content(self, file_path: str) -> str:
        """读取HTML格式文件内容，尝试多种编码"""
        try:
            content, encoding = self.read_text_with_fallback(file_path)
            self.logger.debug("HTML文件使用 %s 编码读取成功", encoding)
            return content[:5000]
        except (OSError, UnicodeDecodeError):
            return ""

    def _read_excel_content(self, file_path: str) -> str:
        """读取Excel格式文件内容"""
        try:
            df = pd.read_excel(file_path, header=None, nrows=15)
            content_parts = []
            for i in range(len(df)):
                for j in range(len(df.columns)):
                    cell_value = str(df.iloc[i, j]) if not pd.isna(df.iloc[i, j]) else ""
                    content_parts.append(cell_value)
            content = " ".join(content_parts)
            self.logger.debug("pandas读取Excel成功，内容长度: %d", len(content))
            return content
        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug("pandas读取Excel失败: %s", e)
            return ""

    def _is_icbc_content(self, content: str) -> bool:
        """判断内容是否为工商银行账单"""
        # 工商银行强特征标识
        icbc_strong_indicators = ["中国工商银行", "工商银行", "ICBC"]

        # v6.76: 工商银行专属列名特征（包含这些列名一定是工商银行，优先级最高）
        # "收入/支出金额" 是工商银行特有的列名格式，其他银行不会使用
        icbc_exclusive_columns = ["收入/支出金额", "对方账号名称", "交易附言"]

        # 工商银行特有列名组合（核心识别依据）
        icbc_column_patterns = [
            ["储种", "账号", "交易日期"],
            ["账号", "交易日期", "交易时间", "对方账号"],
            ["交易日期", "交易附言", "对方账号名称"],
            # v6.76: 新增工商银行历史明细格式
            ["储种", "交易日期", "收入/支出金额", "对方户名"],
            ["账号", "储种", "币种", "摘要", "对方户名"],
        ]

        # 排除其他银行特征
        other_bank_indicators = [
            "存入金额",
            "凭证类型",  # v6.76: 移除"支出金额"，因为工商银行使用"收入/支出金额"
            "⼾名",
            "账⼾",
            "对⼿信息",
            "记账日",
            "开户机构：",
            "账户明细查询",
            "交易用途",
        ]

        # v6.76: 先检查工商银行专属列名（优先级最高，直接返回True）
        has_icbc_exclusive = any(col in content for col in icbc_exclusive_columns)
        if has_icbc_exclusive:
            self.logger.debug(
                "检测到工商银行专属列名特征: %s", [col for col in icbc_exclusive_columns if col in content]
            )
            return True

        has_icbc_strong = any(ind in content for ind in icbc_strong_indicators)
        has_icbc_columns = any(
            all(col in content for col in pattern)
            for pattern in icbc_column_patterns
        )
        has_other_bank = any(ind in content for ind in other_bank_indicators)

        return (has_icbc_strong or has_icbc_columns) and not has_other_bank

    def _can_parse_excel(self, file_path: str) -> bool:
        """判断Excel是否为工商银行账单"""
        try:
            # 检测文件格式
            is_html_format, error = self._detect_file_format(file_path)
            if error:
                self.logger.debug("读取文件头失败: %s", error)
                return False

            # 根据文件类型读取内容
            if is_html_format:
                content = self._read_html_content(file_path)
            else:
                content = self._read_excel_content(file_path)

            if not content:
                return False

            # 判断是否为工商银行
            is_icbc = self._is_icbc_content(content)
            if is_icbc:
                self.logger.info("识别为工商银行文件: %s", file_path)

            return is_icbc

        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug("判断工商银行Excel失败: %s", e)
            return False

    @log_method
    def parse(self, file_path: str) -> list[dict[str, Any]]:
        """解析工商银行账单"""
        if not self.validate_file(file_path):
            return []

        file_ext = Path(file_path).suffix.lower()

        if file_ext == ".csv":
            return self._parse_csv(file_path)

        if file_ext in [".xlsx", ".xls"]:
            # 检查文件格式（通过魔数判断）
            try:
                with open(file_path, "rb") as f:
                    header_bytes = f.read(8)

                # HTML格式伪装的xls: 以<html开头
                if header_bytes.startswith(b"<htm") or header_bytes.startswith(b"<HTM"):
                    self.logger.info("检测到HTML伪装XLS格式，使用HTML解析器")
                    return self._parse_html_xls(file_path)
                # 真正的Excel格式（XLSX或OLE2 XLS）
                self.logger.info("检测到标准Excel格式，使用Excel解析器")
                return self._parse_excel(file_path)
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("判断Excel格式失败: %s, 尝试标准解析", e)
                return self._parse_excel(file_path)

        self.logger.error("不支持的文件格式: %s", file_ext)
        return []

    def _extract_bill_from_html_row(self, row) -> dict[str, Any] | None:
        """从HTML表格行提取账单信息"""
        try:
            # 提取交易日期
            date_str = str(row.get("交易日期", ""))
            if not date_str or date_str == "nan":
                return None

            # 提取金额
            amount_str = str(row.get("收入/支出金额", "") or row.get("金额", ""))
            if not amount_str or amount_str == "nan" or amount_str == "0":
                return None

            try:
                amount_value = float(amount_str.replace(",", ""))
            except (ValueError, TypeError):
                return None

            if amount_value == 0:
                return None

            # 判断收支类型
            transaction_type = "收入" if amount_value > 0 else "支出"

            return {
                "date": date_str,
                "type": transaction_type,
                "counterparty": str(row.get("对方户名", "") or row.get("交易对方", "")),
                "opponent_account": str(row.get("对方账号", "") or ""),
                "description": str(row.get("摘要", "") or row.get("用途", "")),
                "abstract": str(row.get("摘要", "") or row.get("交易附言", "")),
                "amount": str(abs(amount_value)),
                "transaction_id": str(row.get("交易流水号", "") or ""),
            }
        except Exception as e:  # pylint: disable=broad-except
            self.logger.debug("解析HTML行失败: %s", e)
            return None

    @log_method
    def _parse_html_xls(self, file_path: str) -> list[dict[str, Any]]:
        """解析HTML格式伪装的xls文件(工商银行导出的格式)"""
        bills = []

        try:
            # 尝试多种编码读取HTML表格
            dfs = None
            encoding_used = None
            for encoding in ["utf-8", "gbk", "gb2312", "gb18030"]:
                try:
                    dfs = pd.read_html(file_path, encoding=encoding)
                    encoding_used = encoding
                    self.logger.debug("HTML文件使用 %s 编码读取成功", encoding)
                    break
                except (UnicodeDecodeError, ValueError):
                    continue

            if not dfs:
                self.logger.warning("未找到HTML表格或所有编码尝试失败")
                return []

            # 通常第一个表格是账单数据
            df = dfs[0]

            self.logger.info(
                "读取到 %d 行数据，列名: %s，编码: %s",
                len(df),
                df.columns.tolist(),
                encoding_used,
            )

            # 查找列名(可能在不同行)
            column_row_idx = None
            for idx in range(min(10, len(df))):
                row_values = df.iloc[idx].tolist()
                if any("交易日期" in str(v) for v in row_values):
                    column_row_idx = idx
                    break

            if column_row_idx is not None:
                # 使用找到的行作为列名
                df.columns = df.iloc[column_row_idx].tolist()
                df = df.iloc[column_row_idx + 1 :]  # 从下一行开始是数据

            # 解析每一行
            for _, row in df.iterrows():
                bill = self._extract_bill_from_html_row(row)
                if bill:
                    bills.append(bill)

            self.logger.info("[解析完成] HTML工商银行账单: %d 条", len(bills))

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析HTML工商银行账单失败: %s", e)
            return []

        return self.post_process(bills)

    @log_method
    def _parse_csv(self, file_path: str) -> list[dict[str, Any]]:
        """解析CSV格式的工商银行账单"""
        bills = []

        try:
            lines, encoding = self.read_lines_with_fallback(file_path)

            # 查找数据起始行
            data_start = -1
            for i, line in enumerate(lines):
                if "交易日期" in line or "记账日期" in line:
                    data_start = i
                    break

            if data_start < 0:
                self.logger.warning("未找到数据起始行")
                return []

            reader = csv.DictReader(lines[data_start:])

            for row in reader:
                bill = self._extract_bill_from_csv_row(row)
                if bill:
                    bills.append(bill)

            self.logger.info("CSV工商银行账单解析完成: %d 条 (encoding=%s)", len(bills), encoding)

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("解析CSV工商银行账单失败: %s", e)
            return []

        return self.post_process(bills)

    @log_method
    def _parse_excel(self, file_path: str) -> list[dict[str, Any]]:
        """解析Excel格式的工商银行账单"""
        bills = []

        try:
            wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
            ws = wb.active
            if ws is None:
                self.logger.error("未找到活动工作表")
                wb.close()
                return []

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

    def _extract_bill_from_csv_row(self, row: dict[str, str]) -> dict[str, Any] | None:
        """从CSV行数据提取账单信息"""
        try:
            # 获取日期字段
            date_field = row.get("交易日期") or row.get("记账日期") or ""
            if not date_field or len(date_field.strip()) == 0:
                return None

            # 获取金额和类型
            amount_str = row.get("交易金额") or row.get("金额") or "0"
            amount_value = float(str(amount_str).replace(",", ""))
            transaction_type = "收入" if amount_value > 0 else "支出"

            # 判断收支类型
            if row.get("收/支"):
                transaction_type = row.get("收/支")
            elif row.get("借贷标志") == "贷" or "income" in str(row).lower():
                transaction_type = "收入"

            return {
                "date": date_field,
                "type": transaction_type,
                "counterparty": row.get("对方户名") or row.get("交易对方") or "",
                "opponent_account": row.get("对方账号") or "",
                "description": row.get("摘要") or row.get("用途") or "",
                "abstract": row.get("摘要") or row.get("交易摘要") or "",
                "amount": str(amount_str).replace("+", "").replace("-", ""),
                "transaction_id": row.get("交易流水号") or "",
            }

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("提取CSV账单信息失败: %s", e)
            return None

    def _extract_bill_from_excel_row(
        self,
        row: tuple,
        column_map: dict[str, int],
    ) -> dict[str, Any] | None:
        """从Excel行数据提取账单信息"""
        try:

            def get_cell(col_name: str) -> str:
                idx = column_map.get(col_name)
                if idx is not None and idx < len(row):
                    value = row[idx]
                    return str(value) if value is not None else ""
                return ""

            # 提取交易日期（可能带换行符）
            date_str = get_cell("交易日期")
            if not date_str:
                return None

            # 处理换行符（如 "2016-08-11\n20:15:27"）
            date_str = date_str.replace("\n", " ")

            # 提取金额
            amount_str = get_cell("收入/支出金额")
            if not amount_str or amount_str == "0":
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
            transaction_type = "收入" if amount_value > 0 else "支出"
            amount_str = str(abs(amount_value))

            return {
                "date": date_str,
                "type": transaction_type,
                "counterparty": get_cell("对方户名"),
                "opponent_account": get_cell("对方账号"),
                "description": get_cell("摘要"),
                "abstract": get_cell("摘要") or get_cell("交易附言"),
                "amount": amount_str,
                "transaction_id": get_cell("交易流水号"),
            }

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("提取Excel账单信息失败: %s", e)
            return None
