"""Central-bank and global exchange-rate providers."""

# pylint: disable=duplicate-code

import re
import xml.etree.ElementTree as ET

import aiohttp

from ...utils.logger import log_method
from .base import ExchangeRateProvider


class ECBProvider(ExchangeRateProvider):
    """European Central Bank provider."""

    BASE_URL = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml"
    HIST_URL = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.xml"

    SUPPORTED_CURRENCIES = [
        "USD",
        "JPY",
        "BGN",
        "CZK",
        "DKK",
        "GBP",
        "HUF",
        "PLN",
        "RON",
        "SEK",
        "CHF",
        "ISK",
        "NOK",
        "TRY",
        "AUD",
        "BRL",
        "CAD",
        "CNY",
        "HKD",
        "IDR",
        "ILS",
        "INR",
        "KRW",
        "MXN",
        "MYR",
        "NZD",
        "PHP",
        "SGD",
        "THB",
        "ZAR",
    ]

    def get_name(self) -> str:
        return "European Central Bank (ECB)"

    def get_supported_currencies(self) -> list[str]:
        return ["EUR"] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates from ECB XML feeds."""
        try:
            url = self.BASE_URL if date is None else self.HIST_URL

            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error("ECB API请求失败: %s", response.status)
                        return {}

                    xml_data = await response.text()
                    rates = self._parse_ecb_xml(xml_data, date)
                    return self._convert_base_currency(
                        rates,
                        "EUR",
                        base_currency,
                        target_currencies,
                        rate_format="base_to_target",
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取ECB汇率失败: %s", exc)
            return {}

    def _parse_ecb_xml(self, xml_data: str, target_date: str | None = None) -> dict[str, float]:
        """Parse ECB XML data."""
        rates = {"EUR": 1.0}

        try:
            root = ET.fromstring(xml_data)
            ns = {
                "gesmes": "http://www.gesmes.org/xml/2002-08-01",
                "default": "http://www.ecb.int/vocabulary/2002-08-01/eurofxref",
            }

            for cube_date in root.findall(".//default:Cube[@time]", ns):
                date = cube_date.get("time")

                if target_date is None or date == target_date:
                    for cube in cube_date.findall("default:Cube[@currency]", ns):
                        currency = cube.get("currency")
                        rate = float(cube.get("rate"))
                        rates[currency] = rate

                    if target_date is None:
                        break
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("解析ECB XML失败: %s", exc)

        return rates


class BOCProvider(ExchangeRateProvider):
    """Bank of Canada provider."""

    BASE_URL = "https://www.bankofcanada.ca/valet/observations"
    SUPPORTED_CURRENCIES = ["USD", "EUR", "GBP", "JPY", "CNY", "AUD", "CHF", "MXN", "INR", "BRL"]

    def get_name(self) -> str:
        return "Bank of Canada (BOC)"

    def get_supported_currencies(self) -> list[str]:
        return ["CAD"] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates from the Bank of Canada Valet API."""
        # pylint: disable=too-many-locals
        try:
            series_map = {
                "USD": "FXUSDCAD",
                "EUR": "FXEURCAD",
                "GBP": "FXGBPCAD",
                "JPY": "FXJPYCAD",
                "CNY": "FXCNYCAD",
            }

            rates = {"CAD": 1.0}
            currencies_to_fetch = set(target_currencies)
            if base_currency in series_map:
                currencies_to_fetch.add(base_currency)

            series_codes = [
                series_map[currency]
                for currency in currencies_to_fetch
                if currency in series_map
            ]
            if not series_codes:
                return {}

            series_str = ",".join(series_codes)
            url = f"{self.BASE_URL}/{series_str}/json"

            if date:
                url += f"?start_date={date}&end_date={date}"

            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error("BOC API请求失败: %s", response.status)
                        return {}

                    data = await response.json()

                    for obs in data.get("observations", []):
                        for currency, series_code in series_map.items():
                            if series_code in obs:
                                rate_value = obs[series_code].get("v")
                                if rate_value:
                                    rates[currency] = float(rate_value)

                    return self._convert_base_currency(
                        rates,
                        "CAD",
                        base_currency,
                        target_currencies,
                        rate_format="target_to_base",
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取BOC汇率失败: %s", exc)
            return {}


class RBAProvider(ExchangeRateProvider):
    """Reserve Bank of Australia provider."""

    BASE_URL = "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml"
    SUPPORTED_CURRENCIES = ["USD", "EUR", "GBP", "JPY", "CNY", "NZD", "CAD", "CHF", "SGD", "THB"]

    def get_name(self) -> str:
        return "Reserve Bank of Australia (RBA)"

    def get_supported_currencies(self) -> list[str]:
        return ["AUD"] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates from the RBA RSS feed."""
        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error("RBA API请求失败: %s", response.status)
                        return {}

                    xml_data = await response.text()
                    rates = self._parse_rba_xml(xml_data)
                    return self._convert_base_currency(
                        rates,
                        "AUD",
                        base_currency,
                        target_currencies,
                        rate_format="base_to_target",
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取RBA汇率失败: %s", exc)
            return {}

    def _parse_rba_xml(self, xml_data: str) -> dict[str, float]:
        """Parse RBA RSS XML data."""
        rates = {"AUD": 1.0}

        try:
            item_pattern = r"<item[^>]*>.*?</item>"
            items = re.findall(item_pattern, xml_data, re.DOTALL)

            for item in items:
                currency_match = re.search(r"<cb:targetCurrency>(\w+)</cb:targetCurrency>", item)
                value_match = re.search(r"<cb:value>([0-9.]+)</cb:value>", item)

                if currency_match and value_match:
                    currency = currency_match.group(1)
                    rate_value = float(value_match.group(1))
                    rates[currency] = rate_value
                    self.logger.debug("RBA 解析: %s = %s", currency, rate_value)
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("解析RBA XML失败: %s", exc)

        return rates


class NBPProvider(ExchangeRateProvider):
    """National Bank of Poland provider."""

    BASE_URL = "https://api.nbp.pl/api/exchangerates/tables/A"
    SUPPORTED_CURRENCIES = ["USD", "EUR", "GBP", "CHF", "JPY", "CZK", "DKK", "NOK", "SEK", "CAD"]

    def get_name(self) -> str:
        return "National Bank of Poland (NBP)"

    def get_supported_currencies(self) -> list[str]:
        return ["PLN"] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates from the NBP JSON API."""
        try:
            url = self.BASE_URL
            if date:
                url += f"/{date}"
            url += "?format=json"

            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error("NBP API请求失败: %s", response.status)
                        return {}

                    data = await response.json()
                    rates = self._parse_nbp_json(data)
                    return self._convert_base_currency(
                        rates,
                        "PLN",
                        base_currency,
                        target_currencies,
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取NBP汇率失败: %s", exc)
            return {}

    def _parse_nbp_json(self, data: list[dict]) -> dict[str, float]:
        """Parse NBP JSON table data."""
        rates = {"PLN": 1.0}

        try:
            if data and len(data) > 0:
                table = data[0]
                for rate_info in table.get("rates", []):
                    currency = rate_info.get("code")
                    mid_rate = rate_info.get("mid")
                    if currency and mid_rate:
                        rates[currency] = float(mid_rate)
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("解析NBP JSON失败: %s", exc)

        return rates


class SNBProvider(ExchangeRateProvider):
    """Swiss National Bank provider."""

    BASE_URL = "https://data.snb.ch/api/cube/devkum/data/csv/en"
    SUPPORTED_CURRENCIES = ["EUR", "USD", "GBP", "JPY", "CAD", "AUD", "SEK", "DKK", "NOK"]

    def get_name(self) -> str:
        return "Swiss National Bank (SNB)"

    def get_supported_currencies(self) -> list[str]:
        return ["CHF"] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch rates from the SNB CSV endpoint."""
        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error("SNB API请求失败: %s", response.status)
                        return {}

                    csv_data = await response.text()
                    rates = self._parse_snb_csv(csv_data, date)
                    return self._convert_base_currency(
                        rates,
                        "CHF",
                        base_currency,
                        target_currencies,
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取SNB汇率失败: %s", exc)
            return {}

    def _parse_snb_csv(
        self,
        csv_data: str,
        target_date: str | None = None,
    ) -> dict[str, float]:
        """Parse SNB CSV data."""
        del target_date
        rates = {"CHF": 1.0}

        try:
            lines = csv_data.strip().split("\n")
            if len(lines) > 1:
                pass
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("解析SNB CSV失败: %s", exc)

        return rates
