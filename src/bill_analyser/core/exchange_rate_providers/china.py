"""China-based exchange-rate providers."""

import json
import re
from io import StringIO

import aiohttp
import pandas as pd

from ...utils.logger import log_method
from .base import ExchangeRateProvider
from .parsing import (
    CHINESE_CURRENCY_NAME_MAP,
    _convert_cny_quote_map_to_rates,
    _extract_numeric_values,
    _normalize_chinese_currency_name,
)


class BOCChinaProvider(ExchangeRateProvider):
    """Bank of China foreign exchange quotations."""

    BASE_URL = "https://www.boc.cn/sourcedb/whpj/"

    def get_name(self) -> str:
        return "Bank of China (CN)"

    def get_supported_currencies(self) -> list[str]:
        return ["CNY"] + sorted(set(CHINESE_CURRENCY_NAME_MAP.values()) - {"CNY"})

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch exchange rates from the Bank of China quotation page."""
        del date

        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error("中国银行汇率页面请求失败: %s", response.status)
                        return {}

                    html = await response.text()
                    quote_map = self._parse_quote_map(html)
                    return _convert_cny_quote_map_to_rates(
                        quote_map,
                        base_currency,
                        target_currencies,
                    )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取中国银行汇率失败: %s", exc)
            return {}

    def _parse_quote_map(self, html: str) -> dict[str, float]:
        """Parse Bank of China quotation tables."""
        quote_map: dict[str, float] = {}

        try:
            tables = pd.read_html(StringIO(html), flavor="lxml")
        except ValueError as exc:
            self.logger.error("解析中国银行页面表格失败: %s", exc)
            return quote_map

        for table in tables:
            if table.empty:
                continue

            for row in table.itertuples(index=False):
                row_values = list(row)
                currency_code = _normalize_chinese_currency_name(str(row_values[0]))
                if not currency_code or currency_code == "CNY":
                    continue

                numeric_values = _extract_numeric_values(row_values[1:])
                if not numeric_values:
                    continue

                quote = numeric_values[-1]
                if quote > 0:
                    quote_map[currency_code] = quote

        self.logger.debug("中国银行解析到 %d 个报价", len(quote_map))
        return quote_map


class CMBChinaProvider(ExchangeRateProvider):
    """China Merchants Bank real-time exchange rates."""

    BASE_URL = "https://fx.cmbchina.com/hq/"
    API_URL = "https://fx.cmbchina.com/api/v1/fx/rate"

    def get_name(self) -> str:
        return "China Merchants Bank (CMB)"

    def get_supported_currencies(self) -> list[str]:
        return ["CNY"] + sorted(set(CHINESE_CURRENCY_NAME_MAP.values()) - {"CNY"})

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        """Fetch exchange rates from the CMB JSON API or HTML fallback."""
        del date

        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                quote_map = await self._fetch_quote_map_from_api(session)

                if not quote_map:
                    self.logger.warning("招商银行接口未返回有效数据，回退到页面解析")
                    async with session.get(self.BASE_URL) as response:
                        if response.status != 200:
                            self.logger.error("招商银行汇率页面请求失败: %s", response.status)
                            return {}

                        html = await response.text()
                        quote_map = self._parse_quote_map(html)

                return _convert_cny_quote_map_to_rates(
                    quote_map,
                    base_currency,
                    target_currencies,
                )
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("获取招商银行汇率失败: %s", exc)
            return {}

    async def _fetch_quote_map_from_api(self, session: aiohttp.ClientSession) -> dict[str, float]:
        """Fetch quotation data from the CMB JSON API."""
        async with session.get(self.API_URL) as response:
            if response.status != 200:
                self.logger.error("招商银行汇率接口请求失败: %s", response.status)
                return {}

            raw_content = await response.read()
            try:
                payload = json.loads(raw_content.decode("utf-8", errors="replace"))
            except json.JSONDecodeError as exc:
                self.logger.error("解析招商银行汇率接口响应失败: %s", exc)
                return {}

            return self._parse_quote_map_from_api_payload(payload)

    def _parse_quote_map_from_api_payload(self, payload: dict[str, object]) -> dict[str, float]:
        """Parse quotation rows returned by the CMB JSON API."""
        quote_map: dict[str, float] = {}
        rows = payload.get("body")

        if not isinstance(rows, list):
            self.logger.error("招商银行汇率接口返回格式异常: body 不是列表")
            return quote_map

        for row in rows:
            if not isinstance(row, dict):
                continue

            currency_code = self._extract_currency_code_from_api_row(row)
            if not currency_code or currency_code == "CNY":
                continue

            quote_candidates: list[float] = []
            for field_name in ["rthOfr", "rthBid", "rtcOfr", "rtcBid", "rtbBid"]:
                field_value = row.get(field_name)
                if field_value in [None, ""]:
                    continue

                try:
                    numeric_value = float(str(field_value).replace(",", "").strip())
                except ValueError:
                    continue

                if numeric_value > 0:
                    quote_candidates.append(numeric_value)

            if quote_candidates:
                quote_map[currency_code] = sum(quote_candidates) / len(quote_candidates)

        self.logger.debug("招商银行接口解析到 %d 个报价", len(quote_map))
        return quote_map

    @staticmethod
    def _extract_currency_code_from_api_row(row: dict[str, object]) -> str:
        """Extract a currency code from one CMB API row."""
        ccy_nbr_eng = str(row.get("ccyNbrEng", "") or "").strip()
        if ccy_nbr_eng:
            matched = re.search(r"\b([A-Z]{3})\b\s*$", ccy_nbr_eng)
            if matched:
                return matched.group(1)

        return _normalize_chinese_currency_name(str(row.get("ccyNbr", "") or ""))

    def _parse_quote_map(self, html: str) -> dict[str, float]:
        """Parse CMB real-time exchange-rate tables."""
        quote_map: dict[str, float] = {}

        try:
            tables = pd.read_html(StringIO(html), flavor="lxml")
        except ValueError as exc:
            self.logger.error("解析招商银行页面表格失败: %s", exc)
            return quote_map

        for table in tables:
            if table.empty:
                continue

            for row in table.itertuples(index=False):
                row_values = list(row)
                currency_code = _normalize_chinese_currency_name(str(row_values[0]))
                if not currency_code or currency_code == "CNY":
                    continue

                numeric_values = _extract_numeric_values(row_values[1:])
                if not numeric_values:
                    continue

                if len(numeric_values) >= 5 and abs(numeric_values[0] - 100.0) < 0.001:
                    quote_candidates = numeric_values[1:5]
                else:
                    quote_candidates = numeric_values[:4]

                valid_quotes = [value for value in quote_candidates if value > 0]
                if valid_quotes:
                    quote_map[currency_code] = sum(valid_quotes) / len(valid_quotes)

        self.logger.debug("招商银行解析到 %d 个报价", len(quote_map))
        return quote_map
