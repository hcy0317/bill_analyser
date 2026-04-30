"""AI识图 REST 收口回归测试。

C1 升级后该端点支持：
    - 默认无 provider → 501 + ``provider_unconfigured``
    - 注入 stub provider → 200 + typed payload schema
    - per-user rate limit 超阈值 → 429 + ``rate_limited``
    - timeout / parse_error / cancelled typed error
    - 不再保留 v1 legacy route。
"""

# pylint: disable=redefined-outer-name,unused-argument


import asyncio
from io import BytesIO

import pytest

from bill_analyser.core.ocr_provider import OcrRawResult
from bill_analyser.core.ocr_service import OcrRateLimiter, OcrService


class _StubProvider:
    name = "stub"

    def __init__(self, text: str = "金额 12.34\n2025-01-02 10:00:00\n商品: 拿铁咖啡", confidence: float = 0.88) -> None:
        self._text = text
        self._confidence = confidence

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:  # noqa: D401
        return OcrRawResult(
            text=self._text,
            confidence=self._confidence,
            model="stub-1",
            raw_provider_response={"echo_bytes": len(image_bytes), "mime": mime},
        )


class _TimeoutProvider:
    name = "timeout-stub"

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        # 让 wait_for 超时
        await asyncio.sleep(5.0)
        return OcrRawResult(text="", confidence=0.0, model="never")


class _ParseFailProvider:
    name = "parse-fail-stub"

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        raise ValueError("non-json blob")


@pytest.fixture
def reset_ocr_service(client):
    """每个用例后清理 app.config 中的 OCR_SERVICE / 限流器。"""
    from bill_analyser.api.app import app
    yield
    app.config.pop("OCR_SERVICE", None)


def _post_image(client, headers, *, content: bytes = b"\x89PNG\r\n\x1a\nfake", mime: str = "image/png", cancelled: bool = False):
    data = {"image": (BytesIO(content), "receipt.png", mime)}
    if cancelled:
        data["cancelled"] = "true"
    return client.post(
        "/api/ml/receipt-recognition",
        headers=headers,
        data=data,
        content_type="multipart/form-data",
    )


def test_ai_receipt_recognition_rest_disabled_safe(client, auth_headers, reset_ocr_service):
    """无 provider 时返回 501 + provider_unconfigured。"""
    from bill_analyser.api.app import app

    app.config["OCR_SERVICE"] = OcrService(provider=None)

    response = client.post('/api/ml/receipt-recognition', headers=auth_headers)
    assert response.status_code in (404, 501), response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is False
    error_message = f"{data.get('errorMessage') or ''} {data.get('message') or ''}".lower()
    assert 'not implemented' in error_message or 'not found' in error_message or 'provider' in error_message
    assert data.get("errorCode") == "provider_unconfigured"


def test_ai_receipt_recognition_legacy_route_removed(client, auth_headers):
    """旧 AI 识图 v1 路径应已移除。"""
    response = client.post('/api/v1/llm/transactions/recognize_receipt_image.json', headers=auth_headers)
    assert response.status_code == 404


def test_ai_receipt_recognition_success_with_stub_provider(client, auth_headers, reset_ocr_service):
    from bill_analyser.api.app import app

    app.config["OCR_SERVICE"] = OcrService(provider=_StubProvider(), rate_limiter=OcrRateLimiter(60.0, 100))

    response = _post_image(client, auth_headers)
    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data["success"] is True
    result = data["result"]
    assert set(result.keys()) >= {"amount", "trade_time", "description", "provenance", "confidence", "raw_provider_response"}
    assert result["amount"] == pytest.approx(12.34)
    assert "2025" in (result["trade_time"] or "")
    assert (result["description"] or "").startswith("拿铁") or "拿铁" in (result["description"] or "")
    provenance = result["provenance"]
    assert provenance["provider"] == "stub"
    assert provenance["model"] == "stub-1"
    assert provenance["request_id"]
    assert 0.0 <= result["confidence"] <= 1.0


def test_ai_receipt_recognition_rate_limited(client, auth_headers, reset_ocr_service):
    from bill_analyser.api.app import app

    # 阈值 2，第三次必拒
    app.config["OCR_SERVICE"] = OcrService(
        provider=_StubProvider(),
        rate_limiter=OcrRateLimiter(window_seconds=60.0, max_requests=2),
    )

    r1 = _post_image(client, auth_headers)
    r2 = _post_image(client, auth_headers)
    r3 = _post_image(client, auth_headers)
    assert r1.status_code == 200
    assert r2.status_code == 200
    assert r3.status_code == 429, r3.get_data(as_text=True)
    body = r3.get_json() or {}
    assert body.get("errorCode") == "rate_limited"
    assert body.get("success") is False


def test_ai_receipt_recognition_timeout(client, auth_headers, reset_ocr_service):
    from bill_analyser.api.app import app

    app.config["OCR_SERVICE"] = OcrService(
        provider=_TimeoutProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
        timeout_seconds=0.01,
    )
    response = _post_image(client, auth_headers)
    assert response.status_code == 504, response.get_data(as_text=True)
    body = response.get_json() or {}
    assert body.get("errorCode") == "timeout"


def test_ai_receipt_recognition_parse_error(client, auth_headers, reset_ocr_service):
    from bill_analyser.api.app import app

    app.config["OCR_SERVICE"] = OcrService(
        provider=_ParseFailProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
    )
    response = _post_image(client, auth_headers)
    assert response.status_code == 422, response.get_data(as_text=True)
    body = response.get_json() or {}
    assert body.get("errorCode") == "parse_error"


def test_ai_receipt_recognition_cancelled(client, auth_headers, reset_ocr_service):
    from bill_analyser.api.app import app

    app.config["OCR_SERVICE"] = OcrService(
        provider=_StubProvider(),
        rate_limiter=OcrRateLimiter(60.0, 100),
    )
    response = _post_image(client, auth_headers, cancelled=True)
    assert response.status_code == 499, response.get_data(as_text=True)
    body = response.get_json() or {}
    assert body.get("errorCode") == "cancelled"
