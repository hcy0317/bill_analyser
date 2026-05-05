"""OCR service 单元测试。"""

# pylint: disable=missing-function-docstring,redefined-outer-name

from __future__ import annotations

import asyncio

import pytest

from bill_analyser.core.ai.ocr.provider import OcrRawResult
from bill_analyser.core.ai.ocr.service import (
    ERR_CANCELLED,
    ERR_PARSE_ERROR,
    ERR_PROVIDER_UNCONFIGURED,
    ERR_RATE_LIMITED,
    ERR_TIMEOUT,
    OcrRateLimiter,
    OcrService,
    OcrServiceError,
    parse_receipt_text,
)


class _OkProvider:
    name = "ok-stub"

    def __init__(self, text: str = "金额 99.50\n2024-12-31 23:59\n商品: 跨年套餐", confidence: float = 0.7) -> None:
        self._text = text
        self._confidence = confidence

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        return OcrRawResult(
            text=self._text,
            confidence=self._confidence,
            model="ok-stub-1",
            raw_provider_response={"echo": len(image_bytes), "mime": mime},
        )


class _SlowProvider:
    name = "slow-stub"

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        await asyncio.sleep(2.0)
        return OcrRawResult(text="", confidence=0.0, model="never")


class _ParseFailProvider:
    name = "parse-fail-stub"

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        raise ValueError("garbage")


# --------------------------------------------------------------------------- #
# Rate limiter
# --------------------------------------------------------------------------- #


def test_rate_limiter_basic_window():
    limiter = OcrRateLimiter(window_seconds=1.0, max_requests=2)
    assert limiter.try_acquire("u1", now=0.0)
    assert limiter.try_acquire("u1", now=0.1)
    assert not limiter.try_acquire("u1", now=0.2)
    # 滑窗外应放行
    assert limiter.try_acquire("u1", now=2.0)


def test_rate_limiter_per_user_isolated():
    limiter = OcrRateLimiter(window_seconds=1.0, max_requests=1)
    assert limiter.try_acquire("u1", now=0.0)
    assert limiter.try_acquire("u2", now=0.0)
    assert not limiter.try_acquire("u1", now=0.1)


def test_rate_limiter_reset():
    limiter = OcrRateLimiter(window_seconds=1.0, max_requests=1)
    assert limiter.try_acquire("u1", now=0.0)
    limiter.reset("u1")
    assert limiter.try_acquire("u1", now=0.1)
    limiter.reset()  # 全部清空
    assert limiter.try_acquire("u2", now=0.2)


# --------------------------------------------------------------------------- #
# parse_receipt_text
# --------------------------------------------------------------------------- #


def test_parse_receipt_text_amount_via_keyword():
    out = parse_receipt_text("付款金额 12.34\n其它信息")
    assert out["amount"] == pytest.approx(12.34)


def test_parse_receipt_text_amount_via_currency():
    out = parse_receipt_text("¥99.50 实付")
    assert out["amount"] == pytest.approx(99.50)


def test_parse_receipt_text_time():
    out = parse_receipt_text("2025-01-02 10:30:00 拿铁")
    assert out["trade_time"] == "2025-01-02 10:30:00"


def test_parse_receipt_text_description_keyword():
    out = parse_receipt_text("商家: 星巴克\n金额 12.34")
    assert out["description"] == "星巴克"


def test_parse_receipt_text_description_fallback_first_line():
    out = parse_receipt_text("\n  Hello World line  \n金额 1.00\n")
    assert out["description"].startswith("Hello World")


def test_parse_receipt_text_empty():
    out = parse_receipt_text("")
    assert out == {
        "amount": None,
        "trade_time": None,
        "description": None,
        "payment_platform": None,
        "payment_confidence": 0.0,
    }


# --------------------------------------------------------------------------- #
# Service
# --------------------------------------------------------------------------- #


def _run(coro):
    return asyncio.run(coro)


def test_service_provider_unconfigured_raises_typed_error():
    service = OcrService(provider=None)
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"abc", mime="image/png"))
    assert info.value.code == ERR_PROVIDER_UNCONFIGURED
    assert info.value.http_status == 501


def test_service_cancelled_short_circuits():
    service = OcrService(provider=_OkProvider())
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"abc", mime="image/png", cancelled=True))
    assert info.value.code == ERR_CANCELLED
    assert info.value.http_status == 499


def test_service_rate_limited_after_threshold():
    service = OcrService(
        provider=_OkProvider(),
        rate_limiter=OcrRateLimiter(60.0, 1),
    )
    _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    assert info.value.code == ERR_RATE_LIMITED
    assert info.value.http_status == 429


def test_service_timeout_typed():
    service = OcrService(
        provider=_SlowProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
        timeout_seconds=0.01,
    )
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    assert info.value.code == ERR_TIMEOUT
    assert info.value.http_status == 504


def test_service_parse_error_typed():
    service = OcrService(
        provider=_ParseFailProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
    )
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    assert info.value.code == ERR_PARSE_ERROR
    assert info.value.http_status == 422


def test_service_success_payload_shape():
    service = OcrService(
        provider=_OkProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
    )
    result = _run(service.recognize(user_id=42, image_bytes=b"\x89PNG", mime="image/png"))
    payload = result.to_payload()
    assert set(payload.keys()) == {
        "amount",
        "trade_time",
        "description",
        "payment_platform",
        "provenance",
        "confidence",
        "raw_provider_response",
    }
    assert payload["amount"] == pytest.approx(99.50)
    assert payload["trade_time"] == "2024-12-31 23:59"
    assert payload["payment_platform"] is None
    assert "跨年" in (payload["description"] or "")
    prov = payload["provenance"]
    assert prov["provider"] == "ok-stub"
    assert prov["model"] == "ok-stub-1"
    assert prov["request_id"]
    assert 0.0 <= payload["confidence"] <= 1.0
    # raw 透传，不进 typed contract（除了 raw_provider_response 字段本身）
    assert payload["raw_provider_response"]["mime"] == "image/png"


def test_service_from_environment_disabled_default(monkeypatch):
    from bill_analyser.core.ai.ocr import provider as op
    monkeypatch.delenv(op.DEFAULT_PROVIDER_ENV, raising=False)
    service = OcrService.from_environment()
    assert service.provider is None


def test_service_from_environment_unknown_provider_falls_back(monkeypatch):
    from bill_analyser.core.ai.ocr import provider as op
    monkeypatch.setenv(op.DEFAULT_PROVIDER_ENV, "totally_made_up")
    service = OcrService.from_environment()
    assert service.provider is None


def test_service_from_environment_cloud_stub(monkeypatch):
    from bill_analyser.core.ai.ocr import provider as op
    monkeypatch.setenv(op.DEFAULT_PROVIDER_ENV, "cloud_stub")
    service = OcrService.from_environment()
    # cloud_stub 可被构造，但 recognize 时会被 service 翻译为 provider_unconfigured
    assert service.provider is not None
    with pytest.raises(OcrServiceError) as info:
        _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    assert info.value.code == ERR_PROVIDER_UNCONFIGURED


def test_service_confidence_clamped():
    class HighProvider:
        name = "hi"

        async def recognize(self, image_bytes, mime):  # noqa: D401
            return OcrRawResult(text="金额 1.00", confidence=5.0, model="hi-m")

    service = OcrService(provider=HighProvider(), rate_limiter=OcrRateLimiter(60.0, 100))
    res = _run(service.recognize(user_id=1, image_bytes=b"a", mime="image/png"))
    assert 0.0 <= res.confidence <= 1.0
