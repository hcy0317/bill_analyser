"""Bills domain constants and heuristic thresholds."""

from __future__ import annotations

from types import MappingProxyType

ALLOWED_BILLS_FILE_EXTENSIONS = frozenset({"csv", "xlsx", "xls", "txt"})
ALLOWED_BILLS_PICTURE_EXTENSIONS = frozenset({"png", "jpg", "jpeg", "gif", "webp", "bmp"})
MAX_BILLS_FILE_SIZE = 10 * 1024 * 1024

IMPORT_COLUMN_TYPE_KEYWORDS = MappingProxyType(
	{
		1: ("交易时间", "入账时间", "记账时间", "发生时间", "交易日期", "时间", "日期", "datetime", "date", "time"),
		2: ("时区", "timezone", "tz"),
		3: ("交易类型", "收支类型", "收支", "收/支", "类型", "类别", "type"),
		4: ("分类", "一级分类", "主分类", "category"),
		5: ("子分类", "二级分类", "次分类", "subcategory", "subcategoryname"),
		6: ("账户", "账户名", "账户名称", "账号", "付款账户", "支付账户", "account"),
		7: ("币种", "货币", "currency"),
		8: ("金额", "交易金额", "发生金额", "收支金额", "amount", "money"),
		9: (
			"对方账户",
			"对方账号",
			"对方名称",
			"对方户名",
			"相关账户",
			"转入账户",
			"目标账户",
			"收款账户",
			"destinationaccount",
			"relatedaccount",
		),
		10: ("对方币种", "目标币种", "转入币种", "destinationcurrency", "relatedcurrency"),
		11: ("对方金额", "目标金额", "转入金额", "收款金额", "destinationamount", "relatedamount"),
		12: ("地理位置", "位置", "经纬度", "坐标", "location", "geolocation"),
		13: ("标签", "标记", "tags", "tag"),
		14: (
			"备注",
			"摘要",
			"描述",
			"说明",
			"附言",
			"用途",
			"memo",
			"remark",
			"description",
			"note",
			"detail",
		),
	}
)

LEGACY_IMPORT_FIELD_TO_COLUMN_TYPE = MappingProxyType(
	{
		"date": 1,
		"time": 1,
		"type": 3,
		"category": 4,
		"subcategory": 5,
		"sub_category": 5,
		"account": 6,
		"accountname": 6,
		"currency": 7,
		"amount": 8,
		"relatedaccount": 9,
		"relatedaccountname": 9,
		"relatedcurrency": 10,
		"relatedamount": 11,
		"geolocation": 12,
		"tags": 13,
		"description": 14,
		"comment": 14,
		"memo": 14,
	}
)

AUTO_TRANSACTION_TYPE_MAPPING = MappingProxyType(
	{
		"支出": 3,
		"收入": 2,
		"支": 3,
		"收": 2,
		"转账": 4,
		"投资": 5,
		"退款": 2,
		"expense": 3,
		"income": 2,
		"transfer": 4,
		"investment": 5,
	}
)

GENERIC_IMPORT_DATE_HEADERS = ("交易日期", "日期", "入账日期", "记账日期")
GENERIC_IMPORT_TIME_HEADERS = ("交易时间", "入账时间", "记账时间", "发生时间", "时间")
GENERIC_IMPORT_INCOME_AMOUNT_HEADERS = ("收入金额", "存入金额", "贷方金额", "入账金额", "收款金额", "收入")
GENERIC_IMPORT_EXPENSE_AMOUNT_HEADERS = ("支出金额", "借方金额", "出账金额", "付款金额", "付出金额", "支出")

DEFAULT_BILL_CATEGORY_MAPPING = MappingProxyType(
	{
		"收入": ("工资", ""),
		"支出": ("其他", "日常支出"),
		"转账": ("转账", ""),
		"投资": ("投资理财", "证券投资"),
	}
)

IMPORT_CONFIG_BASE_WEIGHT = 3.0
IMPORT_CONFIG_USE_COUNT_CAP = 20.0
IMPORT_CONFIG_USE_COUNT_FACTOR = 0.1

IMPORT_HEADER_STRONG_EXACT_SCORE = 12.0
IMPORT_HEADER_EXACT_SCORE = 10.0
IMPORT_HEADER_SEMANTIC_MEDIUM_SCORE = 9.5
IMPORT_HEADER_SEMANTIC_SCORE = 9.0
IMPORT_HEADER_TYPE_HINT_SCORE = 8.0
IMPORT_HEADER_CONTEXT_SCORE = 7.0
IMPORT_HEADER_PARTIAL_SCORE = 6.0
IMPORT_HEADER_CANDIDATE_MIN_SCORE = 5.0

IMPORT_HEADER_MATCH_UNIQUE_BONUS = 2.5
IMPORT_HEADER_MATCH_REPEAT_BONUS = 0.5
IMPORT_HEADER_MATCHED_TYPES_WEIGHT = 3.0
IMPORT_HEADER_NON_EMPTY_CELL_WEIGHT = 0.5
IMPORT_HEADER_NON_EMPTY_CELL_CAP = 6
IMPORT_HEADER_DATA_LIKE_PENALTY = 2.0
IMPORT_HEADER_LOW_MATCH_TYPES_PENALTY = 6.0
IMPORT_HEADER_MIN_MATCHED_TYPES = 2
IMPORT_HEADER_MIN_REVIEW_SCORE = 8.0
IMPORT_HEADER_ACCEPT_SCORE = 10.0
IMPORT_HEADER_SCAN_LIMIT = 30
IMPORT_HEADER_NEXT_ROW_SCORE_CAP = 4.0
IMPORT_HEADER_NEXT_ROW_WEIGHT = 1.5
