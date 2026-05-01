"""Parsing helpers shared by exchange-rate providers."""

import re

CHINESE_CURRENCY_NAME_MAP = {
    "人民币": "CNY",
    "美元": "USD",
    "欧元": "EUR",
    "英镑": "GBP",
    "日元": "JPY",
    "港币": "HKD",
    "韩元": "KRW",
    "韩国元": "KRW",
    "澳大利亚元": "AUD",
    "澳币": "AUD",
    "加拿大元": "CAD",
    "加拿大币": "CAD",
    "新加坡元": "SGD",
    "新加坡币": "SGD",
    "新台币": "TWD",
    "林吉特": "MYR",
    "马来币": "MYR",
    "泰国铢": "THB",
    "泰币": "THB",
    "越南盾": "VND",
    "瑞士法郎": "CHF",
    "新西兰元": "NZD",
    "纽元": "NZD",
}


def _normalize_chinese_currency_name(value: str) -> str:
    """Normalize a Chinese currency name into a currency code."""
    text = str(value or "").strip()
    text = re.sub(r"\s*\([^)]*\)", "", text)
    text = text.replace("（", "(").replace("）", ")")
    text = text.replace(" ", "")
    return CHINESE_CURRENCY_NAME_MAP.get(text, "")


def _extract_numeric_values(values: list[object]) -> list[float]:
    """Extract numeric columns from a scraped table row."""
    numbers: list[float] = []

    for value in values:
        text = str(value or "").strip()
        if not text or text in {"-", "--", "nan", "NaN"}:
            continue
        if ":" in text:
            continue

        text = text.replace(",", "")
        if re.fullmatch(r"-?\d+(?:\.\d+)?", text):
            try:
                numbers.append(float(text))
            except ValueError:
                continue

    return numbers


def _convert_cny_quote_map_to_rates(
    quote_map: dict[str, float],
    base_currency: str,
    target_currencies: list[str],
) -> dict[str, float]:
    """Convert quotes expressed as 100 foreign currency units to CNY."""
    result: dict[str, float] = {}

    if base_currency == "CNY":
        for currency in target_currencies:
            quote = quote_map.get(currency)
            if quote and quote > 0:
                result[currency] = 100.0 / quote
        return result

    base_quote = quote_map.get(base_currency)
    if not base_quote or base_quote <= 0:
        return result

    for currency in target_currencies:
        if currency == "CNY":
            result[currency] = base_quote / 100.0
            continue

        target_quote = quote_map.get(currency)
        if target_quote and target_quote > 0:
            result[currency] = base_quote / target_quote

    return result
