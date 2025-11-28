"""
Exchange Rate Providers - 汇率数据源提供者

支持多个央行和金融机构的汇率数据获取
"""

import aiohttp
import asyncio
from abc import ABC, abstractmethod
from datetime import datetime, timedelta
from typing import Dict, List, Optional
import xml.etree.ElementTree as ET

from ..utils.logger import get_logger, log_method


class ExchangeRateProvider(ABC):
    """汇率提供者基类"""

    def __init__(self):
        self.logger = get_logger(self.__class__.__name__)
        self.timeout = aiohttp.ClientTimeout(total=30)

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
            
            async with aiohttp.ClientSession(timeout=self.timeout) as session:
                async with session.get(url) as response:
                    if response.status != 200:
                        self.logger.error(f"ECB API请求失败: {response.status}")
                        return {}
                    
                    xml_data = await response.text()
                    rates = self._parse_ecb_xml(xml_data, date)
                    
                    # 转换基准货币
                    return self._convert_base_currency(rates, 'EUR', base_currency, target_currencies)
                    
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

    def _convert_base_currency(
        self,
        rates: Dict[str, float],
        original_base: str,
        target_base: str,
        target_currencies: List[str]
    ) -> Dict[str, float]:
        """转换基准货币"""
        if original_base == target_base:
            return {c: rates.get(c, 0) for c in target_currencies if c in rates}
        
        result = {}
        base_rate = rates.get(target_base)
        if not base_rate:
            return result
        
        for currency in target_currencies:
            if currency == target_base:
                result[currency] = 1.0
            elif currency in rates:
                # 转换公式: 新汇率 = 原汇率 / 新基准汇率
                result[currency] = rates[currency] / base_rate
                
        return result


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
            
            # 构建请求的系列列表
            series_codes = [series_map.get(c) for c in target_currencies if c in series_map]
            if not series_codes:
                return {}
            
            series_str = ','.join(series_codes)
            url = f"{self.BASE_URL}/{series_str}/json"
            
            if date:
                url += f"?start_date={date}&end_date={date}"
            
            async with aiohttp.ClientSession(timeout=self.timeout) as session:
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
                    
                    return self._convert_base_currency(rates, 'CAD', base_currency, target_currencies)
                    
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
            async with aiohttp.ClientSession(timeout=self.timeout) as session:
                async with session.get(self.BASE_URL) as response:
                    if response.status != 200:
                        self.logger.error(f"RBA API请求失败: {response.status}")
                        return {}
                    
                    xml_data = await response.text()
                    rates = self._parse_rba_xml(xml_data)
                    
                    return self._convert_base_currency(rates, 'AUD', base_currency, target_currencies)
                    
        except Exception as e:
            self.logger.error(f"获取RBA汇率失败: {e}")
            return {}

    def _parse_rba_xml(self, xml_data: str) -> Dict[str, float]:
        """解析RBA XML数据"""
        rates = {'AUD': 1.0}
        
        try:
            root = ET.fromstring(xml_data)
            
            # RBA RSS格式
            for item in root.findall('.//item'):
                title = item.find('title')
                description = item.find('description')
                
                if title is not None and description is not None:
                    # 标题格式: "1 AUD = X.XX USD"
                    title_text = title.text
                    if '=' in title_text:
                        parts = title_text.split('=')
                        if len(parts) == 2:
                            rate_part = parts[1].strip().split()
                            if len(rate_part) >= 2:
                                rate_value = float(rate_part[0])
                                currency = rate_part[1]
                                rates[currency] = rate_value
                                
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
            
            async with aiohttp.ClientSession(timeout=self.timeout) as session:
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
            async with aiohttp.ClientSession(timeout=self.timeout) as session:
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
