"""OCR provider 抽象层单元测试。"""

# pylint: disable=missing-function-docstring

from __future__ import annotations

import asyncio
import os

import pytest

from bill_analyser.core import ocr_provider as op


def test_factory_disabled_default(monkeypatch):
    monkeypatch.delenv(op.DEFAULT_PROVIDER_ENV, raising=False)
    name = op.OcrProviderFactory.resolve_default_provider_name()
    assert name == op.DISABLED_PROVIDER_NAME


def test_factory_disabled_raises_provider_unavailable():
    with pytest.raises(op.ProviderUnavailable):
        op.OcrProviderFactory.create("disabled", {})
    with pytest.raises(op.ProviderUnavailable):
        op.OcrProviderFactory.create("", {})


def test_factory_unknown_provider_raises_value_error():
    with pytest.raises(ValueError):
        op.OcrProviderFactory.create("not_a_provider", {})


def test_factory_can_create_cloud_stub_and_call_raises():
    provider = op.OcrProviderFactory.create("cloud_stub", {})
    assert provider.name == "cloud_stub"
    with pytest.raises(op.CloudOcrNotConfigured):
        asyncio.run(provider.recognize(b"abc", "image/png"))


def test_factory_resolves_env(monkeypatch):
    monkeypatch.setenv(op.DEFAULT_PROVIDER_ENV, "cloud_stub")
    assert op.OcrProviderFactory.resolve_default_provider_name() == "cloud_stub"


def test_available_providers_lists_known():
    available = op.OcrProviderFactory.available_providers()
    assert "tesseract" in available
    assert "cloud_stub" in available


def test_safe_log_ctx_does_not_leak_bytes():
    ctx = op._safe_log_ctx(b"secret-image-bytes", "image/png")  # pylint: disable=protected-access
    assert ctx["bytes"] == len(b"secret-image-bytes")
    assert ctx["mime"] == "image/png"
    assert len(ctx["sha256"]) == 12
    # 不应包含原始字节
    assert "secret" not in ctx["sha256"]


def test_tesseract_provider_raises_provider_unavailable_when_missing(monkeypatch):
    """无 pytesseract / PIL 时 recognize 抛 ProviderUnavailable。"""
    provider = op.TesseractProvider({})

    # 通过 sys.modules patch 模拟 import 失败
    import sys
    original_pyt = sys.modules.pop("pytesseract", None)
    original_pil = sys.modules.pop("PIL", None)
    sys.modules["pytesseract"] = None  # type: ignore[assignment]
    try:
        with pytest.raises(op.ProviderUnavailable):
            asyncio.run(provider.recognize(b"\x89PNG", "image/png"))
    finally:
        sys.modules.pop("pytesseract", None)
        if original_pyt is not None:
            sys.modules["pytesseract"] = original_pyt
        if original_pil is not None:
            sys.modules["PIL"] = original_pil


def test_factory_normalizes_case():
    # disabled aliases
    with pytest.raises(op.ProviderUnavailable):
        op.OcrProviderFactory.create("DISABLED", {})
    with pytest.raises(op.ProviderUnavailable):
        op.OcrProviderFactory.create("None", {})
    with pytest.raises(op.ProviderUnavailable):
        op.OcrProviderFactory.create("off", {})


def test_resolve_default_provider_name_strip(monkeypatch):
    monkeypatch.setenv(op.DEFAULT_PROVIDER_ENV, "  Cloud_Stub  ")
    assert op.OcrProviderFactory.resolve_default_provider_name() == "cloud_stub"
