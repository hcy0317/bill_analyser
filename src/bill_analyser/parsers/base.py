"""
解析器基类模块

定义账单解析器的抽象接口和标准化输出格式。

标准账单格式（StandardBill）：
    - date: 交易时间 (str, YYYY-MM-DD HH:MM:SS 格式, 必须包含时分秒)
    - amount: 金额 (float, 带正负号：支出为负, 收入为正)
    - type: 类型 (str, 收入/支出/转账/投资/退款)
    - description: 描述聚合字段 (str, 聚合商品说明/交易对方/对方账号/备注/交易摘要等)
    - source_account_id: 来源账户ID (str, 解析器标识如 'wechat'/'alipay'/'icbc' 等)
    - counterparty: 交易对方 (str, 可为空)
    - payment_method: 支付方式 (str, 可为空)
    - parser_tags: 解析器元标签 (list[str], 受控词表)
    - original_type: 原始交易类型 (str, 保留原始值用于智能识别)
    - original_category: 原始分类 (str, 如支付宝的"投资理财"等)
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from ..utils.logger import get_logger, log_method
from .parser_tags import resolve_parser_tags


@dataclass
class StandardBill:
    """标准账单数据结构

    所有解析器的输出都必须转换为这个标准格式，
    便于后续的去重、分类和导入处理。
    """  # pylint: disable=too-many-instance-attributes

    # 必需字段
    date: str  # YYYY-MM-DD HH:MM:SS 格式
    amount: float  # 带正负号：支出为负，收入为正
    type: str  # 收入/支出/转账/投资/退款
    description: str  # 聚合描述字段
    source_account_id: str  # 解析器标识：wechat/alipay/icbc/cmbc/abc/ccb

    # 可选字段
    counterparty: str = ""  # 交易对方
    payment_method: str = ""  # 支付方式
    parser_tags: list[str] = field(default_factory=list)  # 解析器元标签
    original_type: str = ""  # 原始交易类型
    original_category: str = ""  # 原始分类
    transaction_id: str = ""  # 原始交易单号
    merchant_id: str = ""  # 商户订单号
    status: str = ""  # 交易状态

    # 分类字段 (由分类引擎填充)
    main_category: str = ""
    sub_category: str = ""

    def to_dict(self) -> dict[str, Any]:
        """转换为字典格式"""
        return {
            "date": self.date,
            "amount": self.amount,
            "type": self.type,
            "description": self.description,
            "source_account_id": self.source_account_id,
            "counterparty": self.counterparty,
            "payment_method": self.payment_method,
            "parser_tags": list(self.parser_tags),
            "original_type": self.original_type,
            "original_category": self.original_category,
            "transaction_id": self.transaction_id,
            "merchant_id": self.merchant_id,
            "status": self.status,
            "main_category": self.main_category,
            "sub_category": self.sub_category,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> StandardBill:
        """从字典创建实例"""
        return cls(
            date=data.get("date", ""),
            amount=float(data.get("amount", 0)),
            type=data.get("type", "支出"),
            description=data.get("description", ""),
            source_account_id=data.get("source_account_id", ""),
            counterparty=data.get("counterparty", ""),
            payment_method=data.get("payment_method", ""),
            parser_tags=resolve_parser_tags(
                data.get("parser_tags"),
                parser_id=data.get("source_account_id", ""),
                payment_method=data.get("payment_method", ""),
            ),
            original_type=data.get("original_type", ""),
            original_category=data.get("original_category", ""),
            transaction_id=data.get("transaction_id", ""),
            merchant_id=data.get("merchant_id", ""),
            status=data.get("status", ""),
            main_category=data.get("main_category", ""),
            sub_category=data.get("sub_category", ""),
        )


class ParserBase(ABC):
    """账单解析器抽象基类

    所有银行/支付平台的解析器都必须继承此类并实现：
    1. can_parse(): 判断是否能解析指定文件
    2. parse(): 解析文件并返回标准格式账单列表

    解析器需要将各平台的字段映射为标准格式，特别注意：
    - date: 必须包含时分秒
    - amount: 支出为负数，收入为正数
    - description: 聚合多个字段便于关键词匹配
    - source_account_id: 使用解析器标识便于账户匹配
    """

    # 解析器标识符，子类必须覆盖
    PARSER_ID: str = "unknown"
    PARSER_NAME: str = "未知解析器"
    TEXT_READ_ENCODINGS: tuple[str, ...] = ("utf-8-sig", "utf-8", "gbk", "gb18030", "gb2312")

    def __init__(self):
        """初始化解析器"""
        self.logger = get_logger(self.__class__.__name__)
        self.supported_extensions = [".csv", ".xlsx", ".xls"]

    @abstractmethod
    def parse(self, file_path: str) -> list[dict[str, Any]]:
        """
        解析账单文件

        参数：
            file_path: 账单文件路径

        返回：
            List[Dict]: 标准格式账单列表，每个账单包含以下字段:
                - date: 交易时间 (str, YYYY-MM-DD HH:MM:SS格式)
                - amount: 金额 (float, 支出为负, 收入为正)
                - type: 类型 (收入/支出/转账/投资/退款)
                - description: 描述聚合字段 (str, 聚合多个信息字段)
                - source_account_id: 来源账户标识 (str, 如 'wechat'/'alipay')
                - counterparty: 交易对方 (str, 可为空)
                - payment_method: 支付方式 (str, 可为空)
                - original_type: 原始交易类型 (str)
                - original_category: 原始分类 (str)
        """
        raise NotImplementedError

    @abstractmethod
    def can_parse(self, file_path: str) -> bool:
        """
        判断是否能解析此文件

        参数：
            file_path: 文件路径

        返回：
            bool: 是否能解析
        """
        raise NotImplementedError

    @log_method
    def validate_file(self, file_path: str) -> bool:
        """
        验证文件是否存在且格式正确

        参数：
            file_path: 文件路径

        返回：
            bool: 是否有效
        """
        path = Path(file_path)

        if not path.exists():
            self.logger.error("文件不存在: %s", file_path)
            return False

        if not path.is_file():
            self.logger.error("不是文件: %s", file_path)
            return False

        if path.suffix.lower() not in self.supported_extensions:
            self.logger.warning(
                "文件扩展名不支持: %s, 支持的扩展名: %s",
                path.suffix,
                ", ".join(self.supported_extensions),
            )
            return False

        return True

    def read_text_with_fallback(
        self, file_path: str, encodings: list[str] | tuple[str, ...] | None = None
    ) -> tuple[str, str]:
        """按多种编码回退读取文本文件。"""
        candidates = list(encodings or self.TEXT_READ_ENCODINGS)
        seen: set[str] = set()

        for encoding in candidates:
            normalized = str(encoding or "").strip().lower()
            if not normalized or normalized in seen:
                continue
            seen.add(normalized)

            try:
                with open(file_path, encoding=normalized) as file_obj:
                    return file_obj.read().lstrip("\ufeff"), normalized
            except UnicodeDecodeError:
                continue

        raise UnicodeDecodeError("unknown", b"", 0, 1, f"unable to decode text file: {file_path}")

    def read_lines_with_fallback(
        self, file_path: str, encodings: list[str] | tuple[str, ...] | None = None
    ) -> tuple[list[str], str]:
        """按多种编码回退读取文本文件并保留原始换行。"""
        text, encoding = self.read_text_with_fallback(file_path, encodings)
        return text.splitlines(keepends=True), encoding

    def normalize_date(self, date_str: str) -> str:
        """
        规范化日期格式为 YYYY-MM-DD HH:MM:SS

        参数：
            date_str: 日期字符串

        返回：
            str: 规范化后的日期字符串
        """
        # 常见日期格式
        formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d",
            "%Y年%m月%d日 %H:%M:%S",
            "%Y年%m月%d日",
            "%Y.%m.%d %H:%M:%S",
            "%Y.%m.%d",
        ]

        for fmt in formats:
            try:
                dt = datetime.strptime(date_str.strip(), fmt)
                return dt.strftime("%Y-%m-%d %H:%M:%S")
            except ValueError:
                continue

        # 如果都失败，返回原字符串
        self.logger.warning("无法解析日期格式: %s", date_str)
        return date_str

    def normalize_amount(self, amount_str: str) -> float:
        """
        规范化金额格式

        参数：
            amount_str: 金额字符串

        返回：
            float: 金额数值
        """
        try:
            # 移除常见符号
            cleaned = (
                str(amount_str)
                .replace("¥", "")
                .replace("$", "")
                .replace(",", "")
                .replace("，", "")
                .strip()
            )

            return float(cleaned)
        except (ValueError, AttributeError) as e:
            self.logger.warning("无法解析金额: %s - %s", amount_str, e)
            return 0.0

    def normalize_type(self, type_str: str) -> str:
        """
        规范化交易类型

        参数：
            type_str: 类型字符串

        返回：
            str: 规范化的类型（收入/支出/转账/退款）
        """
        type_map = {
            "收入": "收入",
            "收款": "收入",
            "入账": "收入",
            "支出": "支出",
            "支付": "支出",
            "消费": "支出",
            "付款": "支出",
            "转账": "转账",
            "转出": "转账",
            "转入": "转账",
            "退款": "退款",
            "退钱": "退款",
            "不计收支": "转账",
            "投资理财": "投资",
            "投资": "投资",
            "理财": "投资",
        }

        type_str = str(type_str).strip()

        for key, value in type_map.items():
            if key in type_str:
                return value

        # 默认根据金额符号判断
        self.logger.debug("未识别的交易类型: %s", type_str)
        return "支出"

    def aggregate_description(self, bill: dict[str, Any]) -> str:
        """
        聚合账单的多个字段生成统一的description

        聚合字段包括：商品说明、交易对方、对方账号、备注、交易摘要等
        便于后续关键词匹配分类

        参数：
            bill: 账单字典

        返回：
            str: 聚合后的描述字符串
        """
        # 可能包含有用信息的字段列表
        description_fields = [
            "description",
            "counterparty",
            "goods",  # 商品
            "product",  # 商品说明
            "remark",  # 备注
            "note",  # 备注
            "memo",  # 备注
            "abstract",  # 交易摘要
            "summary",  # 摘要
            "payment_method",  # 支付方式
            "original_category",  # 原始分类
            "merchant",  # 商户
            "shop",  # 店铺
            "transaction_type",  # 交易类型
            "opponent_account",  # 对方账号
        ]

        parts = []
        seen = set()  # 避免重复内容

        for field_name in description_fields:
            value = bill.get(field_name, "")
            if value and str(value).strip():
                cleaned = str(value).strip()
                # 过滤无意义的值
                if cleaned not in ["/", "-", "无", "空", "", "nan", "None"]:
                    if cleaned not in seen:
                        parts.append(cleaned)
                        seen.add(cleaned)

        return " | ".join(parts) if parts else ""

    @log_method
    def post_process(self, bills: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """
        后处理账单数据，转换为标准格式

        标准格式字段:
        - date: 交易时间 (YYYY-MM-DD HH:MM:SS)
        - amount: 金额 (支出为负数, 收入为正数)
        - type: 类型 (收入/支出/转账/投资/退款)
        - description: 聚合描述 (商品+对方+备注等)
        -   : 解析器标识
        - counterparty: 交易对方
        - payment_method: 支付方式
        - original_type: 原始交易类型
        - original_category: 原始分类

        参数：
            bills: 原始账单列表

        返回：
            List[Dict]: 标准格式账单列表
        """
        # pylint: disable=too-many-branches
        processed_bills = []

        for bill in bills:
            try:
                # 创建标准格式账单
                processed_bill = {}

                # 1. 处理交易时间
                time_value = bill.get("date") or bill.get("trade_time", "")
                if time_value:
                    processed_bill["date"] = self.normalize_date(time_value)
                else:
                    self.logger.warning("账单缺少交易时间")
                    continue

                # 2. 处理交易类型
                # v6.36规则: 在智能配对成功之前，所有账单类型只能是收入或支出
                # 投资/转账类型只在SmartDeduplicationEngine配对成功后设置
                type_value = bill.get("type", "")
                original_type = type_value
                original_category = bill.get("original_category", "") or bill.get("category", "")

                # 类型规范化，但不自动设置为投资（即使original_category包含投资理财）
                if type_value:
                    normalized_type = self.normalize_type(type_value)
                    # 如果normalize_type返回投资/转账，改为根据金额符号判断收入/支出
                    if normalized_type in ["投资", "转账"]:
                        # 保持原始金额符号判断: 正数=收入, 负数=支出
                        amount_value = bill.get("amount", 0)
                        amount = self.normalize_amount(amount_value)
                        processed_bill["type"] = "收入" if amount >= 0 else "支出"
                    else:
                        processed_bill["type"] = normalized_type
                else:
                    processed_bill["type"] = "支出"

                # 3. 处理金额 - 统一为带符号的数值
                amount_value = bill.get("amount", 0)
                amount = self.normalize_amount(amount_value)

                # 根据类型调整金额符号
                if processed_bill["type"] == "支出":
                    processed_bill["amount"] = -abs(amount)  # 支出为负
                elif processed_bill["type"] in ["收入", "退款"]:
                    processed_bill["amount"] = abs(amount)  # 收入为正
                else:
                    # 转账/投资保持原值
                    processed_bill["amount"] = amount

                # 4. 处理来源账户 - 使用解析器标识
                processed_bill["source_account_id"] = self.PARSER_ID

                # 5. 聚合描述字段
                processed_bill["description"] = self.aggregate_description(bill)

                # 6. 保留原始字段
                processed_bill["counterparty"] = str(bill.get("counterparty", ""))
                processed_bill["payment_method"] = str(
                    bill.get("payment_method", "") or bill.get("channel", "")
                )
                processed_bill["parser_tags"] = resolve_parser_tags(
                    bill.get("parser_tags"),
                    parser_id=self.PARSER_ID,
                    payment_method=processed_bill["payment_method"],
                    channel=str(bill.get("channel", "")),
                )
                processed_bill["original_type"] = original_type
                processed_bill["original_category"] = original_category
                processed_bill["transaction_id"] = str(
                    bill.get("transaction_id", "") or bill.get("order_id", "")
                )
                processed_bill["merchant_id"] = str(bill.get("merchant_id", ""))
                processed_bill["status"] = str(bill.get("status", ""))

                # 7. 分类字段初始为空
                processed_bill["main_category"] = ""
                processed_bill["sub_category"] = ""

                # 验证必需字段
                if processed_bill["date"] and processed_bill["amount"] != 0:
                    processed_bills.append(processed_bill)
                else:
                    self.logger.warning(
                        "账单数据不完整: date=%s, amount=%s",
                        processed_bill.get("date"),
                        processed_bill.get("amount"),
                    )

            except Exception as exc:  # pylint: disable=broad-except
                self.logger.error("处理账单时出错: %s", exc)

        self.logger.info("后处理完成: %d/%d 条有效", len(processed_bills), len(bills))
        return processed_bills
