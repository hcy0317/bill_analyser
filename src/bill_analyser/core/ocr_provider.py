"""OCR Provider 抽象层 — 小票识图。

privacy contract:
    - 任何 provider 都禁止把原始 ``image_bytes`` 写盘或写日志。
    - 可记录的元信息：``len(image_bytes)``、``mime``、``sha256[:12]``。
    - 调用方（``ocr_service``）负责"请求即用即弃"语义：
      不持久化图像，仅审计字节数 / mime / hash / user_id / 状态。

设计参考 ``bill_analyser.core.llm_provider``：Protocol + 多实现 + Factory。
"""

# pylint: disable=too-few-public-methods

from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass, field
from typing import Any, Protocol

from ..utils.logger import get_logger

logger = get_logger("OcrProvider")

DEFAULT_PROVIDER_ENV = "BILL_OCR_PROVIDER"
DISABLED_PROVIDER_NAME = "disabled"


class ProviderUnavailable(RuntimeError):
    """指定的 OCR provider 在当前环境无法加载（例如缺少软依赖）。"""


class CloudOcrNotConfigured(NotImplementedError):
    """云 OCR provider 未接入实现，仅占位。"""


@dataclass(frozen=True)
class OcrRawResult:
    """OCR provider 的原始返回。

    - ``text``: provider 抽取出的纯文本（用于 service 层进一步解析）。
    - ``confidence``: 0–1 浮点；provider 无法估计时返回 ``0.0``。
    - ``model``: 模型/引擎标识，例如 ``tesseract`` 或云端 model 名。
    - ``raw_provider_response``: provider-specific 原始负载，service 层透传，
      不作为 typed contract 暴露给前端。
    """

    text: str
    confidence: float
    model: str
    raw_provider_response: dict[str, Any] = field(default_factory=dict)


class OcrProvider(Protocol):
    """OCR provider Protocol。"""

    name: str

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult: ...


def _safe_log_ctx(image_bytes: bytes, mime: str) -> dict[str, Any]:
    """构造安全的日志上下文（不含图像本体）。"""
    digest = hashlib.sha256(image_bytes).hexdigest()
    return {"bytes": len(image_bytes), "mime": mime, "sha256": digest[:12]}


class TesseractProvider:
    """本地 Tesseract OCR provider。

    privacy: ``image_bytes`` 仅在内存中转换为 ``PIL.Image``，不落盘；日志
    只记录 ``len(image_bytes)``、``mime``、``sha256[:12]``。
    """

    name = "tesseract"

    def __init__(self, config: dict[str, Any] | None = None) -> None:
        self._config = dict(config or {})
        self._lang: str = self._config.get("lang") or "chi_sim+eng"

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        ctx = _safe_log_ctx(image_bytes, mime)
        logger.info("tesseract OCR start: %s", ctx)
        try:
            # pylint: disable=import-outside-toplevel
            import pytesseract
            from PIL import Image
        except ImportError as exc:  # pragma: no cover - 运行时软依赖检查
            raise ProviderUnavailable(
                f"tesseract provider 缺少软依赖: {exc}. 请安装 pytesseract + pillow."
            ) from exc

        from io import BytesIO  # pylint: disable=import-outside-toplevel

        try:
            image = Image.open(BytesIO(image_bytes))
        except (OSError, ValueError) as exc:
            raise ProviderUnavailable(f"无法解析图片: {exc}") from exc

        text: str = pytesseract.image_to_string(image, lang=self._lang)

        try:
            data = pytesseract.image_to_data(
                image, lang=self._lang, output_type=pytesseract.Output.DICT
            )
            confidences = [
                int(c) for c in data.get("conf", []) if str(c).lstrip("-").isdigit() and int(c) >= 0
            ]
            avg_conf = (sum(confidences) / len(confidences) / 100.0) if confidences else 0.0
        except (AttributeError, ValueError, TypeError):  # pragma: no cover - 容错
            avg_conf = 0.0

        return OcrRawResult(
            text=text or "",
            confidence=max(0.0, min(1.0, float(avg_conf))),
            model="tesseract",
            raw_provider_response={"engine": "tesseract", "lang": self._lang},
        )


class StubCloudProvider:
    """云 OCR 占位 provider，用于证明 factory 抽象到位。

    任何调用都会抛 ``CloudOcrNotConfigured``，由 service 层 catch 并归一为
    ``provider_unconfigured`` typed error。
    """

    name = "cloud_stub"

    def __init__(self, config: dict[str, Any] | None = None) -> None:
        self._config = dict(config or {})

    async def recognize(self, image_bytes: bytes, mime: str) -> OcrRawResult:
        ctx = _safe_log_ctx(image_bytes, mime)
        logger.info("stub cloud OCR invoked (will raise): %s", ctx)
        raise CloudOcrNotConfigured("cloud_ocr_not_configured")


class OcrProviderFactory:
    """OCR provider 工厂。

    用法::

        provider = OcrProviderFactory.create("tesseract", {"lang": "chi_sim+eng"})

    provider 名通过 env ``BILL_OCR_PROVIDER`` 选择，默认 ``disabled`` 表示
    后端关闭 OCR（service 层会返回 501 + ``provider_unconfigured``）。
    """

    _PROVIDERS: dict[str, type] = {
        "tesseract": TesseractProvider,
        "cloud_stub": StubCloudProvider,
    }

    @classmethod
    def available_providers(cls) -> list[str]:
        return sorted(cls._PROVIDERS.keys())

    @classmethod
    def resolve_default_provider_name(cls) -> str:
        """从环境变量读取默认 provider 名。"""
        return (os.environ.get(DEFAULT_PROVIDER_ENV) or DISABLED_PROVIDER_NAME).strip().lower()

    @classmethod
    def create(cls, provider_name: str, config: dict[str, Any] | None = None) -> OcrProvider:
        """根据名字创建 provider 实例。

        - ``disabled`` / 空值：抛 ``ProviderUnavailable``，由 service 转 501。
        - 未知 provider：抛 ``ValueError``。
        - tesseract 缺软依赖：``__init__`` 不会失败，调用时抛 ``ProviderUnavailable``。
        """
        normalized = (provider_name or DISABLED_PROVIDER_NAME).strip().lower()
        if normalized in {"", DISABLED_PROVIDER_NAME, "none", "off"}:
            raise ProviderUnavailable("ocr provider disabled by configuration")
        provider_cls = cls._PROVIDERS.get(normalized)
        if provider_cls is None:
            raise ValueError(
                f"Unknown OCR provider '{provider_name}'. "
                f"Available: {cls.available_providers()}"
            )
        return provider_cls(config or {})  # type: ignore[return-value]
