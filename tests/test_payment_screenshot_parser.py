"""Payment screenshot OCR text parser tests."""

from __future__ import annotations

import pytest

from bill_analyser.core.ai.ocr.payment_screenshot_parser import parse_payment_screenshot_text


def test_parse_wechat_pay_detail_text():
    text = """
    微信支付
    支付成功
    ￥28.50
    交易对方
    美团外卖
    支付时间
    2026年5月4日 12:34:56
    交易单号 420000000000000000
    """

    result = parse_payment_screenshot_text(text)

    assert result.payment_platform == "wechat_pay"
    assert result.amount == pytest.approx(28.50)
    assert result.trade_time == "2026-05-04 12:34:56"
    assert result.description == "美团外卖"
    assert result.confidence >= 0.9


def test_parse_alipay_bill_detail_text():
    text = """
    支付宝
    交易成功
    付款金额 88.00
    商家服务
    星巴克咖啡
    创建时间 2026-05-04 09:08:07
    商户订单号 202605040001
    """

    result = parse_payment_screenshot_text(text)

    assert result.payment_platform == "alipay"
    assert result.amount == pytest.approx(88.00)
    assert result.trade_time == "2026-05-04 09:08:07"
    assert result.description == "星巴克咖啡"


def test_parse_generic_receipt_text_still_extracts_basic_fields():
    result = parse_payment_screenshot_text("商家: 便利店\n金额 12.34\n2026-05-04 08:00")

    assert result.payment_platform is None
    assert result.amount == pytest.approx(12.34)
    assert result.trade_time == "2026-05-04 08:00"
    assert result.description == "便利店"
