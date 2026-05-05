"""OCR 服务编排：rate limit + provider 调用 + payload normalize + typed error。

privacy contract（与 ``ocr_provider`` 一致）：
    - "请求即用即弃"：image_bytes 不落盘、不进日志正文。
    - audit 仅可记录：``len(image_bytes)``、``mime``、``sha256[:12]``、
      ``user_id``、最终结果状态码。

Typed error code（5 类，前端 contract）：
    - ``provider_unconfigured`` → HTTP 501
    - ``timeout``               → HTTP 504
    - ``parse_error``           → HTTP 422
    - ``cancelled``             → HTTP 499
    - ``rate_limited``          → HTTP 429
"""

# pylint: disable=too-few-public-methods

from __future__ import annotations

import asyncio
import hashlib
import json
import re
import time
import uuid
from collections import deque
from dataclasses import dataclass, field
from typing import Any, Deque

from bill_analyser.utils.logger import get_logger

from .provider import (
    CloudOcrNotConfigured,
    DISABLED_PROVIDER_NAME,
    OcrProvider,
    OcrProviderFactory,
    OcrRawResult,
    ProviderUnavailable,
)
from .payment_screenshot_parser import parse_payment_screenshot_text

logger = get_logger("OcrService")

# Typed error codes
ERR_PROVIDER_UNCONFIGURED = "provider_unconfigured"
ERR_TIMEOUT = "timeout"
ERR_PARSE_ERROR = "parse_error"
ERR_CANCELLED = "cancelled"
ERR_RATE_LIMITED = "rate_limited"

ERROR_HTTP_STATUS: dict[str, int] = {
    ERR_PROVIDER_UNCONFIGURED: 501,
    ERR_TIMEOUT: 504,
    ERR_PARSE_ERROR: 422,
    ERR_CANCELLED: 499,
    ERR_RATE_LIMITED: 429,
}


class OcrServiceError(RuntimeError):
    """携带 typed error code 的 OCR service 错误。"""

    def __init__(self, code: str, message: str = "") -> None:
        super().__init__(message or code)
        self.code = code
        self.message = message or code

    @property
    def http_status(self) -> int:
        """HTTP status mapped from the typed OCR error code."""
        return ERROR_HTTP_STATUS.get(self.code, 500)


@dataclass
class OcrRecognitionResult:
    """OCR 服务返回的成功结构化结果。

    typed contract 字段：
        - ``amount`` (float | None)
        - ``trade_time`` (str | None, ISO-like)
        - ``description`` (str | None)
        - ``provenance`` (dict, 至少含 ``provider``/``model``/``request_id``)
        - ``confidence`` (float, 0–1)

    其它扩展字段透传到 ``raw_provider_response``，不进 typed contract。
    """

    amount: float | None
    trade_time: str | None
    description: str | None
    payment_platform: str | None
    provenance: dict[str, Any]
    confidence: float
    raw_provider_response: dict[str, Any] = field(default_factory=dict)

    def to_payload(self) -> dict[str, Any]:
        """Return the frontend OCR response payload."""
        return {
            "amount": self.amount,
            "trade_time": self.trade_time,
            "description": self.description,
            "payment_platform": self.payment_platform,
            "provenance": dict(self.provenance),
            "confidence": float(self.confidence),
            "raw_provider_response": dict(self.raw_provider_response),
        }


# --------------------------------------------------------------------------- #
# Rate limiter
# --------------------------------------------------------------------------- #


class OcrRateLimiter:
    """简单内存 token-bucket / 滑窗 per-user 限流。

    - 默认 60 秒窗口、每用户 10 次。
    - 测试可通过构造参数把窗口/阈值降到 1s/2 次。
    """

    def __init__(self, window_seconds: float = 60.0, max_requests: int = 10) -> None:
        self.window_seconds = float(window_seconds)
        self.max_requests = int(max_requests)
        self._buckets: dict[Any, Deque[float]] = {}

    def try_acquire(self, user_id: Any, *, now: float | None = None) -> bool:
        """尝试为 ``user_id`` 占用一次配额。返回 True 表示放行。"""
        current = float(now if now is not None else time.monotonic())
        bucket = self._buckets.setdefault(user_id, deque())
        cutoff = current - self.window_seconds
        while bucket and bucket[0] < cutoff:
            bucket.popleft()
        if len(bucket) >= self.max_requests:
            return False
        bucket.append(current)
        return True

    def reset(self, user_id: Any | None = None) -> None:
        """Clear all buckets, or only the specified user's bucket."""
        if user_id is None:
            self._buckets.clear()
        else:
            self._buckets.pop(user_id, None)


# --------------------------------------------------------------------------- #
# Receipt text → structured fields heuristics
# --------------------------------------------------------------------------- #

def parse_receipt_text(text: str) -> dict[str, Any]:
    """启发式从 OCR 纯文本抽取金额/时间/描述。

    无法抽取的字段返回 ``None``，调用方负责标注 ``confidence``。
    """
    parsed = parse_payment_screenshot_text(text)
    return {
        "amount": parsed.amount,
        "trade_time": parsed.trade_time,
        "description": parsed.description,
        "payment_platform": parsed.payment_platform,
        "payment_confidence": parsed.confidence,
    }


# --------------------------------------------------------------------------- #
# Service entrypoint
# --------------------------------------------------------------------------- #


_DEFAULT_TIMEOUT_SECONDS = 30.0
_DEFAULT_OCR_LANG = "chi_sim+eng"


def normalize_ocr_config(config: Any) -> dict[str, Any]:
    """Normalize OCR runtime config from DB/API/env-compatible payloads."""
    raw_config = config if isinstance(config, dict) else {}
    provider = str(raw_config.get("provider") or DISABLED_PROVIDER_NAME).strip().lower()
    if provider in {"", "none", "off"}:
        provider = DISABLED_PROVIDER_NAME

    allowed_providers = {DISABLED_PROVIDER_NAME, *OcrProviderFactory.available_providers()}
    if provider not in allowed_providers:
        provider = DISABLED_PROVIDER_NAME

    lang = str(raw_config.get("lang") or _DEFAULT_OCR_LANG).strip()
    if not lang or len(lang) > 64 or not re.fullmatch(r"[A-Za-z0-9_+.-]+", lang):
        lang = _DEFAULT_OCR_LANG

    return {
        "provider": provider,
        "lang": lang,
    }


class OcrService:
    """OCR 服务编排器。

    职责：
        - rate-limit per-user
        - 调度 provider，捕获 timeout / cancel / 解析失败
        - 把 provider raw 文本归一化成 typed contract payload
        - 透传 provider raw 到 ``raw_provider_response``，不进 typed contract
    """

    def __init__(
        self,
        provider: OcrProvider | None,
        rate_limiter: OcrRateLimiter | None = None,
        timeout_seconds: float = _DEFAULT_TIMEOUT_SECONDS,
    ) -> None:
        self._provider = provider
        self._rate_limiter = rate_limiter or OcrRateLimiter()
        self._timeout = float(timeout_seconds)

    @property
    def provider(self) -> OcrProvider | None:
        """Return the active provider, or None when OCR is disabled."""
        return self._provider

    @property
    def rate_limiter(self) -> OcrRateLimiter:
        """Return the service rate limiter."""
        return self._rate_limiter

    @classmethod
    def from_environment(cls) -> "OcrService":
        """根据 env ``BILL_OCR_PROVIDER`` 构造服务；无 provider 时构造 disabled 实例。"""
        return cls.from_config({"provider": OcrProviderFactory.resolve_default_provider_name()})

    @classmethod
    def from_config(cls, config: dict[str, Any] | None) -> "OcrService":
        """根据持久化配置构造服务；无 provider 时构造 disabled 实例。"""
        normalized_config = normalize_ocr_config(config)
        provider_name = normalized_config["provider"]
        try:
            provider = OcrProviderFactory.create(provider_name, normalized_config)
        except (ProviderUnavailable, ValueError):
            provider = None
        return cls(provider=provider)

    async def recognize(
        self,
        *,
        user_id: Any,
        image_bytes: bytes,
        mime: str,
        cancelled: bool = False,
    ) -> OcrRecognitionResult:
        """执行 OCR 识别。

        - 任何错误以 ``OcrServiceError`` 抛出，外层 route 层翻译为 HTTP 状态码。
        """
        request_id = uuid.uuid4().hex
        sha_short = hashlib.sha256(image_bytes).hexdigest()[:12]
        audit = {
            "request_id": request_id,
            "user_id": user_id,
            "bytes": len(image_bytes),
            "mime": mime,
            "sha256": sha_short,
        }

        if cancelled:
            logger.info("ocr cancelled by client: %s", audit)
            raise OcrServiceError(ERR_CANCELLED, "request cancelled by client")

        if self._provider is None:
            logger.info("ocr provider unconfigured: %s", audit)
            raise OcrServiceError(ERR_PROVIDER_UNCONFIGURED, "ocr provider not configured")

        if not self._rate_limiter.try_acquire(user_id):
            logger.warning("ocr rate limited: %s", audit)
            raise OcrServiceError(ERR_RATE_LIMITED, "ocr per-user rate limit exceeded")

        try:
            raw: OcrRawResult = await asyncio.wait_for(
                self._provider.recognize(image_bytes, mime),
                timeout=self._timeout,
            )
        except asyncio.TimeoutError as exc:
            logger.warning("ocr timeout: %s", audit)
            raise OcrServiceError(ERR_TIMEOUT, "ocr provider timeout") from exc
        except asyncio.CancelledError as exc:
            logger.info("ocr cancelled mid-flight: %s", audit)
            raise OcrServiceError(ERR_CANCELLED, "ocr provider call cancelled") from exc
        except (CloudOcrNotConfigured, ProviderUnavailable) as exc:
            logger.info("ocr provider unavailable mid-flight: %s err=%s", audit, exc)
            raise OcrServiceError(ERR_PROVIDER_UNCONFIGURED, str(exc)) from exc
        except (ValueError, TypeError, json.JSONDecodeError) as exc:
            logger.warning("ocr parse error: %s err=%s", audit, exc)
            raise OcrServiceError(ERR_PARSE_ERROR, f"ocr parse error: {exc}") from exc

        try:
            structured = parse_receipt_text(raw.text or "")
        except (ValueError, TypeError) as exc:
            logger.warning("ocr post-parse error: %s err=%s", audit, exc)
            raise OcrServiceError(ERR_PARSE_ERROR, f"receipt parse error: {exc}") from exc

        provenance = {
            "provider": getattr(self._provider, "name", "unknown"),
            "model": raw.model or getattr(self._provider, "name", "unknown"),
            "request_id": request_id,
        }

        result = OcrRecognitionResult(
            amount=_coerce_float(structured.get("amount")),
            trade_time=_coerce_str(structured.get("trade_time")),
            description=_coerce_str(structured.get("description")),
            payment_platform=_coerce_str(structured.get("payment_platform")),
            provenance=provenance,
            confidence=max(
                0.0,
                min(
                    1.0,
                    max(
                        float(raw.confidence or 0.0),
                        float(structured.get("payment_confidence") or 0.0),
                    ),
                ),
            ),
            raw_provider_response=dict(raw.raw_provider_response or {}),
        )
        logger.info("ocr success: %s confidence=%.3f", audit, result.confidence)
        return result


def _coerce_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _coerce_str(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None
