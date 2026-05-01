"""Exchange-rate manager."""

from datetime import datetime

from ...utils.logger import get_logger, log_method
from .base import ExchangeRateProvider
from .global_providers import BOCProvider, ECBProvider, NBPProvider, RBAProvider, SNBProvider


class ExchangeRateManager:
    """Coordinate exchange-rate providers and cached database rates."""

    def __init__(self, db):
        self.db = db
        self.logger = get_logger("ExchangeRateManager")

        self.providers: dict[str, ExchangeRateProvider] = {
            "ecb": ECBProvider(),
            "boc": BOCProvider(),
            "rba": RBAProvider(),
            "nbp": NBPProvider(),
            "snb": SNBProvider(),
        }

    @log_method
    async def get_rate(
        self,
        from_currency: str,
        to_currency: str,
        date: str | None = None,
    ) -> float | None:
        """Get an exchange rate from the database or external providers."""
        if from_currency == to_currency:
            return 1.0

        conn = await self.db._get_connection()  # pylint: disable=protected-access

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

        self.logger.info(
            "数据库无汇率，尝试从API获取: %s -> %s",
            from_currency,
            to_currency,
        )
        rate = await self._fetch_from_providers(from_currency, to_currency, date)

        if rate:
            await self._save_rate(from_currency, to_currency, rate, "api", date)

        return rate

    async def _fetch_from_providers(
        self,
        from_currency: str,
        to_currency: str,
        date: str | None = None,
    ) -> float | None:
        """Fetch one rate from providers in priority order."""
        for provider_name, provider in self.providers.items():
            try:
                rates = await provider.fetch_rates(from_currency, [to_currency], date)
                if to_currency in rates:
                    self.logger.info("从 %s 获取汇率成功", provider_name)
                    return rates[to_currency]
            except Exception as exc:  # pylint: disable=broad-exception-caught
                self.logger.warning("从 %s 获取汇率失败: %s", provider_name, exc)
                continue

        return None

    # pylint: disable-next=too-many-arguments,too-many-positional-arguments
    async def _save_rate(
        self,
        from_currency: str,
        to_currency: str,
        rate: float,
        source: str,
        date: str | None = None,
    ):
        """Save an exchange rate to the database cache."""
        conn = await self.db._get_connection()  # pylint: disable=protected-access
        effective_date = date or datetime.now().strftime("%Y-%m-%d")

        await conn.execute(
            """
            INSERT OR REPLACE INTO user_exchange_rates
            (from_currency, to_currency, rate, source, effective_date)
            VALUES (?, ?, ?, ?, ?)
        """,
            (from_currency, to_currency, rate, source, effective_date),
        )

        await conn.commit()

    @log_method
    async def sync_rates(self, currencies: list[str], base_currency: str = "CNY"):
        """Synchronize rates for a currency list."""
        self.logger.info("开始同步汇率: %s -> %s", base_currency, currencies)

        success_count = 0
        for provider_name, provider in self.providers.items():
            try:
                rates = await provider.fetch_rates(base_currency, currencies)

                for currency, rate in rates.items():
                    await self._save_rate(base_currency, currency, rate, provider_name)
                    success_count += 1

                self.logger.info("%s: 同步了 %d 个汇率", provider_name, len(rates))
            except Exception as exc:  # pylint: disable=broad-exception-caught
                self.logger.error("%s 同步失败: %s", provider_name, exc)
                continue

        self.logger.info("汇率同步完成，成功 %d 条", success_count)
        return success_count

    @log_method
    async def convert_amount(
        self,
        amount: float,
        from_currency: str,
        to_currency: str,
        date: str | None = None,
    ) -> float | None:
        """Convert an amount between two currencies."""
        rate = await self.get_rate(from_currency, to_currency, date)
        if rate:
            return amount * rate
        return None
