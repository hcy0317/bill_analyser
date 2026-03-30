"""Authentication domain constants and static templates."""

from __future__ import annotations

from typing import Final

TOKEN_TYPE_DEFAULT: Final[int] = 0
TOKEN_TYPE_MCP: Final[int] = 5
TOKEN_TYPE_API: Final[int] = 8

CLOUD_SETTING_TYPE_STRING: Final[str] = "string"
CLOUD_SETTING_TYPE_NUMBER: Final[str] = "number"
CLOUD_SETTING_TYPE_BOOLEAN: Final[str] = "boolean"
CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP: Final[str] = "string_boolean_map"

SUPPORTED_APPLICATION_CLOUD_SETTING_KEY_TYPES: Final[dict[str, str]] = {
    "showAccountBalance": CLOUD_SETTING_TYPE_BOOLEAN,
    "showAmountInHomePage": CLOUD_SETTING_TYPE_BOOLEAN,
    "timezoneUsedForStatisticsInHomePage": CLOUD_SETTING_TYPE_NUMBER,
    "overviewAccountFilterInHomePage": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "overviewTransactionCategoryFilterInHomePage": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "itemsCountInTransactionListPage": CLOUD_SETTING_TYPE_NUMBER,
    "showTotalAmountInTransactionListPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "showTagInTransactionListPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "autoSaveTransactionDraft": CLOUD_SETTING_TYPE_STRING,
    "autoGetCurrentGeoLocation": CLOUD_SETTING_TYPE_BOOLEAN,
    "alwaysShowTransactionPicturesInMobileTransactionEditPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "totalAmountExcludeAccountIds": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "currencySortByInExchangeRatesPage": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultChartDataType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTimezoneType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAccountFilter": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "statistics.defaultTransactionCategoryFilter": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "statistics.defaultSortingType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultCategoricalChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultCategoricalChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTrendChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTrendChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAssetTrendsChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAssetTrendsChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
}

AUTH_DEFAULT_ACCOUNT_TEMPLATES: Final[dict[str, tuple[dict[str, object], ...]]] = {
    "zh": (
        {
            "name": "现金",
            "type": 1,
            "category": 1,
            "currency": "CNY",
            "icon": "1",
            "color": "4caf50",
            "aliases": ["现金", "现金钱包", "cash"],
            "display_order": 0,
        },
        {
            "name": "借记卡",
            "type": 1,
            "category": 2,
            "currency": "CNY",
            "icon": "100",
            "color": "2196f3",
            "aliases": ["借记卡", "储蓄卡", "银行卡", "debit card"],
            "display_order": 1,
        },
        {
            "name": "信用卡",
            "type": 1,
            "category": 3,
            "currency": "CNY",
            "icon": "100",
            "color": "ff9800",
            "aliases": ["信用卡", "贷记卡", "credit card"],
            "display_order": 2,
        },
        {
            "name": "支付宝",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "1677ff",
            "aliases": ["支付宝", "alipay", "花呗", "余额宝"],
            "display_order": 3,
        },
        {
            "name": "微信",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "07c160",
            "aliases": ["微信", "微信支付", "wechat"],
            "display_order": 4,
        },
    ),
    "default": (
        {
            "name": "Cash",
            "type": 1,
            "category": 1,
            "currency": "CNY",
            "icon": "1",
            "color": "4caf50",
            "aliases": ["cash", "wallet"],
            "display_order": 0,
        },
        {
            "name": "Debit Card",
            "type": 1,
            "category": 2,
            "currency": "CNY",
            "icon": "100",
            "color": "2196f3",
            "aliases": ["debit card", "bank card", "checking"],
            "display_order": 1,
        },
        {
            "name": "Credit Card",
            "type": 1,
            "category": 3,
            "currency": "CNY",
            "icon": "100",
            "color": "ff9800",
            "aliases": ["credit card"],
            "display_order": 2,
        },
        {
            "name": "Alipay",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "1677ff",
            "aliases": ["alipay"],
            "display_order": 3,
        },
        {
            "name": "WeChat",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "07c160",
            "aliases": ["wechat", "wechat pay"],
            "display_order": 4,
        },
    ),
}
