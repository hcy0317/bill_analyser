"""Exchange-rate provider public facade."""

import aiohttp
import pandas as pd

from .base import ExchangeRateProvider
from .china import BOCChinaProvider, CMBChinaProvider
from .global_providers import BOCProvider, ECBProvider, NBPProvider, RBAProvider, SNBProvider
from .manager import ExchangeRateManager
from .parsing import (
    CHINESE_CURRENCY_NAME_MAP,
    _convert_cny_quote_map_to_rates,
    _extract_numeric_values,
    _normalize_chinese_currency_name,
)

__all__ = [
    "BOCChinaProvider",
    "BOCProvider",
    "CHINESE_CURRENCY_NAME_MAP",
    "CMBChinaProvider",
    "ECBProvider",
    "ExchangeRateManager",
    "ExchangeRateProvider",
    "NBPProvider",
    "RBAProvider",
    "SNBProvider",
    "_convert_cny_quote_map_to_rates",
    "_extract_numeric_values",
    "_normalize_chinese_currency_name",
    "aiohttp",
    "pd",
]
