"""
Exchange Rate Providers - 汇率数据源提供者

支持多个央行和金融机构的汇率数据获取

v6.79: 添加SSL证书支持，修复SSLCertVerificationError问题
"""

import aiohttp
import asyncio
import json
import re
import ssl
import certifi
from abc import ABC, abstractmethod
from datetime import datetime, timedelta
from io import StringIO
from typing import Dict, List, Optional
import xml.etree.ElementTree as ET
import pandas as pd

from ..utils.logger import get_logger, log_method


CHINESE_CURRENCY_NAME_MAP = {
    '人民币': 'CNY',
    '美元': 'USD',
    '欧元': 'EUR',
    '英镑': 'GBP',
    '日元': 'JPY',
    '港币': 'HKD',
    '韩元': 'KRW',
    '韩国元': 'KRW',
    '澳大利亚元': 'AUD',
    '澳币': 'AUD',
    '加拿大元': 'CAD',
    '加拿大币': 'CAD',
    '新加坡元': 'SGD',
    '新加坡币': 'SGD',
    '新台币': 'TWD',
    '林吉特': 'MYR',
    '马来币': 'MYR',
    '泰国铢': 'THB',
    '泰币': 'THB',
    '越南盾': 'VND',
    '瑞士法郎': 'CHF',
    '新西兰元': 'NZD',
    '纽元': 'NZD'
}


def _normalize_chinese_currency_name(value: str) -> str:
    """将中文币种名称规范化为标准货币代码。"""
    text = str(value or '').strip()
    text = re.sub(r'\s*\([^)]*\)', '', text)
    text = text.replace('（', '(').replace('）', ')')
    text = text.replace(' ', '')
    return CHINESE_CURRENCY_NAME_MAP.get(text, '')


def _extract_numeric_values(values: List[object]) -> List[float]:
    """从表格行中提取数值列。"""
    numbers: List[float] = []

    for value in values:
        text = str(value or '').strip()
        if not text or text in {'-', '--', 'nan', 'NaN'}:
            continue
        if ':' in text:
            continue

        text = text.replace(',', '')
        if re.fullmatch(r'-?\d+(?:\.\d+)?', text):
            try:
                numbers.append(float(text))
            except ValueError:
                continue

    return numbers


def _convert_cny_quote_map_to_rates(
    quote_map: Dict[str, float],
    base_currency: str,
    target_currencies: List[str]
) -> Dict[str, float]:
    """将“100 外币兑人民币”的报价转换为任意基准币种的汇率。"""
    result: Dict[str, float] = {}

    if base_currency == 'CNY':
        for currency in target_currencies:
            quote = quote_map.get(currency)
            if quote and quote > 0:
                result[currency] = 100.0 / quote
        return result

    base_quote = quote_map.get(base_currency)
    if not base_quote or base_quote <= 0:
        return result

    for currency in target_currencies:
        if currency == 'CNY':
            result[currency] = base_quote / 100.0
            continue

        target_quote = quote_map.get(currency)
        if target_quote and target_quote > 0:
            result[currency] = base_quote / target_quote

    return result


class ExchangeRateProvider(ABC):
    """汇率提供者基类"""

    def __init__(self):
        self.logger = get_logger(self.__class__.__name__)
        self.timeout = aiohttp.ClientTimeout(total=30)
        # v6.79: 创建使用certifi证书的SSL上下文，解决证书验证失败问题
        self.ssl_context = ssl.create_default_context(cafile=certifi.where())

    @abstractmethod
    async def fetch_rates(
        self, 
        base_currency: str, 
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """
        获取汇率
        
        Args:
            base_currency: 基准货币代码
            target_currencies: 目标货币列表
            date: 日期 (YYYY-MM-DD)，None表示最新
            
        Returns:
            Dict[str, float]: {货币代码: 汇率}
        """
        pass

    @abstractmethod
    def get_name(self) -> str:
        """获取提供者名称"""
        pass

    @abstractmethod
    def get_supported_currencies(self) -> List[str]:
        """获取支持的货币列表"""
        pass

    def _convert_base_currency(
        self,
        rates: Dict[str, float],
        original_base: str,
        target_base: str,
        target_currencies: List[str],
        rate_format: str = 'base_to_target'
    ) -> Dict[str, float]:
        """
        转换基准货币
        
        将以 original_base 为基准的汇率转换为以 target_base 为基准的汇率
        
        Args:
            rates: 原始汇率字典 {货币: 汇率}
            original_base: 原始基准货币 (如 EUR, CAD)
            target_base: 目标基准货币 (如 CNY)
            target_currencies: 需要返回的目标货币列表
            rate_format: 汇率数据格式
                - 'base_to_target': rates[X] = "1 original_base = ? X"
                  例如 ECB: rates['CNY']=7.69 表示 1 EUR = 7.69 CNY
                - 'target_to_base': rates[X] = "1 X = ? original_base"
                  例如 BOC: rates['USD']=1.38 表示 1 USD = 1.38 CAD
            
        Returns:
            Dict[str, float]: {货币: 相对于target_base的汇率}，格式为 "1 target_base = ? currency"
        """
        if original_base == target_base:
            return {c: rates.get(c, 0) for c in target_currencies if c in rates}
        
        result = {}
        base_rate = rates.get(target_base)
        if not base_rate:
            self.logger.warning(f"目标基准货币 {target_base} 不在汇率数据中")
            return result
        
        for currency in target_currencies:
            if currency == target_base:
                result[currency] = 1.0
            elif currency == original_base:
                # 原基准货币相对于新基准货币的汇率
                if rate_format == 'base_to_target':
                    # ECB格式: rates[CNY]=7.69 表示 1 EUR = 7.69 CNY
                    # CNY/EUR = 1/7.69 (1 CNY = 1/7.69 EUR)
                    result[currency] = 1.0 / base_rate
                else:
                    # BOC格式: rates[CNY]=0.19 表示 1 CNY = 0.19 CAD
                    # CNY/CAD = 0.19 (1 CNY = 0.19 CAD)
                    result[currency] = base_rate
            elif currency in rates:
                if rate_format == 'base_to_target':
                    # ECB格式: rates[CNY]=7.69, rates[USD]=1.09
                    # 表示 1 EUR = 7.69 CNY, 1 EUR = 1.09 USD
                    # CNY/USD = rates[USD] / rates[CNY] = 1.09/7.69 ≈ 0.1417
                    # (1 CNY = 0.1417 USD)
                    result[currency] = rates[currency] / base_rate
                else:
                    # BOC格式: rates[CNY]=0.19, rates[USD]=1.38
                    # 表示 1 CNY = 0.19 CAD, 1 USD = 1.38 CAD
                    # CNY/USD = rates[CNY] / rates[USD] = 0.19/1.38 ≈ 0.1377
                    # (1 CNY = 0.1377 USD)
                    result[currency] = base_rate / rates[currency]
                
        return result


class ECBProvider(ExchangeRateProvider):
    """欧洲央行 (European Central Bank)"""
    
    BASE_URL = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml"
    HIST_URL = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.xml"
    
    # ECB支持的货币（相对于EUR）
    SUPPORTED_CURRENCIES = [
        'USD', 'JPY', 'BGN', 'CZK', 'DKK', 'GBP', 'HUF', 'PLN', 'RON', 'SEK',
        'CHF', 'ISK', 'NOK', 'TRY', 'AUD', 'BRL', 'CAD', 'CNY', 'HKD', 'IDR',
        'ILS', 'INR', 'KRW', 'MXN', 'MYR', 'NZD', 'PHP', 'SGD', 'THB', 'ZAR'
    ]

    def get_name(self) -> str:
        return "European Central Bank (ECB)"

    def get_supported_currencies(self) -> List[str]:
        return ['EUR'] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从ECB获取汇率"""
        try:
            # ECB的基准货币是EUR
            url = self.BASE_URL if date is None else self.HIST_URL
            
            # v6.79: 使用SSL连接器解决证书验证问题
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error(f"ECB API请求失败: {response.status}")
                        return {}
                    
                    xml_data = await response.text()
                    rates = self._parse_ecb_xml(xml_data, date)
                    
                    # 转换基准货币 (ECB 使用 base_to_target 格式: rates[USD]=1.09 表示 1 EUR = 1.09 USD)
                    return self._convert_base_currency(
                        rates, 'EUR', base_currency, target_currencies, 
                        rate_format='base_to_target'
                    )
                    
        except Exception as e:
            self.logger.error(f"获取ECB汇率失败: {e}")
            return {}

    def _parse_ecb_xml(self, xml_data: str, target_date: Optional[str] = None) -> Dict[str, float]:
        """解析ECB XML数据"""
        rates = {'EUR': 1.0}
        
        try:
            root = ET.fromstring(xml_data)
            # ECB XML命名空间
            ns = {'gesmes': 'http://www.gesmes.org/xml/2002-08-01',
                  'default': 'http://www.ecb.int/vocabulary/2002-08-01/eurofxref'}
            
            # 查找最新或指定日期的汇率
            for cube_date in root.findall('.//default:Cube[@time]', ns):
                date = cube_date.get('time')
                
                if target_date is None or date == target_date:
                    for cube in cube_date.findall('default:Cube[@currency]', ns):
                        currency = cube.get('currency')
                        rate = float(cube.get('rate'))
                        rates[currency] = rate
                    
                    if target_date is None:
                        break  # 只取最新的
                    
        except Exception as e:
            self.logger.error(f"解析ECB XML失败: {e}")
            
        return rates


class BOCChinaProvider(ExchangeRateProvider):
    """中国银行外汇牌价。"""

    BASE_URL = 'https://www.boc.cn/sourcedb/whpj/'

    def get_name(self) -> str:
        return 'Bank of China (CN)'

    def get_supported_currencies(self) -> List[str]:
        return ['CNY'] + sorted(set(CHINESE_CURRENCY_NAME_MAP.values()) - {'CNY'})

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从中国银行页面抓取汇率。"""
        del date

        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(
                timeout=self.timeout,
                connector=connector
            ) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error('中国银行汇率页面请求失败: %s', response.status)
                        return {}

                    html = await response.text()
                    quote_map = self._parse_quote_map(html)
                    return _convert_cny_quote_map_to_rates(
                        quote_map,
                        base_currency,
                        target_currencies
                    )
        except Exception as exc:
            self.logger.error('获取中国银行汇率失败: %s', exc)
            return {}

    def _parse_quote_map(self, html: str) -> Dict[str, float]:
        """解析中国银行外汇牌价页面。"""
        quote_map: Dict[str, float] = {}

        try:
            tables = pd.read_html(StringIO(html), flavor='lxml')
        except ValueError as exc:
            self.logger.error('解析中国银行页面表格失败: %s', exc)
            return quote_map

        for table in tables:
            if table.empty:
                continue

            for row in table.itertuples(index=False):
                row_values = list(row)
                currency_code = _normalize_chinese_currency_name(str(row_values[0]))
                if not currency_code or currency_code == 'CNY':
                    continue

                numeric_values = _extract_numeric_values(row_values[1:])
                if not numeric_values:
                    continue

                quote = numeric_values[-1]
                if quote > 0:
                    quote_map[currency_code] = quote

        self.logger.debug('中国银行解析到 %d 个报价', len(quote_map))
        return quote_map


class CMBChinaProvider(ExchangeRateProvider):
    """招商银行实时汇率。"""

    BASE_URL = 'https://fx.cmbchina.com/hq/'
    API_URL = 'https://fx.cmbchina.com/api/v1/fx/rate'

    def get_name(self) -> str:
        return 'China Merchants Bank (CMB)'

    def get_supported_currencies(self) -> List[str]:
        return ['CNY'] + sorted(set(CHINESE_CURRENCY_NAME_MAP.values()) - {'CNY'})

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从招商银行接口抓取汇率，失败时回退到页面解析。"""
        del date

        try:
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(
                timeout=self.timeout,
                connector=connector
            ) as session:
                quote_map = await self._fetch_quote_map_from_api(session)

                if not quote_map:
                    self.logger.warning('招商银行接口未返回有效数据，回退到页面解析')
                    async with session.get(self.BASE_URL) as response:
                        if response.status != 200:
                            self.logger.error('招商银行汇率页面请求失败: %s', response.status)
                            return {}

                        html = await response.text()
                        quote_map = self._parse_quote_map(html)

                return _convert_cny_quote_map_to_rates(
                    quote_map,
                    base_currency,
                    target_currencies
                )
        except Exception as exc:
            self.logger.error('获取招商银行汇率失败: %s', exc)
            return {}

    async def _fetch_quote_map_from_api(self, session: aiohttp.ClientSession) -> Dict[str, float]:
        """优先使用招商银行 JSON 接口获取报价。"""
        async with session.get(self.API_URL) as response:
            if response.status != 200:
                self.logger.error('招商银行汇率接口请求失败: %s', response.status)
                return {}

            raw_content = await response.read()
            try:
                payload = json.loads(raw_content.decode('utf-8', errors='replace'))
            except json.JSONDecodeError as exc:
                self.logger.error('解析招商银行汇率接口响应失败: %s', exc)
                return {}

            return self._parse_quote_map_from_api_payload(payload)

    def _parse_quote_map_from_api_payload(self, payload: Dict[str, object]) -> Dict[str, float]:
        """解析招商银行 JSON 接口返回的报价。"""
        quote_map: Dict[str, float] = {}
        rows = payload.get('body')

        if not isinstance(rows, list):
            self.logger.error('招商银行汇率接口返回格式异常: body 不是列表')
            return quote_map

        for row in rows:
            if not isinstance(row, dict):
                continue

            currency_code = self._extract_currency_code_from_api_row(row)
            if not currency_code or currency_code == 'CNY':
                continue

            quote_candidates: List[float] = []
            for field_name in ['rthOfr', 'rthBid', 'rtcOfr', 'rtcBid', 'rtbBid']:
                field_value = row.get(field_name)
                if field_value in [None, '']:
                    continue

                try:
                    numeric_value = float(str(field_value).replace(',', '').strip())
                except ValueError:
                    continue

                if numeric_value > 0:
                    quote_candidates.append(numeric_value)

            if quote_candidates:
                quote_map[currency_code] = sum(quote_candidates) / len(quote_candidates)

        self.logger.debug('招商银行接口解析到 %d 个报价', len(quote_map))
        return quote_map

    @staticmethod
    def _extract_currency_code_from_api_row(row: Dict[str, object]) -> str:
        """从招商银行接口单行数据中提取货币代码。"""
        ccy_nbr_eng = str(row.get('ccyNbrEng', '') or '').strip()
        if ccy_nbr_eng:
            matched = re.search(r'\b([A-Z]{3})\b\s*$', ccy_nbr_eng)
            if matched:
                return matched.group(1)

        return _normalize_chinese_currency_name(str(row.get('ccyNbr', '') or ''))

    def _parse_quote_map(self, html: str) -> Dict[str, float]:
        """解析招商银行实时汇率页面。"""
        quote_map: Dict[str, float] = {}

        try:
            tables = pd.read_html(StringIO(html), flavor='lxml')
        except ValueError as exc:
            self.logger.error('解析招商银行页面表格失败: %s', exc)
            return quote_map

        for table in tables:
            if table.empty:
                continue

            for row in table.itertuples(index=False):
                row_values = list(row)
                currency_code = _normalize_chinese_currency_name(str(row_values[0]))
                if not currency_code or currency_code == 'CNY':
                    continue

                numeric_values = _extract_numeric_values(row_values[1:])
                if not numeric_values:
                    continue

                # 第一列通常是单位 100，后续 4 列是买卖价，取均值作为参考汇率。
                if len(numeric_values) >= 5 and abs(numeric_values[0] - 100.0) < 0.001:
                    quote_candidates = numeric_values[1:5]
                else:
                    quote_candidates = numeric_values[:4]

                valid_quotes = [value for value in quote_candidates if value > 0]
                if valid_quotes:
                    quote_map[currency_code] = sum(valid_quotes) / len(valid_quotes)

        self.logger.debug('招商银行解析到 %d 个报价', len(quote_map))
        return quote_map


class BOCProvider(ExchangeRateProvider):
    """加拿大银行 (Bank of Canada)"""
    
    BASE_URL = "https://www.bankofcanada.ca/valet/observations"
    
    # BOC支持的主要货币（相对于CAD）
    SUPPORTED_CURRENCIES = [
        'USD', 'EUR', 'GBP', 'JPY', 'CNY', 'AUD', 'CHF', 'MXN', 'INR', 'BRL'
    ]

    def get_name(self) -> str:
        return "Bank of Canada (BOC)"

    def get_supported_currencies(self) -> List[str]:
        return ['CAD'] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从BOC获取汇率"""
        try:
            # BOC API使用特定的系列代码
            series_map = {
                'USD': 'FXUSDCAD',
                'EUR': 'FXEURCAD',
                'GBP': 'FXGBPCAD',
                'JPY': 'FXJPYCAD',
                'CNY': 'FXCNYCAD'
            }
            
            rates = {'CAD': 1.0}
            
            # v6.79: 构建请求的货币列表，必须包含 base_currency 以支持基准货币转换
            currencies_to_fetch = set(target_currencies)
            if base_currency in series_map:
                currencies_to_fetch.add(base_currency)
            
            # 构建请求的系列列表
            series_codes = [series_map[c] for c in currencies_to_fetch if c in series_map]
            if not series_codes:
                return {}
            
            series_str = ','.join(series_codes)
            url = f"{self.BASE_URL}/{series_str}/json"
            
            if date:
                url += f"?start_date={date}&end_date={date}"
            
            # v6.79: 使用SSL连接器解决证书验证问题
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error(f"BOC API请求失败: {response.status}")
                        return {}
                    
                    data = await response.json()
                    
                    # 解析响应
                    for obs in data.get('observations', []):
                        for currency, series_code in series_map.items():
                            if series_code in obs:
                                rate_value = obs[series_code].get('v')
                                if rate_value:
                                    rates[currency] = float(rate_value)
                    
                    return self._convert_base_currency(
                        rates, 'CAD', base_currency, target_currencies,
                        rate_format='target_to_base'
                    )
                    
        except Exception as e:
            self.logger.error(f"获取BOC汇率失败: {e}")
            return {}


class RBAProvider(ExchangeRateProvider):
    """澳大利亚储备银行 (Reserve Bank of Australia)"""
    
    BASE_URL = "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml"
    
    SUPPORTED_CURRENCIES = [
        'USD', 'EUR', 'GBP', 'JPY', 'CNY', 'NZD', 'CAD', 'CHF', 'SGD', 'THB'
    ]

    def get_name(self) -> str:
        return "Reserve Bank of Australia (RBA)"

    def get_supported_currencies(self) -> List[str]:
        return ['AUD'] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从RBA获取汇率"""
        try:
            # v6.79: 使用SSL连接器解决证书验证问题
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error(f"RBA API请求失败: {response.status}")
                        return {}
                    
                    xml_data = await response.text()
                    rates = self._parse_rba_xml(xml_data)
                    
                    # RBA格式: rates['USD'] = 0.6602 表示 1 AUD = 0.6602 USD (base_to_target)
                    return self._convert_base_currency(
                        rates, 'AUD', base_currency, target_currencies,
                        rate_format='base_to_target'
                    )
                    
        except Exception as e:
            self.logger.error(f"获取RBA汇率失败: {e}")
            return {}

    def _parse_rba_xml(self, xml_data: str) -> Dict[str, float]:
        """解析RBA XML数据
        
        RBA 使用 RDF/XML 格式，标题格式为 "AU: 0.6602 USD = 1 AUD ..."
        表示 0.6602 USD = 1 AUD，即 1 AUD = 0.6602 USD
        
        我们使用正则表达式从 <cb:targetCurrency> 和 <cb:value> 标签提取数据
        """
        import re
        rates = {'AUD': 1.0}
        
        try:
            # 使用正则表达式提取每个 item 块
            # 格式: <cb:targetCurrency>USD</cb:targetCurrency> 和 <cb:value>0.6602</cb:value>
            item_pattern = r'<item[^>]*>.*?</item>'
            items = re.findall(item_pattern, xml_data, re.DOTALL)
            
            for item in items:
                # 提取目标货币
                currency_match = re.search(r'<cb:targetCurrency>(\w+)</cb:targetCurrency>', item)
                # 提取汇率值
                value_match = re.search(r'<cb:value>([0-9.]+)</cb:value>', item)
                
                if currency_match and value_match:
                    currency = currency_match.group(1)
                    rate_value = float(value_match.group(1))
                    rates[currency] = rate_value
                    self.logger.debug(f"RBA 解析: {currency} = {rate_value}")
                    
        except Exception as e:
            self.logger.error(f"解析RBA XML失败: {e}")
            
        return rates


class NBPProvider(ExchangeRateProvider):
    """波兰国家银行 (National Bank of Poland)"""
    
    BASE_URL = "https://api.nbp.pl/api/exchangerates/tables/A"
    
    SUPPORTED_CURRENCIES = [
        'USD', 'EUR', 'GBP', 'CHF', 'JPY', 'CZK', 'DKK', 'NOK', 'SEK', 'CAD'
    ]

    def get_name(self) -> str:
        return "National Bank of Poland (NBP)"

    def get_supported_currencies(self) -> List[str]:
        return ['PLN'] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从NBP获取汇率"""
        try:
            url = self.BASE_URL
            if date:
                url += f"/{date}"
            url += "?format=json"
            
            # v6.79: 使用SSL连接器解决证书验证问题
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error(f"NBP API请求失败: {response.status}")
                        return {}
                    
                    data = await response.json()
                    rates = self._parse_nbp_json(data)
                    
                    return self._convert_base_currency(rates, 'PLN', base_currency, target_currencies)
                    
        except Exception as e:
            self.logger.error(f"获取NBP汇率失败: {e}")
            return {}

    def _parse_nbp_json(self, data: List[Dict]) -> Dict[str, float]:
        """解析NBP JSON数据"""
        rates = {'PLN': 1.0}
        
        try:
            if data and len(data) > 0:
                table = data[0]
                for rate_info in table.get('rates', []):
                    currency = rate_info.get('code')
                    mid_rate = rate_info.get('mid')
                    if currency and mid_rate:
                        rates[currency] = float(mid_rate)
                        
        except Exception as e:
            self.logger.error(f"解析NBP JSON失败: {e}")
            
        return rates


class SNBProvider(ExchangeRateProvider):
    """瑞士国家银行 (Swiss National Bank)"""
    
    BASE_URL = "https://data.snb.ch/api/cube/devkum/data/csv/en"
    
    SUPPORTED_CURRENCIES = [
        'EUR', 'USD', 'GBP', 'JPY', 'CAD', 'AUD', 'SEK', 'DKK', 'NOK'
    ]

    def get_name(self) -> str:
        return "Swiss National Bank (SNB)"

    def get_supported_currencies(self) -> List[str]:
        return ['CHF'] + self.SUPPORTED_CURRENCIES

    @log_method
    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: List[str],
        date: Optional[str] = None
    ) -> Dict[str, float]:
        """从SNB获取汇率"""
        try:
            # SNB CSV格式较复杂，这里简化处理
            # v6.79: 使用SSL连接器解决证书验证问题
            connector = aiohttp.TCPConnector(ssl=self.ssl_context)
            async with aiohttp.ClientSession(timeout=self.timeout, connector=connector) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error(f"SNB API请求失败: {response.status}")
                        return {}
                    
                    csv_data = await response.text()
                    rates = self._parse_snb_csv(csv_data, date)
                    
                    return self._convert_base_currency(rates, 'CHF', base_currency, target_currencies)
                    
        except Exception as e:
            self.logger.error(f"获取SNB汇率失败: {e}")
            return {}

    def _parse_snb_csv(self, csv_data: str, target_date: Optional[str] = None) -> Dict[str, float]:
        """解析SNB CSV数据"""
        rates = {'CHF': 1.0}
        
        try:
            lines = csv_data.strip().split('\n')
            # 简化处理：只取最新数据
            # 实际实现需要更复杂的CSV解析
            if len(lines) > 1:
                # 这里需要根据实际SNB CSV格式实现
                pass
                
        except Exception as e:
            self.logger.error(f"解析SNB CSV失败: {e}")
            
        return rates


class ExchangeRateManager:
    """汇率管理器 - 协调多个提供者"""

    def __init__(self, db):
        """
        初始化汇率管理器
        
        Args:
            db: 数据库实例
        """
        self.db = db
        self.logger = get_logger('ExchangeRateManager')
        
        # 初始化所有提供者
        self.providers = {
            'ecb': ECBProvider(),
            'boc': BOCProvider(),
            'rba': RBAProvider(),
            'nbp': NBPProvider(),
            'snb': SNBProvider()
        }

    @log_method
    async def get_rate(
        self,
        from_currency: str,
        to_currency: str,
        date: Optional[str] = None
    ) -> Optional[float]:
        """
        获取汇率（优先从数据库，否则从API）
        
        Args:
            from_currency: 源货币
            to_currency: 目标货币
            date: 日期，None表示最新
            
        Returns:
            float: 汇率，失败返回None
        """
        if from_currency == to_currency:
            return 1.0
        
        # 1. 先从数据库查询
        conn = await self.db._get_connection()
        
        if date:
            query = """
                SELECT rate FROM user_exchange_rates
                WHERE from_currency = ? AND to_currency = ?
                AND effective_date = ?
                ORDER BY created_at DESC LIMIT 1
            """
            cursor = await conn.execute(query, (from_currency, to_currency, date))
        else:
            query = """
                SELECT rate FROM user_exchange_rates
                WHERE from_currency = ? AND to_currency = ?
                ORDER BY effective_date DESC, created_at DESC LIMIT 1
            """
            cursor = await conn.execute(query, (from_currency, to_currency))
        
        row = await cursor.fetchone()
        if row:
            return row[0]
        
        # 2. 从API获取并缓存
        self.logger.info(f"数据库无汇率，尝试从API获取: {from_currency} -> {to_currency}")
        rate = await self._fetch_from_providers(from_currency, to_currency, date)
        
        if rate:
            # 缓存到数据库
            await self._save_rate(from_currency, to_currency, rate, 'api', date)
            
        return rate

    async def _fetch_from_providers(
        self,
        from_currency: str,
        to_currency: str,
        date: Optional[str] = None
    ) -> Optional[float]:
        """从提供者获取汇率"""
        # 按优先级尝试各个提供者
        for provider_name, provider in self.providers.items():
            try:
                rates = await provider.fetch_rates(from_currency, [to_currency], date)
                if to_currency in rates:
                    self.logger.info(f"从 {provider_name} 获取汇率成功")
                    return rates[to_currency]
            except Exception as e:
                self.logger.warning(f"从 {provider_name} 获取汇率失败: {e}")
                continue
        
        return None

    async def _save_rate(
        self,
        from_currency: str,
        to_currency: str,
        rate: float,
        source: str,
        date: Optional[str] = None
    ):
        """保存汇率到数据库"""
        conn = await self.db._get_connection()
        effective_date = date or datetime.now().strftime('%Y-%m-%d')
        
        await conn.execute("""
            INSERT OR REPLACE INTO user_exchange_rates
            (from_currency, to_currency, rate, source, effective_date)
            VALUES (?, ?, ?, ?, ?)
        """, (from_currency, to_currency, rate, source, effective_date))
        
        await conn.commit()

    @log_method
    async def sync_rates(self, currencies: List[str], base_currency: str = 'CNY'):
        """
        同步汇率数据
        
        Args:
            currencies: 要同步的货币列表
            base_currency: 基准货币
        """
        self.logger.info(f"开始同步汇率: {base_currency} -> {currencies}")
        
        success_count = 0
        for provider_name, provider in self.providers.items():
            try:
                rates = await provider.fetch_rates(base_currency, currencies)
                
                for currency, rate in rates.items():
                    await self._save_rate(base_currency, currency, rate, provider_name)
                    success_count += 1
                    
                self.logger.info(f"{provider_name}: 同步了 {len(rates)} 个汇率")
                
            except Exception as e:
                self.logger.error(f"{provider_name} 同步失败: {e}")
                continue
        
        self.logger.info(f"汇率同步完成，成功 {success_count} 条")
        return success_count

    @log_method
    async def convert_amount(
        self,
        amount: float,
        from_currency: str,
        to_currency: str,
        date: Optional[str] = None
    ) -> Optional[float]:
        """
        转换金额
        
        Args:
            amount: 金额
            from_currency: 源货币
            to_currency: 目标货币
            date: 日期
            
        Returns:
            float: 转换后的金额
        """
        rate = await self.get_rate(from_currency, to_currency, date)
        if rate:
            return amount * rate
        return None
