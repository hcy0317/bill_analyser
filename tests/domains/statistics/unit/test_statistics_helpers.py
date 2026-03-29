from __future__ import annotations
# pyright: reportPrivateUsage=false

from datetime import datetime
from typing import Any

import pytest
from flask import Flask

from bill_analyser.api.routes import statistics as statistics_module


@pytest.fixture(name="statistics_helper_app")
def statistics_helper_app_fixture() -> Flask:
    """Create a tiny Flask app for statistics helper tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeProvider:
    """Configurable async provider stub."""

    def __init__(self, rates: dict[str, float] | None = None, error: Exception | None = None) -> None:
        self.rates = rates or {}
        self.error = error
        self.calls: list[tuple[str, tuple[str, ...]]] = []

    async def fetch_rates(self, base_currency: str, target_currencies: list[str], date: str | None = None) -> dict[str, float]:
        _ = date
        self.calls.append((base_currency, tuple(target_currencies)))
        if self.error is not None:
            raise self.error
        return self.rates



def test_get_app_context_and_request_base_currency_cover_arg_override_and_user_fallback(
    statistics_helper_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """统计 helper 应优先取 query 参数，否则回退到用户默认币种。"""
    sentinel_db = object()
    statistics_helper_app.config["DB_INSTANCE"] = sentinel_db

    with statistics_helper_app.app_context():
        assert statistics_module.get_app_context() is sentinel_db

    with statistics_helper_app.test_request_context("/?base_currency= usd "):
        assert statistics_module._get_request_base_currency(object()) == "USD"

    with statistics_helper_app.test_request_context("/"):
        monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 7)
        monkeypatch.setattr(statistics_module, "_run_async", lambda _coro: {"default_currency": "eur"})

        class FakeDB:
            async def get_user_by_id(self, user_id: int) -> dict[str, Any]:
                assert user_id == 7
                return {"default_currency": "eur"}

        assert statistics_module._get_request_base_currency(FakeDB()) == "eur"



def test_exchange_rate_provider_selection_helpers_and_custom_result_builder(
    statistics_helper_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """provider 选择与自定义汇率结果 helper 应覆盖 auto、fallback 和非法日期分支。"""
    monkeypatch.setattr(statistics_module.time, "time", lambda: 100)
    result = statistics_module._build_user_custom_exchange_rates_result(
        "CNY",
        [
            {"to_currency": "USD", "rate": 7.1234, "effective_date": "2026-03-01T12:00:00"},
            {"to_currency": "EUR", "rate": 8.2345, "effective_date": "bad-date"},
        ],
    )
    expected_update = int(datetime.fromisoformat("2026-03-01T12:00:00").timestamp())
    assert result["providerKey"] == "user_custom"
    assert result["baseCurrency"] == "CNY"
    assert result["updateTime"] == expected_update
    assert result["exchangeRates"][0] == {"currency": "CNY", "rate": "1.0"}
    assert result["exchangeRates"][1] == {"currency": "USD", "rate": "7.1234"}

    with statistics_helper_app.test_request_context("/"):
        assert statistics_module._normalize_requested_exchange_rate_provider() == "auto"
    with statistics_helper_app.test_request_context("/?provider= ECB "):
        assert statistics_module._normalize_requested_exchange_rate_provider() == "ecb"

    assert statistics_module._build_provider_candidate_order("auto") == ["boc_cn", "cmb_cn", "ecb", "rba"]
    assert statistics_module._build_provider_candidate_order("ecb") == ["ecb", "boc_cn", "cmb_cn", "rba"]
    assert statistics_module._build_provider_candidate_order("unsupported") == []


@pytest.mark.asyncio
async def test_fetch_exchange_rates_from_providers_covers_success_fallback_and_unsupported(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """多 provider 拉取应覆盖首个成功、指定 provider 回退与非法 provider。"""
    failing_provider = FakeProvider(error=RuntimeError("down"))
    empty_provider = FakeProvider(rates={})
    success_provider = FakeProvider(rates={"USD": 0.139})

    monkeypatch.setattr(statistics_module, "BOCChinaProvider", lambda: failing_provider)
    monkeypatch.setattr(statistics_module, "CMBChinaProvider", lambda: empty_provider)
    monkeypatch.setattr(statistics_module, "ECBProvider", lambda: success_provider)
    monkeypatch.setattr(statistics_module, "RBAProvider", lambda: FakeProvider(error=RuntimeError("unused")))

    auto_result = await statistics_module._fetch_exchange_rates_from_providers("CNY", ["USD"], "auto")
    assert auto_result == {
        "rates": {"USD": 0.139},
        "source": statistics_module.EXCHANGE_RATE_PROVIDER_OPTIONS["ecb"]["label"],
        "url": statistics_module.EXCHANGE_RATE_PROVIDER_OPTIONS["ecb"]["reference_url"],
        "provider_key": "ecb",
        "fallback_used": False,
    }

    specific_result = await statistics_module._fetch_exchange_rates_from_providers("CNY", ["USD"], "cmb_cn")
    assert specific_result["provider_key"] == "ecb"
    assert specific_result["fallback_used"] is True

    with pytest.raises(RuntimeError, match="Unsupported exchange rate provider"):
        await statistics_module._fetch_exchange_rates_from_providers("CNY", ["USD"], "bad-provider")



def test_fallback_exchange_rates_return_expected_json_for_cny_and_cross_currency(
    statistics_helper_app: Flask,
) -> None:
    """内置回退汇率应同时支持 CNY 基准和交叉货币基准。"""
    with statistics_helper_app.app_context():
        cny_payload = statistics_module._get_fallback_exchange_rates("CNY").get_json()
        assert cny_payload["success"] is True
        assert cny_payload["result"]["providerKey"] == "fallback"
        assert cny_payload["result"]["exchangeRates"][0] == {"currency": "CNY", "rate": "1.0"}
        assert any(rate["currency"] == "USD" for rate in cny_payload["result"]["exchangeRates"])

        usd_payload = statistics_module._get_fallback_exchange_rates("USD").get_json()
        assert usd_payload["success"] is True
        assert usd_payload["result"]["baseCurrency"] == "USD"
        assert usd_payload["result"]["exchangeRates"][0] == {"currency": "USD", "rate": "1.0"}
        assert any(rate["currency"] == "CNY" for rate in usd_payload["result"]["exchangeRates"])
        assert any(rate["currency"] == "EUR" for rate in usd_payload["result"]["exchangeRates"])
