"""Shared exchange-rate provider base class."""

import ssl
from abc import ABC, abstractmethod

import aiohttp
import certifi

from ...utils.logger import get_logger


class ExchangeRateProvider(ABC):
    """Base class for exchange-rate providers."""

    def __init__(self):
        self.logger = get_logger(self.__class__.__name__)
        self.timeout = aiohttp.ClientTimeout(total=30)
        self.ssl_context = ssl.create_default_context(cafile=certifi.where())

    @abstractmethod
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates keyed by target currency."""
        raise NotImplementedError

    @abstractmethod
    def get_name(self) -> str:
        """Return provider display name."""
        raise NotImplementedError

    @abstractmethod
    def get_supported_currencies(self) -> list[str]:
        """Return supported currency codes."""
        raise NotImplementedError

    # pylint: disable-next=too-many-arguments,too-many-positional-arguments
    def _convert_base_currency(
        self,
        rates: dict[str, float],
        original_base: str,
        target_base: str,
        target_currencies: list[str],
        rate_format: str = "base_to_target",
    ) -> dict[str, float]:
        """Convert rates from one base currency into another."""
        if original_base == target_base:
            return {
                currency: rates.get(currency, 0)
                for currency in target_currencies
                if currency in rates
            }

        result = {}
        base_rate = rates.get(target_base)
        if not base_rate:
            self.logger.warning("目标基准货币 %s 不在汇率数据中", target_base)
            return result

        for currency in target_currencies:
            if currency == target_base:
                result[currency] = 1.0
            elif currency == original_base:
                if rate_format == "base_to_target":
                    result[currency] = 1.0 / base_rate
                else:
                    result[currency] = base_rate
            elif currency in rates:
                if rate_format == "base_to_target":
                    result[currency] = rates[currency] / base_rate
                else:
                    result[currency] = base_rate / rates[currency]

        return result
