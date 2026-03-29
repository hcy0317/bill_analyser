from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from typing import Any, cast

import pandas as pd
import pytest

from bill_analyser.core import exchange_rate_providers as exchange_module
from bill_analyser.core.exchange_rate_providers import (
    BOCChinaProvider,
    BOCProvider,
    CMBChinaProvider,
    ECBProvider,
    ExchangeRateManager,
    ExchangeRateProvider,
    NBPProvider,
    RBAProvider,
    SNBProvider,
)


class DummyProvider(ExchangeRateProvider):
    """Concrete provider for testing protected base-conversion helpers."""

    async def fetch_rates(
        self,
        base_currency: str,
        target_currencies: list[str],
        date: str | None = None,
    ) -> dict[str, float]:
        _ = (base_currency, target_currencies, date)
        return {}

    def get_name(self) -> str:
        return "Dummy"

    def get_supported_currencies(self) -> list[str]:
        return ["CNY"]


class FakeResponse:
    """Async HTTP response stub."""

    def __init__(
        self,
        *,
        status: int = 200,
        text_data: str = "",
        json_data: Any = None,
        read_data: bytes | None = None,
    ) -> None:
        self.status = status
        self._text_data = text_data
        self._json_data = json_data
        self._read_data = read_data if read_data is not None else text_data.encode("utf-8")

    async def __aenter__(self) -> FakeResponse:
        return self

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> bool:
        _ = (exc_type, exc, tb)
        return False

    async def text(self) -> str:
        return self._text_data

    async def json(self) -> Any:
        return self._json_data

    async def read(self) -> bytes:
        return self._read_data


class FakeSession:
    """Async HTTP session stub with queued responses."""

    def __init__(self, responses: list[FakeResponse]) -> None:
        self._responses = responses
        self.requested_urls: list[str] = []

    async def __aenter__(self) -> FakeSession:
        return self

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> bool:
        _ = (exc_type, exc, tb)
        return False

    def get(self, url: str) -> FakeResponse:
        self.requested_urls.append(url)
        return self._responses.pop(0)


class SessionFactory:
    """Factory that returns fake sessions while retaining the last one for assertions."""

    def __init__(self, *responses: FakeResponse) -> None:
        self.responses = list(responses)
        self.created_sessions: list[FakeSession] = []

    def __call__(self, *_args: object, **_kwargs: object) -> FakeSession:
        session = FakeSession(self.responses.copy())
        self.created_sessions.append(session)
        return session


class FakeCursor:
    def __init__(self, row: tuple[Any, ...] | None) -> None:
        self.row = row

    async def fetchone(self) -> tuple[Any, ...] | None:
        return self.row


class FakeConnection:
    """Minimal async DB connection for ExchangeRateManager tests."""

    def __init__(self, row: tuple[Any, ...] | None = None) -> None:
        self.row = row
        self.executed: list[tuple[str, tuple[Any, ...]]] = []
        self.commit_calls = 0

    async def execute(self, query: str, params: tuple[Any, ...]) -> FakeCursor:
        self.executed.append((query, params))
        if query.lstrip().upper().startswith("SELECT"):
            return FakeCursor(self.row)
        return FakeCursor(None)

    async def commit(self) -> None:
        self.commit_calls += 1


class FakeDB:
    """Minimal DB wrapper exposing the protected connection getter used by the manager."""

    def __init__(self, connection: FakeConnection) -> None:
        self.connection = connection

    async def _get_connection(self) -> FakeConnection:
        return self.connection



def test_helper_functions_normalize_currency_names_extract_numbers_and_convert_quotes() -> None:
    """底层辅助函数应稳定完成名称归一、数字提取和报价换算。"""
    assert exchange_module._normalize_chinese_currency_name(" 美元 (USD) ") == "USD"
    assert exchange_module._normalize_chinese_currency_name("韩国元") == "KRW"
    assert exchange_module._normalize_chinese_currency_name("未知币种") == ""

    extracted_numbers = exchange_module._extract_numeric_values([
        "100",
        "1,234.56",
        "08:30",
        "-",
        "nan",
        "NaN",
        "text",
        "0.75",
    ])
    assert extracted_numbers == [100.0, 1234.56, 0.75]

    cny_based_rates = exchange_module._convert_cny_quote_map_to_rates({"USD": 730.0, "EUR": 800.0}, "CNY", ["USD", "EUR"])
    assert cny_based_rates["USD"] == pytest.approx(100.0 / 730.0)
    assert cny_based_rates["EUR"] == pytest.approx(100.0 / 800.0)

    usd_based_rates = exchange_module._convert_cny_quote_map_to_rates({"USD": 730.0, "EUR": 800.0}, "USD", ["CNY", "EUR"])
    assert usd_based_rates["CNY"] == pytest.approx(7.3)
    assert usd_based_rates["EUR"] == pytest.approx(730.0 / 800.0)



def test_base_currency_conversion_supports_both_rate_formats() -> None:
    """受保护的基准货币换算 helper 应覆盖两种汇率格式和缺失基准回退。"""
    provider = DummyProvider()

    ecb_style = provider._convert_base_currency(
        {"EUR": 1.0, "USD": 1.1, "CNY": 7.7},
        "EUR",
        "CNY",
        ["USD", "EUR", "CNY"],
        rate_format="base_to_target",
    )
    assert ecb_style == {
        "USD": pytest.approx(1.1 / 7.7),
        "EUR": pytest.approx(1.0 / 7.7),
        "CNY": 1.0,
    }

    boc_style = provider._convert_base_currency(
        {"CAD": 1.0, "USD": 1.38, "CNY": 0.19},
        "CAD",
        "CNY",
        ["USD", "CAD", "CNY"],
        rate_format="target_to_base",
    )
    assert boc_style == {
        "USD": pytest.approx(0.19 / 1.38),
        "CAD": 0.19,
        "CNY": 1.0,
    }

    assert provider._convert_base_currency({"USD": 1.1}, "EUR", "CNY", ["USD"]) == {}


@pytest.mark.parametrize(
    ("provider_cls", "expected_name", "base_currency"),
    [
        (ECBProvider, "European Central Bank (ECB)", "EUR"),
        (BOCChinaProvider, "Bank of China (CN)", "CNY"),
        (CMBChinaProvider, "China Merchants Bank (CMB)", "CNY"),
        (BOCProvider, "Bank of Canada (BOC)", "CAD"),
        (RBAProvider, "Reserve Bank of Australia (RBA)", "AUD"),
        (NBPProvider, "National Bank of Poland (NBP)", "PLN"),
        (SNBProvider, "Swiss National Bank (SNB)", "CHF"),
    ],
)
def test_provider_names_and_supported_currencies_have_expected_bases(
    provider_cls: type[ExchangeRateProvider],
    expected_name: str,
    base_currency: str,
) -> None:
    """各 provider 的名称和支持币种列表应至少包含自己的基准货币。"""
    provider = provider_cls()
    assert provider.get_name() == expected_name
    assert base_currency in provider.get_supported_currencies()


@pytest.mark.asyncio
async def test_ecb_provider_parses_xml_and_fetches_converted_rates(monkeypatch: pytest.MonkeyPatch) -> None:
    """ECB provider 应能解析 XML，并按目标基准货币换算。"""
    xml_payload = """
    <gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
      <Cube>
        <Cube time="2025-01-02">
          <Cube currency="USD" rate="1.10"/>
          <Cube currency="CNY" rate="7.70"/>
        </Cube>
      </Cube>
    </gesmes:Envelope>
    """
    session_factory = SessionFactory(FakeResponse(status=200, text_data=xml_payload))
    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", session_factory)

    provider = ECBProvider()
    parsed = provider._parse_ecb_xml(xml_payload, target_date="2025-01-02")
    assert parsed == {"EUR": 1.0, "USD": 1.1, "CNY": 7.7}

    rates = await provider.fetch_rates("CNY", ["USD", "EUR"], date=None)
    assert rates["USD"] == pytest.approx(1.1 / 7.7)
    assert rates["EUR"] == pytest.approx(1.0 / 7.7)
    assert session_factory.created_sessions[0].requested_urls == [provider.BASE_URL]


@pytest.mark.asyncio
async def test_boc_china_provider_parses_quotes_and_handles_http_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    """中国银行 provider 应能从表格提取报价，并在 HTTP 错误时返回空结果。"""
    provider = BOCChinaProvider()
    table = pd.DataFrame([
        ["美元", "100", "720.00"],
        ["欧元", "100", "800.00"],
    ])
    monkeypatch.setattr(exchange_module.pd, "read_html", lambda *_args, **_kwargs: [table])

    parsed = provider._parse_quote_map("<html></html>")
    assert parsed == {"USD": 720.0, "EUR": 800.0}

    success_factory = SessionFactory(FakeResponse(status=200, text_data="<html></html>"))
    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", success_factory)
    rates = await provider.fetch_rates("CNY", ["USD", "EUR"])
    assert rates["USD"] == pytest.approx(100.0 / 720.0)
    assert rates["EUR"] == pytest.approx(100.0 / 800.0)

    error_factory = SessionFactory(FakeResponse(status=500, text_data="server error"))
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", error_factory)
    assert await provider.fetch_rates("CNY", ["USD"]) == {}


@pytest.mark.asyncio
async def test_cmb_provider_parses_api_payload_fetches_api_and_falls_back_to_html(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """招商银行 provider 应先走 JSON API，再在空结果时回退到页面表格。"""
    provider = CMBChinaProvider()

    payload = {
        "body": [
            {"ccyNbrEng": "United States Dollar USD", "rthOfr": "730.00", "rthBid": "720.00"},
            {"ccyNbr": "欧元", "rtcOfr": "810.00", "rtcBid": "800.00"},
            {"ccyNbr": "人民币", "rthOfr": "100.00"},
        ]
    }
    parsed_payload = provider._parse_quote_map_from_api_payload(cast(dict[str, object], payload))
    assert parsed_payload["USD"] == pytest.approx((730.0 + 720.0) / 2)
    assert parsed_payload["EUR"] == pytest.approx((810.0 + 800.0) / 2)
    assert provider._extract_currency_code_from_api_row({"ccyNbrEng": "Japanese Yen JPY"}) == "JPY"
    assert provider._extract_currency_code_from_api_row({"ccyNbr": "美元"}) == "USD"

    api_factory = SessionFactory(
        FakeResponse(status=200, read_data='{"body": [{"ccyNbr": "美元", "rthOfr": "730"}]}'.encode("utf-8"))
    )
    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", api_factory)
    assert await provider._fetch_quote_map_from_api(cast(Any, api_factory())) == {"USD": 730.0}

    invalid_factory = SessionFactory(FakeResponse(status=200, read_data=b"not-json"))
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", invalid_factory)
    assert await provider._fetch_quote_map_from_api(cast(Any, invalid_factory())) == {}

    async def fake_fetch_quote_map_from_api(_session: object) -> dict[str, float]:
        return {}

    monkeypatch.setattr(provider, "_fetch_quote_map_from_api", fake_fetch_quote_map_from_api)
    monkeypatch.setattr(provider, "_parse_quote_map", lambda _html: {"USD": 740.0})
    fallback_factory = SessionFactory(
        FakeResponse(status=200, text_data="unused api response"),
        FakeResponse(status=200, text_data="<html>fallback</html>"),
    )
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", fallback_factory)
    rates = await provider.fetch_rates("CNY", ["USD"])
    assert rates["USD"] == pytest.approx(100.0 / 740.0)


@pytest.mark.asyncio
async def test_boc_rba_nbp_and_snb_providers_cover_parse_and_fetch_paths(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加拿大、澳洲、波兰和瑞士 provider 应覆盖其解析与抓取主路径。"""
    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())

    boc_factory = SessionFactory(
        FakeResponse(
            status=200,
            json_data={
                "observations": [
                    {"FXUSDCAD": {"v": "1.38"}, "FXCNYCAD": {"v": "0.19"}},
                ]
            },
        )
    )
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", boc_factory)
    boc_provider = BOCProvider()
    boc_rates = await boc_provider.fetch_rates("CNY", ["USD", "CAD"])
    assert boc_rates["USD"] == pytest.approx(0.19 / 1.38)
    assert boc_rates["CAD"] == pytest.approx(0.19)

    rba_xml = """
    <rss><channel>
      <item><cb:targetCurrency>USD</cb:targetCurrency><cb:value>0.6602</cb:value></item>
      <item><cb:targetCurrency>CNY</cb:targetCurrency><cb:value>4.7610</cb:value></item>
    </channel></rss>
    """
    rba_provider = RBAProvider()
    assert rba_provider._parse_rba_xml(rba_xml) == {"AUD": 1.0, "USD": 0.6602, "CNY": 4.761}
    rba_factory = SessionFactory(FakeResponse(status=200, text_data=rba_xml))
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", rba_factory)
    rba_rates = await rba_provider.fetch_rates("CNY", ["USD", "AUD"])
    assert rba_rates["USD"] == pytest.approx(0.6602 / 4.761)
    assert rba_rates["AUD"] == pytest.approx(1.0 / 4.761)

    nbp_provider = NBPProvider()
    assert nbp_provider._parse_nbp_json([{"rates": [{"code": "USD", "mid": 4.0}, {"code": "EUR", "mid": 4.5}]}]) == {
        "PLN": 1.0,
        "USD": 4.0,
        "EUR": 4.5,
    }
    nbp_factory = SessionFactory(FakeResponse(status=200, json_data=[{"rates": [{"code": "USD", "mid": 4.0}, {"code": "CNY", "mid": 0.55}]}]))
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", nbp_factory)
    nbp_rates = await nbp_provider.fetch_rates("CNY", ["USD", "PLN"], date="2025-01-02")
    assert nbp_rates["USD"] == pytest.approx(4.0 / 0.55)
    assert nbp_rates["PLN"] == pytest.approx(1.0 / 0.55)

    snb_provider = SNBProvider()
    assert snb_provider._parse_snb_csv("header\nvalue") == {"CHF": 1.0}
    snb_factory = SessionFactory(FakeResponse(status=500, text_data="error"))
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", snb_factory)
    assert await snb_provider.fetch_rates("CHF", ["USD"]) == {}


@pytest.mark.asyncio
async def test_exchange_provider_error_paths_cover_empty_and_invalid_inputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """补足空报价、坏 XML、坏表格和 API 非法 body 等低成本错误分支。"""
    assert exchange_module._convert_cny_quote_map_to_rates({"USD": 730.0}, "EUR", ["USD"]) == {}

    ecb_provider = ECBProvider()
    assert ecb_provider._parse_ecb_xml("<broken>") == {"EUR": 1.0}

    error_factory = SessionFactory(FakeResponse(status=500, text_data="server error"))
    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())
    monkeypatch.setattr(exchange_module.aiohttp, "ClientSession", error_factory)
    assert await ecb_provider.fetch_rates("EUR", ["USD"], date="2025-01-02") == {}

    boc_china_provider = BOCChinaProvider()
    monkeypatch.setattr(exchange_module.pd, "read_html", lambda *_args, **_kwargs: (_ for _ in ()).throw(ValueError("bad html")))
    assert boc_china_provider._parse_quote_map("<html></html>") == {}

    cmb_provider = CMBChinaProvider()
    assert cmb_provider._parse_quote_map_from_api_payload(cast(dict[str, object], {"body": "bad"})) == {}


@pytest.mark.asyncio
async def test_cmb_html_parser_and_boc_empty_series_short_circuit(monkeypatch: pytest.MonkeyPatch) -> None:
    """覆盖招行页面表格解析，以及加拿大银行在没有支持币种时的快速返回。"""
    cmb_provider = CMBChinaProvider()
    table = pd.DataFrame([
        ["美元", "100", "730.0", "720.0", "710.0", "700.0"],
        ["欧元", "800.0", "790.0", "780.0", "770.0"],
    ])
    monkeypatch.setattr(exchange_module.pd, "read_html", lambda *_args, **_kwargs: [table])
    parsed = cmb_provider._parse_quote_map("<html></html>")
    assert parsed["USD"] == pytest.approx((730.0 + 720.0 + 710.0 + 700.0) / 4)
    assert parsed["EUR"] == pytest.approx((800.0 + 790.0 + 780.0 + 770.0) / 4)

    monkeypatch.setattr(exchange_module.aiohttp, "TCPConnector", lambda **_kwargs: object())
    boc_provider = BOCProvider()
    assert await boc_provider.fetch_rates("CAD", ["ZZZ"]) == {}


@pytest.mark.asyncio
async def test_exchange_rate_manager_reads_db_falls_back_to_providers_and_syncs(monkeypatch: pytest.MonkeyPatch) -> None:
    """汇率管理器应覆盖数据库命中、provider fallback、批量同步和金额转换。"""
    db_hit_connection = FakeConnection(row=(7.25,))
    manager = ExchangeRateManager(FakeDB(db_hit_connection))
    assert await manager.get_rate("USD", "USD") == 1.0
    assert await manager.get_rate("USD", "CNY", date="2025-01-01") == 7.25
    assert db_hit_connection.executed[0][1] == ("USD", "CNY", "2025-01-01")

    db_miss_connection = FakeConnection(row=None)
    manager = ExchangeRateManager(FakeDB(db_miss_connection))

    class FailingProvider:
        async def fetch_rates(self, _base: str, _targets: list[str], _date: str | None = None) -> dict[str, float]:
            raise RuntimeError("provider down")

    class SuccessfulProvider:
        async def fetch_rates(self, _base: str, targets: list[str], _date: str | None = None) -> dict[str, float]:
            return {targets[0]: 7.31}

    manager.providers = {"broken": FailingProvider(), "ok": SuccessfulProvider()}
    rate = await manager.get_rate("USD", "CNY")
    assert rate == 7.31
    insert_params = db_miss_connection.executed[-1][1]
    assert insert_params[:4] == ("USD", "CNY", 7.31, "api")
    assert db_miss_connection.commit_calls == 1

    fetched_rate = await manager._fetch_from_providers("USD", "CNY")
    assert fetched_rate == 7.31

    sync_connection = FakeConnection(row=None)
    sync_manager = ExchangeRateManager(FakeDB(sync_connection))

    class ProviderOne:
        async def fetch_rates(self, _base: str, currencies: list[str], _date: str | None = None) -> dict[str, float]:
            return {currencies[0]: 7.2}

    class ProviderTwo:
        async def fetch_rates(self, _base: str, _currencies: list[str], _date: str | None = None) -> dict[str, float]:
            raise RuntimeError("boom")

    sync_manager.providers = {"one": ProviderOne(), "two": ProviderTwo()}
    success_count = await sync_manager.sync_rates(["USD"], base_currency="CNY")
    assert success_count == 1
    assert sync_connection.commit_calls == 1

    assert await sync_manager.convert_amount(100.0, "USD", "USD") == 100.0
    async def fake_get_rate_success(*_args: object, **_kwargs: object) -> float:
        return 7.0

    monkeypatch.setattr(sync_manager, "get_rate", fake_get_rate_success)
    assert await sync_manager.convert_amount(10.0, "USD", "CNY") == 70.0

    async def fake_get_rate_none(*_args: object, **_kwargs: object) -> None:
        return None

    monkeypatch.setattr(sync_manager, "get_rate", fake_get_rate_none)
    assert await sync_manager.convert_amount(10.0, "USD", "CNY") is None
