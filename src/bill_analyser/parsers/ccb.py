"""
CCB Parser - 建设银行账单解析器

解析中国建设银行账单导出的 Excel 文件。

输出标准格式:
- date: 交易时间 (YYYY-MM-DD HH:MM:SS)
- amount: 金额 (支出为负, 收入为正)
- type: 类型 (收入/支出)
- description: 聚合描述
- source_account_id: 'ccb'
"""

from typing import Any

import pandas as pd

from ..utils.logger import log_method
from .base import ParserBase


class CCBParser(ParserBase):
    """建设银行账单解析器"""

    # 解析器标识符
    PARSER_ID = "ccb"
    PARSER_NAME = "建设银行"
    CCB_STRONG_INDICATORS = (
        "China Construction Bank",
        "中国建设银行",
        "开户机构：",
        "账\u3000\u3000号：",
        "币\u3000\u3000种：",
    )
    CCB_COLUMN_PATTERNS = (
        ("记账日", "交易日期", "支出", "收入"),
        ("记账日", "摘要", "账户余额"),
        ("交易日期", "支出", "收入", "账户余额"),
    )
    OTHER_BANK_INDICATORS = (
        "储种",
        "对方开户行",
        "凭证类型",
        "⼾名",
        "账⼾",
        "对⼿信息",
        "支出金额",
        "存入金额",
    )

    def __init__(self):
        """初始化"""
        super().__init__()
        self.supported_extensions = [".xlsx", ".xls"]
        self.logger.info("建设银行账单解析器已初始化 [ID=%s]", self.PARSER_ID)

    def _build_trade_time(self, row_dict: dict[str, str]) -> str:
        """合并建设银行的日期与时间字段。"""
        date_str = (row_dict.get("交易日期") or row_dict.get("记账日") or "").strip()
        time_str = (row_dict.get("交易时间") or "").strip()

        if not date_str or date_str == "nan":
            return ""

        if len(date_str) == 8 and date_str.isdigit():
            date_str = f"{date_str[0:4]}-{date_str[4:6]}-{date_str[6:8]}"

        if time_str and time_str != "nan":
            if len(time_str) == 6 and time_str.isdigit():
                time_str = f"{time_str[0:2]}:{time_str[2:4]}:{time_str[4:6]}"
            return f"{date_str} {time_str}"

        return date_str

    @staticmethod
    def _clean_text(value: str | None) -> str:
        """清理单元格文本并过滤空值占位。"""
        text = (value or "").strip()
        return "" if text == "nan" else text

    def _read_probe_content(self, file_path: str) -> str:
        """读取探测区内容并拼接为文本。"""
        dataframe = pd.read_excel(file_path, header=None, nrows=15)
        fragments: list[str] = []

        for row_index in range(len(dataframe)):
            for column_index in range(len(dataframe.columns)):
                cell_value = dataframe.iloc[row_index, column_index]
                if not pd.isna(cell_value):
                    fragments.append(str(cell_value))

        return " ".join(fragments)

    def _is_ccb_content(self, content: str) -> bool:
        """根据内容特征判断是否为建设银行账单。"""
        has_ccb_strong = any(indicator in content for indicator in self.CCB_STRONG_INDICATORS)
        has_ccb_columns = any(
            all(column_name in content for column_name in pattern)
            for pattern in self.CCB_COLUMN_PATTERNS
        )
        has_other_bank = any(indicator in content for indicator in self.OTHER_BANK_INDICATORS)
        return (has_ccb_strong or has_ccb_columns) and not has_other_bank

    def _find_header_row(self, dataframe: pd.DataFrame) -> int:
        """查找表头所在行。"""
        for row_index in range(min(10, len(dataframe))):
            row_text = " ".join(
                str(dataframe.iloc[row_index, column_index])
                for column_index in range(len(dataframe.columns))
                if not pd.isna(dataframe.iloc[row_index, column_index])
            )
            if "记账日" in row_text and "交易日期" in row_text:
                return row_index

        return -1

    def _extract_headers(self, dataframe: pd.DataFrame, header_row: int) -> list[str]:
        """提取表头列名。"""
        headers: list[str] = []

        for column_index in range(len(dataframe.columns)):
            cell_value = dataframe.iloc[header_row, column_index]
            headers.append(
                str(cell_value).strip() if not pd.isna(cell_value) else f"col_{column_index}"
            )

        return headers

    def _build_row_dict(
        self,
        dataframe: pd.DataFrame,
        headers: list[str],
        row_index: int,
    ) -> dict[str, str]:
        """将 Excel 数据行映射为字典。"""
        return {
            header: self._clean_text(
                str(dataframe.iloc[row_index, column_index])
                if not pd.isna(dataframe.iloc[row_index, column_index])
                else ""
            )
            for column_index, header in enumerate(headers)
        }

    def _parse_transaction_amount(self, row_dict: dict[str, str]) -> tuple[str, str] | None:
        """解析收支金额并返回交易类型与金额字符串。"""
        debit_str = row_dict.get("支出", "0").strip()
        credit_str = row_dict.get("收入", "0").strip()

        try:
            debit = float(debit_str) if debit_str else 0.0
            credit = float(credit_str) if credit_str else 0.0
        except (TypeError, ValueError):
            return None

        if credit > 0:
            return "收入", str(credit)
        if debit > 0:
            return "支出", str(debit)

        return None

    def _build_bill(self, row_dict: dict[str, str]) -> dict[str, Any] | None:
        """将原始行数据转换为建设银行账单记录。"""
        trade_time = self._build_trade_time(row_dict)
        if not trade_time:
            return None

        amount_details = self._parse_transaction_amount(row_dict)
        if amount_details is None:
            return None

        transaction_type, amount_str = amount_details
        description = self._clean_text(row_dict.get("摘要", ""))
        counterparty = self._clean_text(row_dict.get("对方户名", "")) or description

        return {
            "date": trade_time,
            "type": transaction_type,
            "counterparty": counterparty,
            "description": description,
            "amount": amount_str,
            "channel": "建设银行",
        }

    def _parse_data_rows(
        self,
        dataframe: pd.DataFrame,
        headers: list[str],
        start_row: int,
    ) -> list[dict[str, Any]]:
        """解析表头后的所有数据行。"""
        bills: list[dict[str, Any]] = []

        for row_index in range(start_row, len(dataframe)):
            try:
                row_dict = self._build_row_dict(dataframe, headers, row_index)
                bill = self._build_bill(row_dict)
            except Exception as exc:  # pylint: disable=broad-except
                self.logger.error("解析建设银行行数据失败: %s", exc)
                continue

            if bill is not None:
                bills.append(bill)

        return bills

    @log_method
    def can_parse(self, file_path: str) -> bool:
        """判断是否为建设银行账单"""
        try:
            if not file_path.endswith((".xlsx", ".xls")):
                return False

            # 检查文件名（优先级最低）
            if "ccb" in file_path.lower() or "建设" in file_path:
                return True

            content = self._read_probe_content(file_path)
            is_ccb = self._is_ccb_content(content)

            if is_ccb:
                self.logger.info("识别为建设银行文件: %s", file_path)

            return is_ccb

        except Exception as exc:  # pylint: disable=broad-except
            self.logger.debug("判断建设银行文件失败: %s, 错误: %s", file_path, exc)
            return False

    @log_method
    def parse(self, file_path: str) -> list[dict[str, Any]]:
        """解析建设银行账单"""
        if not self.validate_file(file_path):
            return []

        try:
            dataframe = pd.read_excel(file_path, header=None)
            header_row = self._find_header_row(dataframe)

            if header_row == -1:
                self.logger.warning("未找到表头行")
                return []

            headers = self._extract_headers(dataframe, header_row)
            bills = self._parse_data_rows(dataframe, headers, header_row + 1)

            self.logger.info("建设银行账单解析完成: %d 条", len(bills))

        except Exception as exc:  # pylint: disable=broad-except
            self.logger.error("解析建设银行账单失败: %s", exc)
            return []

        return self.post_process(bills)
